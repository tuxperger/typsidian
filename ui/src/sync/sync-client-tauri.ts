import { listen as tauriListen } from "@tauri-apps/api/event";
import { invoke as tauriInvoke } from "../tauri-invoke.js";
import type {
  Change,
  Conflict,
  Content,
  SyncProgress,
  DeviceVaultSync,
  FileDiff,
  HistoryPage,
  IgnoreCheck,
  IgnoreDocument,
  IgnoreReport,
  IgnoreSaveOutcome,
  Keeper,
  PullOutcome,
  RemoteStatus,
  Settings,
  ShellCapabilities,
  Snapshot,
  SyncEvent,
  SyncOverview,
  TickOutcome,
  VaultSyncSettings,
} from "../wasm-core/typsidian_core.js";
import type { VaultOpenResult } from "../vault-tree/useVault.js";
import type { ShellTransport } from "../vault-client-tauri.js";
import { changeOf } from "../settings/useSettings.js";
import {
  CloneFailure,
  asSyncError,
  remoteCapabilities,
  type DiscoveredSyncthing,
  type SyncClient,
  type VaultCloner,
} from "./sync-client.js";

// Синхронизация поверх команд Tauri (фича 011, T018) — общая для рабочего
// стола и Android. Каждый метод — один `invoke` команды `sync_*` из
// `packages/shell/src/sync.rs`; правил здесь нет (принцип I).
//
// Значения устройства (автор, посредник) лежат в настройках устройства (004),
// поэтому читаются и пишутся теми же командами, что и панель настроек.
//
// С фичи 038 тот же клиент работает поверх HTTP с сервером (`openServerSync`):
// команды те же, но настроек устройства сервер не отдаёт — значения синхронизации
// vault идут командами `sync_device_values` и `sync_set_author`.

const tauriTransport: ShellTransport = { invoke: tauriInvoke, listen: tauriListen };

/**
 * Открывает сессию синхронизации для только что открытого vault и отдаёт её
 * клиента. `vaultId` — идентификатор записи в хранилище устройства.
 */
export async function openTauriSync(vaultId: string): Promise<SyncClient> {
  await tauriInvoke<void>("sync_open", { vaultId });
  return tauriSyncClient(vaultId, tauriTransport, false);
}

/** Сессия синхронизации vault на сервере: id vault назначает сервер. */
export async function openServerSync(transport: ShellTransport): Promise<SyncClient> {
  await transport.invoke<void>("sync_open", { vaultId: "" });
  return tauriSyncClient("", transport, true);
}

function tauriSyncClient(vaultId: string, transport: ShellTransport, server: boolean): SyncClient {
  const { invoke, listen } = transport;
  const deviceSync = async (): Promise<DeviceVaultSync> => {
    if (server) return invoke<DeviceVaultSync>("sync_device_values");
    const snapshot = await invoke<{ settings: Settings }>("device_settings");
    const entry = (snapshot.settings.vault_sync ?? []).find((item) => item.vault === vaultId);
    return entry?.value ?? {};
  };

  const writeDeviceSync = async (
    change: (current: DeviceVaultSync) => DeviceVaultSync,
  ): Promise<void> => {
    const value = change(await deviceSync());
    await invoke("apply_device_settings", {
      change: changeOf({ vault_sync: { vault: vaultId, value } }),
    });
  };

  return {
    overview: () => invoke<SyncOverview>("sync_overview"),
    pollEvents: () => invoke<SyncEvent[]>("sync_poll_events"),
    progress: async () => (await invoke<SyncProgress | null>("sync_progress")) ?? null,
    onChanged: (listener) => {
      // Подписка асинхронная: отписка до её готовности не должна оставить
      // слушателя навсегда.
      let stop: (() => void) | null = null;
      let cancelled = false;
      void listen("sync-changed", () => listener()).then((unlisten) => {
        if (cancelled) unlisten();
        else stop = unlisten;
      });
      return () => {
        cancelled = true;
        stop?.();
      };
    },
    noteChange: () => invoke<void>("sync_note_change"),
    cancel: () => void invoke("sync_cancel"),

    ignoreReports: () => invoke<IgnoreReport[]>("sync_ignore_reports"),
    applyIgnore: (file) => invoke<void>("sync_apply_ignore", { file }),
    declineStignore: () => invoke<void>("sync_decline_stignore"),

    ignoreOpen: (file) => invoke<IgnoreDocument>("sync_ignore_open", { file }),
    ignoreCheck: (file, text) => invoke<IgnoreCheck>("sync_ignore_check", { file, text }),
    ignoreRestore: (file, text) => invoke<string>("sync_ignore_restore", { file, text }),
    ignoreSave: (request) => invoke<IgnoreSaveOutcome>("sync_ignore_save", { request }),
    enableHistory: (message) => invoke<Snapshot>("sync_enable_history", { message }),

    changes: () => invoke<Change[]>("sync_changes"),
    snapshot: (request) =>
      invoke<Snapshot>("sync_snapshot", {
        paths: request.paths,
        message: request.message,
        autoPrefix: request.autoPrefix,
        flushed: request.flushed,
        confirmNotKeeper: request.confirmNotKeeper,
      }),

    history: (path, cursor, limit) =>
      invoke<HistoryPage>("sync_history", { path, cursor, limit }),
    diff: (path, from, to) => invoke<FileDiff>("sync_diff", { path, from, to }),
    fileAt: (path, revision) => invoke<Content>("sync_file_at", { path, revision }),
    snapshotChanges: (snapshot) => invoke<Change[]>("sync_snapshot_changes", { snapshot }),
    restore: (path, snapshot) => invoke<Content>("sync_restore", { path, snapshot }),

    connectRemote: (url, credentials) =>
      invoke<RemoteStatus>("sync_connect_remote", { url, credentials }),
    disconnectRemote: () => invoke<void>("sync_disconnect_remote"),
    push: (confirmNotKeeper) => invoke<RemoteStatus>("sync_push", { confirmNotKeeper }),
    pull: (confirmNotKeeper) => invoke<PullOutcome>("sync_pull", { confirmNotKeeper }),
    restoreHistory: (confirmNotKeeper) =>
      invoke<PullOutcome>("sync_restore_history", { confirmNotKeeper }),
    abortPull: () => invoke<void>("sync_abort_pull"),
    conflicts: () => invoke<Conflict[]>("sync_conflicts"),
    resolve: (id, resolution) => invoke<void>("sync_resolve", { id, resolution }),

    setKeeper: () => invoke<Keeper>("sync_set_keeper"),
    clearKeeper: () => invoke<void>("sync_clear_keeper"),
    updateSettings: (patch) => invoke<VaultSyncSettings>("sync_update_settings", { patch }),
    refreshMirror: () => invoke<void>("sync_refresh_mirror"),

    syncthingDiscover: () => invoke<DiscoveredSyncthing | null>("sync_syncthing_discover"),
    syncthingConnect: (endpoint, apiKey) =>
      invoke<void>("sync_syncthing_connect", { endpoint, apiKey }),
    syncthingDisconnect: () => invoke<void>("sync_syncthing_disconnect"),

    tick: (autoPrefix) => invoke<TickOutcome>("sync_tick", { autoPrefix }),

    deviceSync,
    setAuthor: (author) =>
      server
        ? invoke<void>("sync_set_author", { author })
        : writeDeviceSync((current) => ({ ...current, author: author ?? undefined })),
    // Сервер обменивается с удалённым напрямую: посредник ему не нужен.
    setCorsProxy: (url) =>
      server
        ? Promise.resolve()
        : writeDeviceSync((current) => ({ ...current, cors_proxy: url ?? undefined })),
  };
}

/**
 * Жест «создать vault из удалённого репозитория» для нативных оболочек.
 *
 * Выбор пустой папки платформенный (диалог ОС на рабочем столе, свой обзор на
 * Android) и приходит аргументом — как у `tauriDeviceSettings`. `open` —
 * обычное открытие vault по пути (`openVaultAt`): после клонирования папка
 * открывается тем же путём, что и любая другая.
 */
export function tauriVaultCloner(
  pickFolder: () => Promise<string | null>,
  open: (path: string) => Promise<VaultOpenResult>,
): VaultCloner {
  const { invoke } = tauriTransport;
  return {
    capabilities: async () =>
      remoteCapabilities(await invoke<ShellCapabilities>("sync_capabilities")),
    cancel: () => void invoke("sync_cancel"),
    async run(request) {
      const path = await pickFolder();
      if (path === null) return { ok: false, error: { kind: "PickerCancelled" } };
      try {
        // Запись в списке известных vault нужна до клонирования: под её
        // идентификатором сохраняются данные доступа.
        const vaultId = await invoke<string>("add_known_vault", { path });
        await invoke<void>("sync_clone_vault", {
          path,
          vaultId,
          url: request.url,
          credentials: request.credentials,
        });
      } catch (cause) {
        throw new CloneFailure(asSyncError(cause));
      }
      return open(path);
    },
  };
}
