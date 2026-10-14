//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `highlight_native.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::highlight_cases`: подсветка
//! платформенно нейтральна, и расхождение между таргетами является дефектом.
#![cfg(target_arch = "wasm32")]

mod common;

use common::highlight_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn invariants_hold_on_sample_and_edge_texts() {
    cases::invariants_hold_on_sample_and_edge_texts();
}

#[wasm_bindgen_test]
fn offsets_are_utf16_after_cyrillic_and_emoji() {
    cases::offsets_are_utf16_after_cyrillic_and_emoji();
}

#[wasm_bindgen_test]
fn span_crossing_window_edge_is_returned_whole() {
    cases::span_crossing_window_edge_is_returned_whole();
}

#[wasm_bindgen_test]
fn windows_union_matches_separate_windows() {
    cases::windows_union_matches_separate_windows();
}

#[wasm_bindgen_test]
fn empty_window_list_means_whole_text() {
    cases::empty_window_list_means_whole_text();
}

#[wasm_bindgen_test]
fn headings_mark_marker_and_text() {
    cases::headings_mark_marker_and_text();
}

#[wasm_bindgen_test]
fn strong_and_emph_keep_their_markers() {
    cases::strong_and_emph_keep_their_markers();
}

#[wasm_bindgen_test]
fn list_markers_and_terms() {
    cases::list_markers_and_terms();
}

#[wasm_bindgen_test]
fn raw_content_is_not_markup() {
    cases::raw_content_is_not_markup();
}

#[wasm_bindgen_test]
fn labels_refs_links_and_escapes() {
    cases::labels_refs_links_and_escapes();
}

#[wasm_bindgen_test]
fn stray_symbols_do_not_recolor_text() {
    cases::stray_symbols_do_not_recolor_text();
}

#[wasm_bindgen_test]
fn old_highlighter_mistakes_in_markup_are_gone() {
    cases::old_highlighter_mistakes_in_markup_are_gone();
}

#[wasm_bindgen_test]
fn keywords_are_keywords() {
    cases::keywords_are_keywords();
}

#[wasm_bindgen_test]
fn call_with_named_args_and_content() {
    cases::call_with_named_args_and_content();
}

#[wasm_bindgen_test]
fn callee_path_is_a_function_whole() {
    cases::callee_path_is_a_function_whole();
}

#[wasm_bindgen_test]
fn numbers_and_constants() {
    cases::numbers_and_constants();
}

#[wasm_bindgen_test]
fn string_escapes_and_nested_comments() {
    cases::string_escapes_and_nested_comments();
}

#[wasm_bindgen_test]
fn multiline_call_stays_code() {
    cases::multiline_call_stays_code();
}

#[wasm_bindgen_test]
fn math_parts_are_distinct() {
    cases::math_parts_are_distinct();
}

#[wasm_bindgen_test]
fn nesting_three_levels_deep() {
    cases::nesting_three_levels_deep();
}

#[wasm_bindgen_test]
fn old_highlighter_mistakes_in_code_are_gone() {
    cases::old_highlighter_mistakes_in_code_are_gone();
}

#[wasm_bindgen_test]
fn unclosed_pairs_stay_within_paragraph() {
    cases::unclosed_pairs_stay_within_paragraph();
}

#[wasm_bindgen_test]
fn unclosed_comment_and_raw_run_to_the_end() {
    cases::unclosed_comment_and_raw_run_to_the_end();
}

#[wasm_bindgen_test]
fn colors_recover_after_an_error() {
    cases::colors_recover_after_an_error();
}

#[wasm_bindgen_test]
fn very_long_line_is_one_or_two_spans() {
    cases::very_long_line_is_one_or_two_spans();
}

#[wasm_bindgen_test]
fn packed_answer_matches_reference() {
    cases::packed_answer_matches_reference();
}

#[wasm_bindgen_test]
fn session_matches_fresh_parse_after_edits() {
    cases::session_matches_fresh_parse_after_edits();
}

#[wasm_bindgen_test]
fn session_reports_desync() {
    cases::session_reports_desync();
}
