//! wasm-bindgen-test — только wasm32 (принцип II). Парный файл —
//! `search_native.rs`:
//! оба вызывают одну и ту же таблицу `common::search_cases`, потому что правила
//! совпадения платформенно нейтральны, а расхождение между таргетами является
//! дефектом, а не особенностью платформы.
#![cfg(target_arch = "wasm32")]

mod common;

use common::search_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn matches_are_grouped_by_folder() {
    cases::matches_are_grouped_by_folder().await;
}

#[wasm_bindgen_test]
async fn case_sensitivity_is_switchable() {
    cases::case_sensitivity_is_switchable().await;
}

#[wasm_bindgen_test]
async fn the_matched_line_arrives_split_into_segments() {
    cases::the_matched_line_arrives_split_into_segments().await;
}

#[wasm_bindgen_test]
async fn context_lines_come_with_the_match() {
    cases::context_lines_come_with_the_match().await;
}

#[wasm_bindgen_test]
async fn boards_are_not_searched() {
    cases::boards_are_not_searched().await;
}

#[wasm_bindgen_test]
async fn an_empty_query_finds_nothing() {
    cases::an_empty_query_finds_nothing().await;
}

#[wasm_bindgen_test]
async fn regex_mode_matches_by_pattern() {
    cases::regex_mode_matches_by_pattern().await;
}

#[wasm_bindgen_test]
async fn without_the_regex_chip_the_pattern_is_literal() {
    cases::without_the_regex_chip_the_pattern_is_literal().await;
}

#[wasm_bindgen_test]
async fn a_broken_pattern_is_reported_with_its_reason() {
    cases::a_broken_pattern_is_reported_with_its_reason().await;
}

#[wasm_bindgen_test]
async fn the_links_to_filter_narrows_the_text_query() {
    cases::the_links_to_filter_narrows_the_text_query().await;
}

#[wasm_bindgen_test]
async fn the_or_mode_widens_the_result() {
    cases::the_or_mode_widens_the_result().await;
}
