//! wasm-bindgen-test — только wasm32-unknown-unknown (принцип II). Парный файл
//! — `placement_native.rs`.
//!
//! Оба прогоняют **одну и ту же** таблицу из `common::placement_cases`: если
//! расчёт расположения или хранилище настроек уровня vault разойдутся между
//! таргетами, упадёт ровно один из двух файлов, и станет видно, какой именно.
#![cfg(target_arch = "wasm32")]

mod common;

use common::placement_cases::{
    check_classification, check_embed_recognition, check_markup_cases, check_snapshot_cases,
};
use common::vault_settings_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn snapshot_paths_match_the_contract() {
    check_snapshot_cases();
}

#[wasm_bindgen_test]
fn markup_paths_match_the_contract() {
    check_markup_cases();
}

#[wasm_bindgen_test]
fn classification_covers_both_layouts() {
    check_classification();
}

#[wasm_bindgen_test]
fn embeds_are_recognised_in_both_layouts() {
    check_embed_recognition();
}

#[wasm_bindgen_test]
async fn absent_file_is_not_an_error() {
    cases::absent_file_is_not_an_error().await;
}

#[wasm_bindgen_test]
async fn values_survive_a_round_trip() {
    cases::values_survive_a_round_trip().await;
}

#[wasm_bindgen_test]
async fn the_file_lives_in_the_service_dir() {
    cases::the_file_lives_in_the_service_dir().await;
}

#[wasm_bindgen_test]
async fn hand_written_keys_survive_a_write() {
    cases::hand_written_keys_survive_a_write().await;
}

#[wasm_bindgen_test]
async fn a_broken_file_is_reported_not_overwritten() {
    cases::a_broken_file_is_reported_not_overwritten().await;
}

#[wasm_bindgen_test]
async fn marking_a_section_is_idempotent_and_ordered() {
    cases::marking_a_section_is_idempotent_and_ordered().await;
}

#[wasm_bindgen_test]
async fn unmarking_moves_no_files() {
    cases::unmarking_moves_no_files().await;
}

#[wasm_bindgen_test]
async fn the_vault_registry_has_nowhere_to_live_here() {
    cases::the_vault_registry_has_nowhere_to_live_here().await;
}

#[wasm_bindgen_test]
async fn a_read_only_vault_gets_no_service_dir() {
    cases::a_read_only_vault_gets_no_service_dir().await;
}

#[wasm_bindgen_test]
async fn settings_loss_is_reported_only_when_there_was_something_to_lose() {
    cases::settings_loss_is_reported_only_when_there_was_something_to_lose().await;
}

// ── Сочетания клавиш ────────────────────────────────────────────────────────

#[wasm_bindgen_test]
async fn a_keybinding_survives_a_round_trip() {
    cases::a_keybinding_survives_a_round_trip().await;
}

#[wasm_bindgen_test]
async fn an_unusable_shortcut_is_refused_before_the_file_is_touched() {
    cases::an_unusable_shortcut_is_refused_before_the_file_is_touched().await;
}

#[wasm_bindgen_test]
async fn clearing_differs_from_resetting() {
    cases::clearing_differs_from_resetting().await;
}

#[wasm_bindgen_test]
async fn reassigning_replaces_rather_than_appends() {
    cases::reassigning_replaces_rather_than_appends().await;
}

#[wasm_bindgen_test]
async fn resetting_an_untouched_command_writes_nothing() {
    cases::resetting_an_untouched_command_writes_nothing().await;
}

#[wasm_bindgen_test]
async fn a_bad_template_source_is_refused_before_writing() {
    cases::a_bad_template_source_is_refused_before_writing().await;
}

#[wasm_bindgen_test]
async fn the_export_folder_is_validated_and_normalized() {
    cases::the_export_folder_is_validated_and_normalized().await;
}

#[wasm_bindgen_test]
async fn a_bad_export_folder_in_the_file_is_dropped() {
    cases::a_bad_export_folder_in_the_file_is_dropped().await;
}

#[wasm_bindgen_test]
async fn the_attachments_settings_are_validated_and_survive_a_round_trip() {
    cases::the_attachments_settings_are_validated_and_survive_a_round_trip().await;
}

#[wasm_bindgen_test]
async fn bad_attachments_values_are_dropped_and_unknown_keys_kept() {
    cases::bad_attachments_values_are_dropped_and_unknown_keys_kept().await;
}
