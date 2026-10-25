//! Команды ядра для открытого vault — одна реализация для оболочки Tauri и сервера
//! (фича 037, research.md §1, конституция «Архитектурные ограничения → Сервер»).
//!
//! Здесь нет Tauri: команда — обычная функция, которой передают аргументы клиента и
//! состояния открытого vault (`&Vault`, `&Preview`, …). Оболочка (`packages/shell`)
//! оборачивает каждую в `#[tauri::command]` с прежним именем, сервер (`apps/server`)
//! вызывает их через [`dispatch::dispatch`] по имени. Обе обёртки строит
//! `scripts/dev/gen-host-dispatch.mjs`; руками их не правят.
//!
//! Правил про vault здесь по-прежнему нет: тела команд — получение backend из состояния
//! и вызов ядра (принцип I).

/// Имена, которыми пользуются команды. Обёртки оболочки берут отсюда же типы своих
/// сигнатур, поэтому импорты открыты.
pub mod prelude {

    pub use std::collections::HashMap;
    pub use std::path::{Path, PathBuf};
    pub use std::sync::Arc;
    pub use tokio::sync::Mutex;

    pub use typsidian_core::attachments;
    pub use typsidian_core::boards::{
        self, Board, BoardEvent, CreateBoardError, Embed, EmbedError, EmbedPlan, EmbedRef,
        ReadBoardError, SnapshotWriteError, WriteBoardError,
    };
    pub use typsidian_core::bookmarks;
    pub use typsidian_core::capture;
    pub use typsidian_core::completion;
    pub use typsidian_core::export;
    pub use typsidian_core::graph::view as graph_view;
    pub use typsidian_core::import;
    pub use typsidian_core::links;
    pub use typsidian_core::preview::{self, CompiledDocument, SnapshotError};
    pub use typsidian_core::properties;
    pub use typsidian_core::settings::apply::{
        self as settings_apply, ApplyPlan, ChangeTarget, RoutedChange, SettingsChange,
    };
    pub use typsidian_core::settings::diagnostics::{self, Diagnostics};
    pub use typsidian_core::settings::events::SettingsEvent;
    pub use typsidian_core::settings::folder_ref::{FolderRegistry, Resolved};
    pub use typsidian_core::settings::keybindings::{
        self as keybindings_core, CommandId, ResolvedBinding, Shortcut,
    };
    pub use typsidian_core::settings::native::NativeStore;
    pub use typsidian_core::settings::store::{
        SettingsStore, StorageState, StoreCapabilities, StoreError,
    };
    pub use typsidian_core::settings::vaults::{VaultId, VaultRecord};
    pub use typsidian_core::settings::Settings;
    pub use typsidian_core::slides;
    pub use typsidian_core::snapshots;
    pub use typsidian_core::sources;
    pub use typsidian_core::spellcheck;
    pub use typsidian_core::stamp::FileStamp;
    pub use typsidian_core::structure;
    pub use typsidian_core::sync::ignore::{
        self as sync_ignore, IgnoreCheck, IgnoreDocument, IgnoreFile, IgnoreReport,
        IgnoreSaveOutcome, IgnoreSaveRequest,
    };
    pub use typsidian_core::sync::session::{
        Conflict, Credentials, PullOutcome, Resolution, TickOutcome,
    };
    pub use typsidian_core::sync::settings::VaultSyncSettingsPatch;
    pub use typsidian_core::sync::{
        Author, Change, Content, DeviceVaultSync, FileDiff, HistoryPage, Keeper, RemoteStatus,
        Revision, ShellCapabilities, Snapshot, SyncError, SyncEvent, SyncOverview,
        VaultSyncSettings,
    };
    pub use typsidian_core::tasks;
    pub use typsidian_core::templates::{
        self, NameCheck, PreviewState, ReadTemplateError, SaveTemplateError, TemplateLibraryView,
    };
    pub use typsidian_core::vault::backend::{
        CreateError, DeleteError, OpenError, ReadError, ReadOnly, RenameError, ResolveError,
        VaultBackend, WriteError,
    };
    pub use typsidian_core::vault::conflict::{DiffLine, EditConflict};
    pub use typsidian_core::vault::events::VaultEvent;
    pub use typsidian_core::vault::index::Note;
    pub use typsidian_core::vault::native::NativeBackend;
    pub use typsidian_core::vault::relocate::{
        MoveError, MoveImpact, MoveOutcome, MoveRefusal, MoveRequest, MoveTarget, NoControl,
    };
    pub use typsidian_core::vault::vault_settings::{
        self, SetKeybindingError, VaultOverrides, VaultSettingsError,
    };
    pub use typsidian_core::vault::{classify, AccessState, FileKind};
    pub use typsidian_core::workspace;
}

use prelude::*;

/// Состояния одного открытого vault: всё, что нужно командам ядра.
///
/// Оболочка Tauri держит те же состояния по отдельности (`.manage(...)`); сервер —
/// один `Host` на пару (аккаунт, vault).
#[derive(Default)]
pub struct Host {
    pub vault: Vault,
    pub template_previews: TemplatePreviews,
    pub sources: Sources,
    pub preview: Preview,
    pub speller: Speller,
    pub highlight_sessions: HighlightSessions,
    pub properties_reindex: PropertiesReindex,
    pub import_source: ImportSource,
    /// Окружение синхронизации: на сервере — одно на аккаунт (фича 038).
    pub sync_home: Arc<SyncHome>,
    pub sync_state: SyncState,
    pub sync_cancel: SyncCancel,
    pub sync_progress_slot: SyncProgressSlot,
}

/// Право этой сборки писать доски (фича 002, FR-033, FR-034).
///
/// Здесь больше не ветвления по таргету. Раньше сборка под Android/iOS брала
/// `ViewOnly` — так требовал FR-034 в первой редакции спеки: рисование на
/// мобильном не входило в скоуп версии. Ограничение снято 2026-09-23: путь
/// записи доски (автосохранение, гейты конфликта, снимки) платформенно
/// нейтрален и на рабочем столе отработан, а единственным его следствием на
/// планшете был холст без единого инструмента.
///
/// Константой, а не полем состояния, это остаётся намеренно: профиль —
/// свойство клиента, и точкой, где он назначается, окно быть не должно. Сам
/// тип не исчезает и гейт в ядре тоже: `ViewOnly` — по-прежнему выразимое
/// состояние, и тесты ядра проверяют, что запись под ним отклоняется.
pub const BOARD_WRITE_PROFILE: boards::BoardWriteProfile = boards::BoardWriteProfile::ReadWrite;

/// Открытый vault. `None` — пользователь ещё не выбрал папку.
///
/// `Mutex`, а не `RwLock`: половина операций `VaultBackend` требует `&mut self`
/// (запись, `begin_edit`, разрешение конфликта), поэтому разделяемого чтения
/// всё равно не получится.
#[derive(Default)]
pub struct Vault(pub Mutex<Option<NativeBackend>>);

/// Кэш превью заготовок (фича 003, research.md §4).
///
/// Отдельным состоянием окна, а не полем `Vault`: он не принадлежит бэкенду и
/// не обязан переживать его — производное восстановимое состояние, ключуемое
/// хэшем содержимого. Отдельный `Mutex` затем, чтобы построение превью не
/// становилось вторым поводом держать блокировку самого vault.
#[derive(Default)]
pub struct TemplatePreviews(pub Mutex<templates::PreviewCache>);

/// Библиотека источников и индекс цитат (фича 017, research.md §8): состояние
/// живёт здесь, а не ездит через границу, и сбрасывается при смене vault.
#[derive(Default)]
pub struct Sources(pub Mutex<sources::SourceEngine>);

/// Движок предпросмотра (фича 005, T042): встроенные шрифты и кэш файлов
/// внешних шаблонов.
///
/// То же, чем на web является `PreviewEngine` — и по той же причине отдельным
/// состоянием, а не полем `Vault`: предпросмотр не читает vault и не владеет
/// им, а vault получает на время одного вызова. Своего `Mutex` здесь нет
/// намеренно: `PackageCache` синхронизируется изнутри, а `FontSet` только
/// клонируется на вызов — лишняя блокировка сделала бы сборку документа
/// поводом ждать другую сборку.
///
/// Время жизни — всё окно, и именно оно делает кэш шаблонов полезным: без него
/// заметка с большим внешним шаблоном поднимала бы его заново на каждое
/// нажатие клавиши.
pub struct Preview {
    pub fonts: preview::FontSet,
    pub packages: preview::PackageCache,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            fonts: preview::FontSet::builtin(),
            packages: preview::PackageCache::new(),
        }
    }
}

/// Vault ещё не открыт, а команда требует его.
///
/// Отдельный вариант, а не общий `Io`: UI обязан различать «папка не выбрана» и
/// «файл не прочитался» (FR-008).
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind")]
pub enum NoVault {
    NoVaultOpen,
}

pub type Answer<T, E> = Result<T, E>;

#[macro_export]
macro_rules! with_vault {
    ($state:expr, |$backend:ident| $body:expr) => {{
        let mut guard = $state.0.lock().await;
        let Some($backend) = guard.as_mut() else {
            return Err(NoVault::NoVaultOpen.into());
        };
        $body
    }};
}

/// Отказ команды: либо vault не открыт, либо причина от ядра.
///
/// Сериализуется как то же размеченное объединение, что и ошибки ядра
/// (`{"kind": …}`), поэтому общий UI разбирает отказы desktop и web одним кодом
/// (contracts/frontend-events.md).
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum Failure<E> {
    NoVault(NoVault),
    Core(E),
}

impl<E> From<NoVault> for Failure<E> {
    fn from(value: NoVault) -> Self {
        Failure::NoVault(value)
    }
}

// ── Закладки (фича 032) ─────────────────────────────────────────────────────

pub type BookmarkAnswer = Answer<bookmarks::BookmarksView, Failure<bookmarks::BookmarkError>>;

// ── Проверка орфографии (фича 030) ──────────────────────────────────────────

/// Проверяющий открытого vault: настройки и словарь vault (030 research.md §5).
/// Обычный `Mutex`: его берут и синхронная команда сессии подсветки, и
/// асинхронные — те держат его только между `await`, на копии.
#[derive(Default)]
pub struct Speller(pub std::sync::Mutex<spellcheck::SpellChecker>);

impl Speller {
    fn get(&self) -> spellcheck::SpellChecker {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn set(&self, checker: spellcheck::SpellChecker) {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = checker;
    }
}

// ── Предпросмотр документа (фича 005, T042) ─────────────────────────────────

/// Собирает заметку из текста редактора.
///
/// Текст приходит из буфера окна, а не с диска: иначе требование «без кнопки
/// сохранить» невыполнимо (FR-002, FR-006). Файл vault при этом не читается и
/// не изменяется.
///
/// Vault всё же нужен — фазе резолва: связанные файлы (изображения, вставки
/// досок, импорты) читаются через него, своего доступа к диску у предпросмотра
/// нет (принцип I).
///
/// Очереди вызовов, обязательной на web, здесь нет и не требуется: ограничение
/// «recursive use of an object» — свойство границы wasm-bindgen, а не ядра.
/// Нативный бэкенд защищён обычным `Mutex` состояния vault.
/// Синхронная фаза сборки — в пуле блокирующих потоков, с пределом времени
/// (005 FR-007, FR-026, T021).
///
/// Команда и так идёт не в потоке окна, но без этого долгая сборка занимала бы
/// рабочий поток асинхронной среды, на котором живут остальные команды.
/// Поток нельзя прервать снаружи: по истечении предела результат
/// отбрасывается, а поток дорабатывает сам (research.md §3).
pub struct ThreadExecutor;

impl preview::CompileExecutor for ThreadExecutor {
    async fn run(
        &self,
        job: preview::CompileJob,
    ) -> Result<preview::CompileOutput, preview::ExecutorError> {
        let limit = std::time::Duration::from_secs(u64::from(preview::BUILD_TIME_LIMIT_SECS));
        let work = tokio::task::spawn_blocking(move || job.run_inline());
        match tokio::time::timeout(limit, work).await {
            Ok(Ok(output)) => Ok(output),
            // Паника компилятора — не превышение предела: пусть будет видна.
            Ok(Err(failure)) => std::panic::resume_unwind(failure.into_panic()),
            Err(_) => Err(preview::ExecutorError::TimedOut),
        }
    }
}

/// Почему тело `attachments_batch_store` не разобрано. Состояние у окна
/// остаётся прежним: команда отказала, ничего не записав.
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind")]
pub enum StoreBodyProblem {
    /// Тело не сырое, короче заголовка или длины JSON.
    Truncated,
    /// JSON `{state, index}` не разбирается.
    Malformed,
}

#[derive(serde::Deserialize)]
pub struct StoreBodyHead {
    state: attachments::AttachmentBatchState,
    index: u32,
}

/// Разбирает тело записи: `u32` little-endian — длина JSON, JSON
/// `{"state", "index"}`, затем байты файла до конца тела.
pub fn parse_store_body(
    body: &[u8],
) -> Result<(attachments::AttachmentBatchState, u32, &[u8]), StoreBodyProblem> {
    let header: [u8; 4] = body
        .get(..4)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(StoreBodyProblem::Truncated)?;
    let length = u32::from_le_bytes(header) as usize;
    let json = body.get(4..4 + length).ok_or(StoreBodyProblem::Truncated)?;
    let head: StoreBodyHead =
        serde_json::from_slice(json).map_err(|_| StoreBodyProblem::Malformed)?;
    Ok((head.state, head.index, &body[4 + length..]))
}

/// Запись вложения из сырого тела (`attachments_batch_store`): тело не JSON, поэтому
/// команда не в таблице `dispatch` — оболочка и сервер достают байты каждый своим
/// способом и зовут это.
pub async fn store_attachment_body(
    state: &Vault,
    body: &[u8],
) -> Answer<attachments::AttachmentBatchState, Failure<StoreBodyProblem>> {
    let (batch, index, bytes) = parse_store_body(body).map_err(Failure::Core)?;
    with_vault!(state, |backend| Ok(attachments::batch::store(
        backend, batch, index, bytes
    )
    .await))
}

// ── Импорт из Obsidian (фича 021) ───────────────────────────────────────────
//
// Обёртки 1:1 над `typsidian_core::import`: правил здесь нет. Окно зовёт
// `import_open_source` с путём из диалога выбора папки, затем `import_survey`,
// `import_begin`, `import_step` (пока `next < total`, уступая интерфейсу между
// шагами) и `import_finish`. План пакета живёт в ядре и между шагами не
// пересылается; окно носит только `ImportState`.

/// Папка-источник импорта, открытая только для чтения (FR-002, SC-005).
///
/// Отдельным состоянием, а не вторым `Vault`: источник не vault, у него нет
/// слежения, кэша и служебной папки, а тип `ReadOnly` не даёт в него писать.
#[derive(Default)]
pub struct ImportSource(pub Mutex<Option<ReadOnly<NativeBackend>>>);

#[macro_export]
macro_rules! with_import_source {
    ($state:expr, |$source:ident| $body:expr) => {{
        let guard = $state.0.lock().await;
        let Some($source) = guard.as_ref() else {
            return Err(Failure::Core(import::ImportError::NoSource));
        };
        $body
    }};
}

/// Флаг отмены пересчёта свойств. Пересчёт идёт шагами, и между шагами vault
/// отпускается, поэтому «отмени» дойдёт, а остальные команды не ждут конца.
#[derive(Default)]
pub struct PropertiesReindex(pub Arc<std::sync::atomic::AtomicBool>);

/// Сессии подсветки редакторов окна (фича 024, contracts/core-highlight-api.md).
///
/// То же, чем на web является объект `SyntaxSession`: разобранный текст одного
/// редактора, который правки обновляют инкрементально. Отдельным состоянием,
/// а не полем `Vault`: подсветка не читает vault и не должна ждать его
/// блокировку. Сессию закрывает редактор; потерянная (окно перезагрузили) —
/// это только память до закрытия окна.
#[derive(Default)]
pub struct HighlightSessions(
    pub  std::sync::Mutex<(
        u32,
        HashMap<u32, typsidian_core::highlight::HighlightSession>,
    )>,
);

#[cfg(test)]
mod attachments_body_tests {
    use super::*;

    fn state() -> attachments::AttachmentBatchState {
        attachments::AttachmentBatchState {
            note_path: "Глава.typ".into(),
            target_folder: "attachments".into(),
            items: Vec::new(),
            moment: attachments::InsertMoment {
                year: 2026,
                month: 10,
                day: 10,
                hour: 14,
                minute: 23,
                second: 5,
            },
            phase: attachments::AttachmentBatchPhase::Running,
            excluded_by: Vec::new(),
            known: Vec::new(),
        }
    }

    fn body(json: &[u8], bytes: &[u8]) -> Vec<u8> {
        let mut out = (json.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(json);
        out.extend_from_slice(bytes);
        out
    }

    #[test]
    fn the_body_carries_state_index_and_bytes() {
        let json =
            serde_json::to_vec(&serde_json::json!({ "state": state(), "index": 3 })).unwrap();
        let raw = body(&json, &[0x89, b'P', b'N', b'G']);
        let (parsed, index, bytes) = parse_store_body(&raw).unwrap();
        assert_eq!(parsed, state());
        assert_eq!(index, 3);
        assert_eq!(bytes, &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn a_short_or_malformed_body_is_refused() {
        assert_eq!(
            parse_store_body(&[1, 0]).err(),
            Some(StoreBodyProblem::Truncated)
        );
        assert_eq!(
            parse_store_body(&body(b"{}", b"")[..5]).err(),
            Some(StoreBodyProblem::Truncated)
        );
        assert_eq!(
            parse_store_body(&body(b"{\"state\": 1}", b"x")).err(),
            Some(StoreBodyProblem::Malformed)
        );
    }
}

pub mod commands;
pub mod dispatch;
pub mod sync;

pub use sync::{SyncAnswer, SyncCancel, SyncFailure, SyncHome, SyncProgressSlot, SyncState};

pub use commands::*;

/// Пакетный пересчёт свойств с ходом и отменой (`vault_cancel_reindex_properties`).
/// Идёт шагами: между ними замок vault отпускается, и набор текста с остальными
/// командами не ждёт конца пересчёта. Ход оболочка отдаёт событием окна, сервер —
/// событием SSE, поэтому команды из этого нет в таблице `dispatch`.
pub async fn reindex_properties(
    state: &Vault,
    cancel: &PropertiesReindex,
    mut progress: impl FnMut(u32, u32),
) -> Answer<properties::ReindexOutcome, Failure<()>> {
    use std::sync::atomic::Ordering;
    cancel.0.store(false, Ordering::SeqCst);
    let mut from = 0;
    loop {
        if cancel.0.load(Ordering::SeqCst) {
            let total = with_vault!(state, |backend| Ok::<_, Failure<()>>(
                backend
                    .list()
                    .await
                    .iter()
                    .filter(|note| note.kind == FileKind::Note)
                    .count() as u32
            ))?;
            return Ok(properties::ReindexOutcome {
                done: from,
                total,
                cancelled: true,
            });
        }
        let step = with_vault!(state, |backend| Ok::<_, Failure<()>>(
            properties::reindex_step(backend, from, properties::index::REINDEX_BATCH).await
        ))?;
        progress(step.done, step.total);
        if step.finished {
            return Ok(properties::ReindexOutcome {
                done: step.done,
                total: step.total,
                cancelled: false,
            });
        }
        from = step.done;
    }
}
