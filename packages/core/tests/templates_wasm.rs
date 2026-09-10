//! wasm-bindgen-test — только wasm32-unknown-unknown (принцип II). Парный файл
//! — `templates_native.rs`: оба вызывают одну и ту же таблицу
//! `common::template_cases`, потому что правила библиотеки заготовок
//! платформенно нейтральны, а расхождение между таргетами является дефектом, а
//! не особенностью платформы.
#![cfg(target_arch = "wasm32")]

mod common;

use common::template_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// ── Phase 2: состав библиотеки и состояние папки ───────────────────────

#[wasm_bindgen_test]
async fn a_missing_folder_is_not_an_error() {
    cases::a_missing_folder_is_not_an_error().await;
}

#[wasm_bindgen_test]
async fn an_unreadable_folder_does_not_collapse_into_absent() {
    cases::an_unreadable_folder_does_not_collapse_into_absent().await;
}

#[wasm_bindgen_test]
async fn a_read_only_vault_makes_the_folder_not_writable() {
    cases::a_read_only_vault_makes_the_folder_not_writable().await;
}

#[wasm_bindgen_test]
async fn any_note_inside_the_folder_is_a_template() {
    cases::any_note_inside_the_folder_is_a_template().await;
}

#[wasm_bindgen_test]
async fn a_board_in_the_folder_is_not_a_template() {
    cases::a_board_in_the_folder_is_not_a_template().await;
}

#[wasm_bindgen_test]
async fn a_nested_templates_folder_is_someone_elses() {
    cases::a_nested_templates_folder_is_someone_elses().await;
}

// ── US1: превью и применение заготовки ─────────────────────────────────

#[wasm_bindgen_test]
async fn an_unreadable_template_is_reported_and_left_alone() {
    cases::an_unreadable_template_is_reported_and_left_alone().await;
}

#[wasm_bindgen_test]
async fn a_disk_failure_is_not_the_same_as_bad_content() {
    cases::a_disk_failure_is_not_the_same_as_bad_content().await;
}

#[wasm_bindgen_test]
async fn the_excerpt_stops_at_a_line_boundary() {
    cases::the_excerpt_stops_at_a_line_boundary().await;
}

#[wasm_bindgen_test]
async fn the_excerpt_source_never_claims_the_content_is_broken() {
    cases::the_excerpt_source_never_claims_the_content_is_broken().await;
}

#[wasm_bindgen_test]
async fn the_cache_is_keyed_by_content_not_by_path() {
    cases::the_cache_is_keyed_by_content_not_by_path().await;
}

#[wasm_bindgen_test]
async fn an_unreadable_template_previews_as_unavailable() {
    cases::an_unreadable_template_previews_as_unavailable().await;
}

#[wasm_bindgen_test]
async fn applying_a_template_leaves_its_file_untouched() {
    cases::applying_a_template_leaves_its_file_untouched().await;
}

// ── US2: сохранение как заготовки ──────────────────────────────────────

#[wasm_bindgen_test]
async fn an_invalid_name_never_touches_the_disk() {
    cases::an_invalid_name_never_touches_the_disk().await;
}

#[wasm_bindgen_test]
async fn a_taken_name_without_confirmation_keeps_the_existing_file() {
    cases::a_taken_name_without_confirmation_keeps_the_existing_file().await;
}

#[wasm_bindgen_test]
async fn the_first_save_creates_the_folder() {
    cases::the_first_save_creates_the_folder().await;
}

#[wasm_bindgen_test]
async fn a_read_only_vault_refuses_the_save_with_a_reason() {
    cases::a_read_only_vault_refuses_the_save_with_a_reason().await;
}

#[wasm_bindgen_test]
async fn the_saved_content_is_frozen_at_save_time() {
    cases::the_saved_content_is_frozen_at_save_time().await;
}

// ── US3: вставка в позицию курсора ─────────────────────────────────────

#[wasm_bindgen_test]
async fn insertion_is_verbatim_and_replaces_the_selection() {
    cases::insertion_is_verbatim_and_replaces_the_selection().await;
}

#[wasm_bindgen_test]
async fn inserting_does_not_touch_the_template_file() {
    cases::inserting_does_not_touch_the_template_file().await;
}

// ── Phase 7: наследство от vault-manager ───────────────────────────────

#[wasm_bindgen_test]
async fn templates_appear_in_the_tree_as_ordinary_notes() {
    cases::templates_appear_in_the_tree_as_ordinary_notes().await;
}

#[wasm_bindgen_test]
async fn a_template_is_edited_through_the_ordinary_note_path() {
    cases::a_template_is_edited_through_the_ordinary_note_path().await;
}

#[wasm_bindgen_test]
async fn templates_take_part_in_the_link_graph() {
    cases::templates_take_part_in_the_link_graph().await;
}
