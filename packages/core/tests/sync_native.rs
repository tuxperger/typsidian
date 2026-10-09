//! cargo test — только native (принцип II). Парный — `sync_wasm.rs`:
//! оба прогоняют одну таблицу политики синхронизации из `common::sync_cases`.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::sync_cases as cases;

#[tokio::test]
async fn enabling_history_writes_rules_and_the_first_snapshot() {
    cases::enabling_history_writes_rules_and_the_first_snapshot().await;
}

#[tokio::test]
async fn syncthing_rules_come_first_and_can_be_declined() {
    cases::syncthing_rules_come_first_and_can_be_declined().await;
}

#[tokio::test]
async fn existing_history_is_not_enabled_twice() {
    cases::existing_history_is_not_enabled_twice().await;
}

#[tokio::test]
async fn a_snapshot_of_selected_files_leaves_the_rest_pending() {
    cases::a_snapshot_of_selected_files_leaves_the_rest_pending().await;
}

#[tokio::test]
async fn an_empty_message_lists_the_changed_notes() {
    cases::an_empty_message_lists_the_changed_notes().await;
}

#[tokio::test]
async fn a_snapshot_needs_saved_notes_and_an_author() {
    cases::a_snapshot_needs_saved_notes_and_an_author().await;
}

#[tokio::test]
async fn a_rename_is_one_change_and_both_names_go_into_the_snapshot() {
    cases::a_rename_is_one_change_and_both_names_go_into_the_snapshot().await;
}

#[tokio::test]
async fn local_history_works_without_network() {
    cases::local_history_works_without_network().await;
}

#[tokio::test]
async fn file_history_follows_renames_and_restore_does_not_snapshot() {
    cases::file_history_follows_renames_and_restore_does_not_snapshot().await;
}

#[tokio::test]
async fn diff_shows_both_sides_and_binary_files_by_size() {
    cases::diff_shows_both_sides_and_binary_files_by_size().await;
}

#[tokio::test]
async fn a_deleted_note_is_restored_in_place() {
    cases::a_deleted_note_is_restored_in_place().await;
}

#[tokio::test]
async fn the_token_never_lands_in_the_vault() {
    cases::the_token_never_lands_in_the_vault().await;
}

#[tokio::test]
async fn offline_exchange_fails_and_keeps_local_snapshots() {
    cases::offline_exchange_fails_and_keeps_local_snapshots().await;
}

#[tokio::test]
async fn push_and_fast_forward_pull_between_devices() {
    cases::push_and_fast_forward_pull_between_devices().await;
}

#[tokio::test]
async fn disjoint_edits_merge_without_asking() {
    cases::disjoint_edits_merge_without_asking().await;
}

#[tokio::test]
async fn a_conflict_blocks_snapshots_and_abort_restores_files() {
    cases::a_conflict_blocks_snapshots_and_abort_restores_files().await;
}

#[tokio::test]
async fn a_resolved_conflict_completes_the_pull() {
    cases::a_resolved_conflict_completes_the_pull().await;
}

#[tokio::test]
async fn a_push_behind_the_server_is_rejected_not_forced() {
    cases::a_push_behind_the_server_is_rejected_not_forced().await;
}

#[tokio::test]
async fn disconnecting_keeps_history_and_files() {
    cases::disconnecting_keeps_history_and_files().await;
}

#[tokio::test]
async fn syncthing_copies_merge_against_the_last_snapshot() {
    cases::syncthing_copies_merge_against_the_last_snapshot().await;
}

#[tokio::test]
async fn a_copy_can_be_taken_or_kept_as_a_separate_file() {
    cases::a_copy_can_be_taken_or_kept_as_a_separate_file().await;
}

#[tokio::test]
async fn conflict_copies_never_become_notes() {
    cases::conflict_copies_never_become_notes().await;
}

#[tokio::test]
async fn a_non_keeper_needs_explicit_confirmation() {
    cases::a_non_keeper_needs_explicit_confirmation().await;
}

#[tokio::test]
async fn appointing_the_keeper_is_visible_and_a_settings_copy_is_a_dispute() {
    cases::appointing_the_keeper_is_visible_and_a_settings_copy_is_a_dispute().await;
}

#[tokio::test]
async fn missing_syncthing_rules_are_reported_with_history() {
    cases::missing_syncthing_rules_are_reported_with_history().await;
}

#[tokio::test]
async fn the_keeper_snapshots_after_the_pause() {
    cases::the_keeper_snapshots_after_the_pause().await;
}

#[tokio::test]
async fn a_non_keeper_never_snapshots_automatically() {
    cases::a_non_keeper_never_snapshots_automatically().await;
}

#[tokio::test]
async fn a_non_keeper_reads_history_from_the_mirror() {
    cases::a_non_keeper_reads_history_from_the_mirror().await;
}

#[tokio::test]
async fn a_lost_history_is_reported_as_lost() {
    cases::a_lost_history_is_reported_as_lost().await;
}

#[tokio::test]
async fn shells_reject_what_they_cannot_do() {
    cases::shells_reject_what_they_cannot_do().await;
}

#[tokio::test]
async fn settings_survive_in_vault_json_next_to_other_keys() {
    cases::settings_survive_in_vault_json_next_to_other_keys().await;
}

#[tokio::test]
async fn a_remote_address_changed_elsewhere_gets_neither_snapshots_nor_the_token() {
    cases::a_remote_address_changed_elsewhere_gets_neither_snapshots_nor_the_token().await;
}

#[tokio::test]
async fn pulled_paths_that_alias_the_history_folder_are_refused() {
    cases::pulled_paths_that_alias_the_history_folder_are_refused().await;
}

#[tokio::test]
async fn ignore_editor_opens_an_existing_file_and_saves_the_edit() {
    cases::ignore_editor_opens_an_existing_file_and_saves_the_edit().await;
}

#[tokio::test]
async fn ignore_editor_offers_the_block_for_an_absent_file_without_creating_it() {
    cases::ignore_editor_offers_the_block_for_an_absent_file_without_creating_it().await;
}

#[tokio::test]
async fn ignore_editor_keeps_crlf_line_endings() {
    cases::ignore_editor_keeps_crlf_line_endings().await;
}

#[tokio::test]
async fn ignore_editor_does_not_touch_what_it_cannot_edit() {
    cases::ignore_editor_does_not_touch_what_it_cannot_edit().await;
}

#[tokio::test]
async fn ignore_editor_saves_on_a_device_that_is_not_the_keeper() {
    cases::ignore_editor_saves_on_a_device_that_is_not_the_keeper().await;
}

#[tokio::test]
async fn ignore_editor_asks_before_saving_without_required_rules() {
    cases::ignore_editor_asks_before_saving_without_required_rules().await;
}

#[tokio::test]
async fn ignore_editor_complete_rules_lift_the_declined_flag() {
    cases::ignore_editor_complete_rules_lift_the_declined_flag().await;
}

#[tokio::test]
async fn ignore_editor_does_not_overwrite_an_outside_change() {
    cases::ignore_editor_does_not_overwrite_an_outside_change().await;
}

#[tokio::test]
async fn ignore_editor_recreates_a_deleted_file_only_on_request() {
    cases::ignore_editor_recreates_a_deleted_file_only_on_request().await;
}

#[tokio::test]
async fn ignore_editor_waits_for_an_unfinished_pull() {
    cases::ignore_editor_waits_for_an_unfinished_pull().await;
}

#[tokio::test]
async fn a_file_over_the_note_size_limit_is_pulled_like_any_other() {
    cases::a_file_over_the_note_size_limit_is_pulled_like_any_other().await;
}

#[tokio::test]
async fn a_lost_history_is_restored_from_the_remote() {
    cases::a_lost_history_is_restored_from_the_remote().await;
}
