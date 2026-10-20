//! Шов к движку git (фича 011, research §1, contracts/core-sync-api.md).
//!
//! Движок — внешний чёрный ящик по принципу VIII: он хранит объекты, индекс и
//! ссылки и ходит по сети. **Решений он не принимает**: какие файлы снимать,
//! что делать при расхождении, как совмещать правки и в каком порядке писать
//! файлы vault — всё это [`super::session`], одинаково на обоих таргетах.
//!
//! Отсюда форма трейта: операции низкого уровня, без политики. Переименования
//! движок тоже не ищет — их узнаёт сессия по совпадению содержимого, потому
//! что isomorphic-git их не ищет вовсе, а правило обязано быть одним.
//!
//! Рабочие файлы vault движок не пишет — кроме `clone_into` в пустую папку.
//! Слияние, откат и получение пишет сессия через `VaultBackend`: так и
//! атомарность записи (009), и обнаружение внешних изменений (001) остаются
//! теми же, что для любой другой записи.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(test)]
pub mod fake;
#[cfg(all(not(test), feature = "test-support"))]
pub mod fake;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(not(target_arch = "wasm32"))]
pub mod transport_http;
#[cfg(not(target_arch = "wasm32"))]
pub mod transport_ssh;
#[cfg(target_arch = "wasm32")]
pub mod web;

use super::Author;

/// Что лежит в папке vault с точки зрения истории.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum RepoProbe {
    Absent,
    /// История в самой папке vault.
    Local,
    /// Голое зеркало (не-хранитель, research §4).
    Mirror,
    /// Папка vault лежит внутри чужого репозитория. `prefix` — путь vault
    /// внутри него через `/`.
    ForeignParent {
        repo_root: String,
        prefix: String,
    },
}

/// Незавершённая операция, оставленная внешним инструментом.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum InProgressOp {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct HeadInfo {
    /// Имя текущей ветки; `None` — отделённый `HEAD`.
    pub branch: Option<String>,
    /// Снимок, на который указывает `HEAD`; `None` — снимков ещё нет.
    pub oid: Option<String>,
    pub in_progress: Option<InProgressOp>,
}

/// Изменение файла глазами движка: без переименований.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RawChange {
    /// Путь относительно корня vault, через `/`.
    pub path: String,
    pub status: RawStatus,
    /// Содержимое до и после — идентификаторы объектов. Для рабочего файла
    /// после — `None`: его хэш движку считать незачем, переименование рабочих
    /// файлов сессия узнаёт сама.
    pub old_oid: Option<String>,
    pub new_oid: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum RawStatus {
    Added,
    Modified,
    Deleted,
}

/// Снимок глазами движка.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LogEntry {
    pub id: String,
    pub message: String,
    pub author: Author,
    /// Время снимка, мс от эпохи.
    pub time: i64,
    pub parents: Vec<String>,
}

/// Страница журнала. `next` — смещение следующей страницы.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LogPage {
    pub entries: Vec<LogEntry>,
    pub next: Option<u32>,
}

/// Куда ходить по сети.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RemoteSpec {
    /// Адрес без учётных данных, уже нормализованный сессией
    /// (`user@host:path` → `ssh://user@host/path`).
    pub url: String,
    /// Ветка, которой обмениваемся.
    pub branch: String,
    /// Посредник для браузера (FR-037). Только web; native его не читает.
    pub cors_proxy: Option<String>,
}

/// Данные доступа на время одной операции. Нигде не сохраняются движком.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Auth {
    pub user: Option<String>,
    pub token: Option<String>,
}

/// Секрет не печатается ни в журнал, ни в сообщение об ошибке (FR-014).
impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Auth")
            .field("user", &self.user)
            .field("token", &self.token.as_ref().map(|_| "***"))
            .finish()
    }
}

/// Отказ движка. Переводится в [`super::SyncError`] сессией.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum EngineError {
    Offline,
    Timeout,
    AuthRequired,
    AuthRejected,
    NotFound,
    RejectedNonFastForward,
    CorsBlocked,
    UnsupportedScheme {
        scheme: String,
    },
    Cancelled,
    NotARepository,
    /// Файл истории или отправка больше предела памяти (T077).
    TooLarge {
        limit_mb: u32,
    },
    Io {
        message: String,
    },
}

/// Флаг отмены сетевой операции (принцип X).
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// Снимает отмену перед следующей операцией. Тот же флаг, а не новый:
    /// оболочка держит копию токена, чтобы отменять, не дожидаясь блокировки
    /// сессии, и эта копия обязана оставаться действительной.
    pub fn reset(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Движок git (research §1). Один экземпляр обслуживает одну историю: папку
/// vault или зеркало.
#[allow(async_fn_in_trait)]
pub trait GitEngine {
    async fn probe(&self) -> Result<RepoProbe, EngineError>;
    async fn init(&self, branch: &str) -> Result<(), EngineError>;
    async fn head(&self) -> Result<HeadInfo, EngineError>;
    /// Отличия рабочих файлов от `HEAD` с учётом правил исключения истории.
    async fn status(&self) -> Result<Vec<RawChange>, EngineError>;
    /// Снимок указанных путей поверх `HEAD`. `merge_parent` — второй предок
    /// снимка слияния после получения.
    async fn commit(
        &self,
        paths: &[String],
        message: &str,
        author: &Author,
        time: i64,
        merge_parent: Option<&str>,
    ) -> Result<String, EngineError>;
    /// Журнал от `start` (или `HEAD`) вглубь, по времени, постранично.
    async fn log(
        &self,
        start: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<LogPage, EngineError>;
    async fn read_blob(&self, rev: &str, path: &str) -> Result<Option<Vec<u8>>, EngineError>;
    /// Отличия деревьев двух снимков; `from = None` — от пустого дерева.
    async fn tree_diff(&self, from: Option<&str>, to: &str) -> Result<Vec<RawChange>, EngineError>;
    async fn merge_base(&self, a: &str, b: &str) -> Result<Option<String>, EngineError>;
    /// Передвигает текущую ветку на снимок и приводит индекс к его дереву.
    /// Рабочие файлы не трогает — их уже записала сессия.
    async fn set_head(&self, oid: &str) -> Result<(), EngineError>;
    /// Последнее известное локально состояние удалённой ветки.
    async fn remote_head(&self, branch: &str) -> Result<Option<String>, EngineError>;
    async fn ahead_behind(&self, local: &str, upstream: &str) -> Result<(u32, u32), EngineError>;
    async fn fetch(
        &self,
        remote: &RemoteSpec,
        auth: &Auth,
        cancel: &CancelToken,
    ) -> Result<Option<String>, EngineError>;
    async fn push(
        &self,
        remote: &RemoteSpec,
        auth: &Auth,
        cancel: &CancelToken,
    ) -> Result<(), EngineError>;
    /// Клонирует в пустую папку движка: рабочую копию vault или голое зеркало.
    async fn clone_into(
        &self,
        remote: &RemoteSpec,
        auth: &Auth,
        bare: bool,
        cancel: &CancelToken,
    ) -> Result<(), EngineError>;
    /// Записывает адрес `origin` (без учётных данных) или убирает его.
    async fn set_remote_url(&self, url: Option<&str>) -> Result<(), EngineError>;
    /// Автор из глобальных настроек git устройства (FR-007), если они есть.
    async fn global_author(&self) -> Option<Author>;
}
