import type {
  Change,
  Conflict,
  Content,
  Credentials,
  DeviceVaultSync,
  FileDiff,
  HistoryPage,
  IgnoreFile,
  IgnoreReport,
  Keeper,
  PullOutcome,
  RemoteStatus,
  Resolution,
  Revision,
  Snapshot,
  SyncError,
  SyncEvent,
  SyncOverview,
  SyncthingEndpoint,
  TickOutcome,
  VaultSyncSettings,
  VaultSyncSettingsPatch,
} from "../wasm-core/typsidian_core.js";

// Поверхность синхронизации, которой пользуется общий UI (фича 011,
// contracts/core-sync-api.md).
//
// Та же причина, что у `VaultClient`: на web сессия живёт в том же процессе
// (`WebSyncSession`, WASM), на рабочем столе и Android — за командами Tauri
// поверх `SyncSession<NativeGitEngine>`. Формы вызовов совпадают, решений здесь
// нет (принцип I): клиент только доставляет вызов и значения устройства.
//
// Отказы приходят отклонённым промисом со значением `SyncError` — см.
// [`asSyncError`].

/** Параметры локального Syncthing, найденные в его конфигурации (FR-022a). */
export interface DiscoveredSyncthing {
  endpoint: SyncthingEndpoint;
  apiKey: string;
}

export interface SnapshotOptions {
  /** Начало автоматического сообщения — из каталога переводов (FR-039). */
  autoPrefix: string;
  /** Открытые заметки сохранены на диск (FR-006); без этого ядро откажет. */
  flushed: boolean;
  /** Пользователь подтвердил действие не на хранителе (FR-030). */
  confirmNotKeeper: boolean;
}

export interface SyncClient {
  overview(): Promise<SyncOverview>;
  /** События сессии с прошлого вызова. Опрашивает `useSync`. */
  pollEvents(): Promise<SyncEvent[]>;
  /** Файлы vault изменились — пауза автоснимка отсчитывается заново (FR-031). */
  noteChange(): Promise<void>;
  /** Отмена текущей сетевой или массовой операции (принцип X). */
  cancel(): void;

  // ── Правила исключения и включение истории (US1) ────────────────────
  ignoreReports(): Promise<IgnoreReport[]>;
  applyIgnore(file: IgnoreFile): Promise<void>;
  declineStignore(): Promise<void>;
  enableHistory(message: string): Promise<Snapshot>;

  // ── Снимки и история (US1, US2) ─────────────────────────────────────
  changes(): Promise<Change[]>;
  /** `paths: null` — снять все изменения. */
  snapshot(paths: string[] | null, message: string, options: SnapshotOptions): Promise<Snapshot>;
  /** `path: null` — история всего vault. */
  history(path: string | null, cursor: string | null, limit: number): Promise<HistoryPage>;
  diff(path: string, from: Revision, to: Revision): Promise<FileDiff>;
  fileAt(path: string, revision: Revision): Promise<Content>;
  restore(path: string, snapshot: string): Promise<Content>;

  // ── Удалённый репозиторий (US3) ─────────────────────────────────────
  connectRemote(url: string, credentials: Credentials): Promise<RemoteStatus>;
  disconnectRemote(): Promise<void>;
  push(confirmNotKeeper: boolean): Promise<RemoteStatus>;
  pull(confirmNotKeeper: boolean): Promise<PullOutcome>;
  abortPull(): Promise<void>;
  conflicts(): Promise<Conflict[]>;
  resolve(id: string, resolution: Resolution): Promise<void>;
  /** Обновить зеркало истории на не-хранителе (FR-033). */
  refreshMirror(): Promise<void>;

  // ── Хранитель и настройки (US5) ─────────────────────────────────────
  setKeeper(): Promise<Keeper>;
  clearKeeper(): Promise<void>;
  updateSettings(patch: VaultSyncSettingsPatch): Promise<VaultSyncSettings>;
  /** Такт автоматики: решает ядро, таймер — у `useSync` (research §12). */
  tick(autoPrefix: string): Promise<TickOutcome>;

  // ── Syncthing (US4) ─────────────────────────────────────────────────
  /** `null` — не найден или оболочка искать не умеет (FR-022a). */
  syncthingDiscover(): Promise<DiscoveredSyncthing | null>;
  syncthingConnect(endpoint: SyncthingEndpoint, apiKey: string): Promise<void>;
  syncthingDisconnect(): Promise<void>;

  // ── Значения устройства для этого vault ─────────────────────────────
  /** Автор снимков, посредник для браузера и прочее, что не доставляется с vault. */
  deviceSync(): Promise<DeviceVaultSync>;
  /** Заменяет указанные поля; остальные остаются как были. */
  updateDeviceSync(patch: Partial<DeviceVaultSync>): Promise<DeviceVaultSync>;
}

/**
 * Сужение отказа клиента до `SyncError` без `any` (принцип IV).
 *
 * Отказ, который ядро не формулировало (сбой границы процесса, исключение
 * браузера), сохраняет исходный текст в `Io`, а не превращается в «что-то
 * пошло не так».
 */
export function asSyncError(cause: unknown): SyncError {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    const { kind } = cause as { kind: unknown };
    if (typeof kind === "string") {
      // Оболочка отвечает так, пока vault не открыт: для UI это «истории нет».
      if (kind === "NoVaultOpen") return { kind: "NoHistory" };
      return cause as SyncError;
    }
  }
  return { kind: "Io", message: cause instanceof Error ? cause.message : String(cause) };
}
