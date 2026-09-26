//! cargo test — только native (принцип II). Парный файл — `placement_wasm.rs`.
//!
//! Оба файла прогоняют **одну и ту же** таблицу из
//! `common::placement_cases` (contracts/asset-placement.md → «Проверяемые
//! сценарии»): расчёт расположения платформенно нейтрален, и расхождение
//! результата между таргетами является дефектом, а не особенностью платформы.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::placement_cases::{
    check_classification, check_embed_recognition, check_markup_cases, check_snapshot_cases,
};
use common::vault_settings_cases as cases;

#[test]
fn snapshot_paths_match_the_contract() {
    check_snapshot_cases();
}

#[test]
fn markup_paths_match_the_contract() {
    check_markup_cases();
}

#[test]
fn classification_covers_both_layouts() {
    check_classification();
}

#[test]
fn embeds_are_recognised_in_both_layouts() {
    check_embed_recognition();
}

#[tokio::test]
async fn absent_file_is_not_an_error() {
    cases::absent_file_is_not_an_error().await;
}

#[tokio::test]
async fn values_survive_a_round_trip() {
    cases::values_survive_a_round_trip().await;
}

#[tokio::test]
async fn the_file_lives_in_the_service_dir() {
    cases::the_file_lives_in_the_service_dir().await;
}

#[tokio::test]
async fn hand_written_keys_survive_a_write() {
    cases::hand_written_keys_survive_a_write().await;
}

#[tokio::test]
async fn a_broken_file_is_reported_not_overwritten() {
    cases::a_broken_file_is_reported_not_overwritten().await;
}

#[tokio::test]
async fn marking_a_section_is_idempotent_and_ordered() {
    cases::marking_a_section_is_idempotent_and_ordered().await;
}

#[tokio::test]
async fn unmarking_moves_no_files() {
    cases::unmarking_moves_no_files().await;
}

#[tokio::test]
async fn the_vault_registry_has_nowhere_to_live_here() {
    cases::the_vault_registry_has_nowhere_to_live_here().await;
}

#[tokio::test]
async fn a_read_only_vault_gets_no_service_dir() {
    cases::a_read_only_vault_gets_no_service_dir().await;
}

#[tokio::test]
async fn settings_loss_is_reported_only_when_there_was_something_to_lose() {
    cases::settings_loss_is_reported_only_when_there_was_something_to_lose().await;
}

// ── Сочетания клавиш ────────────────────────────────────────────────────────

#[tokio::test]
async fn a_keybinding_survives_a_round_trip() {
    cases::a_keybinding_survives_a_round_trip().await;
}

#[tokio::test]
async fn an_unusable_shortcut_is_refused_before_the_file_is_touched() {
    cases::an_unusable_shortcut_is_refused_before_the_file_is_touched().await;
}

#[tokio::test]
async fn clearing_differs_from_resetting() {
    cases::clearing_differs_from_resetting().await;
}

#[tokio::test]
async fn reassigning_replaces_rather_than_appends() {
    cases::reassigning_replaces_rather_than_appends().await;
}

#[tokio::test]
async fn resetting_an_untouched_command_writes_nothing() {
    cases::resetting_an_untouched_command_writes_nothing().await;
}

#[tokio::test]
async fn a_bad_template_source_is_refused_before_writing() {
    cases::a_bad_template_source_is_refused_before_writing().await;
}
