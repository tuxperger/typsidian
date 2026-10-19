//! cargo test — только native (принцип II). Парный файл — `attachments_wasm.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::attachment_cases`: правила
//! вложений и пачка платформенно нейтральны, и расхождение между таргетами —
//! дефект (FR-025).
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::attachment_cases as cases;

#[test]
fn the_five_kinds_are_recognised_by_content_and_the_rest_refused() {
    cases::the_five_kinds_are_recognised_by_content_and_the_rest_refused();
}

#[test]
fn names_follow_the_note_the_moment_and_the_kind() {
    cases::names_follow_the_note_the_moment_and_the_kind();
}

#[test]
fn folder_and_path_for_both_placements_and_three_depths() {
    cases::folder_and_path_for_both_placements_and_three_depths();
}

#[test]
fn the_folder_rule_is_the_export_folder_rule() {
    cases::the_folder_rule_is_the_export_folder_rule();
}

#[test]
fn markup_is_escaped_and_planned_inline_or_by_lines() {
    cases::markup_is_escaped_and_planned_inline_or_by_lines();
}

#[tokio::test]
async fn a_clipboard_image_lands_in_the_attachments_folder() {
    cases::a_clipboard_image_lands_in_the_attachments_folder().await;
}

#[tokio::test]
async fn an_existing_folder_with_other_images_still_takes_the_new_one() {
    cases::an_existing_folder_with_other_images_still_takes_the_new_one().await;
}

#[tokio::test]
async fn a_busy_name_gets_a_suffix_and_the_old_file_is_untouched() {
    cases::a_busy_name_gets_a_suffix_and_the_old_file_is_untouched().await;
}

#[tokio::test]
async fn the_same_content_in_the_target_folder_is_reused() {
    cases::the_same_content_in_the_target_folder_is_reused().await;
}

#[tokio::test]
async fn an_unsupported_kind_is_skipped_without_writing() {
    cases::an_unsupported_kind_is_skipped_without_writing().await;
}

#[tokio::test]
async fn the_kind_is_checked_before_matching() {
    cases::the_kind_is_checked_before_matching().await;
}

#[tokio::test]
async fn a_read_only_vault_or_a_non_note_refuses_without_touching_the_disk() {
    cases::a_read_only_vault_or_a_non_note_refuses_without_touching_the_disk().await;
}

#[tokio::test]
async fn a_failed_write_leaves_no_file_and_no_markup() {
    cases::a_failed_write_leaves_no_file_and_no_markup().await;
}

#[tokio::test]
async fn steps_hand_out_one_file_at_a_time() {
    cases::steps_hand_out_one_file_at_a_time().await;
}

#[tokio::test]
async fn an_empty_batch_is_finished_at_once() {
    cases::an_empty_batch_is_finished_at_once().await;
}

#[tokio::test]
async fn a_dropped_file_already_in_the_vault_is_referenced() {
    cases::a_dropped_file_already_in_the_vault_is_referenced().await;
}

#[tokio::test]
async fn two_identical_files_in_one_batch_make_one_file() {
    cases::two_identical_files_in_one_batch_make_one_file().await;
}

#[tokio::test]
async fn a_mixed_batch_writes_images_and_skips_the_rest_by_name() {
    cases::a_mixed_batch_writes_images_and_skips_the_rest_by_name().await;
}

#[tokio::test]
async fn large_files_are_confirmed_before_any_copy() {
    cases::large_files_are_confirmed_before_any_copy().await;
}

#[tokio::test]
async fn cancel_keeps_the_current_and_written_files() {
    cases::cancel_keeps_the_current_and_written_files().await;
}

#[test]
fn ignore_rules_exclude_a_folder_by_the_research_table() {
    cases::ignore_rules_exclude_a_folder_by_the_research_table();
}

#[tokio::test]
async fn an_excluded_folder_is_reported_and_still_written() {
    cases::an_excluded_folder_is_reported_and_still_written().await;
}

#[tokio::test]
async fn changing_the_placement_moves_nothing() {
    cases::changing_the_placement_moves_nothing().await;
}

#[tokio::test]
async fn a_camera_shot_is_named_like_a_clipboard_image() {
    cases::a_camera_shot_is_named_like_a_clipboard_image().await;
}

#[tokio::test]
async fn a_cancelled_shot_changes_nothing() {
    cases::a_cancelled_shot_changes_nothing().await;
}

#[tokio::test]
async fn the_batch_state_survives_serialization() {
    cases::the_batch_state_survives_serialization().await;
}
