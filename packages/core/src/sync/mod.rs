//! Синхронизация vault (фича 011): история версий, удалённый репозиторий и
//! совместная работа с Syncthing по модели «хранитель истории».
//!
//! Раскладка:
//! - [`engine`] — шов к движку git (libgit2 на native, isomorphic-git в web);
//! - [`session`] — вся политика поверх движка и `VaultBackend`;
//! - чистые модули правил: [`ignore`], [`conflict_copy`], [`merge`], [`keeper`],
//!   [`autosnapshot`], [`syncthing`] — тестируются на обоих таргетах;
//! - [`settings`] — ключ `sync` файла `.typsidian/vault.json`;
//! - [`secrets`] — шов к защищённому хранилищу устройства.
//!
//! Типы здесь — те, что пересекают границу ядра (data-model.md).

pub mod autosnapshot;
pub mod conflict_copy;
pub mod device;
pub mod engine;
pub mod ignore;
pub mod ignore_match;
pub mod keeper;
pub mod merge;
pub mod secrets;
pub mod session;
pub mod settings;
pub mod syncthing;

#[cfg(target_arch = "wasm32")]
pub mod web;

use serde::{Deserialize, Serialize};

pub use device::DeviceIdentity;
pub use engine::{CancelToken, GitEngine, InProgressOp};
pub use session::SyncSession;
pub use settings::{Keeper, VaultSyncSettings};

/// Время, мс от эпохи (UTC).
pub type Millis = i64;

/// Автор снимка (FR-007).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Author {
    pub name: String,
    pub email: String,
}

impl Author {
    pub fn is_valid(&self) -> bool {
        !self.name.trim().is_empty() && !self.email.trim().is_empty()
    }
}

/// Вид изменения файла.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed { from: String },
}

/// Изменение файла (FR-004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Change {
    pub path: String,
    pub change: ChangeKind,
}

/// Снимок (FR-009).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Snapshot {
    pub id: String,
    /// Первая строка сообщения.
    pub summary: String,
    pub message: String,
    pub author: Author,
    pub time: Millis,
    pub parents: Vec<String>,
    /// Для истории файла: как назывался файл в этом снимке и что с ним
    /// произошло. Для истории vault — `None`.
    pub path: Option<String>,
    pub change: Option<ChangeKind>,
}

/// Страница истории. `cursor` — продолжение, `None` — дальше пусто.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct HistoryPage {
    pub snapshots: Vec<Snapshot>,
    pub cursor: Option<String>,
}

/// Содержимое файла в одной из версий.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum Content {
    Text {
        text: String,
    },
    Binary {
        size: u64,
    },
    /// Файла в этой версии нет.
    Missing,
}

impl Content {
    pub fn from_bytes(bytes: Option<Vec<u8>>) -> Self {
        match bytes {
            None => Content::Missing,
            Some(bytes) if merge::is_binary(&bytes) => Content::Binary {
                size: bytes.len() as u64,
            },
            Some(bytes) => Content::Text {
                // `is_binary` уже проверил UTF-8.
                text: String::from_utf8(bytes).unwrap_or_default(),
            },
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Content::Text { text } => Some(text),
            _ => None,
        }
    }
}

/// Версия, с которой сравнивают (FR-010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum Revision {
    /// Файл на диске сейчас.
    Working,
    /// Последний снимок.
    Head,
    Snapshot {
        id: String,
    },
    /// Предок снимка — «что изменилось в этом снимке».
    ParentOf {
        id: String,
    },
}

/// Сравнение двух версий файла.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum FileDiff {
    /// `Local` — строка только в новой версии, `External` — только в старой.
    Text {
        lines: Vec<crate::vault::conflict::DiffLine>,
    },
    Binary {
        old_size: Option<u64>,
        new_size: Option<u64>,
    },
}

/// Состояние истории vault на этом устройстве.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum HistoryState {
    /// Истории нет и не было.
    Absent,
    /// История была при прошлом открытии, а теперь её нет (Edge Cases).
    Lost,
    /// Vault внутри чужого репозитория (Edge Cases).
    ForeignParent { repo_root: String },
    /// Не-хранитель: история из зеркала удалённого репозитория (FR-033).
    Mirror { fetched_at: Option<Millis> },
    Local {
        branch: Option<String>,
        in_progress: Option<InProgressOp>,
        changes: u32,
    },
}

/// Обмен с удалённым репозиторием (FR-015).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RemoteStatus {
    pub url: String,
    /// Снимков, не отправленных на сервер / не полученных с него — по
    /// последним известным сведениям.
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    /// Когда сведения о сервере обновлялись последний раз.
    pub fetched_at: Option<Millis>,
    pub last_error: Option<SyncError>,
}

/// Причина отказа операции синхронизации. Тексты — в локализации (FR-039).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum SyncError {
    #[error("нет сети")]
    Offline,
    #[error("сервер не ответил вовремя")]
    Timeout,
    #[error("нужны данные доступа")]
    AuthRequired,
    #[error("данные доступа отклонены")]
    AuthRejected,
    #[error("удалённый репозиторий не найден")]
    NotFound,
    #[error("на сервере есть снимки, которых нет здесь")]
    RejectedNonFastForward,
    #[error("сервер не разрешает обращения из браузера")]
    CorsBlocked,
    #[error("способ доступа «{scheme}» недоступен здесь")]
    UnsupportedScheme { scheme: String },
    #[error("адрес удалённого репозитория негоден")]
    InvalidRemoteUrl,
    /// Адрес в настройках vault — не тот, что подтверждён на этом устройстве.
    /// Настройки vault доставляются вместе с ним и доступны на запись всем, с
    /// кем он общий; обмен и токен — только подтверждённому адресу (T068).
    #[error("адрес удалённого репозитория изменён не на этом устройстве")]
    RemoteChanged { url: String },
    #[error("удалённый репозиторий не подключён")]
    NoRemote,
    #[error("снимки делает другое устройство")]
    KeeperElsewhere { keeper: Keeper },
    #[error("хранитель истории не определён однозначно")]
    KeeperDisputed,
    #[error("в истории не завершена операция")]
    InProgressOperation,
    /// Обмен с удалённым этого vault уже идёт (фича 038): второй параллельный
    /// занял бы ещё поток и делил бы с первым отмену.
    #[error("обмен уже идёт")]
    ExchangeRunning,
    /// Удалённый репозиторий в этой оболочке не подключается (демо-сервер, 039).
    #[error("удалённый репозиторий недоступен")]
    RemoteUnavailable,
    #[error("есть неразрешённые конфликты получения")]
    PullConflictsPending,
    #[error("истории нет")]
    NoHistory,
    #[error("история уже включена")]
    HistoryExists,
    #[error("vault лежит внутри другого репозитория")]
    ForeignRepository,
    #[error("правила исключения Syncthing не подтверждены")]
    StIgnoreNotConfirmed,
    #[error("не указан автор снимков")]
    AuthorRequired,
    #[error("открытые заметки не сохранены")]
    NotFlushed,
    #[error("изменений нет")]
    NothingToSnapshot,
    #[error("конфликт не найден")]
    ConflictNotFound,
    #[error("папка для vault не пуста")]
    TargetNotEmpty,
    #[error("операция отменена")]
    Cancelled,
    #[error("файл истории или отправка больше {limit_mb} МБ")]
    TooLarge { limit_mb: u32 },
    #[error("ошибка чтения или записи")]
    Io { message: String },
}

impl From<engine::EngineError> for SyncError {
    fn from(error: engine::EngineError) -> Self {
        use engine::EngineError as E;
        match error {
            E::Offline => SyncError::Offline,
            E::Timeout => SyncError::Timeout,
            E::AuthRequired => SyncError::AuthRequired,
            E::AuthRejected => SyncError::AuthRejected,
            E::NotFound => SyncError::NotFound,
            E::RejectedNonFastForward => SyncError::RejectedNonFastForward,
            E::CorsBlocked => SyncError::CorsBlocked,
            E::UnsupportedScheme { scheme } => SyncError::UnsupportedScheme { scheme },
            E::Cancelled => SyncError::Cancelled,
            E::NotARepository => SyncError::NoHistory,
            E::TooLarge { limit_mb } => SyncError::TooLarge { limit_mb },
            E::Io { message } => SyncError::Io { message },
        }
    }
}

/// Что умеет текущая оболочка (FR-036, FR-037).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ShellCapabilities {
    /// Доступ по ключам (desktop).
    pub ssh: bool,
    /// Обмен с любым сервером напрямую; `false` — только с разрешающими
    /// обращения из браузера или через посредника.
    pub direct_remote: bool,
    /// Можно читать состояние Syncthing.
    pub syncthing_status: bool,
    /// Секреты лежат в защищённом хранилище ОС, а не в хранилище браузера.
    pub secure_secrets: bool,
    /// Ядро работает на сервере (фича 038): «это устройство» — сервер, и адреса
    /// на нём (каталог, `http://localhost`) — файлы и службы сервера, в том
    /// числе чужие vault. Такие адреса удалённого репозитория отклоняются.
    #[serde(default)]
    pub server: bool,
    /// Можно подключать удалённый репозиторий (фича 039: на демо-сервере нельзя).
    #[serde(default = "remote_default")]
    pub remote: bool,
}

fn remote_default() -> bool {
    true
}

impl ShellCapabilities {
    /// Возможности сборки, в которой работает ядро.
    pub fn current(android: bool) -> Self {
        if cfg!(target_arch = "wasm32") {
            Self {
                ssh: false,
                direct_remote: false,
                syncthing_status: false,
                secure_secrets: false,
                server: false,
                remote: true,
            }
        } else {
            Self {
                ssh: !android,
                direct_remote: true,
                syncthing_status: true,
                secure_secrets: true,
                server: false,
                remote: true,
            }
        }
    }

    /// Возможности сервера (фича 038): обмен напрямую и только по HTTPS, без ssh
    /// и Syncthing; секреты — в файле аккаунта на сервере.
    pub fn server() -> Self {
        Self {
            ssh: false,
            direct_remote: true,
            syncthing_status: false,
            secure_secrets: true,
            server: true,
            remote: true,
        }
    }
}

/// Значения синхронизации, принадлежащие устройству (data-model
/// «DeviceVaultSync»), — по одному на vault. Не доставляются с vault.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(default)]
pub struct DeviceVaultSync {
    pub author: Option<Author>,
    /// Посредник для обмена из браузера (FR-037).
    pub cors_proxy: Option<String>,
    pub syncthing: Option<syncthing::SyncthingEndpoint>,
    /// Хранитель, которого устройство видело в прошлый раз (research §4).
    pub last_seen_keeper: Option<String>,
    /// Пользователь явно отказался от правил исключения Syncthing (FR-028).
    pub stignore_declined: bool,
    /// История была при прошлом открытии (Edge Cases: «история удалена»).
    pub last_seen_history: bool,
}

impl DeviceVaultSync {
    /// Значения в допустимом виде: посредник, который посредником быть не
    /// может (не `https` и не адрес этого устройства, либо с вписанными
    /// учётными данными), не сохраняется — см. [`settings::remote_url::is_proxy`].
    pub fn normalized(mut self) -> Self {
        self.cors_proxy = self
            .cors_proxy
            .map(|proxy| proxy.trim().to_string())
            .filter(|proxy| settings::remote_url::is_proxy(proxy));
        self
    }
}

/// Запись `DeviceVaultSync` для одного vault в настройках устройства.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DeviceVaultSyncEntry {
    pub vault: crate::settings::VaultId,
    pub value: DeviceVaultSync,
}

/// Всё, что сессии нужно знать об устройстве на время одного вызова.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SyncContext {
    pub device: Option<DeviceIdentity>,
    pub device_sync: DeviceVaultSync,
    pub now: Millis,
    pub capabilities: ShellCapabilities,
}

/// Сводка для индикатора и панели (FR-035).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SyncOverview {
    pub history: HistoryState,
    pub remote: Option<RemoteStatus>,
    pub syncthing: syncthing::SyncthingFolder,
    pub keeper: Option<Keeper>,
    pub keeper_state: keeper::KeeperState,
    pub last_snapshot_at: Option<Millis>,
    /// Конфликтные копии Syncthing и конфликты получения.
    pub conflicts: u32,
    pub pull_in_progress: bool,
    pub ignore_warnings: Vec<ignore::IgnoreReport>,
    pub settings: VaultSyncSettings,
    pub capabilities: ShellCapabilities,
}

/// Событие синхронизации — для индикатора без опроса.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum SyncEvent {
    /// Что-то изменилось — пора перечитать сводку.
    OverviewChanged,
    Progress {
        operation: Operation,
        done: u32,
        total: u32,
    },
    AutoSnapshotDone {
        snapshot: Snapshot,
    },
    /// Ошибка фонового обмена (FR-019): в индикатор, не во всплывающее окно.
    ExchangeFailed {
        error: SyncError,
    },
    /// Файлы vault переписаны получением или откатом: открытые заметки
    /// обновляются тем же путём, что и при внешнем изменении (001).
    FilesChanged {
        paths: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Operation {
    FirstSnapshot,
    Pull,
    Push,
    Clone,
    /// Пачка решений по конфликтам (фича 040).
    Resolve,
}

/// Ход массовой операции: что делается и сколько из скольких (принцип X).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SyncProgress {
    pub operation: Operation,
    pub done: u32,
    pub total: u32,
}

/// Последний ход идущей операции — общий у сессии и оболочки (T075). Событие
/// `Progress` очереди сессии читается только между операциями: пока операция
/// идёт, сессия занята. Приёмник читается в любой момент, без блокировки
/// сессии и без очереди вызовов wasm.
#[derive(Debug, Clone, Default)]
pub struct ProgressSink(std::sync::Arc<std::sync::Mutex<Option<SyncProgress>>>);

impl ProgressSink {
    pub fn set(&self, progress: Option<SyncProgress>) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = progress;
        }
    }

    /// Ход сейчас; `None` — массовой операции нет.
    pub fn current(&self) -> Option<SyncProgress> {
        self.0.lock().ok().and_then(|slot| *slot)
    }
}

/// Изменение настроек устройства, выдающее ему тождество, если его ещё нет
/// (research §4). `None` — тождество уже есть или случайность недоступна.
pub fn ensure_device_change(
    settings: &crate::settings::Settings,
) -> Option<crate::settings::apply::SettingsChange> {
    if settings.device.is_some() {
        return None;
    }
    let device = DeviceIdentity::generate(DeviceIdentity::default_name())?;
    Some(crate::settings::apply::SettingsChange {
        device: Some(device),
        ..Default::default()
    })
}

/// Изменение настроек устройства с новыми значениями синхронизации vault.
pub fn device_sync_change(
    vault: crate::settings::VaultId,
    value: DeviceVaultSync,
) -> crate::settings::apply::SettingsChange {
    crate::settings::apply::SettingsChange {
        vault_sync: Some(DeviceVaultSyncEntry { vault, value }),
        ..Default::default()
    }
}

/// Собирает контекст вызова из настроек устройства (оболочкам незачем знать,
/// какие из значений устройства нужны сессии).
pub fn context_for(
    settings: &crate::settings::Settings,
    vault: &crate::settings::VaultId,
    now: Millis,
    capabilities: ShellCapabilities,
) -> SyncContext {
    SyncContext {
        device: settings.device.clone(),
        device_sync: settings.vault_sync_for(vault),
        now,
        capabilities,
    }
}
