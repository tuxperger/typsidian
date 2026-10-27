//! cargo test — только native (принцип II). Парный файл — `merge_view_wasm.rs`.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::merge_view_cases as cases;

#[test]
fn disputed_segments_match_conflict_hunks() {
    cases::disputed_segments_match_conflict_hunks();
}

#[test]
fn columns_mark_each_side_against_the_base() {
    cases::columns_mark_each_side_against_the_base();
}

#[test]
fn agreed_segments_remember_their_origin() {
    cases::agreed_segments_remember_their_origin();
}

#[test]
fn identical_edits_are_both() {
    cases::identical_edits_are_both();
}

#[test]
fn without_base_sides_are_compared_to_each_other() {
    cases::without_base_sides_are_compared_to_each_other();
}

#[test]
fn trailing_whitespace_counts_as_a_change() {
    cases::trailing_whitespace_counts_as_a_change();
}

#[test]
fn picks_assemble_the_result_with_origins() {
    cases::picks_assemble_the_result_with_origins();
}

#[test]
fn both_without_final_newline_keeps_lines_apart() {
    cases::both_without_final_newline_keeps_lines_apart();
}

#[test]
fn manual_edit_marks_only_touched_lines() {
    cases::manual_edit_marks_only_touched_lines();
}

#[test]
fn five_thousand_lines_stay_consistent() {
    cases::five_thousand_lines_stay_consistent();
}

#[test]
fn versions_over_the_limit_get_no_line_diff() {
    cases::versions_over_the_limit_get_no_line_diff();
}
