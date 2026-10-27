//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `merge_view_native.rs`.
#![cfg(target_arch = "wasm32")]

mod common;

use common::merge_view_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn disputed_segments_match_conflict_hunks() {
    cases::disputed_segments_match_conflict_hunks();
}

#[wasm_bindgen_test]
fn columns_mark_each_side_against_the_base() {
    cases::columns_mark_each_side_against_the_base();
}

#[wasm_bindgen_test]
fn agreed_segments_remember_their_origin() {
    cases::agreed_segments_remember_their_origin();
}

#[wasm_bindgen_test]
fn identical_edits_are_both() {
    cases::identical_edits_are_both();
}

#[wasm_bindgen_test]
fn without_base_sides_are_compared_to_each_other() {
    cases::without_base_sides_are_compared_to_each_other();
}

#[wasm_bindgen_test]
fn trailing_whitespace_counts_as_a_change() {
    cases::trailing_whitespace_counts_as_a_change();
}

#[wasm_bindgen_test]
fn picks_assemble_the_result_with_origins() {
    cases::picks_assemble_the_result_with_origins();
}

#[wasm_bindgen_test]
fn both_without_final_newline_keeps_lines_apart() {
    cases::both_without_final_newline_keeps_lines_apart();
}

#[wasm_bindgen_test]
fn manual_edit_marks_only_touched_lines() {
    cases::manual_edit_marks_only_touched_lines();
}

#[wasm_bindgen_test]
fn five_thousand_lines_stay_consistent() {
    cases::five_thousand_lines_stay_consistent();
}

#[wasm_bindgen_test]
fn versions_over_the_limit_get_no_line_diff() {
    cases::versions_over_the_limit_get_no_line_diff();
}
