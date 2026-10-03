//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `export_native.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::export_cases`: сборка PDF,
//! путь результата и правила пакета платформенно нейтральны, и расхождение
//! между таргетами является дефектом.
#![cfg(target_arch = "wasm32")]

mod common;

use common::export_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn a_note_exports_to_a_pdf_with_the_preview_page_count() {
    cases::a_note_exports_to_a_pdf_with_the_preview_page_count().await;
}

#[wasm_bindgen_test]
async fn fonts_and_metadata_are_embedded() {
    cases::fonts_and_metadata_are_embedded().await;
}

#[wasm_bindgen_test]
async fn the_editor_text_wins_over_the_disk() {
    cases::the_editor_text_wins_over_the_disk().await;
}

#[wasm_bindgen_test]
async fn without_editor_text_the_note_is_read_from_disk() {
    cases::without_editor_text_the_note_is_read_from_disk().await;
}

#[wasm_bindgen_test]
async fn a_missing_note_is_reported_as_missing() {
    cases::a_missing_note_is_reported_as_missing().await;
}

#[wasm_bindgen_test]
async fn a_broken_note_writes_nothing_and_keeps_the_old_pdf() {
    cases::a_broken_note_writes_nothing_and_keeps_the_old_pdf().await;
}

#[wasm_bindgen_test]
async fn a_second_export_replaces_the_first() {
    cases::a_second_export_replaces_the_first().await;
}

#[wasm_bindgen_test]
async fn the_same_note_exports_to_the_same_bytes() {
    cases::the_same_note_exports_to_the_same_bytes().await;
}

#[wasm_bindgen_test]
async fn a_folder_in_place_of_the_pdf_is_not_touched() {
    cases::a_folder_in_place_of_the_pdf_is_not_touched().await;
}

#[wasm_bindgen_test]
async fn a_read_only_vault_refuses_the_write() {
    cases::a_read_only_vault_refuses_the_write().await;
}

#[wasm_bindgen_test]
async fn the_export_folder_setting_is_honoured() {
    cases::the_export_folder_setting_is_honoured().await;
}

#[wasm_bindgen_test]
async fn nothing_but_the_pdf_is_written() {
    cases::nothing_but_the_pdf_is_written().await;
}

#[wasm_bindgen_test]
fn a_batch_runs_in_list_order_one_at_a_time() {
    cases::a_batch_runs_in_list_order_one_at_a_time();
}

#[wasm_bindgen_test]
fn the_estimate_appears_after_the_first_note() {
    cases::the_estimate_appears_after_the_first_note();
}

#[wasm_bindgen_test]
fn a_failure_does_not_stop_the_batch_by_default() {
    cases::a_failure_does_not_stop_the_batch_by_default();
}

#[wasm_bindgen_test]
fn stop_on_error_cancels_the_rest() {
    cases::stop_on_error_cancels_the_rest();
}

#[wasm_bindgen_test]
fn cancel_lets_the_current_note_finish() {
    cases::cancel_lets_the_current_note_finish();
}

#[wasm_bindgen_test]
fn only_a_queued_note_can_be_skipped() {
    cases::only_a_queued_note_can_be_skipped();
}

#[wasm_bindgen_test]
fn a_stray_report_changes_nothing() {
    cases::a_stray_report_changes_nothing();
}

#[wasm_bindgen_test]
fn warnings_mark_the_row() {
    cases::warnings_mark_the_row();
}

#[wasm_bindgen_test]
fn an_empty_batch_is_already_over() {
    cases::an_empty_batch_is_already_over();
}

#[wasm_bindgen_test]
fn the_batch_remembers_the_export_folder() {
    cases::the_batch_remembers_the_export_folder();
}

#[wasm_bindgen_test]
async fn a_typst_error_carries_no_app_message() {
    cases::a_typst_error_carries_no_app_message().await;
}

#[wasm_bindgen_test]
async fn an_unreadable_dependency_is_an_app_message() {
    cases::an_unreadable_dependency_is_an_app_message().await;
}

#[wasm_bindgen_test]
async fn a_note_outside_the_vault_is_an_app_message() {
    cases::a_note_outside_the_vault_is_an_app_message().await;
}
