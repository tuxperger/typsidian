//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `workspace_native.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::workspace_cases`.
#![cfg(target_arch = "wasm32")]

mod common;

use common::workspace_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn the_empty_state_is_one_empty_group() {
    cases::the_empty_state_is_one_empty_group();
}

#[wasm_bindgen_test]
fn open_adds_a_tab_and_reopening_activates_it() {
    cases::open_adds_a_tab_and_reopening_activates_it();
}

#[wasm_bindgen_test]
fn open_of_a_missing_path_is_not_found() {
    cases::open_of_a_missing_path_is_not_found();
}

#[wasm_bindgen_test]
fn activate_and_cycle_wrap_in_both_directions() {
    cases::activate_and_cycle_wrap_in_both_directions();
}

#[wasm_bindgen_test]
fn move_reorders_within_a_group() {
    cases::move_reorders_within_a_group();
}

#[wasm_bindgen_test]
fn an_unknown_tab_is_rejected_without_panic() {
    cases::an_unknown_tab_is_rejected_without_panic();
}

#[wasm_bindgen_test]
fn closing_the_active_tab_activates_the_most_recent_one() {
    cases::closing_the_active_tab_activates_the_most_recent_one();
}

#[wasm_bindgen_test]
fn unsaved_and_conflicting_tabs_refuse_to_close() {
    cases::unsaved_and_conflicting_tabs_refuse_to_close();
}

#[wasm_bindgen_test]
fn an_unsaved_board_refuses_to_close() {
    cases::an_unsaved_board_refuses_to_close();
}

#[wasm_bindgen_test]
fn close_others_closes_clean_tabs_and_lists_refusals() {
    cases::close_others_closes_clean_tabs_and_lists_refusals();
}

#[wasm_bindgen_test]
fn closing_the_last_tab_leaves_an_empty_group() {
    cases::closing_the_last_tab_leaves_an_empty_group();
}

#[wasm_bindgen_test]
fn renamed_keeps_the_tab_id_and_deleted_closes_the_tab() {
    cases::renamed_keeps_the_tab_id_and_deleted_closes_the_tab();
}

#[wasm_bindgen_test]
fn view_titles_hints_and_flags() {
    cases::view_titles_hints_and_flags();
}

#[wasm_bindgen_test]
fn back_and_forward_return_to_the_place_of_leaving() {
    cases::back_and_forward_return_to_the_place_of_leaving();
}

#[wasm_bindgen_test]
fn open_after_back_drops_the_forward_branch() {
    cases::open_after_back_drops_the_forward_branch();
}

#[wasm_bindgen_test]
fn history_reopens_closed_and_skips_deleted() {
    cases::history_reopens_closed_and_skips_deleted();
}

#[wasm_bindgen_test]
fn open_with_a_target_in_the_active_note_adds_an_entry() {
    cases::open_with_a_target_in_the_active_note_adds_an_entry();
}

#[wasm_bindgen_test]
fn history_is_bounded() {
    cases::history_is_bounded();
}

#[wasm_bindgen_test]
fn back_from_an_empty_group_returns_the_last_closed() {
    cases::back_from_an_empty_group_returns_the_last_closed();
}

#[wasm_bindgen_test]
fn split_duplicates_the_note_and_moves_focus() {
    cases::split_duplicates_the_note_and_moves_focus();
}

#[wasm_bindgen_test]
fn split_moves_a_board_and_refuses_when_nothing_to_split() {
    cases::split_moves_a_board_and_refuses_when_nothing_to_split();
}

#[wasm_bindgen_test]
fn moving_between_groups_keeps_the_tab_id_and_collapses_empty_groups() {
    cases::moving_between_groups_keeps_the_tab_id_and_collapses_empty_groups();
}

#[wasm_bindgen_test]
fn closing_the_last_tab_of_a_group_removes_the_split() {
    cases::closing_the_last_tab_of_a_group_removes_the_split();
}

#[wasm_bindgen_test]
fn unsplit_merges_without_duplicates() {
    cases::unsplit_merges_without_duplicates();
}

#[wasm_bindgen_test]
fn focus_group_and_ratio() {
    cases::focus_group_and_ratio();
}

#[wasm_bindgen_test]
fn a_tab_open_in_the_other_group_closes_despite_unsaved() {
    cases::a_tab_open_in_the_other_group_closes_despite_unsaved();
}

#[wasm_bindgen_test]
fn every_group_has_its_own_history() {
    cases::every_group_has_its_own_history();
}

#[wasm_bindgen_test]
fn narrow_windows_merge_the_strips_without_touching_the_state() {
    cases::narrow_windows_merge_the_strips_without_touching_the_state();
}

#[wasm_bindgen_test]
fn compact_width_gives_the_list_presentation() {
    cases::compact_width_gives_the_list_presentation();
}

#[wasm_bindgen_test]
fn closing_in_the_merged_presentation_closes_both_tabs() {
    cases::closing_in_the_merged_presentation_closes_both_tabs();
}

#[wasm_bindgen_test]
fn snapshot_and_restore_round_trip() {
    cases::snapshot_and_restore_round_trip();
}

#[wasm_bindgen_test]
fn restore_repairs_a_bad_set() {
    cases::restore_repairs_a_bad_set();
}

#[wasm_bindgen_test]
async fn facts_of_reads_the_index() {
    cases::facts_of_reads_the_index().await;
}

#[wasm_bindgen_test]
async fn restore_with_skips_missing_notes() {
    cases::restore_with_skips_missing_notes().await;
}
