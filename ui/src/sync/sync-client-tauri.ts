import { invoke } from "@tauri-apps/api/core";
import type {
  ApplyPlan,
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
  Snapshot,
  SyncEvent,
  SyncOverview,
  SyncthingEndpoint,
  TickOutcome,
  VaultSyncSettings,
} from "../wasm-core/typsidian_core.js";
import type { SyncClient } from "./sync-client.js";

// `SyncClient` поверх команд Tauri (фича 011, T018) — общий для рабочего стола
// и Android. Каждый метод — один `invoke` команды `sync_*`
// (`packages/shell/src/sync.rs`), за которой стоит `SyncSession` ядра. Контекст
// устройства и сохранение его значений делает оболочка; очередь вызовов здесь
// не нужна — у команд свои мьютексы.

/** Ответ `sync_syncthing_discover`. */
interface ShellDiscovered {
  endpoint: SyncthingEndpoint;
  api_key: string;
}

/** Идентификатор открытого vault в реестре устройства; `null` — не записан. */
async function currentVaultId(): Promise<string | null> {
  const snapshot = await invoke<{ settings: Settings }>("device_settings");
  return snapshot.settings.current_vault ?? null;
}

async function readDeviceSync(vault: string): Promise<DeviceVaultSync> {
  const snapshot = await invoke<{ settings: Settings }>("device_settings");
  const entry = snapshot.settings.vault_sync?.find((known) => known.vault === vault);
  return entry?.value ?? {};
}

/**
 * Открывает сессию синхронизации для только что открытого vault.
 *
 * `null` — vault не записан в реестре устройства (хранилище настроек
 * недоступно): без идентификатора секреты и значения устройства привязать не
 * к чему, и синхронизация в этом окне не предлагается.
 */
export async function openTauriSync(): Promise<SyncClient | null> {
  let vaultId: string | null;
  try {
    vaultId = await currentVaultId();
    if (vaultId === null) return null;
    await invoke<void>("sync_open", { vaultId });
  } catch {
    return null;
  }
  return tauriSyncClient(vaultId);
}

function tauriSyncClient(vault: string): SyncClient {
  return {
    overview: () => invoke<SyncOverview>("sync_overview"),
    pollEvents: () => invoke<SyncEvent[]>("sync_poll_events"),
    noteChange: () => invoke<void>("sync_note_change"),
    cancel: () => void invoke<void>("sync_cancel"),

    ignoreReports: () => invoke<IgnoreReport[]>("sync_ignore_reports"),
    applyIgnore: (file) => invoke<void>("sync_apply_ignore", { file }),
    declineStignore: () => invoke<void>("sync_decline_stignore"),
    enableHistory: (message) => invoke<Snapshot>("sync_enable_history", { message }),

    changes: () => invoke<Change[]>("sync_changes"),
    snapshot: (paths, message, options) =>
      invoke<Snapshot>("sync_snapshot", {
        paths,
        message,
        autoPrefix: options.autoPrefix,
        flushed: options.flushed,
        confirmNotKeeper: options.confirmNotKeeper,
      }),
    history: (path, cursor, limit) =>
      invoke<HistoryPage>("sync_history", { path, cursor, limit }),
    diff: (path, from, to) => invoke<FileDiff>("sync_diff", { path, from, to }),
    fileAt: (path, revision) => invoke<Content>("sync_file_at", { path, revision }),
    restore: (path, snapshot) => invoke<Content>("sync_restore", { path, snapshot }),

    connectRemote: (url, credentials) =>
      invoke<RemoteStatus>("sync_connect_remote", {
        url,
        // `Option` ядра читает отсутствие поля и `null` одинаково.
        credentials: { user: credentials.user ?? null, token: credentials.token ?? null },
      }),
    disconnectRemote: () => invoke<void>("sync_disconnect_remote"),
    push: (confirmNotKeeper) => invoke<RemoteStatus>("sync_push", { confirmNotKeeper }),
    pull: (confirmNotKeeper) => invoke<PullOutcome>("sync_pull", { confirmNotKeeper }),
    abortPull: () => invoke<void>("sync_abort_pull"),
    conflicts: () => invoke<Conflict[]>("sync_conflicts"),
    resolve: (id, resolution) => invoke<void>("sync_resolve", { id, resolution }),
    refreshMirror: () => invoke<void>("sync_refresh_mirror"),

    setKeeper: () => invoke<Keeper>("sync_set_keeper"),
    clearKeeper: () => invoke<void>("sync_clear_keeper"),
    updateSettings: (patch) => invoke<VaultSyncSettings>("sync_update_settings", { patch }),
    tick: (autoPrefix) => invoke<TickOutcome>("sync_tick", { autoPrefix }),

    syncthingDiscover: async () => {
      const found = await invoke<ShellDiscovered | null>("sync_syncthing_discover");
      return found === null ? null : { endpoint: found.endpoint, apiKey: found.api_key };
    },
    syncthingConnect: (endpoint, apiKey) =>
      invoke<void>("sync_syncthing_connect", { endpoint, apiKey }),
    syncthingDisconnect: () => invoke<void>("sync_syncthing_disconnect"),

    deviceSync: () => readDeviceSync(vault),
    updateDeviceSync: async (patch) => {
      // Запись vault заменяется целиком (`SettingsChange::vault_sync`), поэтому
      // остальные поля берутся из хранилища, а не из памяти окна.
      const value = { ...(await readDeviceSync(vault)), ...patch };
      await invoke<{ plan: ApplyPlan }>("apply_device_settings", {
        change: { vault_sync: { vault, value } },
      });
      return value;
    },
  };
}

/**
 * Клонирует удалённый репозиторий в пустую папку и записывает её в реестр
 * устройства (FR-013). Открывает vault вызывающая сторона — обычным жестом.
 */
export async function cloneTauriVault(
  path: string,
  url: string,
  credentials: { user?: string | undefined; token?: string | undefined },
): Promise<void> {
  // Идентификатор нужен до клонирования: под ним сохраняется токен.
  const vaultId = await invoke<string>("add_known_vault", { path });
  await invoke<void>("sync_clone_vault", {
    path,
    vaultId,
    url,
    credentials: { user: credentials.user ?? null, token: credentials.token ?? null },
  });
}
