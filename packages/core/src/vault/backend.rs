use crate::vault::conflict::{DiffLine, EditConflict};
use crate::vault::events::VaultEvent;
use crate::vault::index::{FileStamp, Note};
use crate::vault::AccessState;
use std::path::{Path, PathBuf};

/// Проверка имени файла заметки на допустимость для файловой системы (FR-013):
/// непустое, без разделителей пути (`/`, `\`) и без NUL-байта. Общая для
/// `NativeBackend` и `WebBackend` (принцип II, NON-NEGOTIABLE) — оба backend
/// MUST отклонять один и тот же набор имён, а не расходиться по платформе.
pub fn is_valid_note_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', '\0'])
}

/// Извлекает и валидирует конечный компонент имени из пути операции
/// (`create`'s `path`, `rename`'s `to`) — `None`, если путь не заканчивается
/// допустимым именем (пустой путь, спецсимволы и т.п.). Вызывается ДО любого
/// обращения к диску (FR-013: «...до внесения каких-либо изменений на диск»).
pub fn valid_note_name(path: &Path) -> Option<&str> {
    path.file_name()
        .and_then(|n| n.to_str())
        .filter(|n| is_valid_note_name(n))
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum OpenError {
    #[error("папка не найдена")]
    NotFound,
    #[error("нет прав доступа к папке")]
    AccessDenied,
    #[error("путь не является папкой")]
    NotADirectory,
    /// FR-038: заметок больше [`crate::vault::SUPPORTED_NOTE_COUNT`]. Отдельный
    /// вариант, а не `Io`: пользователю MUST быть названа именно эта причина,
    /// а приложение MUST остаться работоспособным — это отказ открыть
    /// конкретную папку, а не сбой.
    #[error("vault превышает поддерживаемый размер")]
    TooManyNotes,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ReadError {
    #[error("заметка не найдена")]
    NotFound,
    #[error("не удалось прочитать содержимое")]
    Io,
    /// Заметка больше [`crate::vault::NOTE_SIZE_LIMIT_BYTES`] (spec Assumptions,
    /// Edge Cases). Сама заметка остаётся в индексе и видна в дереве — отказ
    /// касается только открытия, и причина обязана быть названа.
    #[error("заметка превышает поддерживаемый размер")]
    TooLarge,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum WriteError {
    #[error("нет прав на запись")]
    AccessDenied,
    #[error("версия на диске разошлась с ожидаемой")]
    Conflict,
    #[error("не удалось записать на диск")]
    Io,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum CreateError {
    #[error("имя уже занято в этой папке")]
    NameConflict,
    #[error("имя недопустимо для файловой системы")]
    InvalidName,
    /// FR-036: права на запись отозваны. Отдельный вариант, а не `Io`, —
    /// причина отказа обязана быть названа пользователем, а «не удалось
    /// создать файл» причиной не является.
    #[error("нет прав на запись")]
    AccessDenied,
    #[error("не удалось создать файл")]
    Io,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum RenameError {
    #[error("имя уже занято в этой папке")]
    NameConflict,
    #[error("имя недопустимо для файловой системы")]
    InvalidName,
    #[error("заметка не найдена")]
    NotFound,
    /// FR-036 (права на запись отозваны) и FR-020 (vault недоступен —
    /// переименование запрещено явно, как разрушительное действие).
    #[error("нет прав на запись")]
    AccessDenied,
    #[error("не удалось переименовать файл")]
    Io,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum DeleteError {
    #[error("заметка не найдена")]
    NotFound,
    /// FR-036 (права на запись отозваны) и FR-020 (vault недоступен —
    /// удаление запрещено явно, как разрушительное действие).
    #[error("нет прав на запись")]
    AccessDenied,
    #[error("не удалось удалить файл")]
    Io,
}

/// Отказ разрешения конфликта редактирования (FR-022, FR-023). Отдельный от
/// `WriteError` тип: «разрешать нечего» — не ошибка записи, а рассогласование
/// UI с ядром (диалог показан по конфликту, которого уже нет), и UI обязан
/// отличать её от отказа диска.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ResolveError {
    #[error("для этой заметки нет неразрешённого конфликта")]
    NoPendingConflict,
    #[error("нет прав на запись")]
    AccessDenied,
    #[error("не удалось прочитать или записать заметку")]
    Io,
}

impl From<WriteError> for ResolveError {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::AccessDenied => ResolveError::AccessDenied,
            // `Conflict` здесь недостижим: разрешение конфликта пишет без
            // `expected`-снимка — пользователь уже сделал явный выбор.
            WriteError::Conflict | WriteError::Io => ResolveError::Io,
        }
    }
}

/// Единственная точка входа в vault-логику (принцип II конституции,
/// NON-NEGOTIABLE): один трейт, две реализации (`NativeBackend`, `WebBackend`) за
/// одной абстракцией. Контракт — contracts/vault-backend-trait.md.
///
/// `async fn` в трейте — осознанный выбор: у нас нет `dyn VaultBackend` (каждый
/// таргет выбирает конкретный тип через cfg на этапе компиляции), поэтому
/// отсутствие явных auto-trait bounds на будущем не является проблемой.
#[allow(async_fn_in_trait)]
pub trait VaultBackend: Sized {
    async fn open(root_path: &Path) -> Result<Self, OpenError>;

    fn root_path(&self) -> &Path;
    fn access_state(&self) -> AccessState;

    async fn list(&self) -> Vec<Note>;
    async fn list_folder(&self, folder: &Path) -> Vec<Note>;
    async fn read(&self, path: &Path) -> Result<String, ReadError>;

    /// `expected` — снимок диска, от которого отталкивался редактор. Если снимок
    /// на диске разошёлся — `WriteError::Conflict`, а не молчаливая перезапись
    /// (FR-021).
    async fn write(
        &mut self,
        path: &Path,
        content: &str,
        expected: Option<FileStamp>,
    ) -> Result<(), WriteError>;

    async fn create(&mut self, path: &Path) -> Result<(), CreateError>;

    /// Создаёт папку внутри vault, если её ещё нет (фича 002, FR-019).
    ///
    /// Появилась вместе со снимками вставок: снимок лежит в
    /// `<заметка>.assets/`, и эту папку создаёт первая же вставка
    /// (research.md §4 фичи 002). Отдельная операция, а не создание папок внутри
    /// `write`: «записать в несуществующую папку» обязано оставаться отказом
    /// (001 FR-014), иначе опечатка в пути молча заводит папку в vault
    /// пользователя. Существующая папка — не ошибка: вторая вставка в ту же
    /// заметку вызывает это на уже созданной папке.
    async fn ensure_folder(&mut self, path: &Path) -> Result<(), WriteError>;
    async fn rename(&mut self, from: &Path, to: &Path) -> Result<(), RenameError>;
    async fn delete(&mut self, path: &Path) -> Result<(), DeleteError>;

    /// Делегирует модулю `graph` (research.md §8) — vault-manager не парсит
    /// содержимое заметок сам.
    async fn backlinks(&self, path: &Path) -> Vec<Note>;

    /// Цели ссылок заметки, которых нет среди заметок vault (FR-030). Именно
    /// это позволяет пользователю найти битые ссылки после операции: сама
    /// таблица ссылок «битости» не знает — её определяет индекс.
    async fn broken_links(&self, path: &Path) -> Vec<PathBuf>;

    /// Забирает накопившиеся с прошлого вызова события внешних изменений
    /// (FR-016) — реализация `watch()` из contracts/vault-backend-trait.md в виде
    /// pull-опроса вместо push-потока, чтобы не требовать object-safe Stream.
    fn poll_events(&mut self) -> Vec<VaultEvent>;

    /// Редактор объявляет ядру несохранённые правки и передаёт сам буфер
    /// (FR-035: только в памяти работающего приложения). Без этого решение
    /// «поднять конфликт или тихо обновить» пришлось бы принимать в UI, где
    /// ему не место (принцип I): именно наличие буфера отличает `FR-017` от
    /// `FR-021`.
    async fn begin_edit(&mut self, path: &Path, buffer: String);

    /// Правки отброшены самим редактором (закрытие без сохранения по явному
    /// действию пользователя) — снимает буфер и всё, что от него зависело.
    async fn discard_edit(&mut self, path: &Path);

    fn has_unsaved_edits(&self, path: &Path) -> bool;

    /// Неразрешённый конфликт по заметке (FR-021), если он есть.
    fn pending_conflict(&self, path: &Path) -> Option<EditConflict>;

    /// Всё, что будет потеряно при закрытии окна прямо сейчас (FR-035, FR-037):
    /// неразрешённые конфликты, внешние удаления с правками и просто
    /// несохранённые буферы.
    fn unresolved_edit_paths(&self) -> Vec<PathBuf>;

    /// Отличия между версией пользователя и версией на диске (FR-024). Сам по
    /// себе конфликт не разрешает — пользователь возвращается к выбору.
    async fn conflict_diff(&self, path: &Path) -> Vec<DiffLine>;

    /// «Оставить свою версию» (FR-022, US4 AS2): записывает локальный буфер на
    /// диск без `expected`-снимка — пользователь уже сделал явный выбор
    /// перезаписать внешнюю версию.
    async fn resolve_conflict_keep_local(&mut self, path: &Path) -> Result<(), ResolveError>;

    /// «Принять внешнюю» (FR-022, FR-023, US4 AS3): отбрасывает локальный буфер
    /// и возвращает содержимое, которое редактор обязан показать вместо него.
    /// `Ok(None)` — внешней версии не существует: заметка удалена снаружи
    /// (FR-025), и «принять внешнюю» означает принять удаление.
    async fn resolve_conflict_accept_external(
        &mut self,
        path: &Path,
    ) -> Result<Option<String>, ResolveError>;
}
