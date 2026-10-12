//! cargo test — только native (принцип II). Парный файл — `links_wasm.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::links_cases`: правила связей
//! заметки платформенно нейтральны, и расхождение между таргетами является
//! дефектом.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::links_cases as cases;

#[tokio::test]
async fn sources_come_with_highlighted_fragments_nearest_first() {
    cases::sources_come_with_highlighted_fragments_nearest_first().await;
}

#[tokio::test]
async fn three_links_from_one_note_are_one_group() {
    cases::three_links_from_one_note_are_one_group().await;
}

#[tokio::test]
async fn no_backlinks_is_an_empty_page() {
    cases::no_backlinks_is_an_empty_page().await;
}

#[tokio::test]
async fn pages_follow_limit_and_next_offset() {
    cases::pages_follow_limit_and_next_offset().await;
}

#[tokio::test]
async fn unreadable_sources_become_problem_groups() {
    cases::unreadable_sources_become_problem_groups().await;
}

#[tokio::test]
async fn a_link_to_a_board_is_listed_for_the_board() {
    cases::a_link_to_a_board_is_listed_for_the_board().await;
}

#[test]
fn sites_places_and_windows_are_the_same_everywhere() {
    cases::sites_places_and_windows_are_the_same_everywhere();
}

#[tokio::test]
async fn the_page_follows_writes() {
    cases::the_page_follows_writes().await;
}

#[tokio::test]
async fn open_text_wins_over_the_disk() {
    cases::open_text_wins_over_the_disk().await;
}

#[tokio::test]
async fn deleted_and_renamed_sources_follow_the_index() {
    cases::deleted_and_renamed_sources_follow_the_index().await;
}

#[tokio::test]
async fn a_page_belongs_to_its_target() {
    cases::a_page_belongs_to_its_target().await;
}

#[tokio::test]
async fn links_to_a_missing_note_wait_for_it() {
    cases::links_to_a_missing_note_wait_for_it().await;
}

#[test]
fn case_is_ignored() {
    cases::case_is_ignored();
}

#[test]
fn word_boundaries_links_code_and_formulas() {
    cases::word_boundaries_links_code_and_formulas();
}

#[tokio::test]
async fn a_short_title_reads_nothing() {
    cases::a_short_title_reads_nothing().await;
}

#[tokio::test]
async fn the_scan_skips_the_target_and_boards() {
    cases::the_scan_skips_the_target_and_boards().await;
}

#[tokio::test]
async fn a_long_scan_finishes_with_exact_counters() {
    cases::a_long_scan_finishes_with_exact_counters().await;
}

#[tokio::test]
async fn cancel_stops_reading() {
    cases::cancel_stops_reading().await;
}

#[tokio::test]
async fn a_note_added_between_steps_does_not_break_the_cursor() {
    cases::a_note_added_between_steps_does_not_break_the_cursor().await;
}

#[tokio::test]
async fn unreadable_notes_are_skipped_and_open_texts_win() {
    cases::unreadable_notes_are_skipped_and_open_texts_win().await;
}

#[tokio::test]
async fn in_notes_answers_only_for_the_named_notes() {
    cases::in_notes_answers_only_for_the_named_notes().await;
}

#[tokio::test]
async fn linking_changes_only_the_mention() {
    cases::linking_changes_only_the_mention().await;
}

#[tokio::test]
async fn the_link_resolves_to_the_requested_note() {
    cases::the_link_resolves_to_the_requested_note().await;
}

#[tokio::test]
async fn shifted_text_is_stale() {
    cases::shifted_text_is_stale().await;
}

#[tokio::test]
async fn a_stale_stamp_conflicts_into_stale() {
    cases::a_stale_stamp_conflicts_into_stale().await;
}

#[tokio::test]
async fn a_read_only_vault_refuses() {
    cases::a_read_only_vault_refuses().await;
}

#[tokio::test]
async fn the_editor_gets_an_edit_and_the_disk_nothing() {
    cases::the_editor_gets_an_edit_and_the_disk_nothing().await;
}

#[tokio::test]
async fn unsaved_edits_without_an_editor_refuse() {
    cases::unsaved_edits_without_an_editor_refuse().await;
}

#[tokio::test]
async fn an_unwritable_target_is_unexpressible() {
    cases::an_unwritable_target_is_unexpressible().await;
}

#[test]
fn planning_is_pure_and_strict() {
    cases::planning_is_pure_and_strict();
}

#[test]
fn link_text_round_trips_through_resolve() {
    cases::link_text_round_trips_through_resolve();
}
