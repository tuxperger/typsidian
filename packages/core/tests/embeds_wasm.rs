//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `embeds_native.rs`.
//!
//! Оба прогоняют таблицу `common::embeds_cases` (фича 031).
#![cfg(target_arch = "wasm32")]

mod common;

use common::embeds_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn targets_sections_and_labels_are_cut() {
    cases::targets_sections_and_labels_are_cut();
}

#[wasm_bindgen_test]
async fn preview_shows_the_embedded_section() {
    cases::preview_shows_the_embedded_section().await;
}

#[wasm_bindgen_test]
async fn export_contains_the_embedded_text() {
    cases::export_contains_the_embedded_text().await;
}

#[wasm_bindgen_test]
async fn completion_offers_notes_sections_and_labels() {
    cases::completion_offers_notes_sections_and_labels().await;
}

#[wasm_bindgen_test]
async fn insert_command_writes_library_and_import() {
    cases::insert_command_writes_library_and_import().await;
}

#[wasm_bindgen_test]
async fn broken_embeds_answer_with_a_reason() {
    cases::broken_embeds_answer_with_a_reason().await;
}

#[wasm_bindgen_test]
async fn embeds_are_links_and_broken_ones_are_marked() {
    cases::embeds_are_links_and_broken_ones_are_marked().await;
}
