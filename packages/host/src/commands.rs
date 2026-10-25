//! Тела команд ядра (перенесены из `packages/shell/src/lib.rs`, фича 037 T001).
//!
//! Каждая `pub fn` верхнего уровня этого файла — команда: по ним
//! `scripts/dev/gen-host-dispatch.mjs` строит обёртки Tauri и таблицу сервера.
//! Помощник, который командой не является, живёт в `lib.rs`.

use crate::*;

// ── Дерево и файлы ──────────────────────────────────────────────────────────

pub async fn list(state: &Vault) -> Answer<Vec<Note>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend.list().await))
}

pub async fn read(state: &Vault, path: String) -> Answer<String, Failure<ReadError>> {
    with_vault!(state, |backend| backend
        .read(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

pub async fn write(
    state: &Vault,
    path: String,
    content: String,
    expected: Option<FileStamp>,
) -> Answer<(), Failure<WriteError>> {
    with_vault!(state, |backend| backend
        .write(Path::new(&path), &content, expected)
        .await
        .map_err(Failure::Core))
}

pub async fn create(state: &Vault, path: String) -> Answer<(), Failure<CreateError>> {
    with_vault!(state, |backend| backend
        .create(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

pub async fn rename(state: &Vault, from: String, to: String) -> Answer<(), Failure<RenameError>> {
    with_vault!(state, |backend| backend
        .rename(Path::new(&from), Path::new(&to))
        .await
        .map_err(Failure::Core))
}

/// Папки, которые могут принять `from` (фича 023, FR-012, FR-022).
///
/// Что допустимо и как фильтруется, решает ядро; окно получает готовый список.
pub async fn move_targets(
    state: &Vault,
    from: String,
    query: String,
) -> Answer<Vec<MoveTarget>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .move_targets(Path::new(&from), &query)
        .await))
}

/// Что сломает перенос (фича 023, FR-016): заметки, чьи ссылки станут битыми.
pub async fn move_impact(
    state: &Vault,
    request: MoveRequest,
) -> Answer<MoveImpact, Failure<MoveRefusal>> {
    with_vault!(state, |backend| backend
        .move_impact(&request)
        .await
        .map_err(Failure::Core))
}

/// Перенос заметки, доски или папки (фича 023, FR-001, FR-002).
///
/// `NoControl`: на native перенос — атомарный `rename`, ни хода, ни отмены у
/// него нет (023 research.md §5).
pub async fn move_entry(
    state: &Vault,
    request: MoveRequest,
) -> Answer<MoveOutcome, Failure<MoveError>> {
    with_vault!(state, |backend| backend
        .move_entry(&request, &NoControl)
        .await
        .map_err(Failure::Core))
}

pub async fn delete(state: &Vault, path: String) -> Answer<(), Failure<DeleteError>> {
    with_vault!(state, |backend| backend
        .delete(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

pub async fn backlinks(state: &Vault, path: String) -> Answer<Vec<Note>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .backlinks(Path::new(&path))
        .await))
}

pub async fn broken_links(state: &Vault, path: String) -> Answer<Vec<PathBuf>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .broken_links(Path::new(&path))
        .await))
}

pub async fn access_state(state: &Vault) -> Answer<AccessState, Failure<()>> {
    with_vault!(state, |backend| Ok(backend.access_state()))
}

/// Расширенный поиск по vault (макет «Утилиты» § Поиск).
///
/// Обёртка, как и остальные команды: правила совпадения живут в ядре, здесь
/// только пересылка через границу процесса (принцип I).
pub async fn search(
    state: &Vault,
    query: typsidian_core::search::SearchQuery,
) -> Answer<typsidian_core::search::SearchResults, Failure<typsidian_core::search::SearchError>> {
    with_vault!(state, |backend| typsidian_core::search::search(
        backend, &query
    )
    .await
    .map_err(Failure::Core))
}

/// Предпросмотр замены по vault (фича 027). Правила — в ядре.
pub async fn replace_preview(
    state: &Vault,
    query: typsidian_core::search::replace::ReplaceQuery,
    open_texts: Vec<links::OpenText>,
) -> Answer<
    typsidian_core::search::replace::ReplacePreview,
    Failure<typsidian_core::search::replace::ReplaceError>,
> {
    with_vault!(state, |backend| {
        typsidian_core::search::replace::replace_preview(backend, &query, &open_texts)
            .await
            .map_err(Failure::Core)
    })
}

/// Замена отмеченных совпадений одной заметки (фича 027).
pub async fn replace_in_note(
    state: &Vault,
    request: typsidian_core::search::replace::ReplaceNoteRequest,
    editor_text: Option<String>,
) -> Answer<typsidian_core::search::replace::ReplaceNoteOutcome, Failure<()>> {
    with_vault!(state, |backend| Ok(
        typsidian_core::search::replace::replace_in_note(backend, &request, editor_text.as_deref())
            .await
    ))
}

/// Отмена замены в одной заметке (фича 027).
pub async fn replace_undo(
    state: &Vault,
    record: typsidian_core::search::replace::UndoRecord,
    editor_text: Option<String>,
) -> Answer<typsidian_core::search::replace::UndoOutcome, Failure<()>> {
    with_vault!(state, |backend| Ok(
        typsidian_core::search::replace::replace_undo(backend, &record, editor_text.as_deref())
            .await
    ))
}

/// Состояние ссылок vault (фича 028). Правила — в ядре.
pub async fn links_health(
    state: &Vault,
    open_texts: Vec<links::OpenText>,
) -> Answer<links::health::LinkHealth, Failure<()>> {
    with_vault!(state, |backend| Ok(links::health::link_health(
        backend,
        &open_texts
    )
    .await))
}

/// Починка ссылок одной заметки (фича 028); отмена — `replace_undo` 027.
pub async fn links_fix(
    state: &Vault,
    request: links::health::LinkFixRequest,
    editor_text: Option<String>,
) -> Answer<typsidian_core::search::replace::ReplaceNoteOutcome, Failure<()>> {
    with_vault!(state, |backend| Ok(links::health::fix_links(
        backend,
        &request,
        editor_text.as_deref()
    )
    .await))
}

// ── Снимки заметок (фича 029). Правила — в ядре, время — часы устройства. ──

pub async fn snapshot_take(
    state: &Vault,
    path: String,
    text: String,
    reason: snapshots::SnapshotReason,
) -> Answer<snapshots::TakeOutcome, Failure<()>> {
    with_vault!(state, |backend| Ok(snapshots::snapshot_take(
        backend,
        &path,
        &text,
        reason,
        typsidian_core::properties::now_ms()
    )
    .await))
}

pub async fn snapshot_history(
    state: &Vault,
    path: String,
) -> Answer<snapshots::NoteHistory, Failure<()>> {
    with_vault!(state, |backend| Ok(snapshots::snapshot_history(
        backend,
        &path,
        typsidian_core::properties::now_ms()
    )
    .await))
}

pub async fn snapshot_diff(
    state: &Vault,
    id: String,
    against: String,
) -> Answer<Vec<typsidian_core::vault::conflict::DiffLine>, Failure<()>> {
    with_vault!(state, |backend| Ok(snapshots::snapshot_diff(
        backend, &id, &against
    )
    .await
    .unwrap_or_default()))
}

pub async fn snapshot_text(state: &Vault, id: String) -> Answer<Option<String>, Failure<()>> {
    with_vault!(state, |backend| Ok(
        snapshots::snapshot_text(backend, &id).await
    ))
}

pub async fn snapshot_restore(
    state: &Vault,
    path: String,
    id: String,
    editor_text: Option<String>,
) -> Answer<snapshots::RestoreOutcome, Failure<()>> {
    with_vault!(state, |backend| Ok(snapshots::snapshot_restore(
        backend,
        &path,
        &id,
        editor_text.as_deref(),
        typsidian_core::properties::now_ms()
    )
    .await))
}

pub async fn snapshot_deleted_notes(
    state: &Vault,
) -> Answer<Vec<snapshots::DeletedNote>, Failure<()>> {
    with_vault!(state, |backend| Ok(snapshots::deleted_notes(
        backend,
        typsidian_core::properties::now_ms()
    )
    .await))
}

pub async fn snapshot_recover_note(
    state: &Vault,
    path: String,
    as_path: Option<String>,
) -> Answer<snapshots::RecoverOutcome, Failure<()>> {
    with_vault!(state, |backend| Ok(snapshots::recover_note(
        backend,
        &path,
        as_path.as_deref()
    )
    .await))
}

pub async fn snapshot_usage(state: &Vault) -> Answer<snapshots::SnapshotUsage, Failure<()>> {
    with_vault!(
        state,
        |backend| Ok(snapshots::snapshot_usage(backend).await)
    )
}

pub async fn snapshots_clear(state: &Vault) -> Answer<u32, Failure<()>> {
    with_vault!(state, |backend| snapshots::snapshots_clear(backend)
        .await
        .map_err(|_| Failure::Core(())))
}

// ── Задачи из заметок (фича 033) ────────────────────────────────────────────

pub async fn tasks_view(
    filter: tasks::TaskFilter,
    today: String,
    state: &Vault,
) -> Answer<tasks::TasksView, Failure<()>> {
    with_vault!(state, |backend| Ok(tasks::tasks_view(
        backend, &filter, &today
    )
    .await))
}

pub async fn task_toggle(
    request: tasks::TaskToggleRequest,
    editor_text: Option<String>,
    state: &Vault,
) -> Answer<tasks::TaskToggleOutcome, Failure<()>> {
    with_vault!(state, |backend| Ok(tasks::task_toggle(
        backend,
        &request,
        editor_text.as_deref()
    )
    .await))
}

pub fn task_toggle_at(text: String, offset: u32) -> Option<typsidian_core::elements::TextEdit> {
    tasks::task_toggle_at(&text, offset)
}

pub async fn bookmarks_view(state: &Vault) -> Answer<bookmarks::BookmarksView, Failure<()>> {
    with_vault!(
        state,
        |backend| Ok(bookmarks::bookmarks_view(backend).await)
    )
}

pub async fn bookmark_add(
    target: bookmarks::BookmarkTarget,
    name: Option<String>,
    state: &Vault,
) -> Answer<bookmarks::AddOutcome, Failure<bookmarks::BookmarkError>> {
    with_vault!(state, |backend| bookmarks::bookmark_add(
        backend, target, name
    )
    .await
    .map_err(Failure::Core))
}

pub async fn bookmark_remove(id: String, state: &Vault) -> BookmarkAnswer {
    with_vault!(state, |backend| bookmarks::bookmark_remove(backend, &id)
        .await
        .map_err(Failure::Core))
}

pub async fn bookmark_rename(id: String, name: String, state: &Vault) -> BookmarkAnswer {
    with_vault!(state, |backend| bookmarks::bookmark_rename(
        backend, &id, &name
    )
    .await
    .map_err(Failure::Core))
}

pub async fn bookmark_move(
    id: String,
    place: bookmarks::BookmarkPlace,
    state: &Vault,
) -> BookmarkAnswer {
    with_vault!(state, |backend| bookmarks::bookmark_move(
        backend, &id, &place
    )
    .await
    .map_err(Failure::Core))
}

pub async fn bookmark_group_create(name: String, state: &Vault) -> BookmarkAnswer {
    with_vault!(state, |backend| bookmarks::bookmark_group_create(
        backend, &name
    )
    .await
    .map_err(Failure::Core))
}

pub async fn bookmark_group_rename(id: String, name: String, state: &Vault) -> BookmarkAnswer {
    with_vault!(state, |backend| bookmarks::bookmark_group_rename(
        backend, &id, &name
    )
    .await
    .map_err(Failure::Core))
}

pub async fn bookmark_group_toggle(id: String, state: &Vault) -> BookmarkAnswer {
    with_vault!(state, |backend| bookmarks::bookmark_group_toggle(
        backend, &id
    )
    .await
    .map_err(Failure::Core))
}

pub async fn bookmark_group_delete(id: String, state: &Vault) -> BookmarkAnswer {
    with_vault!(state, |backend| bookmarks::bookmark_group_delete(
        backend, &id
    )
    .await
    .map_err(Failure::Core))
}

// ── Встраивание заметок (фича 031) ──────────────────────────────────────────

/// «Встроить заметку»: библиотека в vault, `#import` и `#embed("")` у каретки.
pub async fn embed_insert(
    text: String,
    offset: u32,
    state: &Vault,
) -> Answer<typsidian_core::embeds::EmbedInsertion, Failure<()>> {
    with_vault!(state, |backend| typsidian_core::embeds::insert_embed(
        backend, &text, offset
    )
    .await
    .map_err(|_| Failure::Core(())))
}

pub async fn spell_configure(
    state: &Vault,
    speller: &Speller,
) -> Answer<spellcheck::SpellConfig, Failure<()>> {
    let mut checker = speller.get();
    let config: spellcheck::SpellConfig = with_vault!(state, |backend| Ok::<_, Failure<()>>(
        spellcheck::spell_configure(backend, &mut checker).await
    ))?;
    speller.set(checker);
    Ok(config)
}

pub async fn spell_add_word(
    word: String,
    state: &Vault,
    speller: &Speller,
) -> Answer<spellcheck::SpellConfig, Failure<spellcheck::SpellError>> {
    let mut checker = speller.get();
    let config = with_vault!(state, |backend| spellcheck::spell_add_word(
        backend,
        &mut checker,
        &word
    )
    .await
    .map_err(Failure::Core))?;
    speller.set(checker);
    Ok(config)
}

pub async fn spell_remove_word(
    word: String,
    state: &Vault,
    speller: &Speller,
) -> Answer<spellcheck::SpellConfig, Failure<spellcheck::SpellError>> {
    let mut checker = speller.get();
    let config = with_vault!(state, |backend| spellcheck::spell_remove_word(
        backend,
        &mut checker,
        &word
    )
    .await
    .map_err(Failure::Core))?;
    speller.set(checker);
    Ok(config)
}

pub fn spell_suggest(word: String, speller: &Speller) -> Vec<String> {
    spellcheck::spell_suggest(&speller.get(), &word)
}

/// Слова с ошибками в окнах сессии подсветки — пары `[from, to)`; `None` —
/// сессии нет (её закрыли или она разошлась с редактором).
pub fn syntax_session_spelling(
    id: u32,
    windows: Vec<typsidian_core::highlight::HighlightWindow>,
    sessions: &HighlightSessions,
    speller: &Speller,
) -> Option<Vec<u32>> {
    let checker = speller.get();
    let guard = sessions
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Some(guard.1.get(&id)?.spelling(&windows, &checker))
}

/// Путь папки, открытой как vault — окну для хлебных крошек в шапке.
///
/// Спрашивается у бэкенда, а не запоминается окном при выборе папки: тогда
/// «какой vault открыт» знали бы два места, и после переоткрытия они могли бы
/// разойтись.
pub async fn root_path(state: &Vault) -> Answer<String, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .root_path()
        .to_string_lossy()
        .into_owned()))
}

/// Право сборки писать доски — окну, чтобы объявить ограничение пользователю
/// (фича 002, FR-034).
///
/// Не требует открытого vault: профиль выбран таргетом при сборке, а не
/// состоянием папки, и окно спрашивает о нём до открытия чего-либо. Возвращает
/// ровно ту константу, которую применяет `write_board`, — сказанное
/// пользователю и применённое к записи не могут разойтись.
pub fn board_write_profile() -> boards::BoardWriteProfile {
    BOARD_WRITE_PROFILE
}

/// Вид файла по пути. Не требует открытого vault: классификация идёт по пути,
/// без чтения содержимого (фича 002, research.md §3).
pub fn file_kind(path: String) -> FileKind {
    classify(Path::new(&path))
}

// ── Несохранённые правки и конфликты ────────────────────────────────────────

pub async fn begin_edit(state: &Vault, path: String, buffer: String) -> Answer<(), Failure<()>> {
    with_vault!(state, |backend| {
        backend.begin_edit(Path::new(&path), buffer).await;
        Ok(())
    })
}

/// Снимает буфер несохранённых правок (фича 014, research.md §6): буфер,
/// вернувшийся к сохранённому тексту, иначе не давал бы закрыть вкладку.
pub async fn discard_edit(state: &Vault, path: String) -> Answer<(), Failure<()>> {
    with_vault!(state, |backend| {
        backend.discard_edit(Path::new(&path)).await;
        Ok(())
    })
}

pub async fn pending_conflict(
    state: &Vault,
    path: String,
) -> Answer<Option<EditConflict>, Failure<()>> {
    with_vault!(state, |backend| Ok(
        backend.pending_conflict(Path::new(&path))
    ))
}

pub async fn unresolved_edit_paths(state: &Vault) -> Answer<Vec<PathBuf>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend.unresolved_edit_paths()))
}

pub async fn conflict_diff(state: &Vault, path: String) -> Answer<Vec<DiffLine>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .conflict_diff(Path::new(&path))
        .await))
}

pub async fn resolve_conflict_keep_local(
    state: &Vault,
    path: String,
) -> Answer<(), Failure<ResolveError>> {
    with_vault!(state, |backend| backend
        .resolve_conflict_keep_local(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

pub async fn resolve_conflict_accept_external(
    state: &Vault,
    path: String,
) -> Answer<Option<String>, Failure<ResolveError>> {
    with_vault!(state, |backend| backend
        .resolve_conflict_accept_external(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

// ── Доски и вставки (фича 002) ──────────────────────────────────────────────

/// Профиль записи зашит `ReadWrite` по той же причине, что и в web-биндинге:
/// рабочий стол рисует (FR-033 фичи 002), а просмотр без записи — свойство
/// мобильного клиента, у которого будет своя оболочка.
pub async fn create_board(state: &Vault, path: String) -> Answer<Board, Failure<CreateBoardError>> {
    with_vault!(state, |backend| boards::create_board(
        backend,
        Path::new(&path)
    )
    .await
    .map_err(Failure::Core))
}

pub async fn read_board(state: &Vault, path: String) -> Answer<String, Failure<ReadBoardError>> {
    with_vault!(state, |backend| {
        let content = boards::read_board(backend, Path::new(&path))
            .await
            .map_err(Failure::Core)?;
        String::from_utf8(content.as_bytes().to_vec())
            .map_err(|_| Failure::Core(ReadBoardError::Unreadable))
    })
}

pub async fn write_board(
    state: &Vault,
    path: String,
    content: String,
    expected: Option<FileStamp>,
) -> Answer<(), Failure<WriteBoardError>> {
    with_vault!(state, |backend| boards::write_board(
        backend,
        BOARD_WRITE_PROFILE,
        Path::new(&path),
        &boards::BoardContent::new(content.as_bytes().to_vec()),
        expected,
    )
    .await
    .map_err(Failure::Core))
}

pub async fn write_snapshot(
    state: &Vault,
    snapshot_path: String,
    svg: String,
    board_path: String,
    board_content: String,
) -> Answer<(), Failure<SnapshotWriteError>> {
    with_vault!(state, |backend| {
        let digest = boards::digest(board_content.as_bytes())
            .map_err(|_| Failure::Core(SnapshotWriteError::NotAnSvg))?;
        let metadata = boards::snapshot_metadata(Path::new(&board_path), &digest);
        boards::write_snapshot(backend, Path::new(&snapshot_path), &svg, &metadata)
            .await
            .map_err(Failure::Core)
    })
}

pub async fn embeds_of(
    state: &Vault,
    note_path: String,
    note_text: String,
) -> Answer<Vec<Embed>, Failure<()>> {
    with_vault!(state, |backend| Ok(boards::embeds_with_state(
        backend,
        Path::new(&note_path),
        &note_text
    )
    .await))
}

/// Разбор текста заметки. Vault не требуется: файлы не читаются.
pub fn embeds_in_text(note_path: String, note_text: String) -> Vec<EmbedRef> {
    boards::embeds_of(Path::new(&note_path), &note_text)
}

pub fn embeds_changed(note_path: String, note_text: String, changed: String) -> Option<BoardEvent> {
    boards::embeds_changed(Path::new(&note_path), &note_text, Path::new(&changed))
}

/// Настройки vault приходят целиком, а не разобранными на политику и список
/// разделов: UI их уже прочитал, а решение «что из них влияет на путь»
/// принадлежит ядру (фича 006, принцип I).
pub fn plan_embed(
    note_path: String,
    note_text: String,
    board_path: String,
    overrides: VaultOverrides,
) -> Answer<EmbedPlan, EmbedError> {
    boards::plan_embed(
        Path::new(&note_path),
        &note_text,
        Path::new(&board_path),
        overrides.placement(),
        &overrides.sections,
    )
}

// ── Настройки уровня vault и разделы (фича 006) ─────────────────────────────

/// Настройки уровня vault (FR-002, FR-015, FR-026).
pub async fn vault_settings(state: &Vault) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::read_vault_settings(backend)
            .await
            .map_err(Failure::Core)
    })
}

/// Записывает настройки уровня vault (FR-029).
pub async fn set_vault_settings(
    state: &Vault,
    overrides: VaultOverrides,
) -> Answer<(), Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::write_vault_settings(backend, &overrides)
            .await
            .map_err(Failure::Core)
    })
}

/// Помечает папку как корень раздела (FR-013).
pub async fn mark_section(
    state: &Vault,
    folder: String,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::set_section(backend, Path::new(&folder), true)
            .await
            .map_err(Failure::Core)
    })
}

/// Снимает пометку раздела (FR-013). Уже созданные файлы вставок при этом не
/// двигаются (FR-017).
pub async fn unmark_section(
    state: &Vault,
    folder: String,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::set_section(backend, Path::new(&folder), false)
            .await
            .map_err(Failure::Core)
    })
}

pub fn embed_markup(plan: EmbedPlan) -> String {
    boards::embed_markup(&plan)
}

// ── Сочетания клавиш ────────────────────────────────────────────────────────

/// Действующие сочетания всех команд.
///
/// Не команда над vault: чистая функция от уже прочитанных настроек. Через
/// границу процесса она ходит только потому, что на этой стороне ядро живёт в
/// нативном процессе, а разбирать сочетания второй раз в интерфейсе значило бы
/// завести вторую копию правил (принцип I).
pub fn keybindings(overrides: VaultOverrides, mac: bool) -> Vec<ResolvedBinding> {
    keybindings_core::resolve(&overrides.keybindings, mac)
}

/// Назначает команде сочетание либо снимает его (`shortcut: null`).
pub async fn set_keybinding(
    state: &Vault,
    command: CommandId,
    shortcut: Option<Shortcut>,
) -> Answer<VaultOverrides, Failure<SetKeybindingError>> {
    with_vault!(state, |backend| {
        vault_settings::set_keybinding(backend, command, shortcut)
            .await
            .map_err(Failure::Core)
    })
}

/// Возвращает команду к умолчанию ядра.
pub async fn reset_keybinding(
    state: &Vault,
    command: CommandId,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::reset_keybinding(backend, command)
            .await
            .map_err(Failure::Core)
    })
}

// ── Шаблоны и сниппеты (фича 003) ───────────────────────────────────────────
//
// Те же обёртки, что и всё выше: правила библиотеки живут в
// `typsidian_core::templates`, здесь только пересылка через границу процесса.

pub async fn template_library(state: &Vault) -> Answer<TemplateLibraryView, Failure<()>> {
    with_vault!(state, |backend| Ok(templates::library(backend).await))
}

pub async fn read_template(
    state: &Vault,
    path: String,
) -> Answer<String, Failure<ReadTemplateError>> {
    with_vault!(state, |backend| templates::read_template(
        backend,
        Path::new(&path)
    )
    .await
    .map_err(Failure::Core))
}

/// Превью одной заготовки.
///
/// Кэш — состояние оболочки, а не ядра: на web им владеет `WebBackend`, здесь —
/// эта команда. Разница вынужденная и безобидная: кэш производный,
/// восстановимый и авторитетной копией ничему не является (research.md §4).
/// Правило, по которому он ключуется хэшем содержимого, при этом одно и живёт в
/// ядре.
pub async fn template_preview(
    state: &Vault,
    cache: &TemplatePreviews,
    template: templates::Template,
) -> Answer<PreviewState, Failure<()>> {
    let mut cache = cache.0.lock().await;
    with_vault!(state, |backend| Ok(templates::preview_of(
        backend,
        &templates::ExcerptOnlySource,
        &mut cache,
        &template
    )
    .await))
}

pub async fn preview_compile(
    state: &Vault,
    engine: &Preview,
    note_path: String,
    text: String,
    revision: u64,
    anchor: Option<preview::ScrollAnchor>,
) -> Answer<CompiledDocument, Failure<SnapshotError>> {
    with_vault!(state, |backend| preview::compile_note_resolved_on(
        &ThreadExecutor,
        backend,
        &engine.packages,
        &note_path,
        &text,
        revision,
        engine.fonts.clone(),
    )
    .await
    .map(|document| document.with_anchor(anchor.as_ref()))
    .map_err(Failure::Core))
}

/// Отметка состояния связанных файлов (005 FR-020): изменилась — предпросмотр
/// пора пересобрать. Содержимое файлов не читается.
pub async fn preview_dependency_mark(
    state: &Vault,
    paths: Vec<String>,
) -> Answer<String, Failure<SnapshotError>> {
    with_vault!(state, |backend| Ok(preview::dependency_mark(
        backend, &paths
    )
    .await))
}

/// То же, но недостающие внешние шаблоны доустанавливаются (фича 007, FR-001).
///
/// Отдельная команда, а не флаг у `preview_compile`: этот путь может **писать**
/// в vault и обращаться в сеть, и разница обязана быть видна на границе, а не
/// прятаться в параметре. Заметка без подключений проходит здесь так же, как
/// через `preview_compile`, и в сеть не заходит (FR-008).
///
/// Правила загрузки приходят настройками уровня vault: решение «идти ли в
/// сеть» принимает ядро по ним, а не окно (принцип I).
pub async fn preview_compile_with_templates(
    state: &Vault,
    engine: &Preview,
    note_path: String,
    text: String,
    revision: u64,
    overrides: VaultOverrides,
    anchor: Option<preview::ScrollAnchor>,
) -> Answer<CompiledDocument, Failure<SnapshotError>> {
    with_vault!(state, |backend| preview::compile_note_with_templates_on(
        &ThreadExecutor,
        backend,
        &engine.packages,
        &note_path,
        &text,
        revision,
        engine.fonts.clone(),
        &overrides.template_policy(),
    )
    .await
    .map(|document| document.with_anchor(anchor.as_ref()))
    .map_err(Failure::Core))
}

/// Установить шаблон по явному запросу пользователя (фича 007, FR-010).
///
/// Отдельно от сборки: выключенная автозагрузка запрещает приложению ходить в
/// сеть **само**, но не отнимает у пользователя возможность попросить об этом.
pub async fn preview_install_template(
    state: &Vault,
    engine: &Preview,
    spec: String,
    overrides: VaultOverrides,
) -> Answer<preview::packages::InstallReport, Failure<preview::packages::InstallError>> {
    let result = with_vault!(state, |backend| preview::install_template(
        backend,
        &spec,
        &overrides.template_policy(),
    )
    .await
    .map_err(Failure::Core));

    // Ту же версию могли поставить поверх: кэш и отметка о сверке отпечатка
    // относятся к прежнему содержимому.
    if let Some(parsed) = preview::packages::parse_spec(&spec) {
        engine.packages.forget(&parsed);
        engine.packages.forget_verified(&parsed);
    }
    result
}

/// Установленные шаблоны — для перечня в настройках папки (фича 007, US5).
pub async fn preview_installed_templates(
    state: &Vault,
) -> Answer<Vec<preview::packages::InstalledTemplate>, Failure<()>> {
    with_vault!(state, |backend| Ok(preview::packages::installed_templates(
        backend
    )
    .await))
}

// ── Экспорт в PDF (фича 009) ────────────────────────────────────────────────

/// Экспортирует заметку в PDF (contracts/core-export-api.md).
///
/// Тот же движок, что у предпросмотра: шрифты и кэш шаблонов общие, иначе
/// заметка собиралась бы в одном месте и не собиралась в другом. Команда
/// асинхронная и идёт в пуле tokio, а не в потоке окна: пакет не блокирует
/// интерфейс (принцип X).
pub async fn export_pdf(
    state: &Vault,
    engine: &Preview,
    note_path: String,
    text: Option<String>,
    overrides: VaultOverrides,
) -> Answer<export::ExportReport, Failure<()>> {
    with_vault!(state, |backend| Ok(export::export_note_pdf(
        backend,
        &engine.packages,
        &note_path,
        text.as_deref(),
        engine.fonts.clone(),
        &overrides,
    )
    .await))
}

/// Экспорт презентации (фича 035): тот же движок, что у PDF.
pub async fn export_slides(
    state: &Vault,
    engine: &Preview,
    note_path: String,
    text: Option<String>,
    overrides: VaultOverrides,
    request: typsidian_core::slides::export::SlidesExportRequest,
) -> Answer<typsidian_core::slides::export::SlidesExportReport, Failure<()>> {
    with_vault!(state, |backend| Ok(
        typsidian_core::slides::export::export_slides(
            backend,
            &engine.packages,
            &note_path,
            text.as_deref(),
            engine.fonts.clone(),
            &overrides,
            &request,
        )
        .await
    ))
}

/// Экспортирует заметку страницей HTML (фича 034): тот же движок, что у PDF.
pub async fn export_html(
    state: &Vault,
    engine: &Preview,
    note_path: String,
    text: Option<String>,
    overrides: VaultOverrides,
    site: Vec<String>,
    labels: export::SiteLabels,
) -> Answer<export::ExportReport, Failure<()>> {
    with_vault!(state, |backend| Ok(export::export_note_html(
        backend,
        &engine.packages,
        &note_path,
        text.as_deref(),
        engine.fonts.clone(),
        &overrides,
        &site,
        &labels,
    )
    .await))
}

/// Оглавление сайта HTML (фича 034).
pub async fn export_site_index(
    state: &Vault,
    overrides: VaultOverrides,
    site: Vec<String>,
    labels: export::SiteLabels,
) -> Answer<String, Failure<export::ExportWriteProblem>> {
    with_vault!(state, |backend| export::export_site_index(
        backend, &overrides, &site, &labels
    )
    .await
    .map_err(Failure::Core))
}

pub fn export_batch_with_format(
    state: export::BatchState,
    format: export::ExportFormat,
) -> export::BatchState {
    export::batch::with_format(state, format)
}

pub async fn export_candidates(
    state: &Vault,
    overrides: VaultOverrides,
) -> Answer<Vec<export::ExportCandidate>, Failure<()>> {
    with_vault!(state, |backend| Ok(export::batch::candidates_of(
        backend, &overrides
    )
    .await))
}

// Переходы пакета — чистые функции ядра без состояния в оболочке
// (research.md §5): значение пакета держит окно.

pub fn export_batch_start(
    paths: Vec<String>,
    stop_on_error: bool,
    overrides: VaultOverrides,
) -> export::BatchState {
    export::batch::start(paths, stop_on_error, &overrides)
}

pub fn export_batch_next(state: export::BatchState) -> export::BatchStep {
    export::batch::step(state)
}

pub fn export_batch_record(
    state: export::BatchState,
    report: export::ExportReport,
    elapsed_ms: u32,
) -> export::BatchState {
    export::batch::record(state, report, elapsed_ms)
}

pub fn export_batch_skip(state: export::BatchState, note_path: String) -> export::BatchState {
    export::batch::skip(state, &note_path)
}

pub fn export_batch_cancel(state: export::BatchState) -> export::BatchState {
    export::batch::cancel(state)
}

pub fn export_batch_view(state: export::BatchState) -> export::BatchView {
    export::batch::view(state)
}

// ── Источники (фича 017) ────────────────────────────────────────────────────
//
// Движок живёт в состоянии `Sources`; команды с vault берут обе блокировки —
// сначала vault, затем движок, всегда в этом порядке.

pub async fn sources_refresh(
    state: &Vault,
    engine: &Sources,
    registered: Vec<String>,
) -> Answer<sources::RefreshStep, Failure<()>> {
    with_vault!(state, |backend| {
        let mut engine = engine.0.lock().await;
        Ok(engine.refresh(&*backend, &registered).await)
    })
}

pub async fn sources_summary(engine: &Sources) -> Answer<sources::LibrarySummary, ()> {
    Ok(engine.0.lock().await.summary())
}

pub async fn sources_search(
    engine: &Sources,
    query: String,
    limit: u32,
    uncited_only: bool,
) -> Answer<sources::SourceHits, ()> {
    Ok(engine.0.lock().await.search(&query, limit, uncited_only))
}

pub async fn sources_for_completion(
    engine: &Sources,
    prefix: String,
    limit: u32,
) -> Answer<Vec<sources::SourceHit>, ()> {
    Ok(engine.0.lock().await.sources_for_completion(&prefix, limit))
}

pub async fn sources_scan_start(
    state: &Vault,
    engine: &Sources,
) -> Answer<sources::CitationScanView, Failure<()>> {
    with_vault!(state, |backend| {
        let notes = backend.list().await;
        Ok(engine.0.lock().await.scan_start(&notes))
    })
}

pub async fn sources_scan_step(
    state: &Vault,
    engine: &Sources,
    max_notes: u32,
) -> Answer<sources::CitationScanView, Failure<()>> {
    with_vault!(state, |backend| {
        let mut engine = engine.0.lock().await;
        Ok(engine.scan_step(&*backend, max_notes).await)
    })
}

pub async fn sources_scan_cancel(engine: &Sources) -> Answer<sources::CitationScanView, ()> {
    Ok(engine.0.lock().await.scan_cancel())
}

pub async fn sources_scan_view(engine: &Sources) -> Answer<sources::CitationScanView, ()> {
    Ok(engine.0.lock().await.scan_view())
}

pub async fn sources_note_changed(
    state: &Vault,
    engine: &Sources,
    path: String,
) -> Answer<(), Failure<()>> {
    with_vault!(state, |backend| {
        engine.0.lock().await.note_changed(&*backend, &path).await;
        Ok(())
    })
}

pub async fn sources_note_removed(engine: &Sources, path: String) -> Answer<(), ()> {
    engine.0.lock().await.note_removed(&path);
    Ok(())
}

pub async fn sources_note_renamed(engine: &Sources, from: String, to: String) -> Answer<(), ()> {
    engine.0.lock().await.note_renamed(&from, &to);
    Ok(())
}

pub async fn sources_citing(engine: &Sources, key: String) -> Answer<Vec<sources::CitingNote>, ()> {
    Ok(engine.0.lock().await.citing(&key))
}

pub async fn sources_missing_keys(engine: &Sources) -> Answer<Vec<sources::MissingKey>, ()> {
    Ok(engine.0.lock().await.missing_keys())
}

pub async fn sources_check_note(
    engine: &Sources,
    text: String,
    known_labels: Vec<String>,
) -> Answer<sources::NoteCheck, ()> {
    Ok(engine.0.lock().await.check_note(&text, &known_labels))
}

pub async fn sources_plan_citation(
    engine: &Sources,
    text: String,
    caret: usize,
    file: String,
    key: String,
) -> Answer<sources::CitationPlan, sources::CitationPlanError> {
    engine
        .0
        .lock()
        .await
        .plan_citation(&text, caret, &file, &key)
}

pub async fn sources_bibliography_offer(
    engine: &Sources,
    text: String,
    note_path: String,
    file: String,
) -> Answer<sources::BibliographyOffer, ()> {
    Ok(engine
        .0
        .lock()
        .await
        .bibliography_offer(&text, &note_path, &file))
}

pub async fn sources_create_file(
    state: &Vault,
) -> Answer<String, Failure<sources::CreateSourceFileError>> {
    with_vault!(state, |backend| sources::create_source_file(backend)
        .await
        .map_err(Failure::Core))
}
// ── Вложения (фича 016) ─────────────────────────────────────────────────────
//
// Как у пакета экспорта, значение пачки держит окно; оболочка только вызывает
// чистые переходы ядра. Запись файла — единственная команда с сырым телом:
// байты в аргументах JSON выросли бы в 3–4 раза (research.md §11).

pub async fn attachments_batch_start(
    state: &Vault,
    overrides: VaultOverrides,
    note_path: String,
    items: Vec<attachments::IncomingItem>,
    moment: attachments::InsertMoment,
) -> Answer<attachments::AttachmentBatchState, Failure<()>> {
    with_vault!(state, |backend| Ok(attachments::batch::start(
        &*backend, &overrides, &note_path, items, moment
    )
    .await))
}

pub fn attachments_batch_next(
    state: attachments::AttachmentBatchState,
) -> attachments::AttachmentBatchStep {
    attachments::batch::step(state)
}

pub fn attachments_batch_confirm(
    state: attachments::AttachmentBatchState,
    index: u32,
    accept: bool,
) -> attachments::AttachmentBatchState {
    attachments::batch::confirm(state, index, accept)
}

pub fn attachments_batch_source_failed(
    state: attachments::AttachmentBatchState,
    index: u32,
) -> attachments::AttachmentBatchState {
    attachments::batch::source_failed(state, index)
}

pub fn attachments_batch_cancel(
    state: attachments::AttachmentBatchState,
) -> attachments::AttachmentBatchState {
    attachments::batch::cancel(state)
}

pub fn attachments_batch_view(
    state: attachments::AttachmentBatchState,
) -> attachments::AttachmentBatchView {
    attachments::batch::view(state)
}

pub fn attachments_plan_markup(
    text_length: usize,
    point: typsidian_core::templates::InsertionPoint,
    before: String,
    after: String,
    paths: Vec<String>,
    block: bool,
) -> Option<typsidian_core::templates::InsertionEdit> {
    attachments::plan_markup(text_length, &point, &before, &after, &paths, block)
}

pub fn attachments_validate_folder(
    raw: String,
) -> Result<String, attachments::AttachmentsFolderProblem> {
    attachments::validate_folder(&raw)
}

/// Закрывает источник (окно диалога закрыто или импорт окончен).
pub async fn import_close_source(state: &ImportSource) -> Answer<(), Failure<()>> {
    *state.0.lock().await = None;
    Ok(())
}

pub async fn import_survey(
    vault: &Vault,
    source: &ImportSource,
    job: import::ImportJob,
) -> Answer<import::ImportSurvey, Failure<import::ImportError>> {
    with_import_source!(source, |source| with_vault!(vault, |backend| Ok(
        import::survey(source, &*backend, &job).await
    )))
}

pub async fn import_begin(
    vault: &Vault,
    source: &ImportSource,
    job: import::ImportJob,
) -> Answer<import::ImportState, Failure<import::ImportError>> {
    with_import_source!(source, |source| with_vault!(vault, |backend| Ok(
        import::import_begin(source, &*backend, &job).await
    )))
}

pub async fn import_step(
    vault: &Vault,
    source: &ImportSource,
    state: import::ImportState,
) -> Answer<import::ImportState, Failure<import::ImportError>> {
    with_import_source!(source, |source| with_vault!(vault, |backend| {
        import::import_step(source, backend, state)
            .await
            .map_err(Failure::Core)
    }))
}

pub async fn import_finish(
    vault: &Vault,
    state: import::ImportState,
) -> Answer<String, Failure<import::ImportError>> {
    with_vault!(vault, |backend| {
        import::import_finish(backend, &state)
            .await
            .map_err(Failure::Core)
    })
}

// ── Связи заметки (фича 013) ────────────────────────────────────────────────
//
// Обёртки 1:1 над `typsidian_core::links`: правил здесь нет. Проход
// упоминаний — чистые переходы состояния, которое держит окно (как пакет
// экспорта).

pub async fn links_backlinks_page(
    state: &Vault,
    target: String,
    open_texts: Vec<links::OpenText>,
    offset: u32,
    limit: u32,
) -> Answer<links::BacklinksPage, Failure<()>> {
    with_vault!(state, |backend| Ok(links::backlinks_page(
        backend,
        &target,
        &open_texts,
        offset,
        limit
    )
    .await))
}

pub fn links_mentions_availability(target: String) -> links::MentionsAvailability {
    links::availability(&target)
}

pub async fn links_mentions_start(
    state: &Vault,
    target: String,
) -> Answer<links::MentionsStart, Failure<()>> {
    with_vault!(state, |backend| Ok(links::mentions::start(
        backend, &target
    )
    .await
    .into()))
}

pub async fn links_mentions_step(
    state: &Vault,
    scan: links::MentionScan,
    open_texts: Vec<links::OpenText>,
) -> Answer<links::MentionStep, Failure<()>> {
    with_vault!(state, |backend| Ok(links::mentions::step(
        backend,
        scan,
        &open_texts
    )
    .await))
}

pub fn links_mentions_cancel(scan: links::MentionScan) -> links::MentionScan {
    links::mentions::cancel(scan)
}

pub async fn links_mentions_in(
    state: &Vault,
    target: String,
    paths: Vec<String>,
    open_texts: Vec<links::OpenText>,
) -> Answer<Vec<links::SourceGroup>, Failure<()>> {
    with_vault!(state, |backend| Ok(links::in_notes(
        backend,
        &target,
        &paths,
        &open_texts
    )
    .await))
}

pub async fn links_link_mention(
    state: &Vault,
    request: links::LinkRequest,
    editor_text: Option<String>,
) -> Answer<links::LinkOutcome, Failure<()>> {
    with_vault!(state, |backend| Ok(links::link_mention(
        backend,
        &request,
        editor_text.as_deref()
    )
    .await))
}

// ── Вкладки и разделение редактора (фича 014) ───────────────────────────────
//
// Обёртки 1:1 над `typsidian_core::workspace`: правил здесь нет. Состояние
// держит окно и передаёт его в каждый вызов, как пакет экспорта. Аргумент
// состояния называется `workspace`, а не `state`: так зовётся `State<Vault>`.

pub fn workspace_empty() -> workspace::WorkspaceState {
    workspace::WorkspaceState::empty()
}

pub async fn workspace_apply(
    state: &Vault,
    workspace: workspace::WorkspaceState,
    action: workspace::WorkspaceAction,
    width_px: u32,
) -> Answer<workspace::Transition, Failure<()>> {
    with_vault!(state, |backend| Ok(workspace::apply_with(
        backend, workspace, action, width_px
    )
    .await))
}

pub async fn workspace_view(
    state: &Vault,
    workspace: workspace::WorkspaceState,
    width_px: u32,
) -> Answer<workspace::WorkspaceView, Failure<()>> {
    with_vault!(state, |backend| Ok(workspace::view_with(
        backend, &workspace, width_px
    )
    .await))
}

pub fn workspace_snapshot(workspace: workspace::WorkspaceState) -> workspace::SavedWorkspace {
    workspace::snapshot(&workspace)
}

pub async fn workspace_restore(
    state: &Vault,
    saved: workspace::SavedWorkspace,
) -> Answer<workspace::WorkspaceState, Failure<()>> {
    with_vault!(state, |backend| Ok(workspace::restore_with(
        backend, &saved
    )
    .await))
}

// ── Оглавление заметки (фича 015) ───────────────────────────────────────────
//
// Обёртки 1:1 над `typsidian_core::structure` и `preview::anchor`: правил
// здесь нет, состояния оболочки тоже (как у `export_batch_*`): `Outline` и
// запрос держит и передаёт окно. `None` приходит в окно как `null`, а из wasm -
// как `undefined`: клиент приводит ответ к одному виду.

pub fn structure_outline(text: String) -> structure::Outline {
    structure::outline(&text)
}

pub fn structure_outline_view(
    outline: structure::Outline,
    request: structure::OutlineRequest,
) -> structure::OutlineView {
    structure::outline_view(&outline, &request)
}

pub fn structure_outline_active(
    outline: structure::Outline,
    request: structure::OutlineRequest,
) -> Option<String> {
    structure::outline_active(&outline, &request)
}

pub fn structure_outline_carry(
    prev: structure::Outline,
    next: structure::Outline,
    collapsed: Vec<String>,
) -> Vec<String> {
    structure::carry_collapsed(&prev, &next, &collapsed)
}

/// `heading_probe` + `locate_heading`: проба наружу не выводится.
pub fn preview_locate_heading(
    runs: Vec<Vec<preview::TextRun>>,
    outline: structure::Outline,
    key: String,
) -> Option<preview::ResolvedAnchor> {
    let probe = structure::heading_probe(&outline, &key)?;
    preview::anchor::locate_heading(&runs, &probe)
}

pub fn properties_read(text: String) -> properties::NoteProperties {
    properties::read(&text)
}

pub fn properties_edit(
    text: String,
    change: properties::PropertyEdit,
) -> typsidian_core::elements::EditOutcome {
    properties::edit(&text, &change)
}

pub fn select_blocks(text: String) -> Vec<properties::SelectBlock> {
    properties::select_blocks(&text)
}

pub fn select_import_edits(text: String) -> Vec<typsidian_core::elements::TextEdit> {
    properties::select_import_edits(&text)
}

pub fn select_library() -> properties::SelectLibrary {
    properties::select_library()
}

pub fn search_parse_conditions(
    text: String,
    known_keys: Vec<String>,
) -> typsidian_core::search::ParsedConditions {
    typsidian_core::search::parse_conditions(&text, &known_keys)
}

pub async fn vault_select(
    state: &Vault,
    selection: properties::Selection,
) -> Answer<properties::SelectionResult, Failure<()>> {
    with_vault!(state, |backend| Ok(properties::select_notes(
        backend, &selection
    )
    .await))
}

/// Граф связей vault одним запросом (фича 025): узлы и связи с фильтром,
/// группами и локальной выборкой. Обёртка 1:1 над `graph::view::vault_graph`.
/// Метки связанных файлов заметки для дополнения 018: кэш связанных файлов —
/// тот же, что у сборки предпросмотра.
pub async fn completion_linked_index(
    state: &Vault,
    engine: &Preview,
    note_path: String,
    dependencies: Vec<String>,
) -> Answer<completion::LinkedIndex, Failure<()>> {
    with_vault!(state, |backend| Ok(completion::linked_index(
        backend,
        &engine.packages,
        &note_path,
        &dependencies,
    )
    .await))
}

/// Список предложений у каретки (018). Обёртка 1:1 над `completion::completions`.
pub async fn completions(
    state: &Vault,
    request: completion::CompletionRequest,
) -> Answer<completion::CompletionList, Failure<()>> {
    with_vault!(state, |backend| Ok(completion::completions(
        backend, &request
    )
    .await))
}

/// Цель перехода для ссылки под кареткой (018).
pub async fn definition_at(
    state: &Vault,
    request: completion::DefinitionRequest,
) -> Answer<completion::DefinitionOutcome, Failure<()>> {
    with_vault!(state, |backend| Ok(completion::definition_at(
        backend, &request
    )
    .await))
}

/// Отметки ссылок в никуда и повторных меток (018).
pub async fn reference_marks(
    state: &Vault,
    request: completion::MarksRequest,
) -> Answer<Vec<completion::ReferenceMark>, Failure<()>> {
    with_vault!(state, |backend| Ok(completion::reference_marks(
        backend, &request
    )
    .await))
}

pub async fn vault_graph(
    state: &Vault,
    request: graph_view::GraphRequest,
) -> Answer<graph_view::VaultGraph, Failure<()>> {
    with_vault!(state, |backend| Ok(graph_view::vault_graph(
        backend, &request
    )
    .await))
}

pub async fn vault_property_keys(
    state: &Vault,
) -> Answer<Vec<properties::PropertyKeyInfo>, Failure<()>> {
    with_vault!(
        state,
        |backend| Ok(properties::property_keys(backend).await)
    )
}

pub async fn vault_properties_current(state: &Vault) -> Answer<bool, Failure<()>> {
    with_vault!(state, |backend| Ok(VaultBackend::properties_current(
        backend
    )
    .await))
}

pub fn vault_cancel_reindex_properties(cancel: &PropertiesReindex) {
    cancel.0.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub async fn vault_ensure_select_library(
    state: &Vault,
) -> Answer<properties::LibraryStatus, Failure<WriteError>> {
    with_vault!(state, |backend| properties::ensure_select_library(backend)
        .await
        .map_err(Failure::Core))
}

pub async fn vault_select_digest(
    state: &Vault,
    text: String,
) -> Answer<Option<String>, Failure<()>> {
    with_vault!(state, |backend| Ok(properties::inputs_digest(
        backend, &text
    )
    .await))
}

// ── Показ слайдов (фича 019) ────────────────────────────────────────────────
//
// Обёртки 1:1 над `typsidian_core::slides`: правил и состояния оболочки здесь
// нет, `ShowState` и сведения о страницах держит и передаёт окно. Имена
// аргументов в `invoke` - camelCase (`nowMs`).

pub fn slides_start(
    slides: Vec<slides::PageSlideInfo>,
    caret: Option<u32>,
    now_ms: f64,
) -> Result<slides::ShowState, slides::SlidesError> {
    slides::start(&slides, caret, now_ms)
}

pub fn slides_navigate(
    state: slides::ShowState,
    slides: Vec<slides::PageSlideInfo>,
    action: slides::ShowAction,
) -> slides::ShowState {
    slides::navigate(state, &slides, action)
}

pub fn slides_carry(
    state: slides::ShowState,
    slides: Vec<slides::PageSlideInfo>,
) -> Result<slides::ShowState, slides::SlidesError> {
    slides::carry(state, &slides)
}

pub fn slides_caret(slides: Vec<slides::PageSlideInfo>, page: u32) -> Option<slides::SlidePlace> {
    slides::caret_at_page(&slides, page)
}

pub fn slides_label(slides: Vec<slides::PageSlideInfo>, page: u32) -> slides::SlideLabel {
    slides::slide_label(&slides, page)
}

pub fn slides_keymap() -> slides::ShowKeymap {
    slides::show_keymap()
}

// ── Быстрый захват и ежедневная заметка (фича 022) ──────────────────────────
//
// Обёртки 1:1 над `typsidian_core::capture`: правил здесь нет. Очередь захватов
// Android живёт в плагине `capture`, не здесь. Имена аргументов в `invoke` -
// camelCase (`openBuffer`, `imagePath`).

pub fn capture_settings(overrides: VaultOverrides) -> capture::CaptureSettings {
    capture::capture_settings(overrides.capture.as_ref())
}

pub fn capture_entry(
    settings: capture::CaptureSettings,
    capture: capture::Capture,
    image_path: Option<String>,
) -> String {
    capture::entry(&settings, &capture, image_path.as_deref())
}

pub fn capture_target_path(
    settings: capture::CaptureSettings,
    capture: capture::Capture,
) -> Option<String> {
    capture::capture_target_path(&settings, &capture)
}

pub fn daily_path(settings: capture::CaptureSettings, date: capture::DailyDate) -> Option<String> {
    capture::daily_path(&settings, &date)
}

pub fn daily_date_of(
    settings: capture::CaptureSettings,
    path: String,
) -> Option<capture::DailyDate> {
    capture::daily_date_of(&settings, &path)
}

/// Дописывает захват в заметку назначения. `open_buffer` - текст заметки в
/// редакторе, если она там открыта; `image` - байты картинки (массив чисел).
pub async fn capture_write(
    state: &Vault,
    capture: capture::Capture,
    open_buffer: Option<String>,
    image: Option<Vec<u8>>,
) -> Answer<capture::CaptureOutcome, Failure<()>> {
    with_vault!(state, |backend| Ok(capture::capture_write(
        backend,
        &capture,
        open_buffer.as_deref(),
        image.as_deref()
    )
    .await))
}

pub async fn open_daily(
    state: &Vault,
    date: capture::DailyDate,
) -> Answer<capture::OpenDaily, Failure<capture::DailyError>> {
    with_vault!(state, |backend| {
        let overrides = vault_settings::read_vault_settings(backend)
            .await
            .unwrap_or_default();
        let settings = capture::capture_settings(overrides.capture.as_ref());
        capture::open_daily(backend, &settings, &date)
            .await
            .map_err(Failure::Core)
    })
}

pub async fn daily_neighbours(
    state: &Vault,
    date: capture::DailyDate,
) -> Answer<capture::DailyNeighbours, Failure<()>> {
    with_vault!(state, |backend| {
        let overrides = vault_settings::read_vault_settings(backend)
            .await
            .unwrap_or_default();
        let settings = capture::capture_settings(overrides.capture.as_ref());
        Ok(capture::daily_neighbours(backend, &settings, &date).await)
    })
}

pub async fn check_template_name(state: &Vault, name: String) -> Answer<NameCheck, Failure<()>> {
    with_vault!(state, |backend| Ok(
        templates::check_name(backend, &name).await
    ))
}

pub async fn save_as_template(
    state: &Vault,
    name: String,
    content: String,
    confirm_overwrite: bool,
) -> Answer<templates::Template, Failure<SaveTemplateError>> {
    with_vault!(state, |backend| templates::save_as_template(
        backend,
        &name,
        &content,
        confirm_overwrite
    )
    .await
    .map_err(Failure::Core))
}

/// Какую правку совершить над текстом, чтобы вставить заготовку.
///
/// Открытого vault не требует: правило вставки текста от папки не зависит.
pub fn plan_insertion(
    text_length: usize,
    point: templates::InsertionPoint,
    content: String,
) -> templates::InsertionEdit {
    templates::plan_insertion(text_length, &point, &content)
}

// ── Параметры элементов (фича 008, contracts/element-params.md) ─────────
//
// Открытого vault не требуют: функции чистые. Те же формы запроса и ответа,
// что на wasm-границе (FR-022).

/// Схема элемента по ключу (`call:figure`) — для правки; `None` — параметров
/// нет.
pub fn element_schema(id: String) -> Option<typsidian_core::elements::ElementSpec> {
    typsidian_core::elements::element_schema(&id)
}

/// Схема элемента, который вставляет шаблон каталога; `None` — вставляется
/// сразу (FR-003).
pub fn element_schema_for(template: String) -> Option<typsidian_core::elements::ElementSpec> {
    typsidian_core::elements::element_schema_for(&template)
}

/// Элемент под кареткой (FR-006, FR-016).
pub fn element_at(text: String, offset: usize) -> typsidian_core::elements::ElementAtResult {
    typsidian_core::elements::element_at(&text, offset)
}

pub fn syntax_session_open(text: String, sessions: &HighlightSessions) -> u32 {
    let mut guard = sessions
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (next, map) = &mut *guard;
    *next = next.wrapping_add(1);
    map.insert(
        *next,
        typsidian_core::highlight::HighlightSession::new(text),
    );
    *next
}

/// Правки и раскраска окон одним обходом IPC; `None` — сессии нет или она
/// разошлась с редактором, клиент откроет новую.
pub fn syntax_session_update(
    id: u32,
    edits: Vec<typsidian_core::highlight::HighlightEdit>,
    length: usize,
    windows: Vec<typsidian_core::highlight::HighlightWindow>,
    sessions: &HighlightSessions,
) -> Option<Vec<u32>> {
    let mut guard = sessions
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session = guard.1.get_mut(&id)?;
    session
        .update(&edits, length, &windows)
        .map(|spans| spans.packed)
}

pub fn syntax_session_close(id: u32, sessions: &HighlightSessions) {
    let mut guard = sessions
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.1.remove(&id);
}

/// «Будет вставлено» (FR-004).
pub fn render_element(
    request: typsidian_core::elements::RenderRequest,
) -> typsidian_core::elements::RenderOutcome {
    typsidian_core::elements::render(&request)
}

/// Точечные замены для правки на месте (FR-008).
pub fn plan_element_edit(
    request: typsidian_core::elements::EditRequest,
) -> typsidian_core::elements::EditOutcome {
    typsidian_core::elements::plan(&request)
}

/// Затрагивает ли событие файла библиотеку заготовок.
pub fn template_event(event: VaultEvent) -> Option<templates::TemplateEvent> {
    templates::library_event(&event)
}

// ── Синхронизация (фича 011; общая с сервером с фичи 038) ───────────────────
//
// Открытие сессии (`sync_open`), Syncthing и клонирование в папку устройства —
// в оболочке (`packages/shell/src/sync.rs`): серверу они не нужны.

/// Ход идущей массовой операции — без блокировки сессии (011 T075, принцип X).
pub fn sync_progress(progress: &SyncProgressSlot) -> Option<typsidian_core::sync::SyncProgress> {
    progress.0.lock().ok().and_then(|slot| {
        slot.as_ref()
            .and_then(typsidian_core::sync::ProgressSink::current)
    })
}

pub fn sync_capabilities(home: &SyncHome) -> ShellCapabilities {
    home.capabilities()
}

pub async fn sync_overview(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
) -> SyncAnswer<SyncOverview> {
    with_sync!(home, vault, sync_state, |backend, session, ctx| Ok(session
        .overview(backend, &ctx)
        .await))
}

pub async fn sync_poll_events(sync_state: &SyncState) -> SyncAnswer<Vec<SyncEvent>> {
    let mut guard = sync_state.0.lock().await;
    Ok(guard
        .as_mut()
        .map(|open| open.session.poll_events())
        .unwrap_or_default())
}

pub async fn sync_note_change(sync_state: &SyncState) -> SyncAnswer<()> {
    if let Some(open) = sync_state.0.lock().await.as_mut() {
        open.session.note_change(sync::now());
    }
    Ok(())
}

/// Отмена текущей сетевой или массовой операции (принцип X). Не ждёт
/// блокировки сессии: токен общий у сессии и движка.
pub fn sync_cancel(cancel: &SyncCancel) {
    if let Ok(slot) = cancel.0.lock() {
        if let Some(token) = slot.as_ref() {
            token.cancel();
        }
    }
}

pub async fn sync_ignore_reports(
    vault: &Vault,
    sync_state: &SyncState,
) -> SyncAnswer<Vec<IgnoreReport>> {
    sync_read!(vault, sync_state, |backend, session| Ok(session
        .ignore_reports(backend)
        .await))
}

pub async fn sync_apply_ignore(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
    file: IgnoreFile,
) -> SyncAnswer<()> {
    with_sync!(home, vault, sync_state, |backend, session, ctx| {
        session.apply_ignore(backend, &ctx, file).await?;
        Ok(())
    })
}

pub async fn sync_decline_stignore(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
) -> SyncAnswer<()> {
    with_sync!(home, vault, sync_state, |_backend, session, ctx| {
        session.decline_stignore(&ctx);
        Ok(())
    })
}

// Редактор правил исключения (фича 012).

pub async fn sync_ignore_open(
    vault: &Vault,
    sync_state: &SyncState,
    file: IgnoreFile,
) -> SyncAnswer<IgnoreDocument> {
    sync_read!(vault, sync_state, |backend, session| Ok(session
        .ignore_open(backend, file)
        .await))
}

/// Чистая проверка текста: ни vault, ни сессия не нужны, поэтому набор в
/// редакторе не ждёт идущего обмена.
pub fn sync_ignore_check(file: IgnoreFile, text: String) -> IgnoreCheck {
    sync_ignore::check(file, &text)
}

pub fn sync_ignore_restore(file: IgnoreFile, text: String) -> String {
    sync_ignore::restore_block(file, &text)
}

pub async fn sync_ignore_save(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
    request: IgnoreSaveRequest,
) -> SyncAnswer<IgnoreSaveOutcome> {
    with_sync!(home, vault, sync_state, |backend, session, ctx| Ok(session
        .ignore_save(backend, &ctx, request)
        .await?))
}

pub async fn sync_enable_history(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
    message: String,
) -> SyncAnswer<Snapshot> {
    with_sync!(home, vault, sync_state, |backend, session, ctx| Ok(session
        .enable_history(backend, &ctx, &message)
        .await?))
}

pub async fn sync_changes(vault: &Vault, sync_state: &SyncState) -> SyncAnswer<Vec<Change>> {
    sync_read!(vault, sync_state, |backend, session| Ok(session
        .changes(backend)
        .await?))
}

#[allow(clippy::too_many_arguments)]
pub async fn sync_snapshot(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
    paths: Option<Vec<String>>,
    message: String,
    auto_prefix: String,
    flushed: bool,
    confirm_not_keeper: bool,
) -> SyncAnswer<Snapshot> {
    with_sync!(home, vault, sync_state, |backend, session, ctx| Ok(session
        .snapshot(
            backend,
            &ctx,
            paths,
            &message,
            &auto_prefix,
            flushed,
            confirm_not_keeper
        )
        .await?))
}

pub async fn sync_history(
    vault: &Vault,
    sync_state: &SyncState,
    path: Option<String>,
    cursor: Option<String>,
    limit: u32,
) -> SyncAnswer<HistoryPage> {
    sync_read!(vault, sync_state, |backend, session| Ok(session
        .history(backend, path.as_deref(), cursor.as_deref(), limit)
        .await?))
}

pub async fn sync_diff(
    vault: &Vault,
    sync_state: &SyncState,
    path: String,
    from: Revision,
    to: Revision,
) -> SyncAnswer<FileDiff> {
    sync_read!(vault, sync_state, |backend, session| Ok(session
        .diff(backend, &path, &from, &to)
        .await?))
}

pub async fn sync_snapshot_changes(
    vault: &Vault,
    sync_state: &SyncState,
    snapshot: String,
) -> SyncAnswer<Vec<Change>> {
    sync_read!(vault, sync_state, |backend, session| Ok(session
        .snapshot_changes(backend, &snapshot)
        .await?))
}

pub async fn sync_file_at(
    vault: &Vault,
    sync_state: &SyncState,
    path: String,
    revision: Revision,
) -> SyncAnswer<Content> {
    sync_read!(vault, sync_state, |backend, session| Ok(session
        .file_at(backend, &path, &revision)
        .await?))
}

pub async fn sync_restore(
    vault: &Vault,
    sync_state: &SyncState,
    path: String,
    snapshot: String,
) -> SyncAnswer<Content> {
    sync_locked!(vault, sync_state, |backend, session| Ok(session
        .restore(backend, &path, &snapshot)
        .await?))
}

pub async fn sync_connect_remote(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
    url: String,
    credentials: Credentials,
) -> SyncAnswer<RemoteStatus> {
    let _exchange = sync_state.1.enter()?;
    with_sync!(home, vault, sync_state, |backend, session, ctx| Ok(session
        .connect_remote(backend, home.secrets(), &ctx, &url, credentials)
        .await?))
}

pub async fn sync_disconnect_remote(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
) -> SyncAnswer<()> {
    sync_locked!(vault, sync_state, |backend, session| Ok(session
        .disconnect_remote(backend, home.secrets())
        .await?))
}

pub async fn sync_push(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
    confirm_not_keeper: bool,
) -> SyncAnswer<RemoteStatus> {
    let _exchange = sync_state.1.enter()?;
    let ctx = sync::sync_context(home, sync_state).await?;
    let (plan, engine) = sync_locked!(vault, sync_state, |backend, session| (
        session
            .begin_push(backend, home.secrets(), &ctx, confirm_not_keeper)
            .await?,
        session.engine().clone()
    ));
    let sent = plan.push(&engine).await;
    let result = sync_locked!(vault, sync_state, |backend, session| session
        .complete_push(backend, &ctx, &plan, sent)
        .await);
    sync::persist_device_update(home, sync_state).await;
    Ok(result?)
}

pub async fn sync_pull(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
    confirm_not_keeper: bool,
) -> SyncAnswer<PullOutcome> {
    let _exchange = sync_state.1.enter()?;
    let ctx = sync::sync_context(home, sync_state).await?;
    let (plan, engine) = sync_locked!(vault, sync_state, |backend, session| (
        session
            .begin_pull(backend, home.secrets(), &ctx, confirm_not_keeper)
            .await?,
        session.engine().clone()
    ));
    let fetched = plan.exchange.fetch(&engine).await;
    let result = sync_locked!(vault, sync_state, |backend, session| session
        .complete_pull(backend, &ctx, plan, fetched)
        .await);
    sync::persist_device_update(home, sync_state).await;
    Ok(result?)
}

pub async fn sync_restore_history(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
    confirm_not_keeper: bool,
) -> SyncAnswer<PullOutcome> {
    let _exchange = sync_state.1.enter()?;
    with_sync!(home, vault, sync_state, |backend, session, ctx| Ok(session
        .restore_history(backend, home.secrets(), &ctx, confirm_not_keeper)
        .await?))
}

pub async fn sync_abort_pull(vault: &Vault, sync_state: &SyncState) -> SyncAnswer<()> {
    sync_locked!(vault, sync_state, |backend, session| Ok(session
        .abort_pull(backend)
        .await?))
}

pub async fn sync_conflicts(vault: &Vault, sync_state: &SyncState) -> SyncAnswer<Vec<Conflict>> {
    sync_read!(vault, sync_state, |backend, session| Ok(session
        .conflicts(backend)
        .await))
}

pub async fn sync_resolve(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
    id: String,
    resolution: Resolution,
) -> SyncAnswer<()> {
    with_sync!(home, vault, sync_state, |backend, session, ctx| Ok(session
        .resolve(backend, &ctx, &id, resolution)
        .await?))
}

pub async fn sync_set_keeper(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
) -> SyncAnswer<Keeper> {
    with_sync!(home, vault, sync_state, |backend, session, ctx| Ok(session
        .set_keeper(backend, &ctx)
        .await?))
}

pub async fn sync_clear_keeper(vault: &Vault, sync_state: &SyncState) -> SyncAnswer<()> {
    sync_locked!(vault, sync_state, |backend, session| Ok(session
        .clear_keeper(backend)
        .await?))
}

pub async fn sync_update_settings(
    vault: &Vault,
    sync_state: &SyncState,
    patch: VaultSyncSettingsPatch,
) -> SyncAnswer<VaultSyncSettings> {
    sync_locked!(vault, sync_state, |backend, session| Ok(session
        .update_settings(backend, &patch)
        .await?))
}

pub async fn sync_refresh_mirror(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
) -> SyncAnswer<()> {
    let _exchange = sync_state.1.enter()?;
    let ctx = sync::sync_context(home, sync_state).await?;
    let (plan, mirror) = sync_locked!(vault, sync_state, |backend, session| (
        session.begin_mirror(backend, home.secrets(), &ctx).await?,
        session.mirror_engine().cloned()
    ));
    let fetched = match &mirror {
        Some(mirror) => plan.mirror(mirror).await,
        None => Err(SyncError::NoHistory),
    };
    let result = sync_locked!(vault, sync_state, |_backend, session| session
        .complete_mirror(&ctx, fetched));
    sync::persist_device_update(home, sync_state).await;
    Ok(result?)
}

/// Такт автоматики (011 FR-019, FR-031): спрашивает ядро, пора ли снимать и
/// обмениваться. Таймер — у окна; состояние Syncthing держит наблюдатель
/// оболочки, такт его не опрашивает.
pub async fn sync_tick(
    home: &SyncHome,
    vault: &Vault,
    sync_state: &SyncState,
    auto_prefix: String,
) -> SyncAnswer<TickOutcome> {
    // Идёт обмен по команде — такт ждёт следующего раза.
    let Ok(_exchange) = sync_state.1.enter() else {
        return Ok(TickOutcome::idle(sync::now()));
    };
    let ctx = sync::sync_context(home, sync_state).await?;
    let secrets = home.secrets();
    // Такт по шагам (011 T071): сеть автополучения и автоотправки — без блокировок.
    let mut outcome = TickOutcome::idle(ctx.now);
    let pull = sync_locked!(vault, sync_state, |backend, session| {
        if session.tick_pull_due(backend, &ctx).await {
            match session.begin_pull(backend, secrets, &ctx, false).await {
                Ok(plan) => Some((plan, session.engine().clone())),
                Err(error) => {
                    session.tick_pulled(&mut outcome, Err(error));
                    None
                }
            }
        } else {
            None
        }
    });
    let fetched = match &pull {
        Some((plan, engine)) => Some(plan.exchange.fetch(engine).await),
        None => None,
    };
    let push = sync_locked!(vault, sync_state, |backend, session| {
        if let (Some((plan, _)), Some(fetched)) = (pull, fetched) {
            let result = session.complete_pull(backend, &ctx, plan, fetched).await;
            session.tick_pulled(&mut outcome, result);
        }
        if session
            .tick_local(backend, &ctx, &auto_prefix, &mut outcome)
            .await
        {
            match session.begin_push(backend, secrets, &ctx, false).await {
                Ok(plan) => Some((plan, session.engine().clone())),
                Err(error) => {
                    session.tick_pushed(&mut outcome, Err(error));
                    None
                }
            }
        } else {
            None
        }
    });
    let sent = match &push {
        Some((plan, engine)) => Some(plan.push(engine).await),
        None => None,
    };
    sync_locked!(vault, sync_state, |backend, session| {
        if let (Some((plan, _)), Some(sent)) = (push, sent) {
            let result = session.complete_push(backend, &ctx, &plan, sent).await;
            session.tick_pushed(&mut outcome, result);
        }
        session.tick_finish(backend, &ctx, &mut outcome).await;
    });
    sync::persist_device_update(home, sync_state).await;
    Ok(outcome)
}

/// Значения устройства открытого vault: автор снимков (038 research §5).
pub async fn sync_device_values(
    home: &SyncHome,
    sync_state: &SyncState,
) -> SyncAnswer<DeviceVaultSync> {
    sync::device_values(home, sync_state).await
}

/// Автор снимков этого vault на этом устройстве; `None` — сбросить.
pub async fn sync_set_author(
    home: &SyncHome,
    sync_state: &SyncState,
    author: Option<Author>,
) -> SyncAnswer<()> {
    if author.as_ref().is_some_and(|author| !author.is_valid()) {
        return Err(SyncError::Io {
            message: "автор без имени или почты".to_string(),
        }
        .into());
    }
    sync::change_device_values(home, sync_state, |values| values.author = author).await
}
