//! cargo test — только native (принцип II). Парный файл — `capture_wasm.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::capture_cases`: правила
//! захвата платформенно нейтральны, и расхождение между таргетами является
//! дефектом.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::capture_cases as cases;

#[test]
fn defaults_apply_when_nothing_is_set() {
    cases::defaults_apply_when_nothing_is_set();
}

#[test]
fn unknown_values_fall_back_to_defaults() {
    cases::unknown_values_fall_back_to_defaults();
}

#[test]
fn settings_survive_the_round_trip() {
    cases::settings_survive_the_round_trip();
}

#[test]
fn writing_capture_keeps_other_keys() {
    cases::writing_capture_keeps_other_keys();
}

#[tokio::test]
async fn writing_settings_normalizes_and_refuses_bad_paths() {
    cases::writing_settings_normalizes_and_refuses_bad_paths().await;
}

#[test]
fn daily_path_follows_the_three_name_forms() {
    cases::daily_path_follows_the_three_name_forms();
}

#[test]
fn impossible_days_have_no_path() {
    cases::impossible_days_have_no_path();
}

#[test]
fn the_date_of_a_path_is_the_inverse_of_the_path() {
    cases::the_date_of_a_path_is_the_inverse_of_the_path();
}

#[tokio::test]
async fn today_creates_the_note_without_a_template() {
    cases::today_creates_the_note_without_a_template().await;
}

#[tokio::test]
async fn today_creates_the_note_from_the_template() {
    cases::today_creates_the_note_from_the_template().await;
}

#[tokio::test]
async fn a_missing_template_does_not_block_today() {
    cases::a_missing_template_does_not_block_today().await;
}

#[tokio::test]
async fn today_twice_is_one_note_and_a_new_day_is_a_new_one() {
    cases::today_twice_is_one_note_and_a_new_day_is_a_new_one().await;
}

#[tokio::test]
async fn today_in_a_read_only_vault() {
    cases::today_in_a_read_only_vault().await;
}

#[tokio::test]
async fn neighbours_skip_the_gaps() {
    cases::neighbours_skip_the_gaps().await;
}

#[tokio::test]
async fn neighbours_work_with_nested_folders() {
    cases::neighbours_work_with_nested_folders().await;
}

#[test]
fn entry_styles_and_time() {
    cases::entry_styles_and_time();
}

#[test]
fn entry_kinds_and_the_note() {
    cases::entry_kinds_and_the_note();
}

#[test]
fn entry_escapes_markup() {
    cases::entry_escapes_markup();
}

#[test]
fn entry_keeps_multiline_text_together() {
    cases::entry_keeps_multiline_text_together();
}

#[test]
fn append_never_changes_the_previous_text() {
    cases::append_never_changes_the_previous_text();
}

#[test]
fn append_edit_is_in_utf16_units() {
    cases::append_edit_is_in_utf16_units();
}

#[test]
fn target_paths() {
    cases::target_paths();
}

#[tokio::test]
async fn a_capture_creates_the_inbox_and_keeps_the_order() {
    cases::a_capture_creates_the_inbox_and_keeps_the_order().await;
}

#[tokio::test]
async fn a_capture_keeps_the_previous_text() {
    cases::a_capture_keeps_the_previous_text().await;
}

#[tokio::test]
async fn a_capture_to_a_chosen_note() {
    cases::a_capture_to_a_chosen_note().await;
}

#[tokio::test]
async fn a_capture_to_the_daily_note_creates_it_from_the_template() {
    cases::a_capture_to_the_daily_note_creates_it_from_the_template().await;
}

#[tokio::test]
async fn vault_settings_drive_the_entry() {
    cases::vault_settings_drive_the_entry().await;
}

#[tokio::test]
async fn a_broken_settings_file_does_not_stop_a_capture() {
    cases::a_broken_settings_file_does_not_stop_a_capture().await;
}

#[tokio::test]
async fn an_open_note_gets_a_buffer_edit_and_no_file_write() {
    cases::an_open_note_gets_a_buffer_edit_and_no_file_write().await;
}

#[tokio::test]
async fn unsaved_edits_elsewhere_refuse_the_capture() {
    cases::unsaved_edits_elsewhere_refuse_the_capture().await;
}

#[tokio::test]
async fn a_conflict_retries_on_the_fresh_text() {
    cases::a_conflict_retries_on_the_fresh_text().await;
}

#[tokio::test]
async fn a_persistent_conflict_is_refused_after_three_attempts() {
    cases::a_persistent_conflict_is_refused_after_three_attempts().await;
}

#[tokio::test]
async fn a_capture_over_the_size_limit_is_refused() {
    cases::a_capture_over_the_size_limit_is_refused().await;
}

#[tokio::test]
async fn read_only_and_unavailable_vaults_refuse_with_a_reason() {
    cases::read_only_and_unavailable_vaults_refuse_with_a_reason().await;
}

#[tokio::test]
async fn an_empty_capture_is_refused() {
    cases::an_empty_capture_is_refused().await;
}

#[tokio::test]
async fn an_image_becomes_an_attachment_next_to_the_note() {
    cases::an_image_becomes_an_attachment_next_to_the_note().await;
}

#[tokio::test]
async fn the_attachment_follows_the_vault_placement_policy() {
    cases::the_attachment_follows_the_vault_placement_policy().await;
}

#[tokio::test]
async fn an_image_into_an_open_note_returns_the_markup_edit() {
    cases::an_image_into_an_open_note_returns_the_markup_edit().await;
}

#[tokio::test]
async fn attachments_are_not_duplicated_and_names_do_not_collide() {
    cases::attachments_are_not_duplicated_and_names_do_not_collide().await;
}

#[tokio::test]
async fn an_unsupported_image_is_refused() {
    cases::an_unsupported_image_is_refused().await;
}

#[test]
fn queue_json_and_outcomes_cross_the_boundary() {
    cases::queue_json_and_outcomes_cross_the_boundary();
}

/// Свежий vault на диске: папки ежедневных заметок ещё нет, «Сегодня» заводит её.
/// `MemoryVault` папок не моделирует, поэтому случай — на настоящем `NativeBackend`.
/// Пару на web (OPFS) держит `capture_wasm.rs`: там `create` до T020 папок не
/// заводил, и «Сегодня» отказывало; здесь — тот же случай на диске.
#[tokio::test]
async fn today_creates_the_daily_folder_in_a_fresh_vault() {
    use typsidian_core::capture::{open_daily, CaptureSettings, DailyDate};
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    let dir = tempfile::TempDir::new().unwrap();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    let opened = open_daily(
        &mut backend,
        &CaptureSettings::default(),
        &DailyDate {
            year: 2026,
            month: 10,
            day: 16,
        },
    )
    .await
    .expect("«Сегодня» в свежем vault создаёт заметку");
    assert!(opened.created);
    assert!(dir.path().join(&opened.path).is_file(), "{}", opened.path);
}
