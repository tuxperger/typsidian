import type {
  Author,
  Change,
  Conflict,
  Content,
  Credentials,
  DeviceVaultSync,
  SyncProgress,
  FileDiff,
  HistoryPage,
  IgnoreCheck,
  IgnoreDocument,
  IgnoreFile,
  IgnoreReport,
  IgnoreSaveOutcome,
  IgnoreSaveRequest,
  Keeper,
  PullOutcome,
  RemoteStatus,
  Resolution,
  ShellCapabilities,
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
import type { VaultOpenResult } from "../vault-tree/useVault.js";

// Поверхность синхронизации ядра, которой пользуется общий UI (фича 011,
// contracts/core-sync-api.md).
//
// Та же форма, что у `VaultClient`: один интерфейс, две реализации. На web —
// `WebSyncSession` в том же процессе (`sync-client-web.ts`), на рабочем столе и
// Android — команды Tauri поверх `SyncSession<NativeGitEngine>`
// (`sync-client-tauri.ts`). Решений здесь нет (принцип I): что снимать, как
// совмещать, кто хранитель и когда делать автоснимок, отвечает ядро.
//
// Отказы обеих реализаций — значения `SyncError` ядра; `asSyncError` сужает
// пойманное до этого типа.

/** Что снять и с какими подтверждениями (FR-005…FR-007, FR-030). */
export interface SnapshotRequest {
  /** `null` — все изменения, массив — только выбранные пути. */
  paths: string[] | null;
  /** Пустое — сообщение составит ядро из изменённых заметок. */
  message: string;
  /** Начало автосообщения на языке интерфейса. */
  autoPrefix: string;
  /** Открытые заметки сохранены (`VaultClient.flushAll`). */
  flushed: boolean;
  /** Пользователь подтвердил действие на устройстве, которое не хранитель. */
  confirmNotKeeper: boolean;
}

/** Параметры локального Syncthing, найденные в его конфигурации (FR-022a). */
export interface DiscoveredSyncthing {
  /**
   * Только адрес: ключ API окну не отдаётся — подключение к найденному
   * Syncthing перечитывает его на стороне оболочки.
   */
  endpoint: SyncthingEndpoint;
}

export interface SyncClient {
  overview(): Promise<SyncOverview>;
  /** События сессии с прошлого вызова. */
  pollEvents(): Promise<SyncEvent[]>;
  /** Файлы vault изменились — отсчёт паузы автоснимка начинается заново. */
  noteChange(): Promise<void>;
  /** Отменяет текущую сетевую или массовую операцию (принцип X). */
  cancel(): void;
  /**
   * Ход идущей массовой операции — читается, пока она идёт: без ожидания
   * сессии и очереди вызовов (T075). `null` — операции нет.
   */
  progress(): Promise<SyncProgress | null>;
  /**
   * Состояние синхронизации изменилось само (Syncthing доставил файлы,
   * подключилось устройство — T078): окно перечитывает сводку, не дожидаясь
   * такта. Нет — оболочка не сообщает, сводка догоняет по такту.
   */
  onChanged?(listener: () => void): () => void;

  ignoreReports(): Promise<IgnoreReport[]>;
  applyIgnore(file: IgnoreFile): Promise<void>;
  declineStignore(): Promise<void>;

  /** Файл правил для редактора (фича 012). Не отказывает: причина — в `editable`. */
  ignoreOpen(file: IgnoreFile): Promise<IgnoreDocument>;
  /** Проверка текста редактора; к диску не обращается. */
  ignoreCheck(file: IgnoreFile, text: string): Promise<IgnoreCheck>;
  /** Текст с возвращённым блоком правил приложения. */
  ignoreRestore(file: IgnoreFile, text: string): Promise<string>;
  ignoreSave(request: IgnoreSaveRequest): Promise<IgnoreSaveOutcome>;
  enableHistory(message: string): Promise<Snapshot>;

  changes(): Promise<Change[]>;
  snapshot(request: SnapshotRequest): Promise<Snapshot>;

  history(path: string | null, cursor: string | null, limit: number): Promise<HistoryPage>;
  diff(path: string, from: Revision, to: Revision): Promise<FileDiff>;
  fileAt(path: string, revision: Revision): Promise<Content>;
  /** Что изменил снимок — по этому списку находят удалённый файл (FR-011). */
  snapshotChanges(snapshot: string): Promise<Change[]>;
  restore(path: string, snapshot: string): Promise<Content>;

  connectRemote(url: string, credentials: Credentials): Promise<RemoteStatus>;
  disconnectRemote(): Promise<void>;
  push(confirmNotKeeper: boolean): Promise<RemoteStatus>;
  pull(confirmNotKeeper: boolean): Promise<PullOutcome>;
  /**
   * Вернуть утраченную историю из удалённого репозитория: заводит пустую
   * локальную и получает в неё. Расхождения с файлами vault — конфликты, как
   * при обычном получении.
   */
  restoreHistory(confirmNotKeeper: boolean): Promise<PullOutcome>;
  abortPull(): Promise<void>;
  conflicts(): Promise<Conflict[]>;
  resolve(id: string, resolution: Resolution): Promise<void>;

  setKeeper(): Promise<Keeper>;
  clearKeeper(): Promise<void>;
  updateSettings(patch: VaultSyncSettingsPatch): Promise<VaultSyncSettings>;
  /** Обновляет зеркало удалённого репозитория на не-хранителе (FR-033). */
  refreshMirror(): Promise<void>;

  /** `null` — Syncthing не найден или оболочка его не ищет (FR-022a). */
  syncthingDiscover(): Promise<DiscoveredSyncthing | null>;
  /** `apiKey: null` — подключить найденный Syncthing с ключом из его конфигурации. */
  syncthingConnect(endpoint: SyncthingEndpoint, apiKey: string | null): Promise<void>;
  syncthingDisconnect(): Promise<void>;

  /** Такт автоматики (FR-019, FR-031): решения — в ядре, таймер — у окна. */
  tick(autoPrefix: string): Promise<TickOutcome>;

  /** Значения синхронизации этого устройства для открытого vault. */
  deviceSync(): Promise<DeviceVaultSync>;
  /** Автор снимков (FR-007); `null` — спрашивать общую настройку git. */
  setAuthor(author: Author | null): Promise<void>;
  /** Посредник для обмена из браузера (FR-037); `null` — без посредника. */
  setCorsProxy(url: string | null): Promise<void>;
}

/** Откуда и с чем клонировать vault (FR-013). */
export interface CloneRequest {
  url: string;
  credentials: Credentials;
  /** Только web: посредник для обращений из браузера (FR-037). */
  corsProxy: string | null;
}

/**
 * Жест «создать vault из удалённого репозитория». Платформенный, как и выбор
 * папки: пустую папку выбирает оболочка, поэтому приходит он в рабочую область
 * пропом. Отмена идёт через `cancel`, пока `run` не завершился.
 */
export interface VaultCloner {
  run(request: CloneRequest): Promise<VaultOpenResult>;
  cancel(): void;
  /** Что умеет оболочка — для подсказок диалога; отвечает ядро (FR-036). */
  capabilities(): Promise<RemoteCapabilities>;
}

/** Возможности оболочки, которые нужны диалогу удалённого репозитория. */
export interface RemoteCapabilities {
  ssh: boolean;
  directRemote: boolean;
  secureSecrets: boolean;
  /** Ядро работает на сервере (фича 038). */
  server: boolean;
}

/** Та же выборка из `ShellCapabilities` ядра. */
export function remoteCapabilities(capabilities: ShellCapabilities): RemoteCapabilities {
  return {
    ssh: capabilities.ssh,
    directRemote: capabilities.direct_remote,
    secureSecrets: capabilities.secure_secrets,
    server: capabilities.server ?? false,
  };
}

/** Отказ клонирования, пойманный оболочкой: причина — `SyncError` ядра. */
export class CloneFailure extends Error {
  constructor(readonly reason: SyncError) {
    super(reason.kind);
  }
}

const SYNC_ERROR_KINDS: ReadonlySet<string> = new Set<SyncError["kind"]>([
  "Offline",
  "Timeout",
  "AuthRequired",
  "AuthRejected",
  "NotFound",
  "RejectedNonFastForward",
  "CorsBlocked",
  "UnsupportedScheme",
  "InvalidRemoteUrl",
  "RemoteChanged",
  "NoRemote",
  "KeeperElsewhere",
  "KeeperDisputed",
  "InProgressOperation",
  "ExchangeRunning",
  "RemoteUnavailable",
  "PullConflictsPending",
  "NoHistory",
  "HistoryExists",
  "ForeignRepository",
  "StIgnoreNotConfirmed",
  "AuthorRequired",
  "NotFlushed",
  "NothingToSnapshot",
  "ConflictNotFound",
  "TargetNotEmpty",
  "Cancelled",
  "TooLarge",
  "Io",
]);

/**
 * Сужение пойманного до `SyncError` без `any` (принцип IV). Неизвестный отказ
 * сохраняет исходное описание как `Io` — общего «что-то пошло не так» нет.
 */
export function asSyncError(cause: unknown): SyncError {
  if (cause instanceof CloneFailure) return cause.reason;
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    const { kind } = cause as { kind: unknown };
    if (typeof kind === "string" && SYNC_ERROR_KINDS.has(kind)) return cause as SyncError;
  }
  const message = cause instanceof Error ? cause.message : String(cause);
  return { kind: "Io", message };
}
