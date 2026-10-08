import {
  cloneVault,
  deviceSyncChange,
  ensureDeviceChange,
  syncContext,
  WebSyncSession,
  type DeviceVaultSync,
  type SyncContext,
  type WebBackend,
} from "../wasm-core/typsidian_core.js";
import { deviceStore, deviceStoreQueue } from "../settings/deviceStore.js";
import type { WasmCallQueue } from "../wasm-call-queue.js";
import { createWebGitEngine, mirrorDirectory } from "./git-engine-web.js";
import type { SyncClient } from "./sync-client.js";

// `SyncClient` для web (фича 011, T018): `WebSyncSession` в том же процессе,
// движок — isomorphic-git поверх папки пользователя (`git-engine-web.ts`).
//
// Все вызовы сессии идут через **ту же очередь, что и vault**: сессия заимствует
// `WebBackend` на время операции, и наложение на цикл сверки с диском упало бы
// «recursive use of an object» (см. `wasm-call-queue.ts`).
//
// Порядок очередей один на все вызовы — устройство, затем vault, затем снова
// устройство — и хранилище устройства отпускается до обращения к vault: тот же
// порядок блокировок, что у команд оболочки (`packages/shell/src/sync.rs`).

/** Идентификатор папки в реестре устройства; повторное добавление сводится к той же записи. */
export async function webVaultId(handle: FileSystemDirectoryHandle): Promise<string> {
  const store = await deviceStore();
  return deviceStoreQueue.run(() => store.addVault(handle, handle.name));
}

export async function webSyncClient(
  backend: WebBackend,
  queue: WasmCallQueue,
  handle: FileSystemDirectoryHandle,
  vaultId: string,
): Promise<SyncClient> {
  // Зеркало не-хранителя живёт в OPFS (FR-033). Нет OPFS — нет зеркала:
  // остальная синхронизация от этого не зависит.
  let mirror: Record<string, unknown> | null = null;
  try {
    mirror = createWebGitEngine(await mirrorDirectory(vaultId), { mirror: true });
  } catch {
    mirror = null;
  }
  const engine = createWebGitEngine(handle);
  const session = new WebSyncSession(engine, mirror, vaultId);

  const context = async (): Promise<SyncContext> => {
    const store = await deviceStore();
    return deviceStoreQueue.run(async () => {
      // Тождество устройства выдаётся при первом обращении (research §4).
      const identity = ensureDeviceChange(store.settings());
      if (identity !== undefined) await store.apply(identity);
      return syncContext(store.settings(), vaultId, Date.now());
    });
  };

  /** Сохраняет значения устройства, изменённые сессией. */
  const persist = async (): Promise<void> => {
    const update = await queue.runInternal(() => session.takeDeviceUpdate());
    if (update === undefined) return;
    const store = await deviceStore();
    await deviceStoreQueue.run(() => store.apply(deviceSyncChange(vaultId, update)));
  };

  /** Вызов сессии с контекстом устройства и сохранением его значений после. */
  const withContext = async <T>(op: (ctx: SyncContext) => T | Promise<T>): Promise<T> => {
    const ctx = await context();
    try {
      return await queue.run(() => op(ctx));
    } finally {
      await persist();
    }
  };

  const readDeviceSync = async (): Promise<DeviceVaultSync> => {
    const store = await deviceStore();
    return deviceStoreQueue.run(
      async () =>
        store.settings().vault_sync?.find((known) => known.vault === vaultId)?.value ?? {},
    );
  };

  return {
    overview: () => withContext((ctx) => session.overview(backend, ctx)),
    pollEvents: () => queue.runInternal(() => session.pollEvents()),
    noteChange: () => queue.runInternal(() => session.noteChange(Date.now())),
    // Сессию трогать нельзя: она занята операцией, которую и отменяют, а
    // `cancel` требует `&mut self` и встал бы в очередь за ней. Отмена идёт
    // прямо в адаптер движка — сеть рвёт он, и ядро получает `Cancelled`.
    cancel: () => {
      for (const adapter of [engine, mirror]) {
        const abort = adapter?.["abort"];
        if (typeof abort === "function") (abort as () => void).call(adapter);
      }
    },

    ignoreReports: () => queue.run(() => session.ignoreReports(backend)),
    applyIgnore: (file) => withContext((ctx) => session.applyIgnore(backend, ctx, file)),
    declineStignore: () => withContext((ctx) => session.declineStignore(ctx)),
    enableHistory: (message) => withContext((ctx) => session.enableHistory(backend, ctx, message)),

    changes: () => queue.run(() => session.changes(backend)),
    snapshot: (paths, message, options) =>
      withContext((ctx) =>
        session.snapshot(
          backend,
          ctx,
          paths,
          message,
          options.autoPrefix,
          options.flushed,
          options.confirmNotKeeper,
        ),
      ),
    history: (path, cursor, limit) =>
      queue.run(() => session.history(backend, path, cursor, limit)),
    diff: (path, from, to) => queue.run(() => session.diff(backend, path, from, to)),
    fileAt: (path, revision) => queue.run(() => session.fileAt(backend, path, revision)),
    restore: (path, snapshot) => queue.run(() => session.restore(backend, path, snapshot)),

    connectRemote: (url, credentials) =>
      withContext((ctx) => session.connectRemote(backend, ctx, url, credentials)),
    disconnectRemote: () => queue.run(() => session.disconnectRemote(backend)),
    push: (confirmNotKeeper) =>
      withContext((ctx) => session.push(backend, ctx, confirmNotKeeper)),
    pull: (confirmNotKeeper) =>
      withContext((ctx) => session.pull(backend, ctx, confirmNotKeeper)),
    abortPull: () => queue.run(() => session.abortPull(backend)),
    conflicts: () => queue.run(() => session.conflicts(backend)),
    resolve: (id, resolution) =>
      withContext((ctx) => session.resolve(backend, ctx, id, resolution)),
    refreshMirror: () => withContext((ctx) => session.refreshMirror(backend, ctx)),

    setKeeper: () => withContext((ctx) => session.setKeeper(backend, ctx)),
    clearKeeper: () => queue.run(() => session.clearKeeper(backend)),
    updateSettings: (patch) => queue.run(() => session.updateSettings(backend, patch)),
    tick: async (autoPrefix) => {
      const ctx = await context();
      try {
        // Такт — не действие пользователя: на вопрос «есть ли что терять при
        // закрытии» он не влияет.
        return await queue.runInternal(() => session.tick(backend, ctx, autoPrefix));
      } finally {
        await persist();
      }
    },

    // Состояние Syncthing из браузера не читается (FR-036): его API не
    // разрешает обращения со страниц. Искать и отключать здесь нечего.
    syncthingDiscover: async () => null,
    syncthingConnect: (endpoint, apiKey) =>
      withContext((ctx) => session.syncthingConnect(ctx, endpoint, apiKey)),
    syncthingDisconnect: async () => {},

    deviceSync: readDeviceSync,
    updateDeviceSync: async (patch) => {
      const store = await deviceStore();
      return deviceStoreQueue.run(async () => {
        const current =
          store.settings().vault_sync?.find((known) => known.vault === vaultId)?.value ?? {};
        const value = { ...current, ...patch };
        await store.apply(deviceSyncChange(vaultId, value));
        return value;
      });
    },
  };
}

/**
 * Клонирует удалённый репозиторий в пустую папку пользователя (FR-013).
 * Открывает vault вызывающая сторона — обычным жестом.
 */
export async function cloneWebVault(
  handle: FileSystemDirectoryHandle,
  url: string,
  credentials: { user?: string | undefined; token?: string | undefined },
): Promise<void> {
  // Идентификатор нужен до клонирования: под ним сохраняется токен.
  const vaultId = await webVaultId(handle);
  await cloneVault(createWebGitEngine(handle), vaultId, url, {
    user: credentials.user,
    token: credentials.token,
  });
}
