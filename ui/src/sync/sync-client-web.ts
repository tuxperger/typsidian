import {
  WebSyncSession,
  cloneVault,
  deviceSyncChange,
  ensureDeviceChange,
  syncContext,
  type DeviceVaultSync,
  type SyncContext,
  type WebBackend,
} from "../wasm-core/typsidian_core.js";
import type { WasmCallQueue } from "../wasm-call-queue.js";
import { deviceStore, deviceStoreQueue } from "../settings/deviceStore.js";
import type { VaultOpenResult } from "../vault-tree/useVault.js";
import { createWebGitEngine, mirrorDirectory } from "./git-engine-web.js";
import {
  CloneFailure,
  asSyncError,
  remoteCapabilities,
  type SyncClient,
  type VaultCloner,
} from "./sync-client.js";

// Синхронизация в браузере (фича 011, T018): `WebSyncSession` ядра в том же
// процессе, движок git — адаптер isomorphic-git поверх папки пользователя.
//
// Две очереди и порядок между ними. Сессия заимствует `WebBackend` на время
// вызова, поэтому идёт через ту же очередь, что и клиент vault. Контекст
// вызова (тождество устройства, автор, посредник) лежит в хранилище устройства
// со своей очередью. Очереди берутся строго по очереди, а не вложенно:
// контекст читается до вызова сессии, изменённые значения устройства
// сохраняются после него.

/**
 * Клиент синхронизации открытого в браузере vault.
 *
 * `handle` — та же папка, что держит `backend`: история лежит в её `.git`
 * (FR-002). `vaultId` — идентификатор записи в хранилище устройства.
 */
export async function webSyncClient(
  backend: WebBackend,
  queue: WasmCallQueue,
  handle: FileSystemDirectoryHandle,
  vaultId: string,
): Promise<SyncClient> {
  const engine = createWebGitEngine(handle);
  // Зеркало не-хранителя живёт в OPFS (FR-033). Без OPFS зеркала нет —
  // история на не-хранителе тогда недоступна, остальное работает.
  let mirror: ReturnType<typeof createWebGitEngine> | null = null;
  try {
    mirror = createWebGitEngine(await mirrorDirectory(vaultId), { mirror: true });
  } catch {
    mirror = null;
  }
  const session = new WebSyncSession(engine, mirror, vaultId);
  // Ручка отмены — отдельный объект ядра: пока операция идёт, сама сессия
  // заимствована, и звать её метод из обработчика кнопки нельзя.
  const cancelHandle = session.cancelHandle();

  const context = async (): Promise<SyncContext> => {
    const store = await deviceStore();
    return deviceStoreQueue.run(async () => {
      // Тождество устройства выдаётся при первом обращении (research §4).
      const ensure = ensureDeviceChange(store.settings());
      if (ensure !== undefined) await store.apply(ensure);
      return syncContext(store.settings(), vaultId, Date.now());
    });
  };

  const persist = async (update: DeviceVaultSync | undefined): Promise<void> => {
    if (update === undefined) return;
    const store = await deviceStore();
    await deviceStoreQueue.run(() => store.apply(deviceSyncChange(vaultId, update)));
  };

  /** Вызов сессии с контекстом и сохранением значений устройства после него. */
  const withContext = async <T>(op: (ctx: SyncContext) => T | Promise<T>): Promise<T> => {
    const ctx = await context();
    let update: DeviceVaultSync | undefined;
    try {
      return await queue.run(async () => {
        try {
          return await op(ctx);
        } finally {
          update = session.takeDeviceUpdate();
        }
      });
    } finally {
      await persist(update);
    }
  };

  const plain = <T>(op: () => T | Promise<T>): Promise<T> => queue.run(op);

  const writeDeviceSync = async (
    change: (current: DeviceVaultSync) => DeviceVaultSync,
  ): Promise<void> => {
    const current = (await context()).device_sync;
    await persist(change(current));
  };

  return {
    overview: () => withContext((ctx) => session.overview(backend, ctx)),
    pollEvents: () => queue.runInternal(() => session.pollEvents()),
    noteChange: () => queue.runInternal(() => session.noteChange(Date.now())),
    cancel: () => cancelHandle.cancel(),

    ignoreReports: () => plain(() => session.ignoreReports(backend)),
    applyIgnore: (file) => withContext((ctx) => session.applyIgnore(backend, ctx, file)),
    declineStignore: () => withContext((ctx) => session.declineStignore(ctx)),
    enableHistory: (message) =>
      withContext((ctx) => session.enableHistory(backend, ctx, message)),

    changes: () => plain(() => session.changes(backend)),
    snapshot: (request) =>
      withContext((ctx) =>
        session.snapshot(
          backend,
          ctx,
          request.paths,
          request.message,
          request.autoPrefix,
          request.flushed,
          request.confirmNotKeeper,
        ),
      ),

    history: (path, cursor, limit) =>
      plain(() => session.history(backend, path, cursor, limit)),
    diff: (path, from, to) => plain(() => session.diff(backend, path, from, to)),
    fileAt: (path, revision) => plain(() => session.fileAt(backend, path, revision)),
    snapshotChanges: (snapshot) => plain(() => session.snapshotChanges(backend, snapshot)),
    restore: (path, snapshot) => plain(() => session.restore(backend, path, snapshot)),

    connectRemote: (url, credentials) =>
      withContext((ctx) => session.connectRemote(backend, ctx, url, credentials)),
    disconnectRemote: () => plain(() => session.disconnectRemote(backend)),
    push: (confirmNotKeeper) =>
      withContext((ctx) => session.push(backend, ctx, confirmNotKeeper)),
    pull: (confirmNotKeeper) =>
      withContext((ctx) => session.pull(backend, ctx, confirmNotKeeper)),
    abortPull: () => plain(() => session.abortPull(backend)),
    conflicts: () => plain(() => session.conflicts(backend)),
    resolve: (id, resolution) =>
      withContext((ctx) => session.resolve(backend, ctx, id, resolution)),

    setKeeper: () => withContext((ctx) => session.setKeeper(backend, ctx)),
    clearKeeper: () => plain(() => session.clearKeeper(backend)),
    updateSettings: (patch) => plain(() => session.updateSettings(backend, patch)),
    refreshMirror: () => withContext((ctx) => session.refreshMirror(backend, ctx)),

    // GUI Syncthing не отвечает на обращения со страницы (CORS) — состояние в
    // браузере недоступно, и ядро сообщает об этом через `capabilities`
    // (FR-036). Искать и подключать здесь нечего.
    syncthingDiscover: async () => null,
    syncthingConnect: (endpoint, apiKey) =>
      // Найти Syncthing из браузера нельзя, поэтому подключения без ключа
      // здесь не бывает; пустой ключ ядро отклонит при первом же обращении.
      withContext((ctx) => session.syncthingConnect(ctx, endpoint, apiKey ?? "")),
    syncthingDisconnect: async () => undefined,

    tick: (autoPrefix) => withContext((ctx) => session.tick(backend, ctx, autoPrefix)),

    deviceSync: async () => (await context()).device_sync,
    setAuthor: (author) =>
      writeDeviceSync((current) => ({ ...current, author: author ?? undefined })),
    setCorsProxy: (url) =>
      writeDeviceSync((current) => ({ ...current, cors_proxy: url ?? undefined })),
  };
}

interface KeyedDirectory {
  keys(): AsyncIterableIterator<string>;
}

async function isEmptyFolder(handle: FileSystemDirectoryHandle): Promise<boolean> {
  for await (const _name of (handle as unknown as KeyedDirectory).keys()) return false;
  return true;
}

/**
 * Жест «создать vault из удалённого репозитория» в браузере (FR-013).
 *
 * `open` — обычное открытие vault по хендлу папки: после клонирования папка
 * открывается тем же путём, что и любая другая.
 */
export function webVaultCloner(
  ready: () => Promise<unknown>,
  open: (handle: FileSystemDirectoryHandle, vaultId: string) => Promise<VaultOpenResult>,
): VaultCloner {
  let running: ReturnType<typeof createWebGitEngine> | null = null;
  return {
    capabilities: async () => {
      await ready();
      return remoteCapabilities(WebSyncSession.capabilities());
    },
    cancel: () => {
      const abort = running?.["abort"];
      if (typeof abort === "function") (abort as () => void)();
    },
    async run(request) {
      const picker = (
        window as unknown as {
          showDirectoryPicker?: (options: {
            mode: string;
          }) => Promise<FileSystemDirectoryHandle>;
        }
      ).showDirectoryPicker;
      if (!picker) return { ok: false, error: { kind: "NotSupported" } };
      let handle: FileSystemDirectoryHandle;
      try {
        handle = await picker({ mode: "readwrite" });
      } catch (cause) {
        if (cause instanceof DOMException && cause.name === "AbortError") {
          return { ok: false, error: { kind: "PickerCancelled" } };
        }
        return { ok: false, error: { kind: "PermissionDenied" } };
      }
      let vaultId: string;
      try {
        await ready();
        if (!(await isEmptyFolder(handle))) throw { kind: "TargetNotEmpty" };
        const store = await deviceStore();
        vaultId = await deviceStoreQueue.run(() => store.addVault(handle, handle.name));
        running = createWebGitEngine(handle);
        await cloneVault(
          running,
          vaultId,
          request.url,
          request.credentials,
          request.corsProxy ?? undefined,
        );
        if (request.corsProxy !== null) {
          // Посредник запоминается за этим vault: без него следующий обмен
          // упёрся бы в тот же отказ браузера.
          const proxy = request.corsProxy;
          await deviceStoreQueue.run(() =>
            store.apply(deviceSyncChange(vaultId, { cors_proxy: proxy })),
          );
        }
      } catch (cause) {
        throw new CloneFailure(asSyncError(cause));
      } finally {
        running = null;
      }
      return open(handle, vaultId);
    },
  };
}
