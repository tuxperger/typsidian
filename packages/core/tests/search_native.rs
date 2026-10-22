//! cargo test — только native (принцип II). Парный файл — `search_wasm.rs`:
//! оба вызывают одну и ту же таблицу `common::search_cases`, потому что правила
//! совпадения платформенно нейтральны, а расхождение между таргетами является
//! дефектом, а не особенностью платформы.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::search_cases as cases;

#[tokio::test]
async fn matches_are_grouped_by_folder() {
    cases::matches_are_grouped_by_folder().await;
}

#[tokio::test]
async fn case_sensitivity_is_switchable() {
    cases::case_sensitivity_is_switchable().await;
}

#[tokio::test]
async fn the_matched_line_arrives_split_into_segments() {
    cases::the_matched_line_arrives_split_into_segments().await;
}

#[tokio::test]
async fn context_lines_come_with_the_match() {
    cases::context_lines_come_with_the_match().await;
}

#[tokio::test]
async fn boards_are_not_searched() {
    cases::boards_are_not_searched().await;
}

#[tokio::test]
async fn an_empty_query_finds_nothing() {
    cases::an_empty_query_finds_nothing().await;
}

#[tokio::test]
async fn regex_mode_matches_by_pattern() {
    cases::regex_mode_matches_by_pattern().await;
}

#[tokio::test]
async fn without_the_regex_chip_the_pattern_is_literal() {
    cases::without_the_regex_chip_the_pattern_is_literal().await;
}

#[tokio::test]
async fn a_broken_pattern_is_reported_with_its_reason() {
    cases::a_broken_pattern_is_reported_with_its_reason().await;
}

#[tokio::test]
async fn the_links_to_filter_narrows_the_text_query() {
    cases::the_links_to_filter_narrows_the_text_query().await;
}

#[tokio::test]
async fn the_or_mode_widens_the_result() {
    cases::the_or_mode_widens_the_result().await;
}

// Замена по vault (фича 027): общая таблица `common::replace_cases`.

#[tokio::test]
async fn replace_the_preview_lists_every_match_with_before_and_after() {
    common::replace_cases::the_preview_lists_every_match_with_before_and_after().await;
}

#[tokio::test]
async fn replace_groups_expand_only_for_patterns() {
    common::replace_cases::groups_expand_only_for_patterns().await;
}

#[tokio::test]
async fn replace_a_broken_pattern_is_named() {
    common::replace_cases::a_broken_pattern_is_named().await;
}

#[tokio::test]
async fn replace_too_many_matches_are_refused_with_the_count() {
    common::replace_cases::too_many_matches_are_refused_with_the_count().await;
}

#[tokio::test]
async fn replace_only_selected_matches_are_written() {
    common::replace_cases::only_selected_matches_are_written().await;
}

#[tokio::test]
async fn replace_a_note_changed_after_the_preview_is_skipped() {
    common::replace_cases::a_note_changed_after_the_preview_is_skipped().await;
}

#[tokio::test]
async fn replace_an_open_note_gets_one_editor_edit() {
    common::replace_cases::an_open_note_gets_one_editor_edit().await;
}

#[tokio::test]
async fn replace_unsaved_edits_elsewhere_block_the_write() {
    common::replace_cases::unsaved_edits_elsewhere_block_the_write().await;
}

#[tokio::test]
async fn replace_undo_restores_untouched_notes_and_spares_changed_ones() {
    common::replace_cases::undo_restores_untouched_notes_and_spares_changed_ones().await;
}

#[tokio::test]
async fn replace_undo_in_an_open_note_is_an_editor_edit() {
    common::replace_cases::undo_in_an_open_note_is_an_editor_edit().await;
}
