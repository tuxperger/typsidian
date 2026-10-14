//! cargo test — только native (принцип II). Парный файл — `highlight_wasm.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::highlight_cases`: подсветка
//! платформенно нейтральна, и расхождение между таргетами является дефектом.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::highlight_cases as cases;

#[test]
fn invariants_hold_on_sample_and_edge_texts() {
    cases::invariants_hold_on_sample_and_edge_texts();
}

#[test]
fn offsets_are_utf16_after_cyrillic_and_emoji() {
    cases::offsets_are_utf16_after_cyrillic_and_emoji();
}

#[test]
fn span_crossing_window_edge_is_returned_whole() {
    cases::span_crossing_window_edge_is_returned_whole();
}

#[test]
fn windows_union_matches_separate_windows() {
    cases::windows_union_matches_separate_windows();
}

#[test]
fn empty_window_list_means_whole_text() {
    cases::empty_window_list_means_whole_text();
}

#[test]
fn headings_mark_marker_and_text() {
    cases::headings_mark_marker_and_text();
}

#[test]
fn strong_and_emph_keep_their_markers() {
    cases::strong_and_emph_keep_their_markers();
}

#[test]
fn list_markers_and_terms() {
    cases::list_markers_and_terms();
}

#[test]
fn raw_content_is_not_markup() {
    cases::raw_content_is_not_markup();
}

#[test]
fn labels_refs_links_and_escapes() {
    cases::labels_refs_links_and_escapes();
}

#[test]
fn stray_symbols_do_not_recolor_text() {
    cases::stray_symbols_do_not_recolor_text();
}

#[test]
fn old_highlighter_mistakes_in_markup_are_gone() {
    cases::old_highlighter_mistakes_in_markup_are_gone();
}

#[test]
fn keywords_are_keywords() {
    cases::keywords_are_keywords();
}

#[test]
fn call_with_named_args_and_content() {
    cases::call_with_named_args_and_content();
}

#[test]
fn callee_path_is_a_function_whole() {
    cases::callee_path_is_a_function_whole();
}

#[test]
fn numbers_and_constants() {
    cases::numbers_and_constants();
}

#[test]
fn string_escapes_and_nested_comments() {
    cases::string_escapes_and_nested_comments();
}

#[test]
fn multiline_call_stays_code() {
    cases::multiline_call_stays_code();
}

#[test]
fn math_parts_are_distinct() {
    cases::math_parts_are_distinct();
}

#[test]
fn nesting_three_levels_deep() {
    cases::nesting_three_levels_deep();
}

#[test]
fn old_highlighter_mistakes_in_code_are_gone() {
    cases::old_highlighter_mistakes_in_code_are_gone();
}

#[test]
fn unclosed_pairs_stay_within_paragraph() {
    cases::unclosed_pairs_stay_within_paragraph();
}

#[test]
fn unclosed_comment_and_raw_run_to_the_end() {
    cases::unclosed_comment_and_raw_run_to_the_end();
}

#[test]
fn colors_recover_after_an_error() {
    cases::colors_recover_after_an_error();
}

#[test]
fn very_long_line_is_one_or_two_spans() {
    cases::very_long_line_is_one_or_two_spans();
}

#[test]
fn packed_answer_matches_reference() {
    cases::packed_answer_matches_reference();
}

#[test]
fn session_matches_fresh_parse_after_edits() {
    cases::session_matches_fresh_parse_after_edits();
}

#[test]
fn session_reports_desync() {
    cases::session_reports_desync();
}

/// Перечень ролей интерфейса совпадает с ядром по порядку (contracts/core-highlight-api.md).
/// Только native: сверяются два файла репозитория, а в браузере файлов нет.
#[test]
fn roles_match_frontend_table() {
    use typsidian_core::highlight::SyntaxRole;

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../ui/src/editor/highlight/roles.ts"
    );
    let source = std::fs::read_to_string(path).expect("ui/src/editor/highlight/roles.ts");
    let start = source
        .find("SYNTAX_ROLES = [")
        .expect("массив SYNTAX_ROLES");
    let end = start + source[start..].find(']').expect("конец массива");
    let names: Vec<&str> = source[start..end].split('"').skip(1).step_by(2).collect();
    let core: Vec<String> = SyntaxRole::ALL
        .iter()
        .map(|role| format!("{role:?}"))
        .collect();
    assert_eq!(
        names, core,
        "порядок ролей в roles.ts должен совпадать с SyntaxRole"
    );
}
