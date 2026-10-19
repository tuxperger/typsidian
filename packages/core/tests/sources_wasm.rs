//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `sources_native.rs`.
//!
//! Та же таблица `common::sources_cases`.
#![cfg(target_arch = "wasm32")]

mod common;

use common::sources_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn the_reading_crates_behave_as_researched() {
    cases::the_reading_crates_behave_as_researched();
}

#[wasm_bindgen_test]
fn structure_finds_cites_and_bibliographies() {
    cases::structure_finds_cites_and_bibliographies();
}

#[wasm_bindgen_test]
fn mode_at_tells_markup_math_code_string_and_comment() {
    cases::mode_at_tells_markup_math_code_string_and_comment();
}

#[wasm_bindgen_test]
fn bib_and_yaml_give_the_same_source() {
    cases::bib_and_yaml_give_the_same_source();
}

#[wasm_bindgen_test]
fn a_broken_bib_entry_keeps_the_rest_and_is_named() {
    cases::a_broken_bib_entry_keeps_the_rest_and_is_named();
}

#[wasm_bindgen_test]
fn too_many_problems_are_folded() {
    cases::too_many_problems_are_folded();
}

#[wasm_bindgen_test]
fn reading_never_panics_on_arbitrary_text() {
    cases::reading_never_panics_on_arbitrary_text();
}

#[wasm_bindgen_test]
async fn the_library_recognizes_files_and_rereads_only_changes() {
    cases::the_library_recognizes_files_and_rereads_only_changes().await;
}

#[wasm_bindgen_test]
async fn a_removed_file_leaves_the_summary() {
    cases::a_removed_file_leaves_the_summary().await;
}

#[wasm_bindgen_test]
async fn search_matches_every_word_and_orders_by_key_then_family() {
    cases::search_matches_every_word_and_orders_by_key_then_family().await;
}

#[wasm_bindgen_test]
async fn search_over_ten_thousand_sources_keeps_its_order() {
    cases::search_over_ten_thousand_sources_keeps_its_order().await;
}

#[wasm_bindgen_test]
fn every_citation_form_reads_back_as_one_citation() {
    cases::every_citation_form_reads_back_as_one_citation();
}

#[wasm_bindgen_test]
fn strings_comments_and_unknown_keys_are_refused() {
    cases::strings_comments_and_unknown_keys_are_refused();
}

#[wasm_bindgen_test]
fn the_bibliography_offer_adds_extends_or_stays_quiet() {
    cases::the_bibliography_offer_adds_extends_or_stays_quiet();
}

#[wasm_bindgen_test]
async fn creating_the_source_file_refuses_busy_names_and_read_only() {
    cases::creating_the_source_file_refuses_busy_names_and_read_only().await;
}

#[wasm_bindgen_test]
async fn a_bad_source_file_setting_is_refused_before_writing() {
    cases::a_bad_source_file_setting_is_refused_before_writing().await;
}

#[wasm_bindgen_test]
async fn the_citation_index_scans_in_portions_and_answers() {
    cases::the_citation_index_scans_in_portions_and_answers().await;
}

#[wasm_bindgen_test]
async fn cancel_keeps_what_was_scanned_and_a_rescan_skips_it() {
    cases::cancel_keeps_what_was_scanned_and_a_rescan_skips_it().await;
}

#[wasm_bindgen_test]
async fn single_notes_follow_vault_events() {
    cases::single_notes_follow_vault_events().await;
}

#[wasm_bindgen_test]
async fn citation_marks_follow_the_research_table() {
    cases::citation_marks_follow_the_research_table().await;
}

#[wasm_bindgen_test]
async fn writing_a_source_file_is_announced() {
    cases::writing_a_source_file_is_announced().await;
}

#[wasm_bindgen_test]
async fn the_whole_path_runs_without_a_network() {
    cases::the_whole_path_runs_without_a_network().await;
}
