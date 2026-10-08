import { invoke } from "@tauri-apps/api/core";
import type {
  Change,
  Conflict,
  Content,
  DeviceVaultSync,
  FileDiff,
  HistoryPage,
  IgnoreReport,
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

/**
 * Открывает сессию синхронизации для только что открытого vault и отдаёт её
 * клиента. `vaultId` — идентификатор записи в хранилище устройства.
 */
export async function openTauriSync(vaultId: string): Promise<SyncClient> {
  await invoke<void>("sync_open", { vaultId });
  return tauriSyncClient(vaultId);
}

function tauriSyncClient(vaultId: string): SyncClient {
  const deviceSync = async (): Promise<DeviceVaultSync> => {
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
    noteChange: () => invoke<void>("sync_note_change"),
    cancel: () => void invoke("sync_cancel"),

    ignoreReports: () => invoke<IgnoreReport[]>("sync_ignore_reports"),
    applyIgnore: (file) => invoke<void>("sync_apply_ignore", { file }),
    declineStignore: () => invoke<void>("sync_decline_stignore"),
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
      writeDeviceSync((current) => ({ ...current, author: author ?? undefined })),
    setCorsProxy: (url) =>
      writeDeviceSync((current) => ({ ...current, cors_proxy: url ?? undefined })),
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
