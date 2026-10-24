// @generated scripts/dev/gen-host-dispatch.mjs из packages/host/src/commands.rs.
// Не править руками: `nix develop -c node scripts/dev/gen-host-dispatch.mjs`.

//! Вызов команды по имени — для сервера (фича 037, research.md §1).
//!
//! Аргументы — объект JSON с полями в camelCase, как их шлёт клиент Tauri; ответ —
//! значение команды или её отказ, сериализованные так же, как их отдал бы Tauri.

#![allow(clippy::too_many_lines)]

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::*;

/// Ответ команды: `Ok` — значение, `Err` — отказ (ядра, «vault не открыт» или
/// разбора аргументов).
pub type Reply = Result<Value, Value>;

/// Имена всех команд таблицы.
pub const COMMANDS: &[&str] = &[
    "list",
    "read",
    "write",
    "create",
    "rename",
    "move_targets",
    "move_impact",
    "move_entry",
    "delete",
    "backlinks",
    "broken_links",
    "access_state",
    "search",
    "replace_preview",
    "replace_in_note",
    "replace_undo",
    "links_health",
    "links_fix",
    "snapshot_take",
    "snapshot_history",
    "snapshot_diff",
    "snapshot_text",
    "snapshot_restore",
    "snapshot_deleted_notes",
    "snapshot_recover_note",
    "snapshot_usage",
    "snapshots_clear",
    "tasks_view",
    "task_toggle",
    "task_toggle_at",
    "bookmarks_view",
    "bookmark_add",
    "bookmark_remove",
    "bookmark_rename",
    "bookmark_move",
    "bookmark_group_create",
    "bookmark_group_rename",
    "bookmark_group_toggle",
    "bookmark_group_delete",
    "embed_insert",
    "spell_configure",
    "spell_add_word",
    "spell_remove_word",
    "spell_suggest",
    "syntax_session_spelling",
    "root_path",
    "board_write_profile",
    "file_kind",
    "begin_edit",
    "discard_edit",
    "pending_conflict",
    "unresolved_edit_paths",
    "conflict_diff",
    "resolve_conflict_keep_local",
    "resolve_conflict_accept_external",
    "create_board",
    "read_board",
    "write_board",
    "write_snapshot",
    "embeds_of",
    "embeds_in_text",
    "embeds_changed",
    "plan_embed",
    "vault_settings",
    "set_vault_settings",
    "mark_section",
    "unmark_section",
    "embed_markup",
    "keybindings",
    "set_keybinding",
    "reset_keybinding",
    "template_library",
    "read_template",
    "template_preview",
    "preview_compile",
    "preview_dependency_mark",
    "preview_compile_with_templates",
    "preview_install_template",
    "preview_installed_templates",
    "export_pdf",
    "export_slides",
    "export_html",
    "export_site_index",
    "export_batch_with_format",
    "export_candidates",
    "export_batch_start",
    "export_batch_next",
    "export_batch_record",
    "export_batch_skip",
    "export_batch_cancel",
    "export_batch_view",
    "sources_refresh",
    "sources_summary",
    "sources_search",
    "sources_for_completion",
    "sources_scan_start",
    "sources_scan_step",
    "sources_scan_cancel",
    "sources_scan_view",
    "sources_note_changed",
    "sources_note_removed",
    "sources_note_renamed",
    "sources_citing",
    "sources_missing_keys",
    "sources_check_note",
    "sources_plan_citation",
    "sources_bibliography_offer",
    "sources_create_file",
    "attachments_batch_start",
    "attachments_batch_next",
    "attachments_batch_confirm",
    "attachments_batch_source_failed",
    "attachments_batch_cancel",
    "attachments_batch_view",
    "attachments_plan_markup",
    "attachments_validate_folder",
    "import_close_source",
    "import_survey",
    "import_begin",
    "import_step",
    "import_finish",
    "links_backlinks_page",
    "links_mentions_availability",
    "links_mentions_start",
    "links_mentions_step",
    "links_mentions_cancel",
    "links_mentions_in",
    "links_link_mention",
    "workspace_empty",
    "workspace_apply",
    "workspace_view",
    "workspace_snapshot",
    "workspace_restore",
    "structure_outline",
    "structure_outline_view",
    "structure_outline_active",
    "structure_outline_carry",
    "preview_locate_heading",
    "properties_read",
    "properties_edit",
    "select_blocks",
    "select_import_edits",
    "select_library",
    "search_parse_conditions",
    "vault_select",
    "completion_linked_index",
    "completions",
    "definition_at",
    "reference_marks",
    "vault_graph",
    "vault_property_keys",
    "vault_properties_current",
    "vault_cancel_reindex_properties",
    "vault_ensure_select_library",
    "vault_select_digest",
    "slides_start",
    "slides_navigate",
    "slides_carry",
    "slides_caret",
    "slides_label",
    "slides_keymap",
    "capture_settings",
    "capture_entry",
    "capture_target_path",
    "daily_path",
    "daily_date_of",
    "capture_write",
    "open_daily",
    "daily_neighbours",
    "check_template_name",
    "save_as_template",
    "plan_insertion",
    "element_schema",
    "element_schema_for",
    "element_at",
    "syntax_session_open",
    "syntax_session_update",
    "syntax_session_close",
    "render_element",
    "plan_element_edit",
    "template_event",
];

fn args<A: DeserializeOwned>(value: Value) -> Result<A, Value> {
    serde_json::from_value(value).map_err(
        |error| serde_json::json!({ "kind": "InvalidArguments", "message": error.to_string() }),
    )
}

fn value<T: Serialize>(value: T) -> Reply {
    serde_json::to_value(value).map_err(
        |error| serde_json::json!({ "kind": "Unserializable", "message": error.to_string() }),
    )
}

fn answer<T: Serialize, E: Serialize>(result: Result<T, E>) -> Reply {
    match result {
        Ok(ok) => value(ok),
        Err(error) => Err(value(error)?),
    }
}

/// Вызывает команду `name`; `None` — такой команды нет.
pub async fn dispatch(host: &Host, name: &str, arguments: Value) -> Option<Reply> {
    Some(match name {
        "list" => {
            let _ = arguments;
            answer(commands::list(&host.vault).await)
        }
        "read" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::read(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "write" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
                content: String,
                expected: Option<FileStamp>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::write(&host.vault, a.path, a.content, a.expected).await),
                Err(error) => Err(error),
            }
        }
        "create" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::create(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "rename" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                from: String,
                to: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::rename(&host.vault, a.from, a.to).await),
                Err(error) => Err(error),
            }
        }
        "move_targets" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                from: String,
                query: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::move_targets(&host.vault, a.from, a.query).await),
                Err(error) => Err(error),
            }
        }
        "move_impact" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                request: MoveRequest,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::move_impact(&host.vault, a.request).await),
                Err(error) => Err(error),
            }
        }
        "move_entry" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                request: MoveRequest,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::move_entry(&host.vault, a.request).await),
                Err(error) => Err(error),
            }
        }
        "delete" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::delete(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "backlinks" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::backlinks(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "broken_links" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::broken_links(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "access_state" => {
            let _ = arguments;
            answer(commands::access_state(&host.vault).await)
        }
        "search" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                query: typsidian_core::search::SearchQuery,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::search(&host.vault, a.query).await),
                Err(error) => Err(error),
            }
        }
        "replace_preview" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                query: typsidian_core::search::replace::ReplaceQuery,
                open_texts: Vec<links::OpenText>,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::replace_preview(&host.vault, a.query, a.open_texts).await)
                }
                Err(error) => Err(error),
            }
        }
        "replace_in_note" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                request: typsidian_core::search::replace::ReplaceNoteRequest,
                editor_text: Option<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::replace_in_note(&host.vault, a.request, a.editor_text).await)
                }
                Err(error) => Err(error),
            }
        }
        "replace_undo" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                record: typsidian_core::search::replace::UndoRecord,
                editor_text: Option<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::replace_undo(&host.vault, a.record, a.editor_text).await),
                Err(error) => Err(error),
            }
        }
        "links_health" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                open_texts: Vec<links::OpenText>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::links_health(&host.vault, a.open_texts).await),
                Err(error) => Err(error),
            }
        }
        "links_fix" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                request: links::health::LinkFixRequest,
                editor_text: Option<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::links_fix(&host.vault, a.request, a.editor_text).await),
                Err(error) => Err(error),
            }
        }
        "snapshot_take" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
                text: String,
                reason: snapshots::SnapshotReason,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::snapshot_take(&host.vault, a.path, a.text, a.reason).await)
                }
                Err(error) => Err(error),
            }
        }
        "snapshot_history" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::snapshot_history(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "snapshot_diff" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                id: String,
                against: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::snapshot_diff(&host.vault, a.id, a.against).await),
                Err(error) => Err(error),
            }
        }
        "snapshot_text" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                id: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::snapshot_text(&host.vault, a.id).await),
                Err(error) => Err(error),
            }
        }
        "snapshot_restore" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
                id: String,
                editor_text: Option<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::snapshot_restore(&host.vault, a.path, a.id, a.editor_text).await,
                ),
                Err(error) => Err(error),
            }
        }
        "snapshot_deleted_notes" => {
            let _ = arguments;
            answer(commands::snapshot_deleted_notes(&host.vault).await)
        }
        "snapshot_recover_note" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
                as_path: Option<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::snapshot_recover_note(&host.vault, a.path, a.as_path).await)
                }
                Err(error) => Err(error),
            }
        }
        "snapshot_usage" => {
            let _ = arguments;
            answer(commands::snapshot_usage(&host.vault).await)
        }
        "snapshots_clear" => {
            let _ = arguments;
            answer(commands::snapshots_clear(&host.vault).await)
        }
        "tasks_view" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                filter: tasks::TaskFilter,
                today: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::tasks_view(a.filter, a.today, &host.vault).await),
                Err(error) => Err(error),
            }
        }
        "task_toggle" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                request: tasks::TaskToggleRequest,
                editor_text: Option<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::task_toggle(a.request, a.editor_text, &host.vault).await),
                Err(error) => Err(error),
            }
        }
        "task_toggle_at" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
                offset: u32,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::task_toggle_at(a.text, a.offset)),
                Err(error) => Err(error),
            }
        }
        "bookmarks_view" => {
            let _ = arguments;
            answer(commands::bookmarks_view(&host.vault).await)
        }
        "bookmark_add" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                target: bookmarks::BookmarkTarget,
                name: Option<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::bookmark_add(a.target, a.name, &host.vault).await),
                Err(error) => Err(error),
            }
        }
        "bookmark_remove" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                id: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::bookmark_remove(a.id, &host.vault).await),
                Err(error) => Err(error),
            }
        }
        "bookmark_rename" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                id: String,
                name: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::bookmark_rename(a.id, a.name, &host.vault).await),
                Err(error) => Err(error),
            }
        }
        "bookmark_move" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                id: String,
                place: bookmarks::BookmarkPlace,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::bookmark_move(a.id, a.place, &host.vault).await),
                Err(error) => Err(error),
            }
        }
        "bookmark_group_create" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                name: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::bookmark_group_create(a.name, &host.vault).await),
                Err(error) => Err(error),
            }
        }
        "bookmark_group_rename" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                id: String,
                name: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::bookmark_group_rename(a.id, a.name, &host.vault).await),
                Err(error) => Err(error),
            }
        }
        "bookmark_group_toggle" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                id: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::bookmark_group_toggle(a.id, &host.vault).await),
                Err(error) => Err(error),
            }
        }
        "bookmark_group_delete" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                id: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::bookmark_group_delete(a.id, &host.vault).await),
                Err(error) => Err(error),
            }
        }
        "embed_insert" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
                offset: u32,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::embed_insert(a.text, a.offset, &host.vault).await),
                Err(error) => Err(error),
            }
        }
        "spell_configure" => {
            let _ = arguments;
            answer(commands::spell_configure(&host.vault, &host.speller).await)
        }
        "spell_add_word" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                word: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::spell_add_word(a.word, &host.vault, &host.speller).await),
                Err(error) => Err(error),
            }
        }
        "spell_remove_word" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                word: String,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::spell_remove_word(a.word, &host.vault, &host.speller).await)
                }
                Err(error) => Err(error),
            }
        }
        "spell_suggest" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                word: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::spell_suggest(a.word, &host.speller)),
                Err(error) => Err(error),
            }
        }
        "syntax_session_spelling" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                id: u32,
                windows: Vec<typsidian_core::highlight::HighlightWindow>,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::syntax_session_spelling(
                    a.id,
                    a.windows,
                    &host.highlight_sessions,
                    &host.speller,
                )),
                Err(error) => Err(error),
            }
        }
        "root_path" => {
            let _ = arguments;
            answer(commands::root_path(&host.vault).await)
        }
        "board_write_profile" => {
            let _ = arguments;
            value(commands::board_write_profile())
        }
        "file_kind" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::file_kind(a.path)),
                Err(error) => Err(error),
            }
        }
        "begin_edit" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
                buffer: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::begin_edit(&host.vault, a.path, a.buffer).await),
                Err(error) => Err(error),
            }
        }
        "discard_edit" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::discard_edit(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "pending_conflict" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::pending_conflict(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "unresolved_edit_paths" => {
            let _ = arguments;
            answer(commands::unresolved_edit_paths(&host.vault).await)
        }
        "conflict_diff" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::conflict_diff(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "resolve_conflict_keep_local" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::resolve_conflict_keep_local(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "resolve_conflict_accept_external" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::resolve_conflict_accept_external(&host.vault, a.path).await)
                }
                Err(error) => Err(error),
            }
        }
        "create_board" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::create_board(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "read_board" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::read_board(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "write_board" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
                content: String,
                expected: Option<FileStamp>,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::write_board(&host.vault, a.path, a.content, a.expected).await)
                }
                Err(error) => Err(error),
            }
        }
        "write_snapshot" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                snapshot_path: String,
                svg: String,
                board_path: String,
                board_content: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::write_snapshot(
                        &host.vault,
                        a.snapshot_path,
                        a.svg,
                        a.board_path,
                        a.board_content,
                    )
                    .await,
                ),
                Err(error) => Err(error),
            }
        }
        "embeds_of" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                note_path: String,
                note_text: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::embeds_of(&host.vault, a.note_path, a.note_text).await),
                Err(error) => Err(error),
            }
        }
        "embeds_in_text" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                note_path: String,
                note_text: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::embeds_in_text(a.note_path, a.note_text)),
                Err(error) => Err(error),
            }
        }
        "embeds_changed" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                note_path: String,
                note_text: String,
                changed: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::embeds_changed(
                    a.note_path,
                    a.note_text,
                    a.changed,
                )),
                Err(error) => Err(error),
            }
        }
        "plan_embed" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                note_path: String,
                note_text: String,
                board_path: String,
                overrides: VaultOverrides,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::plan_embed(
                    a.note_path,
                    a.note_text,
                    a.board_path,
                    a.overrides,
                )),
                Err(error) => Err(error),
            }
        }
        "vault_settings" => {
            let _ = arguments;
            answer(commands::vault_settings(&host.vault).await)
        }
        "set_vault_settings" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                overrides: VaultOverrides,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::set_vault_settings(&host.vault, a.overrides).await),
                Err(error) => Err(error),
            }
        }
        "mark_section" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                folder: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::mark_section(&host.vault, a.folder).await),
                Err(error) => Err(error),
            }
        }
        "unmark_section" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                folder: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::unmark_section(&host.vault, a.folder).await),
                Err(error) => Err(error),
            }
        }
        "embed_markup" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                plan: EmbedPlan,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::embed_markup(a.plan)),
                Err(error) => Err(error),
            }
        }
        "keybindings" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                overrides: VaultOverrides,
                mac: bool,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::keybindings(a.overrides, a.mac)),
                Err(error) => Err(error),
            }
        }
        "set_keybinding" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                command: CommandId,
                shortcut: Option<Shortcut>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::set_keybinding(&host.vault, a.command, a.shortcut).await),
                Err(error) => Err(error),
            }
        }
        "reset_keybinding" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                command: CommandId,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::reset_keybinding(&host.vault, a.command).await),
                Err(error) => Err(error),
            }
        }
        "template_library" => {
            let _ = arguments;
            answer(commands::template_library(&host.vault).await)
        }
        "read_template" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::read_template(&host.vault, a.path).await),
                Err(error) => Err(error),
            }
        }
        "template_preview" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                template: templates::Template,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::template_preview(&host.vault, &host.template_previews, a.template)
                        .await,
                ),
                Err(error) => Err(error),
            }
        }
        "preview_compile" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                note_path: String,
                text: String,
                revision: u64,
                anchor: Option<preview::ScrollAnchor>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::preview_compile(
                        &host.vault,
                        &host.preview,
                        a.note_path,
                        a.text,
                        a.revision,
                        a.anchor,
                    )
                    .await,
                ),
                Err(error) => Err(error),
            }
        }
        "preview_dependency_mark" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                paths: Vec<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::preview_dependency_mark(&host.vault, a.paths).await),
                Err(error) => Err(error),
            }
        }
        "preview_compile_with_templates" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                note_path: String,
                text: String,
                revision: u64,
                overrides: VaultOverrides,
                anchor: Option<preview::ScrollAnchor>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::preview_compile_with_templates(
                        &host.vault,
                        &host.preview,
                        a.note_path,
                        a.text,
                        a.revision,
                        a.overrides,
                        a.anchor,
                    )
                    .await,
                ),
                Err(error) => Err(error),
            }
        }
        "preview_install_template" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                spec: String,
                overrides: VaultOverrides,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::preview_install_template(
                        &host.vault,
                        &host.preview,
                        a.spec,
                        a.overrides,
                    )
                    .await,
                ),
                Err(error) => Err(error),
            }
        }
        "preview_installed_templates" => {
            let _ = arguments;
            answer(commands::preview_installed_templates(&host.vault).await)
        }
        "export_pdf" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                note_path: String,
                text: Option<String>,
                overrides: VaultOverrides,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::export_pdf(
                        &host.vault,
                        &host.preview,
                        a.note_path,
                        a.text,
                        a.overrides,
                    )
                    .await,
                ),
                Err(error) => Err(error),
            }
        }
        "export_slides" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                note_path: String,
                text: Option<String>,
                overrides: VaultOverrides,
                request: typsidian_core::slides::export::SlidesExportRequest,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::export_slides(
                        &host.vault,
                        &host.preview,
                        a.note_path,
                        a.text,
                        a.overrides,
                        a.request,
                    )
                    .await,
                ),
                Err(error) => Err(error),
            }
        }
        "export_html" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                note_path: String,
                text: Option<String>,
                overrides: VaultOverrides,
                site: Vec<String>,
                labels: export::SiteLabels,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::export_html(
                        &host.vault,
                        &host.preview,
                        a.note_path,
                        a.text,
                        a.overrides,
                        a.site,
                        a.labels,
                    )
                    .await,
                ),
                Err(error) => Err(error),
            }
        }
        "export_site_index" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                overrides: VaultOverrides,
                site: Vec<String>,
                labels: export::SiteLabels,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::export_site_index(&host.vault, a.overrides, a.site, a.labels).await,
                ),
                Err(error) => Err(error),
            }
        }
        "export_batch_with_format" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: export::BatchState,
                format: export::ExportFormat,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::export_batch_with_format(a.state, a.format)),
                Err(error) => Err(error),
            }
        }
        "export_candidates" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                overrides: VaultOverrides,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::export_candidates(&host.vault, a.overrides).await),
                Err(error) => Err(error),
            }
        }
        "export_batch_start" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                paths: Vec<String>,
                stop_on_error: bool,
                overrides: VaultOverrides,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::export_batch_start(
                    a.paths,
                    a.stop_on_error,
                    a.overrides,
                )),
                Err(error) => Err(error),
            }
        }
        "export_batch_next" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: export::BatchState,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::export_batch_next(a.state)),
                Err(error) => Err(error),
            }
        }
        "export_batch_record" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: export::BatchState,
                report: export::ExportReport,
                elapsed_ms: u32,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::export_batch_record(
                    a.state,
                    a.report,
                    a.elapsed_ms,
                )),
                Err(error) => Err(error),
            }
        }
        "export_batch_skip" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: export::BatchState,
                note_path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::export_batch_skip(a.state, a.note_path)),
                Err(error) => Err(error),
            }
        }
        "export_batch_cancel" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: export::BatchState,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::export_batch_cancel(a.state)),
                Err(error) => Err(error),
            }
        }
        "export_batch_view" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: export::BatchState,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::export_batch_view(a.state)),
                Err(error) => Err(error),
            }
        }
        "sources_refresh" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                registered: Vec<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::sources_refresh(&host.vault, &host.sources, a.registered).await,
                ),
                Err(error) => Err(error),
            }
        }
        "sources_summary" => {
            let _ = arguments;
            answer(commands::sources_summary(&host.sources).await)
        }
        "sources_search" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                query: String,
                limit: u32,
                uncited_only: bool,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::sources_search(&host.sources, a.query, a.limit, a.uncited_only).await,
                ),
                Err(error) => Err(error),
            }
        }
        "sources_for_completion" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                prefix: String,
                limit: u32,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::sources_for_completion(&host.sources, a.prefix, a.limit).await)
                }
                Err(error) => Err(error),
            }
        }
        "sources_scan_start" => {
            let _ = arguments;
            answer(commands::sources_scan_start(&host.vault, &host.sources).await)
        }
        "sources_scan_step" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                max_notes: u32,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::sources_scan_step(&host.vault, &host.sources, a.max_notes).await,
                ),
                Err(error) => Err(error),
            }
        }
        "sources_scan_cancel" => {
            let _ = arguments;
            answer(commands::sources_scan_cancel(&host.sources).await)
        }
        "sources_scan_view" => {
            let _ = arguments;
            answer(commands::sources_scan_view(&host.sources).await)
        }
        "sources_note_changed" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::sources_note_changed(&host.vault, &host.sources, a.path).await)
                }
                Err(error) => Err(error),
            }
        }
        "sources_note_removed" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::sources_note_removed(&host.sources, a.path).await),
                Err(error) => Err(error),
            }
        }
        "sources_note_renamed" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                from: String,
                to: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::sources_note_renamed(&host.sources, a.from, a.to).await),
                Err(error) => Err(error),
            }
        }
        "sources_citing" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                key: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::sources_citing(&host.sources, a.key).await),
                Err(error) => Err(error),
            }
        }
        "sources_missing_keys" => {
            let _ = arguments;
            answer(commands::sources_missing_keys(&host.sources).await)
        }
        "sources_check_note" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
                known_labels: Vec<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::sources_check_note(&host.sources, a.text, a.known_labels).await,
                ),
                Err(error) => Err(error),
            }
        }
        "sources_plan_citation" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
                caret: usize,
                file: String,
                key: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::sources_plan_citation(&host.sources, a.text, a.caret, a.file, a.key)
                        .await,
                ),
                Err(error) => Err(error),
            }
        }
        "sources_bibliography_offer" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
                note_path: String,
                file: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::sources_bibliography_offer(
                        &host.sources,
                        a.text,
                        a.note_path,
                        a.file,
                    )
                    .await,
                ),
                Err(error) => Err(error),
            }
        }
        "sources_create_file" => {
            let _ = arguments;
            answer(commands::sources_create_file(&host.vault).await)
        }
        "attachments_batch_start" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                overrides: VaultOverrides,
                note_path: String,
                items: Vec<attachments::IncomingItem>,
                moment: attachments::InsertMoment,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::attachments_batch_start(
                        &host.vault,
                        a.overrides,
                        a.note_path,
                        a.items,
                        a.moment,
                    )
                    .await,
                ),
                Err(error) => Err(error),
            }
        }
        "attachments_batch_next" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: attachments::AttachmentBatchState,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::attachments_batch_next(a.state)),
                Err(error) => Err(error),
            }
        }
        "attachments_batch_confirm" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: attachments::AttachmentBatchState,
                index: u32,
                accept: bool,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::attachments_batch_confirm(
                    a.state, a.index, a.accept,
                )),
                Err(error) => Err(error),
            }
        }
        "attachments_batch_source_failed" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: attachments::AttachmentBatchState,
                index: u32,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::attachments_batch_source_failed(a.state, a.index)),
                Err(error) => Err(error),
            }
        }
        "attachments_batch_cancel" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: attachments::AttachmentBatchState,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::attachments_batch_cancel(a.state)),
                Err(error) => Err(error),
            }
        }
        "attachments_batch_view" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: attachments::AttachmentBatchState,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::attachments_batch_view(a.state)),
                Err(error) => Err(error),
            }
        }
        "attachments_plan_markup" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text_length: usize,
                point: typsidian_core::templates::InsertionPoint,
                before: String,
                after: String,
                paths: Vec<String>,
                block: bool,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::attachments_plan_markup(
                    a.text_length,
                    a.point,
                    a.before,
                    a.after,
                    a.paths,
                    a.block,
                )),
                Err(error) => Err(error),
            }
        }
        "attachments_validate_folder" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                raw: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::attachments_validate_folder(a.raw)),
                Err(error) => Err(error),
            }
        }
        "import_close_source" => {
            let _ = arguments;
            answer(commands::import_close_source(&host.import_source).await)
        }
        "import_survey" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                job: import::ImportJob,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::import_survey(&host.vault, &host.import_source, a.job).await)
                }
                Err(error) => Err(error),
            }
        }
        "import_begin" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                job: import::ImportJob,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::import_begin(&host.vault, &host.import_source, a.job).await)
                }
                Err(error) => Err(error),
            }
        }
        "import_step" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: import::ImportState,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::import_step(&host.vault, &host.import_source, a.state).await)
                }
                Err(error) => Err(error),
            }
        }
        "import_finish" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: import::ImportState,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::import_finish(&host.vault, a.state).await),
                Err(error) => Err(error),
            }
        }
        "links_backlinks_page" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                target: String,
                open_texts: Vec<links::OpenText>,
                offset: u32,
                limit: u32,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::links_backlinks_page(
                        &host.vault,
                        a.target,
                        a.open_texts,
                        a.offset,
                        a.limit,
                    )
                    .await,
                ),
                Err(error) => Err(error),
            }
        }
        "links_mentions_availability" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                target: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::links_mentions_availability(a.target)),
                Err(error) => Err(error),
            }
        }
        "links_mentions_start" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                target: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::links_mentions_start(&host.vault, a.target).await),
                Err(error) => Err(error),
            }
        }
        "links_mentions_step" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                scan: links::MentionScan,
                open_texts: Vec<links::OpenText>,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::links_mentions_step(&host.vault, a.scan, a.open_texts).await)
                }
                Err(error) => Err(error),
            }
        }
        "links_mentions_cancel" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                scan: links::MentionScan,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::links_mentions_cancel(a.scan)),
                Err(error) => Err(error),
            }
        }
        "links_mentions_in" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                target: String,
                paths: Vec<String>,
                open_texts: Vec<links::OpenText>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::links_mentions_in(&host.vault, a.target, a.paths, a.open_texts).await,
                ),
                Err(error) => Err(error),
            }
        }
        "links_link_mention" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                request: links::LinkRequest,
                editor_text: Option<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::links_link_mention(&host.vault, a.request, a.editor_text).await,
                ),
                Err(error) => Err(error),
            }
        }
        "workspace_empty" => {
            let _ = arguments;
            value(commands::workspace_empty())
        }
        "workspace_apply" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                workspace: workspace::WorkspaceState,
                action: workspace::WorkspaceAction,
                width_px: u32,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::workspace_apply(&host.vault, a.workspace, a.action, a.width_px).await,
                ),
                Err(error) => Err(error),
            }
        }
        "workspace_view" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                workspace: workspace::WorkspaceState,
                width_px: u32,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    answer(commands::workspace_view(&host.vault, a.workspace, a.width_px).await)
                }
                Err(error) => Err(error),
            }
        }
        "workspace_snapshot" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                workspace: workspace::WorkspaceState,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::workspace_snapshot(a.workspace)),
                Err(error) => Err(error),
            }
        }
        "workspace_restore" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                saved: workspace::SavedWorkspace,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::workspace_restore(&host.vault, a.saved).await),
                Err(error) => Err(error),
            }
        }
        "structure_outline" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::structure_outline(a.text)),
                Err(error) => Err(error),
            }
        }
        "structure_outline_view" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                outline: structure::Outline,
                request: structure::OutlineRequest,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::structure_outline_view(a.outline, a.request)),
                Err(error) => Err(error),
            }
        }
        "structure_outline_active" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                outline: structure::Outline,
                request: structure::OutlineRequest,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::structure_outline_active(a.outline, a.request)),
                Err(error) => Err(error),
            }
        }
        "structure_outline_carry" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                prev: structure::Outline,
                next: structure::Outline,
                collapsed: Vec<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::structure_outline_carry(
                    a.prev,
                    a.next,
                    a.collapsed,
                )),
                Err(error) => Err(error),
            }
        }
        "preview_locate_heading" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                runs: Vec<Vec<preview::TextRun>>,
                outline: structure::Outline,
                key: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::preview_locate_heading(a.runs, a.outline, a.key)),
                Err(error) => Err(error),
            }
        }
        "properties_read" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::properties_read(a.text)),
                Err(error) => Err(error),
            }
        }
        "properties_edit" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
                change: properties::PropertyEdit,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::properties_edit(a.text, a.change)),
                Err(error) => Err(error),
            }
        }
        "select_blocks" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::select_blocks(a.text)),
                Err(error) => Err(error),
            }
        }
        "select_import_edits" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::select_import_edits(a.text)),
                Err(error) => Err(error),
            }
        }
        "select_library" => {
            let _ = arguments;
            value(commands::select_library())
        }
        "search_parse_conditions" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
                known_keys: Vec<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::search_parse_conditions(a.text, a.known_keys)),
                Err(error) => Err(error),
            }
        }
        "vault_select" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                selection: properties::Selection,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::vault_select(&host.vault, a.selection).await),
                Err(error) => Err(error),
            }
        }
        "completion_linked_index" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                note_path: String,
                dependencies: Vec<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::completion_linked_index(
                        &host.vault,
                        &host.preview,
                        a.note_path,
                        a.dependencies,
                    )
                    .await,
                ),
                Err(error) => Err(error),
            }
        }
        "completions" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                request: completion::CompletionRequest,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::completions(&host.vault, a.request).await),
                Err(error) => Err(error),
            }
        }
        "definition_at" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                request: completion::DefinitionRequest,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::definition_at(&host.vault, a.request).await),
                Err(error) => Err(error),
            }
        }
        "reference_marks" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                request: completion::MarksRequest,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::reference_marks(&host.vault, a.request).await),
                Err(error) => Err(error),
            }
        }
        "vault_graph" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                request: graph_view::GraphRequest,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::vault_graph(&host.vault, a.request).await),
                Err(error) => Err(error),
            }
        }
        "vault_property_keys" => {
            let _ = arguments;
            answer(commands::vault_property_keys(&host.vault).await)
        }
        "vault_properties_current" => {
            let _ = arguments;
            answer(commands::vault_properties_current(&host.vault).await)
        }
        "vault_cancel_reindex_properties" => {
            let _ = arguments;
            {
                commands::vault_cancel_reindex_properties(&host.properties_reindex);
                value(())
            }
        }
        "vault_ensure_select_library" => {
            let _ = arguments;
            answer(commands::vault_ensure_select_library(&host.vault).await)
        }
        "vault_select_digest" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::vault_select_digest(&host.vault, a.text).await),
                Err(error) => Err(error),
            }
        }
        "slides_start" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                slides: Vec<slides::PageSlideInfo>,
                caret: Option<u32>,
                now_ms: f64,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::slides_start(a.slides, a.caret, a.now_ms)),
                Err(error) => Err(error),
            }
        }
        "slides_navigate" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: slides::ShowState,
                slides: Vec<slides::PageSlideInfo>,
                action: slides::ShowAction,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::slides_navigate(a.state, a.slides, a.action)),
                Err(error) => Err(error),
            }
        }
        "slides_carry" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                state: slides::ShowState,
                slides: Vec<slides::PageSlideInfo>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::slides_carry(a.state, a.slides)),
                Err(error) => Err(error),
            }
        }
        "slides_caret" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                slides: Vec<slides::PageSlideInfo>,
                page: u32,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::slides_caret(a.slides, a.page)),
                Err(error) => Err(error),
            }
        }
        "slides_label" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                slides: Vec<slides::PageSlideInfo>,
                page: u32,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::slides_label(a.slides, a.page)),
                Err(error) => Err(error),
            }
        }
        "slides_keymap" => {
            let _ = arguments;
            value(commands::slides_keymap())
        }
        "capture_settings" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                overrides: VaultOverrides,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::capture_settings(a.overrides)),
                Err(error) => Err(error),
            }
        }
        "capture_entry" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                settings: capture::CaptureSettings,
                capture: capture::Capture,
                image_path: Option<String>,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::capture_entry(a.settings, a.capture, a.image_path)),
                Err(error) => Err(error),
            }
        }
        "capture_target_path" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                settings: capture::CaptureSettings,
                capture: capture::Capture,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::capture_target_path(a.settings, a.capture)),
                Err(error) => Err(error),
            }
        }
        "daily_path" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                settings: capture::CaptureSettings,
                date: capture::DailyDate,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::daily_path(a.settings, a.date)),
                Err(error) => Err(error),
            }
        }
        "daily_date_of" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                settings: capture::CaptureSettings,
                path: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::daily_date_of(a.settings, a.path)),
                Err(error) => Err(error),
            }
        }
        "capture_write" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                capture: capture::Capture,
                open_buffer: Option<String>,
                image: Option<Vec<u8>>,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::capture_write(&host.vault, a.capture, a.open_buffer, a.image).await,
                ),
                Err(error) => Err(error),
            }
        }
        "open_daily" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                date: capture::DailyDate,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::open_daily(&host.vault, a.date).await),
                Err(error) => Err(error),
            }
        }
        "daily_neighbours" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                date: capture::DailyDate,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::daily_neighbours(&host.vault, a.date).await),
                Err(error) => Err(error),
            }
        }
        "check_template_name" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                name: String,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(commands::check_template_name(&host.vault, a.name).await),
                Err(error) => Err(error),
            }
        }
        "save_as_template" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                name: String,
                content: String,
                confirm_overwrite: bool,
            }
            match args::<A>(arguments) {
                Ok(a) => answer(
                    commands::save_as_template(&host.vault, a.name, a.content, a.confirm_overwrite)
                        .await,
                ),
                Err(error) => Err(error),
            }
        }
        "plan_insertion" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text_length: usize,
                point: templates::InsertionPoint,
                content: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::plan_insertion(a.text_length, a.point, a.content)),
                Err(error) => Err(error),
            }
        }
        "element_schema" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                id: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::element_schema(a.id)),
                Err(error) => Err(error),
            }
        }
        "element_schema_for" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                template: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::element_schema_for(a.template)),
                Err(error) => Err(error),
            }
        }
        "element_at" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
                offset: usize,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::element_at(a.text, a.offset)),
                Err(error) => Err(error),
            }
        }
        "syntax_session_open" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                text: String,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::syntax_session_open(
                    a.text,
                    &host.highlight_sessions,
                )),
                Err(error) => Err(error),
            }
        }
        "syntax_session_update" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                id: u32,
                edits: Vec<typsidian_core::highlight::HighlightEdit>,
                length: usize,
                windows: Vec<typsidian_core::highlight::HighlightWindow>,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::syntax_session_update(
                    a.id,
                    a.edits,
                    a.length,
                    a.windows,
                    &host.highlight_sessions,
                )),
                Err(error) => Err(error),
            }
        }
        "syntax_session_close" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                id: u32,
            }
            match args::<A>(arguments) {
                Ok(a) => {
                    commands::syntax_session_close(a.id, &host.highlight_sessions);
                    value(())
                }
                Err(error) => Err(error),
            }
        }
        "render_element" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                request: typsidian_core::elements::RenderRequest,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::render_element(a.request)),
                Err(error) => Err(error),
            }
        }
        "plan_element_edit" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                request: typsidian_core::elements::EditRequest,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::plan_element_edit(a.request)),
                Err(error) => Err(error),
            }
        }
        "template_event" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                event: VaultEvent,
            }
            match args::<A>(arguments) {
                Ok(a) => value(commands::template_event(a.event)),
                Err(error) => Err(error),
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Каждая команда `host::commands` есть в таблице: без vault она отвечает
    /// отказом или значением, но не «нет такой команды».
    #[tokio::test]
    async fn every_command_is_dispatched() {
        let host = Host::default();
        for name in COMMANDS {
            assert!(
                dispatch(&host, name, serde_json::json!({ "__unknown": true }))
                    .await
                    .is_some(),
                "{name}"
            );
        }
        assert!(dispatch(&host, "open_vault", Value::Null).await.is_none());
    }
}
