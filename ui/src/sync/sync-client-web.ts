import {
  WebSyncSession,
  cloneVault,
  deviceSyncChange,
  ensureDeviceChange,
  ignoreCheck,
  ignoreRestore,
  syncContext,
  type DeviceVaultSync,
  type SyncContext,
  type SyncExchange,
  type TickOutcome,
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
  const progressHandle = session.progressHandle();

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

  /**
   * Обмен по шагам (T071): план и итог — под очередью, сеть — вне её. Пока идёт
   * сеть, сохранение заметок и опрос состояния не ждут сервер (FR-020).
   * Значения устройства сохраняются после итога, как у `withContext`.
   */
  const stepped = async <T>(
    begin: (ctx: SyncContext) => Promise<SyncExchange>,
    complete: (ctx: SyncContext, exchange: SyncExchange) => T | Promise<T>,
  ): Promise<T> => {
    const ctx = await context();
    let update: DeviceVaultSync | undefined;
    try {
      const exchange = await queue.run(() => begin(ctx));
      await exchange.run();
      return await queue.run(async () => {
        try {
          return await complete(ctx, exchange);
        } finally {
          update = session.takeDeviceUpdate();
        }
      });
    } finally {
      if (update === undefined) update = await queue.run(() => session.takeDeviceUpdate());
      await persist(update);
    }
  };

  /** Такт автоматики по тем же шагам: сеть автополучения и автоотправки — вне очереди. */
  const tickStepped = async (autoPrefix: string): Promise<TickOutcome> => {
    const ctx = await context();
    let update: DeviceVaultSync | undefined;
    try {
      const pulled = await queue.run(() => session.tickBegin(backend, ctx));
      await pulled?.run();
      const pushed = await queue.run(() => session.tickMiddle(backend, ctx, autoPrefix, pulled));
      await pushed?.run();
      return await queue.run(async () => {
        try {
          return await session.tickEnd(backend, ctx, pushed);
        } finally {
          update = session.takeDeviceUpdate();
        }
      });
    } finally {
      await persist(update);
    }
  };

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
    progress: async () => progressHandle.current() ?? null,

    ignoreReports: () => plain(() => session.ignoreReports(backend)),
    applyIgnore: (file) => withContext((ctx) => session.applyIgnore(backend, ctx, file)),
    declineStignore: () => withContext((ctx) => session.declineStignore(ctx)),

    ignoreOpen: (file) => plain(() => session.ignoreOpen(backend, file)),
    // Чистые функции ядра: очередь сессии им не нужна, и набор в редакторе
    // не ждёт идущего обмена.
    ignoreCheck: async (file, text) => ignoreCheck(file, text),
    ignoreRestore: async (file, text) => ignoreRestore(file, text),
    ignoreSave: (request) => withContext((ctx) => session.ignoreSave(backend, ctx, request)),
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
      stepped(
        (ctx) => session.beginPush(backend, ctx, confirmNotKeeper),
        (ctx, exchange) => session.completePush(backend, ctx, exchange),
      ),
    pull: (confirmNotKeeper) =>
      stepped(
        (ctx) => session.beginPull(backend, ctx, confirmNotKeeper),
        (ctx, exchange) => session.completePull(backend, ctx, exchange),
      ),
    restoreHistory: (confirmNotKeeper) =>
      withContext((ctx) => session.restoreHistory(backend, ctx, confirmNotKeeper)),
    abortPull: () => plain(() => session.abortPull(backend)),
    conflicts: () => plain(() => session.conflicts(backend)),
    resolve: (id, resolution) =>
      withContext((ctx) => session.resolve(backend, ctx, id, resolution)),

    setKeeper: () => withContext((ctx) => session.setKeeper(backend, ctx)),
    clearKeeper: () => plain(() => session.clearKeeper(backend)),
    updateSettings: (patch) => plain(() => session.updateSettings(backend, patch)),
    refreshMirror: () =>
      stepped(
        (ctx) => session.beginMirror(backend, ctx),
        (ctx, exchange) => session.completeMirror(ctx, exchange),
      ),

    // GUI Syncthing не отвечает на обращения со страницы (CORS) — состояние в
    // браузере недоступно, и ядро сообщает об этом через `capabilities`
    // (FR-036). Искать и подключать здесь нечего.
    syncthingDiscover: async () => null,
    syncthingConnect: (endpoint, apiKey) =>
      // Найти Syncthing из браузера нельзя, поэтому подключения без ключа
      // здесь не бывает; пустой ключ ядро отклонит при первом же обращении.
      withContext((ctx) => session.syncthingConnect(ctx, endpoint, apiKey ?? "")),
    syncthingDisconnect: async () => undefined,

    tick: tickStepped,

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
