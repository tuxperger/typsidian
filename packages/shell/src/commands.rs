// @generated scripts/dev/gen-host-dispatch.mjs из packages/host/src/commands.rs.
// Не править руками: `nix develop -c node scripts/dev/gen-host-dispatch.mjs`.
//! Обёртки Tauri над командами `typsidian_host` (фича 037): имена и аргументы
//! прежние, тела — вызов общей реализации.

#![allow(clippy::too_many_arguments)]

use tauri::State;
use typsidian_host::prelude::*;
use typsidian_host::*;

#[tauri::command]
pub async fn list(state: State<'_, Vault>) -> Answer<Vec<Note>, Failure<()>> {
    typsidian_host::commands::list(&state).await
}

#[tauri::command]
pub async fn read(state: State<'_, Vault>, path: String) -> Answer<String, Failure<ReadError>> {
    typsidian_host::commands::read(&state, path).await
}

#[tauri::command]
pub async fn write(
    state: State<'_, Vault>,
    path: String,
    content: String,
    expected: Option<FileStamp>,
) -> Answer<(), Failure<WriteError>> {
    typsidian_host::commands::write(&state, path, content, expected).await
}

#[tauri::command]
pub async fn create(state: State<'_, Vault>, path: String) -> Answer<(), Failure<CreateError>> {
    typsidian_host::commands::create(&state, path).await
}

#[tauri::command]
pub async fn rename(
    state: State<'_, Vault>,
    from: String,
    to: String,
) -> Answer<(), Failure<RenameError>> {
    typsidian_host::commands::rename(&state, from, to).await
}

#[tauri::command]
pub async fn move_targets(
    state: State<'_, Vault>,
    from: String,
    query: String,
) -> Answer<Vec<MoveTarget>, Failure<()>> {
    typsidian_host::commands::move_targets(&state, from, query).await
}

#[tauri::command]
pub async fn move_impact(
    state: State<'_, Vault>,
    request: MoveRequest,
) -> Answer<MoveImpact, Failure<MoveRefusal>> {
    typsidian_host::commands::move_impact(&state, request).await
}

#[tauri::command]
pub async fn move_entry(
    state: State<'_, Vault>,
    request: MoveRequest,
) -> Answer<MoveOutcome, Failure<MoveError>> {
    typsidian_host::commands::move_entry(&state, request).await
}

#[tauri::command]
pub async fn delete(state: State<'_, Vault>, path: String) -> Answer<(), Failure<DeleteError>> {
    typsidian_host::commands::delete(&state, path).await
}

#[tauri::command]
pub async fn backlinks(state: State<'_, Vault>, path: String) -> Answer<Vec<Note>, Failure<()>> {
    typsidian_host::commands::backlinks(&state, path).await
}

#[tauri::command]
pub async fn broken_links(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Vec<PathBuf>, Failure<()>> {
    typsidian_host::commands::broken_links(&state, path).await
}

#[tauri::command]
pub async fn access_state(state: State<'_, Vault>) -> Answer<AccessState, Failure<()>> {
    typsidian_host::commands::access_state(&state).await
}

#[tauri::command]
pub async fn search(
    state: State<'_, Vault>,
    query: typsidian_core::search::SearchQuery,
) -> Answer<typsidian_core::search::SearchResults, Failure<typsidian_core::search::SearchError>> {
    typsidian_host::commands::search(&state, query).await
}

#[tauri::command]
pub async fn replace_preview(
    state: State<'_, Vault>,
    query: typsidian_core::search::replace::ReplaceQuery,
    open_texts: Vec<links::OpenText>,
) -> Answer<
    typsidian_core::search::replace::ReplacePreview,
    Failure<typsidian_core::search::replace::ReplaceError>,
> {
    typsidian_host::commands::replace_preview(&state, query, open_texts).await
}

#[tauri::command]
pub async fn replace_in_note(
    state: State<'_, Vault>,
    request: typsidian_core::search::replace::ReplaceNoteRequest,
    editor_text: Option<String>,
) -> Answer<typsidian_core::search::replace::ReplaceNoteOutcome, Failure<()>> {
    typsidian_host::commands::replace_in_note(&state, request, editor_text).await
}

#[tauri::command]
pub async fn replace_undo(
    state: State<'_, Vault>,
    record: typsidian_core::search::replace::UndoRecord,
    editor_text: Option<String>,
) -> Answer<typsidian_core::search::replace::UndoOutcome, Failure<()>> {
    typsidian_host::commands::replace_undo(&state, record, editor_text).await
}

#[tauri::command]
pub async fn links_health(
    state: State<'_, Vault>,
    open_texts: Vec<links::OpenText>,
) -> Answer<links::health::LinkHealth, Failure<()>> {
    typsidian_host::commands::links_health(&state, open_texts).await
}

#[tauri::command]
pub async fn links_fix(
    state: State<'_, Vault>,
    request: links::health::LinkFixRequest,
    editor_text: Option<String>,
) -> Answer<typsidian_core::search::replace::ReplaceNoteOutcome, Failure<()>> {
    typsidian_host::commands::links_fix(&state, request, editor_text).await
}

#[tauri::command]
pub async fn snapshot_take(
    state: State<'_, Vault>,
    path: String,
    text: String,
    reason: snapshots::SnapshotReason,
) -> Answer<snapshots::TakeOutcome, Failure<()>> {
    typsidian_host::commands::snapshot_take(&state, path, text, reason).await
}

#[tauri::command]
pub async fn snapshot_history(
    state: State<'_, Vault>,
    path: String,
) -> Answer<snapshots::NoteHistory, Failure<()>> {
    typsidian_host::commands::snapshot_history(&state, path).await
}

#[tauri::command]
pub async fn snapshot_diff(
    state: State<'_, Vault>,
    id: String,
    against: String,
) -> Answer<Vec<typsidian_core::vault::conflict::DiffLine>, Failure<()>> {
    typsidian_host::commands::snapshot_diff(&state, id, against).await
}

#[tauri::command]
pub async fn snapshot_text(
    state: State<'_, Vault>,
    id: String,
) -> Answer<Option<String>, Failure<()>> {
    typsidian_host::commands::snapshot_text(&state, id).await
}

#[tauri::command]
pub async fn snapshot_restore(
    state: State<'_, Vault>,
    path: String,
    id: String,
    editor_text: Option<String>,
) -> Answer<snapshots::RestoreOutcome, Failure<()>> {
    typsidian_host::commands::snapshot_restore(&state, path, id, editor_text).await
}

#[tauri::command]
pub async fn snapshot_deleted_notes(
    state: State<'_, Vault>,
) -> Answer<Vec<snapshots::DeletedNote>, Failure<()>> {
    typsidian_host::commands::snapshot_deleted_notes(&state).await
}

#[tauri::command]
pub async fn snapshot_recover_note(
    state: State<'_, Vault>,
    path: String,
    as_path: Option<String>,
) -> Answer<snapshots::RecoverOutcome, Failure<()>> {
    typsidian_host::commands::snapshot_recover_note(&state, path, as_path).await
}

#[tauri::command]
pub async fn snapshot_usage(
    state: State<'_, Vault>,
) -> Answer<snapshots::SnapshotUsage, Failure<()>> {
    typsidian_host::commands::snapshot_usage(&state).await
}

#[tauri::command]
pub async fn snapshots_clear(state: State<'_, Vault>) -> Answer<u32, Failure<()>> {
    typsidian_host::commands::snapshots_clear(&state).await
}

#[tauri::command]
pub async fn tasks_view(
    filter: tasks::TaskFilter,
    today: String,
    state: State<'_, Vault>,
) -> Answer<tasks::TasksView, Failure<()>> {
    typsidian_host::commands::tasks_view(filter, today, &state).await
}

#[tauri::command]
pub async fn task_toggle(
    request: tasks::TaskToggleRequest,
    editor_text: Option<String>,
    state: State<'_, Vault>,
) -> Answer<tasks::TaskToggleOutcome, Failure<()>> {
    typsidian_host::commands::task_toggle(request, editor_text, &state).await
}

#[tauri::command]
pub fn task_toggle_at(text: String, offset: u32) -> Option<typsidian_core::elements::TextEdit> {
    typsidian_host::commands::task_toggle_at(text, offset)
}

#[tauri::command]
pub async fn bookmarks_view(
    state: State<'_, Vault>,
) -> Answer<bookmarks::BookmarksView, Failure<()>> {
    typsidian_host::commands::bookmarks_view(&state).await
}

#[tauri::command]
pub async fn bookmark_add(
    target: bookmarks::BookmarkTarget,
    name: Option<String>,
    state: State<'_, Vault>,
) -> Answer<bookmarks::AddOutcome, Failure<bookmarks::BookmarkError>> {
    typsidian_host::commands::bookmark_add(target, name, &state).await
}

#[tauri::command]
pub async fn bookmark_remove(id: String, state: State<'_, Vault>) -> BookmarkAnswer {
    typsidian_host::commands::bookmark_remove(id, &state).await
}

#[tauri::command]
pub async fn bookmark_rename(id: String, name: String, state: State<'_, Vault>) -> BookmarkAnswer {
    typsidian_host::commands::bookmark_rename(id, name, &state).await
}

#[tauri::command]
pub async fn bookmark_move(
    id: String,
    place: bookmarks::BookmarkPlace,
    state: State<'_, Vault>,
) -> BookmarkAnswer {
    typsidian_host::commands::bookmark_move(id, place, &state).await
}

#[tauri::command]
pub async fn bookmark_group_create(name: String, state: State<'_, Vault>) -> BookmarkAnswer {
    typsidian_host::commands::bookmark_group_create(name, &state).await
}

#[tauri::command]
pub async fn bookmark_group_rename(
    id: String,
    name: String,
    state: State<'_, Vault>,
) -> BookmarkAnswer {
    typsidian_host::commands::bookmark_group_rename(id, name, &state).await
}

#[tauri::command]
pub async fn bookmark_group_toggle(id: String, state: State<'_, Vault>) -> BookmarkAnswer {
    typsidian_host::commands::bookmark_group_toggle(id, &state).await
}

#[tauri::command]
pub async fn bookmark_group_delete(id: String, state: State<'_, Vault>) -> BookmarkAnswer {
    typsidian_host::commands::bookmark_group_delete(id, &state).await
}

#[tauri::command]
pub async fn embed_insert(
    text: String,
    offset: u32,
    state: State<'_, Vault>,
) -> Answer<typsidian_core::embeds::EmbedInsertion, Failure<()>> {
    typsidian_host::commands::embed_insert(text, offset, &state).await
}

#[tauri::command]
pub async fn spell_configure(
    state: State<'_, Vault>,
    speller: State<'_, Speller>,
) -> Answer<spellcheck::SpellConfig, Failure<()>> {
    typsidian_host::commands::spell_configure(&state, &speller).await
}

#[tauri::command]
pub async fn spell_add_word(
    word: String,
    state: State<'_, Vault>,
    speller: State<'_, Speller>,
) -> Answer<spellcheck::SpellConfig, Failure<spellcheck::SpellError>> {
    typsidian_host::commands::spell_add_word(word, &state, &speller).await
}

#[tauri::command]
pub async fn spell_remove_word(
    word: String,
    state: State<'_, Vault>,
    speller: State<'_, Speller>,
) -> Answer<spellcheck::SpellConfig, Failure<spellcheck::SpellError>> {
    typsidian_host::commands::spell_remove_word(word, &state, &speller).await
}

#[tauri::command]
pub fn spell_suggest(word: String, speller: State<'_, Speller>) -> Vec<String> {
    typsidian_host::commands::spell_suggest(word, &speller)
}

#[tauri::command]
pub fn syntax_session_spelling(
    id: u32,
    windows: Vec<typsidian_core::highlight::HighlightWindow>,
    sessions: State<'_, HighlightSessions>,
    speller: State<'_, Speller>,
) -> Option<Vec<u32>> {
    typsidian_host::commands::syntax_session_spelling(id, windows, &sessions, &speller)
}

#[tauri::command]
pub async fn root_path(state: State<'_, Vault>) -> Answer<String, Failure<()>> {
    typsidian_host::commands::root_path(&state).await
}

#[tauri::command]
pub fn board_write_profile() -> boards::BoardWriteProfile {
    typsidian_host::commands::board_write_profile()
}

#[tauri::command]
pub fn file_kind(path: String) -> FileKind {
    typsidian_host::commands::file_kind(path)
}

#[tauri::command]
pub async fn begin_edit(
    state: State<'_, Vault>,
    path: String,
    buffer: String,
) -> Answer<(), Failure<()>> {
    typsidian_host::commands::begin_edit(&state, path, buffer).await
}

#[tauri::command]
pub async fn discard_edit(state: State<'_, Vault>, path: String) -> Answer<(), Failure<()>> {
    typsidian_host::commands::discard_edit(&state, path).await
}

#[tauri::command]
pub async fn pending_conflict(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Option<EditConflict>, Failure<()>> {
    typsidian_host::commands::pending_conflict(&state, path).await
}

#[tauri::command]
pub async fn unresolved_edit_paths(state: State<'_, Vault>) -> Answer<Vec<PathBuf>, Failure<()>> {
    typsidian_host::commands::unresolved_edit_paths(&state).await
}

#[tauri::command]
pub async fn conflict_diff(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Vec<DiffLine>, Failure<()>> {
    typsidian_host::commands::conflict_diff(&state, path).await
}

#[tauri::command]
pub async fn resolve_conflict_keep_local(
    state: State<'_, Vault>,
    path: String,
) -> Answer<(), Failure<ResolveError>> {
    typsidian_host::commands::resolve_conflict_keep_local(&state, path).await
}

#[tauri::command]
pub async fn resolve_conflict_accept_external(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Option<String>, Failure<ResolveError>> {
    typsidian_host::commands::resolve_conflict_accept_external(&state, path).await
}

#[tauri::command]
pub async fn create_board(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Board, Failure<CreateBoardError>> {
    typsidian_host::commands::create_board(&state, path).await
}

#[tauri::command]
pub async fn read_board(
    state: State<'_, Vault>,
    path: String,
) -> Answer<String, Failure<ReadBoardError>> {
    typsidian_host::commands::read_board(&state, path).await
}

#[tauri::command]
pub async fn write_board(
    state: State<'_, Vault>,
    path: String,
    content: String,
    expected: Option<FileStamp>,
) -> Answer<(), Failure<WriteBoardError>> {
    typsidian_host::commands::write_board(&state, path, content, expected).await
}

#[tauri::command]
pub async fn write_snapshot(
    state: State<'_, Vault>,
    snapshot_path: String,
    svg: String,
    board_path: String,
    board_content: String,
) -> Answer<(), Failure<SnapshotWriteError>> {
    typsidian_host::commands::write_snapshot(&state, snapshot_path, svg, board_path, board_content)
        .await
}

#[tauri::command]
pub async fn embeds_of(
    state: State<'_, Vault>,
    note_path: String,
    note_text: String,
) -> Answer<Vec<Embed>, Failure<()>> {
    typsidian_host::commands::embeds_of(&state, note_path, note_text).await
}

#[tauri::command]
pub fn embeds_in_text(note_path: String, note_text: String) -> Vec<EmbedRef> {
    typsidian_host::commands::embeds_in_text(note_path, note_text)
}

#[tauri::command]
pub fn embeds_changed(note_path: String, note_text: String, changed: String) -> Option<BoardEvent> {
    typsidian_host::commands::embeds_changed(note_path, note_text, changed)
}

#[tauri::command]
pub fn plan_embed(
    note_path: String,
    note_text: String,
    board_path: String,
    overrides: VaultOverrides,
) -> Answer<EmbedPlan, EmbedError> {
    typsidian_host::commands::plan_embed(note_path, note_text, board_path, overrides)
}

#[tauri::command]
pub async fn vault_settings(
    state: State<'_, Vault>,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    typsidian_host::commands::vault_settings(&state).await
}

#[tauri::command]
pub async fn set_vault_settings(
    state: State<'_, Vault>,
    overrides: VaultOverrides,
) -> Answer<(), Failure<VaultSettingsError>> {
    typsidian_host::commands::set_vault_settings(&state, overrides).await
}

#[tauri::command]
pub async fn mark_section(
    state: State<'_, Vault>,
    folder: String,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    typsidian_host::commands::mark_section(&state, folder).await
}

#[tauri::command]
pub async fn unmark_section(
    state: State<'_, Vault>,
    folder: String,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    typsidian_host::commands::unmark_section(&state, folder).await
}

#[tauri::command]
pub fn embed_markup(plan: EmbedPlan) -> String {
    typsidian_host::commands::embed_markup(plan)
}

#[tauri::command]
pub fn keybindings(overrides: VaultOverrides, mac: bool) -> Vec<ResolvedBinding> {
    typsidian_host::commands::keybindings(overrides, mac)
}

#[tauri::command]
pub async fn set_keybinding(
    state: State<'_, Vault>,
    command: CommandId,
    shortcut: Option<Shortcut>,
) -> Answer<VaultOverrides, Failure<SetKeybindingError>> {
    typsidian_host::commands::set_keybinding(&state, command, shortcut).await
}

#[tauri::command]
pub async fn reset_keybinding(
    state: State<'_, Vault>,
    command: CommandId,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    typsidian_host::commands::reset_keybinding(&state, command).await
}

#[tauri::command]
pub async fn template_library(state: State<'_, Vault>) -> Answer<TemplateLibraryView, Failure<()>> {
    typsidian_host::commands::template_library(&state).await
}

#[tauri::command]
pub async fn read_template(
    state: State<'_, Vault>,
    path: String,
) -> Answer<String, Failure<ReadTemplateError>> {
    typsidian_host::commands::read_template(&state, path).await
}

#[tauri::command]
pub async fn template_preview(
    state: State<'_, Vault>,
    cache: State<'_, TemplatePreviews>,
    template: templates::Template,
) -> Answer<PreviewState, Failure<()>> {
    typsidian_host::commands::template_preview(&state, &cache, template).await
}

#[tauri::command]
pub async fn preview_compile(
    state: State<'_, Vault>,
    engine: State<'_, Preview>,
    note_path: String,
    text: String,
    revision: u64,
    anchor: Option<preview::ScrollAnchor>,
) -> Answer<CompiledDocument, Failure<SnapshotError>> {
    typsidian_host::commands::preview_compile(&state, &engine, note_path, text, revision, anchor)
        .await
}

#[tauri::command]
pub async fn preview_dependency_mark(
    state: State<'_, Vault>,
    paths: Vec<String>,
) -> Answer<String, Failure<SnapshotError>> {
    typsidian_host::commands::preview_dependency_mark(&state, paths).await
}

#[tauri::command]
pub async fn preview_compile_with_templates(
    state: State<'_, Vault>,
    engine: State<'_, Preview>,
    note_path: String,
    text: String,
    revision: u64,
    overrides: VaultOverrides,
    anchor: Option<preview::ScrollAnchor>,
) -> Answer<CompiledDocument, Failure<SnapshotError>> {
    typsidian_host::commands::preview_compile_with_templates(
        &state, &engine, note_path, text, revision, overrides, anchor,
    )
    .await
}

#[tauri::command]
pub async fn preview_install_template(
    state: State<'_, Vault>,
    engine: State<'_, Preview>,
    spec: String,
    overrides: VaultOverrides,
) -> Answer<preview::packages::InstallReport, Failure<preview::packages::InstallError>> {
    typsidian_host::commands::preview_install_template(&state, &engine, spec, overrides).await
}

#[tauri::command]
pub async fn preview_installed_templates(
    state: State<'_, Vault>,
) -> Answer<Vec<preview::packages::InstalledTemplate>, Failure<()>> {
    typsidian_host::commands::preview_installed_templates(&state).await
}

#[tauri::command]
pub async fn export_pdf(
    state: State<'_, Vault>,
    engine: State<'_, Preview>,
    note_path: String,
    text: Option<String>,
    overrides: VaultOverrides,
) -> Answer<export::ExportReport, Failure<()>> {
    typsidian_host::commands::export_pdf(&state, &engine, note_path, text, overrides).await
}

#[tauri::command]
pub async fn export_slides(
    state: State<'_, Vault>,
    engine: State<'_, Preview>,
    note_path: String,
    text: Option<String>,
    overrides: VaultOverrides,
    request: typsidian_core::slides::export::SlidesExportRequest,
) -> Answer<typsidian_core::slides::export::SlidesExportReport, Failure<()>> {
    typsidian_host::commands::export_slides(&state, &engine, note_path, text, overrides, request)
        .await
}

#[tauri::command]
pub async fn export_html(
    state: State<'_, Vault>,
    engine: State<'_, Preview>,
    note_path: String,
    text: Option<String>,
    overrides: VaultOverrides,
    site: Vec<String>,
    labels: export::SiteLabels,
) -> Answer<export::ExportReport, Failure<()>> {
    typsidian_host::commands::export_html(&state, &engine, note_path, text, overrides, site, labels)
        .await
}

#[tauri::command]
pub async fn export_site_index(
    state: State<'_, Vault>,
    overrides: VaultOverrides,
    site: Vec<String>,
    labels: export::SiteLabels,
) -> Answer<String, Failure<export::ExportWriteProblem>> {
    typsidian_host::commands::export_site_index(&state, overrides, site, labels).await
}

#[tauri::command]
pub fn export_batch_with_format(
    state: export::BatchState,
    format: export::ExportFormat,
) -> export::BatchState {
    typsidian_host::commands::export_batch_with_format(state, format)
}

#[tauri::command]
pub async fn export_candidates(
    state: State<'_, Vault>,
    overrides: VaultOverrides,
) -> Answer<Vec<export::ExportCandidate>, Failure<()>> {
    typsidian_host::commands::export_candidates(&state, overrides).await
}

#[tauri::command]
pub fn export_batch_start(
    paths: Vec<String>,
    stop_on_error: bool,
    overrides: VaultOverrides,
) -> export::BatchState {
    typsidian_host::commands::export_batch_start(paths, stop_on_error, overrides)
}

#[tauri::command]
pub fn export_batch_next(state: export::BatchState) -> export::BatchStep {
    typsidian_host::commands::export_batch_next(state)
}

#[tauri::command]
pub fn export_batch_record(
    state: export::BatchState,
    report: export::ExportReport,
    elapsed_ms: u32,
) -> export::BatchState {
    typsidian_host::commands::export_batch_record(state, report, elapsed_ms)
}

#[tauri::command]
pub fn export_batch_skip(state: export::BatchState, note_path: String) -> export::BatchState {
    typsidian_host::commands::export_batch_skip(state, note_path)
}

#[tauri::command]
pub fn export_batch_cancel(state: export::BatchState) -> export::BatchState {
    typsidian_host::commands::export_batch_cancel(state)
}

#[tauri::command]
pub fn export_batch_view(state: export::BatchState) -> export::BatchView {
    typsidian_host::commands::export_batch_view(state)
}

#[tauri::command]
pub async fn sources_refresh(
    state: State<'_, Vault>,
    engine: State<'_, Sources>,
    registered: Vec<String>,
) -> Answer<sources::RefreshStep, Failure<()>> {
    typsidian_host::commands::sources_refresh(&state, &engine, registered).await
}

#[tauri::command]
pub async fn sources_summary(engine: State<'_, Sources>) -> Answer<sources::LibrarySummary, ()> {
    typsidian_host::commands::sources_summary(&engine).await
}

#[tauri::command]
pub async fn sources_search(
    engine: State<'_, Sources>,
    query: String,
    limit: u32,
    uncited_only: bool,
) -> Answer<sources::SourceHits, ()> {
    typsidian_host::commands::sources_search(&engine, query, limit, uncited_only).await
}

#[tauri::command]
pub async fn sources_for_completion(
    engine: State<'_, Sources>,
    prefix: String,
    limit: u32,
) -> Answer<Vec<sources::SourceHit>, ()> {
    typsidian_host::commands::sources_for_completion(&engine, prefix, limit).await
}

#[tauri::command]
pub async fn sources_scan_start(
    state: State<'_, Vault>,
    engine: State<'_, Sources>,
) -> Answer<sources::CitationScanView, Failure<()>> {
    typsidian_host::commands::sources_scan_start(&state, &engine).await
}

#[tauri::command]
pub async fn sources_scan_step(
    state: State<'_, Vault>,
    engine: State<'_, Sources>,
    max_notes: u32,
) -> Answer<sources::CitationScanView, Failure<()>> {
    typsidian_host::commands::sources_scan_step(&state, &engine, max_notes).await
}

#[tauri::command]
pub async fn sources_scan_cancel(
    engine: State<'_, Sources>,
) -> Answer<sources::CitationScanView, ()> {
    typsidian_host::commands::sources_scan_cancel(&engine).await
}

#[tauri::command]
pub async fn sources_scan_view(
    engine: State<'_, Sources>,
) -> Answer<sources::CitationScanView, ()> {
    typsidian_host::commands::sources_scan_view(&engine).await
}

#[tauri::command]
pub async fn sources_note_changed(
    state: State<'_, Vault>,
    engine: State<'_, Sources>,
    path: String,
) -> Answer<(), Failure<()>> {
    typsidian_host::commands::sources_note_changed(&state, &engine, path).await
}

#[tauri::command]
pub async fn sources_note_removed(engine: State<'_, Sources>, path: String) -> Answer<(), ()> {
    typsidian_host::commands::sources_note_removed(&engine, path).await
}

#[tauri::command]
pub async fn sources_note_renamed(
    engine: State<'_, Sources>,
    from: String,
    to: String,
) -> Answer<(), ()> {
    typsidian_host::commands::sources_note_renamed(&engine, from, to).await
}

#[tauri::command]
pub async fn sources_citing(
    engine: State<'_, Sources>,
    key: String,
) -> Answer<Vec<sources::CitingNote>, ()> {
    typsidian_host::commands::sources_citing(&engine, key).await
}

#[tauri::command]
pub async fn sources_missing_keys(
    engine: State<'_, Sources>,
) -> Answer<Vec<sources::MissingKey>, ()> {
    typsidian_host::commands::sources_missing_keys(&engine).await
}

#[tauri::command]
pub async fn sources_check_note(
    engine: State<'_, Sources>,
    text: String,
    known_labels: Vec<String>,
) -> Answer<sources::NoteCheck, ()> {
    typsidian_host::commands::sources_check_note(&engine, text, known_labels).await
}

#[tauri::command]
pub async fn sources_plan_citation(
    engine: State<'_, Sources>,
    text: String,
    caret: usize,
    file: String,
    key: String,
) -> Answer<sources::CitationPlan, sources::CitationPlanError> {
    typsidian_host::commands::sources_plan_citation(&engine, text, caret, file, key).await
}

#[tauri::command]
pub async fn sources_bibliography_offer(
    engine: State<'_, Sources>,
    text: String,
    note_path: String,
    file: String,
) -> Answer<sources::BibliographyOffer, ()> {
    typsidian_host::commands::sources_bibliography_offer(&engine, text, note_path, file).await
}

#[tauri::command]
pub async fn sources_create_file(
    state: State<'_, Vault>,
) -> Answer<String, Failure<sources::CreateSourceFileError>> {
    typsidian_host::commands::sources_create_file(&state).await
}

#[tauri::command]
pub async fn attachments_batch_start(
    state: State<'_, Vault>,
    overrides: VaultOverrides,
    note_path: String,
    items: Vec<attachments::IncomingItem>,
    moment: attachments::InsertMoment,
) -> Answer<attachments::AttachmentBatchState, Failure<()>> {
    typsidian_host::commands::attachments_batch_start(&state, overrides, note_path, items, moment)
        .await
}

#[tauri::command]
pub fn attachments_batch_next(
    state: attachments::AttachmentBatchState,
) -> attachments::AttachmentBatchStep {
    typsidian_host::commands::attachments_batch_next(state)
}

#[tauri::command]
pub fn attachments_batch_confirm(
    state: attachments::AttachmentBatchState,
    index: u32,
    accept: bool,
) -> attachments::AttachmentBatchState {
    typsidian_host::commands::attachments_batch_confirm(state, index, accept)
}

#[tauri::command]
pub fn attachments_batch_source_failed(
    state: attachments::AttachmentBatchState,
    index: u32,
) -> attachments::AttachmentBatchState {
    typsidian_host::commands::attachments_batch_source_failed(state, index)
}

#[tauri::command]
pub fn attachments_batch_cancel(
    state: attachments::AttachmentBatchState,
) -> attachments::AttachmentBatchState {
    typsidian_host::commands::attachments_batch_cancel(state)
}

#[tauri::command]
pub fn attachments_batch_view(
    state: attachments::AttachmentBatchState,
) -> attachments::AttachmentBatchView {
    typsidian_host::commands::attachments_batch_view(state)
}

#[tauri::command]
pub fn attachments_plan_markup(
    text_length: usize,
    point: typsidian_core::templates::InsertionPoint,
    before: String,
    after: String,
    paths: Vec<String>,
    block: bool,
) -> Option<typsidian_core::templates::InsertionEdit> {
    typsidian_host::commands::attachments_plan_markup(
        text_length,
        point,
        before,
        after,
        paths,
        block,
    )
}

#[tauri::command]
pub fn attachments_validate_folder(
    raw: String,
) -> Result<String, attachments::AttachmentsFolderProblem> {
    typsidian_host::commands::attachments_validate_folder(raw)
}

#[tauri::command]
pub async fn import_close_source(state: State<'_, ImportSource>) -> Answer<(), Failure<()>> {
    typsidian_host::commands::import_close_source(&state).await
}

#[tauri::command]
pub async fn import_survey(
    vault: State<'_, Vault>,
    source: State<'_, ImportSource>,
    job: import::ImportJob,
) -> Answer<import::ImportSurvey, Failure<import::ImportError>> {
    typsidian_host::commands::import_survey(&vault, &source, job).await
}

#[tauri::command]
pub async fn import_begin(
    vault: State<'_, Vault>,
    source: State<'_, ImportSource>,
    job: import::ImportJob,
) -> Answer<import::ImportState, Failure<import::ImportError>> {
    typsidian_host::commands::import_begin(&vault, &source, job).await
}

#[tauri::command]
pub async fn import_step(
    vault: State<'_, Vault>,
    source: State<'_, ImportSource>,
    state: import::ImportState,
) -> Answer<import::ImportState, Failure<import::ImportError>> {
    typsidian_host::commands::import_step(&vault, &source, state).await
}

#[tauri::command]
pub async fn import_finish(
    vault: State<'_, Vault>,
    state: import::ImportState,
) -> Answer<String, Failure<import::ImportError>> {
    typsidian_host::commands::import_finish(&vault, state).await
}

#[tauri::command]
pub async fn links_backlinks_page(
    state: State<'_, Vault>,
    target: String,
    open_texts: Vec<links::OpenText>,
    offset: u32,
    limit: u32,
) -> Answer<links::BacklinksPage, Failure<()>> {
    typsidian_host::commands::links_backlinks_page(&state, target, open_texts, offset, limit).await
}

#[tauri::command]
pub fn links_mentions_availability(target: String) -> links::MentionsAvailability {
    typsidian_host::commands::links_mentions_availability(target)
}

#[tauri::command]
pub async fn links_mentions_start(
    state: State<'_, Vault>,
    target: String,
) -> Answer<links::MentionsStart, Failure<()>> {
    typsidian_host::commands::links_mentions_start(&state, target).await
}

#[tauri::command]
pub async fn links_mentions_step(
    state: State<'_, Vault>,
    scan: links::MentionScan,
    open_texts: Vec<links::OpenText>,
) -> Answer<links::MentionStep, Failure<()>> {
    typsidian_host::commands::links_mentions_step(&state, scan, open_texts).await
}

#[tauri::command]
pub fn links_mentions_cancel(scan: links::MentionScan) -> links::MentionScan {
    typsidian_host::commands::links_mentions_cancel(scan)
}

#[tauri::command]
pub async fn links_mentions_in(
    state: State<'_, Vault>,
    target: String,
    paths: Vec<String>,
    open_texts: Vec<links::OpenText>,
) -> Answer<Vec<links::SourceGroup>, Failure<()>> {
    typsidian_host::commands::links_mentions_in(&state, target, paths, open_texts).await
}

#[tauri::command]
pub async fn links_link_mention(
    state: State<'_, Vault>,
    request: links::LinkRequest,
    editor_text: Option<String>,
) -> Answer<links::LinkOutcome, Failure<()>> {
    typsidian_host::commands::links_link_mention(&state, request, editor_text).await
}

#[tauri::command]
pub fn workspace_empty() -> workspace::WorkspaceState {
    typsidian_host::commands::workspace_empty()
}

#[tauri::command]
pub async fn workspace_apply(
    state: State<'_, Vault>,
    workspace: workspace::WorkspaceState,
    action: workspace::WorkspaceAction,
    width_px: u32,
) -> Answer<workspace::Transition, Failure<()>> {
    typsidian_host::commands::workspace_apply(&state, workspace, action, width_px).await
}

#[tauri::command]
pub async fn workspace_view(
    state: State<'_, Vault>,
    workspace: workspace::WorkspaceState,
    width_px: u32,
) -> Answer<workspace::WorkspaceView, Failure<()>> {
    typsidian_host::commands::workspace_view(&state, workspace, width_px).await
}

#[tauri::command]
pub fn workspace_snapshot(workspace: workspace::WorkspaceState) -> workspace::SavedWorkspace {
    typsidian_host::commands::workspace_snapshot(workspace)
}

#[tauri::command]
pub async fn workspace_restore(
    state: State<'_, Vault>,
    saved: workspace::SavedWorkspace,
) -> Answer<workspace::WorkspaceState, Failure<()>> {
    typsidian_host::commands::workspace_restore(&state, saved).await
}

#[tauri::command]
pub fn structure_outline(text: String) -> structure::Outline {
    typsidian_host::commands::structure_outline(text)
}

#[tauri::command]
pub fn structure_outline_view(
    outline: structure::Outline,
    request: structure::OutlineRequest,
) -> structure::OutlineView {
    typsidian_host::commands::structure_outline_view(outline, request)
}

#[tauri::command]
pub fn structure_outline_active(
    outline: structure::Outline,
    request: structure::OutlineRequest,
) -> Option<String> {
    typsidian_host::commands::structure_outline_active(outline, request)
}

#[tauri::command]
pub fn structure_outline_carry(
    prev: structure::Outline,
    next: structure::Outline,
    collapsed: Vec<String>,
) -> Vec<String> {
    typsidian_host::commands::structure_outline_carry(prev, next, collapsed)
}

#[tauri::command]
pub fn preview_locate_heading(
    runs: Vec<Vec<preview::TextRun>>,
    outline: structure::Outline,
    key: String,
) -> Option<preview::ResolvedAnchor> {
    typsidian_host::commands::preview_locate_heading(runs, outline, key)
}

#[tauri::command]
pub fn properties_read(text: String) -> properties::NoteProperties {
    typsidian_host::commands::properties_read(text)
}

#[tauri::command]
pub fn properties_edit(
    text: String,
    change: properties::PropertyEdit,
) -> typsidian_core::elements::EditOutcome {
    typsidian_host::commands::properties_edit(text, change)
}

#[tauri::command]
pub fn select_blocks(text: String) -> Vec<properties::SelectBlock> {
    typsidian_host::commands::select_blocks(text)
}

#[tauri::command]
pub fn select_import_edits(text: String) -> Vec<typsidian_core::elements::TextEdit> {
    typsidian_host::commands::select_import_edits(text)
}

#[tauri::command]
pub fn select_library() -> properties::SelectLibrary {
    typsidian_host::commands::select_library()
}

#[tauri::command]
pub fn search_parse_conditions(
    text: String,
    known_keys: Vec<String>,
) -> typsidian_core::search::ParsedConditions {
    typsidian_host::commands::search_parse_conditions(text, known_keys)
}

#[tauri::command]
pub async fn vault_select(
    state: State<'_, Vault>,
    selection: properties::Selection,
) -> Answer<properties::SelectionResult, Failure<()>> {
    typsidian_host::commands::vault_select(&state, selection).await
}

#[tauri::command]
pub async fn completion_linked_index(
    state: State<'_, Vault>,
    engine: State<'_, Preview>,
    note_path: String,
    dependencies: Vec<String>,
) -> Answer<completion::LinkedIndex, Failure<()>> {
    typsidian_host::commands::completion_linked_index(&state, &engine, note_path, dependencies)
        .await
}

#[tauri::command]
pub async fn completions(
    state: State<'_, Vault>,
    request: completion::CompletionRequest,
) -> Answer<completion::CompletionList, Failure<()>> {
    typsidian_host::commands::completions(&state, request).await
}

#[tauri::command]
pub async fn definition_at(
    state: State<'_, Vault>,
    request: completion::DefinitionRequest,
) -> Answer<completion::DefinitionOutcome, Failure<()>> {
    typsidian_host::commands::definition_at(&state, request).await
}

#[tauri::command]
pub async fn reference_marks(
    state: State<'_, Vault>,
    request: completion::MarksRequest,
) -> Answer<Vec<completion::ReferenceMark>, Failure<()>> {
    typsidian_host::commands::reference_marks(&state, request).await
}

#[tauri::command]
pub async fn vault_graph(
    state: State<'_, Vault>,
    request: graph_view::GraphRequest,
) -> Answer<graph_view::VaultGraph, Failure<()>> {
    typsidian_host::commands::vault_graph(&state, request).await
}

#[tauri::command]
pub async fn vault_property_keys(
    state: State<'_, Vault>,
) -> Answer<Vec<properties::PropertyKeyInfo>, Failure<()>> {
    typsidian_host::commands::vault_property_keys(&state).await
}

#[tauri::command]
pub async fn vault_properties_current(state: State<'_, Vault>) -> Answer<bool, Failure<()>> {
    typsidian_host::commands::vault_properties_current(&state).await
}

#[tauri::command]
pub fn vault_cancel_reindex_properties(cancel: State<'_, PropertiesReindex>) {
    typsidian_host::commands::vault_cancel_reindex_properties(&cancel)
}

#[tauri::command]
pub async fn vault_ensure_select_library(
    state: State<'_, Vault>,
) -> Answer<properties::LibraryStatus, Failure<WriteError>> {
    typsidian_host::commands::vault_ensure_select_library(&state).await
}

#[tauri::command]
pub async fn vault_select_digest(
    state: State<'_, Vault>,
    text: String,
) -> Answer<Option<String>, Failure<()>> {
    typsidian_host::commands::vault_select_digest(&state, text).await
}

#[tauri::command]
pub fn slides_start(
    slides: Vec<slides::PageSlideInfo>,
    caret: Option<u32>,
    now_ms: f64,
) -> Result<slides::ShowState, slides::SlidesError> {
    typsidian_host::commands::slides_start(slides, caret, now_ms)
}

#[tauri::command]
pub fn slides_navigate(
    state: slides::ShowState,
    slides: Vec<slides::PageSlideInfo>,
    action: slides::ShowAction,
) -> slides::ShowState {
    typsidian_host::commands::slides_navigate(state, slides, action)
}

#[tauri::command]
pub fn slides_carry(
    state: slides::ShowState,
    slides: Vec<slides::PageSlideInfo>,
) -> Result<slides::ShowState, slides::SlidesError> {
    typsidian_host::commands::slides_carry(state, slides)
}

#[tauri::command]
pub fn slides_caret(slides: Vec<slides::PageSlideInfo>, page: u32) -> Option<slides::SlidePlace> {
    typsidian_host::commands::slides_caret(slides, page)
}

#[tauri::command]
pub fn slides_label(slides: Vec<slides::PageSlideInfo>, page: u32) -> slides::SlideLabel {
    typsidian_host::commands::slides_label(slides, page)
}

#[tauri::command]
pub fn slides_keymap() -> slides::ShowKeymap {
    typsidian_host::commands::slides_keymap()
}

#[tauri::command]
pub fn capture_settings(overrides: VaultOverrides) -> capture::CaptureSettings {
    typsidian_host::commands::capture_settings(overrides)
}

#[tauri::command]
pub fn capture_entry(
    settings: capture::CaptureSettings,
    capture: capture::Capture,
    image_path: Option<String>,
) -> String {
    typsidian_host::commands::capture_entry(settings, capture, image_path)
}

#[tauri::command]
pub fn capture_target_path(
    settings: capture::CaptureSettings,
    capture: capture::Capture,
) -> Option<String> {
    typsidian_host::commands::capture_target_path(settings, capture)
}

#[tauri::command]
pub fn daily_path(settings: capture::CaptureSettings, date: capture::DailyDate) -> Option<String> {
    typsidian_host::commands::daily_path(settings, date)
}

#[tauri::command]
pub fn daily_date_of(
    settings: capture::CaptureSettings,
    path: String,
) -> Option<capture::DailyDate> {
    typsidian_host::commands::daily_date_of(settings, path)
}

#[tauri::command]
pub async fn capture_write(
    state: State<'_, Vault>,
    capture: capture::Capture,
    open_buffer: Option<String>,
    image: Option<Vec<u8>>,
) -> Answer<capture::CaptureOutcome, Failure<()>> {
    typsidian_host::commands::capture_write(&state, capture, open_buffer, image).await
}

#[tauri::command]
pub async fn open_daily(
    state: State<'_, Vault>,
    date: capture::DailyDate,
) -> Answer<capture::OpenDaily, Failure<capture::DailyError>> {
    typsidian_host::commands::open_daily(&state, date).await
}

#[tauri::command]
pub async fn daily_neighbours(
    state: State<'_, Vault>,
    date: capture::DailyDate,
) -> Answer<capture::DailyNeighbours, Failure<()>> {
    typsidian_host::commands::daily_neighbours(&state, date).await
}

#[tauri::command]
pub async fn check_template_name(
    state: State<'_, Vault>,
    name: String,
) -> Answer<NameCheck, Failure<()>> {
    typsidian_host::commands::check_template_name(&state, name).await
}

#[tauri::command]
pub async fn save_as_template(
    state: State<'_, Vault>,
    name: String,
    content: String,
    confirm_overwrite: bool,
) -> Answer<templates::Template, Failure<SaveTemplateError>> {
    typsidian_host::commands::save_as_template(&state, name, content, confirm_overwrite).await
}

#[tauri::command]
pub fn plan_insertion(
    text_length: usize,
    point: templates::InsertionPoint,
    content: String,
) -> templates::InsertionEdit {
    typsidian_host::commands::plan_insertion(text_length, point, content)
}

#[tauri::command]
pub fn element_schema(id: String) -> Option<typsidian_core::elements::ElementSpec> {
    typsidian_host::commands::element_schema(id)
}

#[tauri::command]
pub fn element_schema_for(template: String) -> Option<typsidian_core::elements::ElementSpec> {
    typsidian_host::commands::element_schema_for(template)
}

#[tauri::command]
pub fn element_at(text: String, offset: usize) -> typsidian_core::elements::ElementAtResult {
    typsidian_host::commands::element_at(text, offset)
}

#[tauri::command]
pub fn syntax_session_open(text: String, sessions: State<'_, HighlightSessions>) -> u32 {
    typsidian_host::commands::syntax_session_open(text, &sessions)
}

#[tauri::command]
pub fn syntax_session_update(
    id: u32,
    edits: Vec<typsidian_core::highlight::HighlightEdit>,
    length: usize,
    windows: Vec<typsidian_core::highlight::HighlightWindow>,
    sessions: State<'_, HighlightSessions>,
) -> Option<Vec<u32>> {
    typsidian_host::commands::syntax_session_update(id, edits, length, windows, &sessions)
}

#[tauri::command]
pub fn syntax_session_close(id: u32, sessions: State<'_, HighlightSessions>) {
    typsidian_host::commands::syntax_session_close(id, &sessions)
}

#[tauri::command]
pub fn render_element(
    request: typsidian_core::elements::RenderRequest,
) -> typsidian_core::elements::RenderOutcome {
    typsidian_host::commands::render_element(request)
}

#[tauri::command]
pub fn plan_element_edit(
    request: typsidian_core::elements::EditRequest,
) -> typsidian_core::elements::EditOutcome {
    typsidian_host::commands::plan_element_edit(request)
}

#[tauri::command]
pub fn template_event(event: VaultEvent) -> Option<templates::TemplateEvent> {
    typsidian_host::commands::template_event(event)
}

#[tauri::command]
pub fn sync_progress(
    progress: State<'_, SyncProgressSlot>,
) -> Option<typsidian_core::sync::SyncProgress> {
    typsidian_host::commands::sync_progress(&progress)
}

#[tauri::command]
pub fn sync_capabilities(home: State<'_, SyncHome>) -> ShellCapabilities {
    typsidian_host::commands::sync_capabilities(&home)
}

#[tauri::command]
pub async fn sync_overview(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> SyncAnswer<SyncOverview> {
    typsidian_host::commands::sync_overview(&home, &vault, &sync_state).await
}

#[tauri::command]
pub async fn sync_poll_events(sync_state: State<'_, SyncState>) -> SyncAnswer<Vec<SyncEvent>> {
    typsidian_host::commands::sync_poll_events(&sync_state).await
}

#[tauri::command]
pub async fn sync_note_change(sync_state: State<'_, SyncState>) -> SyncAnswer<()> {
    typsidian_host::commands::sync_note_change(&sync_state).await
}

#[tauri::command]
pub fn sync_cancel(cancel: State<'_, SyncCancel>) {
    typsidian_host::commands::sync_cancel(&cancel)
}

#[tauri::command]
pub async fn sync_ignore_reports(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> SyncAnswer<Vec<IgnoreReport>> {
    typsidian_host::commands::sync_ignore_reports(&vault, &sync_state).await
}

#[tauri::command]
pub async fn sync_apply_ignore(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    file: IgnoreFile,
) -> SyncAnswer<()> {
    typsidian_host::commands::sync_apply_ignore(&home, &vault, &sync_state, file).await
}

#[tauri::command]
pub async fn sync_decline_stignore(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> SyncAnswer<()> {
    typsidian_host::commands::sync_decline_stignore(&home, &vault, &sync_state).await
}

#[tauri::command]
pub async fn sync_ignore_open(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    file: IgnoreFile,
) -> SyncAnswer<IgnoreDocument> {
    typsidian_host::commands::sync_ignore_open(&vault, &sync_state, file).await
}

#[tauri::command]
pub fn sync_ignore_check(file: IgnoreFile, text: String) -> IgnoreCheck {
    typsidian_host::commands::sync_ignore_check(file, text)
}

#[tauri::command]
pub fn sync_ignore_restore(file: IgnoreFile, text: String) -> String {
    typsidian_host::commands::sync_ignore_restore(file, text)
}

#[tauri::command]
pub async fn sync_ignore_save(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    request: IgnoreSaveRequest,
) -> SyncAnswer<IgnoreSaveOutcome> {
    typsidian_host::commands::sync_ignore_save(&home, &vault, &sync_state, request).await
}

#[tauri::command]
pub async fn sync_enable_history(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    message: String,
) -> SyncAnswer<Snapshot> {
    typsidian_host::commands::sync_enable_history(&home, &vault, &sync_state, message).await
}

#[tauri::command]
pub async fn sync_changes(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> SyncAnswer<Vec<Change>> {
    typsidian_host::commands::sync_changes(&vault, &sync_state).await
}

#[tauri::command]
pub async fn sync_snapshot(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    paths: Option<Vec<String>>,
    message: String,
    auto_prefix: String,
    flushed: bool,
    confirm_not_keeper: bool,
) -> SyncAnswer<Snapshot> {
    typsidian_host::commands::sync_snapshot(
        &home,
        &vault,
        &sync_state,
        paths,
        message,
        auto_prefix,
        flushed,
        confirm_not_keeper,
    )
    .await
}

#[tauri::command]
pub async fn sync_history(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    path: Option<String>,
    cursor: Option<String>,
    limit: u32,
) -> SyncAnswer<HistoryPage> {
    typsidian_host::commands::sync_history(&vault, &sync_state, path, cursor, limit).await
}

#[tauri::command]
pub async fn sync_diff(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    path: String,
    from: Revision,
    to: Revision,
) -> SyncAnswer<FileDiff> {
    typsidian_host::commands::sync_diff(&vault, &sync_state, path, from, to).await
}

#[tauri::command]
pub async fn sync_snapshot_changes(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    snapshot: String,
) -> SyncAnswer<Vec<Change>> {
    typsidian_host::commands::sync_snapshot_changes(&vault, &sync_state, snapshot).await
}

#[tauri::command]
pub async fn sync_file_at(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    path: String,
    revision: Revision,
) -> SyncAnswer<Content> {
    typsidian_host::commands::sync_file_at(&vault, &sync_state, path, revision).await
}

#[tauri::command]
pub async fn sync_restore(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    path: String,
    snapshot: String,
) -> SyncAnswer<Content> {
    typsidian_host::commands::sync_restore(&vault, &sync_state, path, snapshot).await
}

#[tauri::command]
pub async fn sync_connect_remote(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    url: String,
    credentials: Credentials,
) -> SyncAnswer<RemoteStatus> {
    typsidian_host::commands::sync_connect_remote(&home, &vault, &sync_state, url, credentials)
        .await
}

#[tauri::command]
pub async fn sync_disconnect_remote(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> SyncAnswer<()> {
    typsidian_host::commands::sync_disconnect_remote(&home, &vault, &sync_state).await
}

#[tauri::command]
pub async fn sync_push(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    confirm_not_keeper: bool,
) -> SyncAnswer<RemoteStatus> {
    typsidian_host::commands::sync_push(&home, &vault, &sync_state, confirm_not_keeper).await
}

#[tauri::command]
pub async fn sync_pull(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    confirm_not_keeper: bool,
) -> SyncAnswer<PullOutcome> {
    typsidian_host::commands::sync_pull(&home, &vault, &sync_state, confirm_not_keeper).await
}

#[tauri::command]
pub async fn sync_restore_history(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    confirm_not_keeper: bool,
) -> SyncAnswer<PullOutcome> {
    typsidian_host::commands::sync_restore_history(&home, &vault, &sync_state, confirm_not_keeper)
        .await
}

#[tauri::command]
pub async fn sync_abort_pull(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> SyncAnswer<()> {
    typsidian_host::commands::sync_abort_pull(&vault, &sync_state).await
}

#[tauri::command]
pub async fn sync_conflicts(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> SyncAnswer<Vec<Conflict>> {
    typsidian_host::commands::sync_conflicts(&vault, &sync_state).await
}

#[tauri::command]
pub async fn sync_resolve(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    id: String,
    resolution: Resolution,
) -> SyncAnswer<()> {
    typsidian_host::commands::sync_resolve(&home, &vault, &sync_state, id, resolution).await
}

#[tauri::command]
pub async fn sync_set_keeper(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> SyncAnswer<Keeper> {
    typsidian_host::commands::sync_set_keeper(&home, &vault, &sync_state).await
}

#[tauri::command]
pub async fn sync_clear_keeper(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> SyncAnswer<()> {
    typsidian_host::commands::sync_clear_keeper(&vault, &sync_state).await
}

#[tauri::command]
pub async fn sync_update_settings(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    patch: VaultSyncSettingsPatch,
) -> SyncAnswer<VaultSyncSettings> {
    typsidian_host::commands::sync_update_settings(&vault, &sync_state, patch).await
}

#[tauri::command]
pub async fn sync_refresh_mirror(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> SyncAnswer<()> {
    typsidian_host::commands::sync_refresh_mirror(&home, &vault, &sync_state).await
}

#[tauri::command]
pub async fn sync_tick(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    auto_prefix: String,
) -> SyncAnswer<TickOutcome> {
    typsidian_host::commands::sync_tick(&home, &vault, &sync_state, auto_prefix).await
}

#[tauri::command]
pub async fn sync_device_values(
    home: State<'_, SyncHome>,
    sync_state: State<'_, SyncState>,
) -> SyncAnswer<DeviceVaultSync> {
    typsidian_host::commands::sync_device_values(&home, &sync_state).await
}

#[tauri::command]
pub async fn sync_set_author(
    home: State<'_, SyncHome>,
    sync_state: State<'_, SyncState>,
    author: Option<Author>,
) -> SyncAnswer<()> {
    typsidian_host::commands::sync_set_author(&home, &sync_state, author).await
}
