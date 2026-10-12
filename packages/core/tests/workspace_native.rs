//! cargo test — только native (принцип II). Парный файл — `workspace_wasm.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::workspace_cases`.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::workspace_cases as cases;

#[test]
fn the_empty_state_is_one_empty_group() {
    cases::the_empty_state_is_one_empty_group();
}

#[test]
fn open_adds_a_tab_and_reopening_activates_it() {
    cases::open_adds_a_tab_and_reopening_activates_it();
}

#[test]
fn open_of_a_missing_path_is_not_found() {
    cases::open_of_a_missing_path_is_not_found();
}

#[test]
fn activate_and_cycle_wrap_in_both_directions() {
    cases::activate_and_cycle_wrap_in_both_directions();
}

#[test]
fn move_reorders_within_a_group() {
    cases::move_reorders_within_a_group();
}

#[test]
fn an_unknown_tab_is_rejected_without_panic() {
    cases::an_unknown_tab_is_rejected_without_panic();
}

#[test]
fn closing_the_active_tab_activates_the_most_recent_one() {
    cases::closing_the_active_tab_activates_the_most_recent_one();
}

#[test]
fn unsaved_and_conflicting_tabs_refuse_to_close() {
    cases::unsaved_and_conflicting_tabs_refuse_to_close();
}

#[test]
fn an_unsaved_board_refuses_to_close() {
    cases::an_unsaved_board_refuses_to_close();
}

#[test]
fn close_others_closes_clean_tabs_and_lists_refusals() {
    cases::close_others_closes_clean_tabs_and_lists_refusals();
}

#[test]
fn closing_the_last_tab_leaves_an_empty_group() {
    cases::closing_the_last_tab_leaves_an_empty_group();
}

#[test]
fn renamed_keeps_the_tab_id_and_deleted_closes_the_tab() {
    cases::renamed_keeps_the_tab_id_and_deleted_closes_the_tab();
}

#[test]
fn view_titles_hints_and_flags() {
    cases::view_titles_hints_and_flags();
}

#[test]
fn back_and_forward_return_to_the_place_of_leaving() {
    cases::back_and_forward_return_to_the_place_of_leaving();
}

#[test]
fn open_after_back_drops_the_forward_branch() {
    cases::open_after_back_drops_the_forward_branch();
}

#[test]
fn history_reopens_closed_and_skips_deleted() {
    cases::history_reopens_closed_and_skips_deleted();
}

#[test]
fn open_with_a_target_in_the_active_note_adds_an_entry() {
    cases::open_with_a_target_in_the_active_note_adds_an_entry();
}

#[test]
fn history_is_bounded() {
    cases::history_is_bounded();
}

#[test]
fn back_from_an_empty_group_returns_the_last_closed() {
    cases::back_from_an_empty_group_returns_the_last_closed();
}

#[test]
fn split_duplicates_the_note_and_moves_focus() {
    cases::split_duplicates_the_note_and_moves_focus();
}

#[test]
fn split_moves_a_board_and_refuses_when_nothing_to_split() {
    cases::split_moves_a_board_and_refuses_when_nothing_to_split();
}

#[test]
fn moving_between_groups_keeps_the_tab_id_and_collapses_empty_groups() {
    cases::moving_between_groups_keeps_the_tab_id_and_collapses_empty_groups();
}

#[test]
fn closing_the_last_tab_of_a_group_removes_the_split() {
    cases::closing_the_last_tab_of_a_group_removes_the_split();
}

#[test]
fn unsplit_merges_without_duplicates() {
    cases::unsplit_merges_without_duplicates();
}

#[test]
fn focus_group_and_ratio() {
    cases::focus_group_and_ratio();
}

#[test]
fn a_tab_open_in_the_other_group_closes_despite_unsaved() {
    cases::a_tab_open_in_the_other_group_closes_despite_unsaved();
}

#[test]
fn every_group_has_its_own_history() {
    cases::every_group_has_its_own_history();
}

#[test]
fn narrow_windows_merge_the_strips_without_touching_the_state() {
    cases::narrow_windows_merge_the_strips_without_touching_the_state();
}

#[test]
fn compact_width_gives_the_list_presentation() {
    cases::compact_width_gives_the_list_presentation();
}

#[test]
fn closing_in_the_merged_presentation_closes_both_tabs() {
    cases::closing_in_the_merged_presentation_closes_both_tabs();
}

#[test]
fn snapshot_and_restore_round_trip() {
    cases::snapshot_and_restore_round_trip();
}

#[test]
fn restore_repairs_a_bad_set() {
    cases::restore_repairs_a_bad_set();
}

#[tokio::test]
async fn facts_of_reads_the_index() {
    cases::facts_of_reads_the_index().await;
}

#[tokio::test]
async fn restore_with_skips_missing_notes() {
    cases::restore_with_skips_missing_notes().await;
}
