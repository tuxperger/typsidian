//! cargo test — только native (принцип II). Парный файл — `embeds_wasm.rs`.
//!
//! Оба прогоняют таблицу `common::embeds_cases` (фича 031).
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::embeds_cases as cases;

#[test]
fn targets_sections_and_labels_are_cut() {
    cases::targets_sections_and_labels_are_cut();
}

#[tokio::test]
async fn preview_shows_the_embedded_section() {
    cases::preview_shows_the_embedded_section().await;
}

#[tokio::test]
async fn export_contains_the_embedded_text() {
    cases::export_contains_the_embedded_text().await;
}

#[tokio::test]
async fn completion_offers_notes_sections_and_labels() {
    cases::completion_offers_notes_sections_and_labels().await;
}

#[tokio::test]
async fn insert_command_writes_library_and_import() {
    cases::insert_command_writes_library_and_import().await;
}

#[tokio::test]
async fn broken_embeds_answer_with_a_reason() {
    cases::broken_embeds_answer_with_a_reason().await;
}

#[tokio::test]
async fn embeds_are_links_and_broken_ones_are_marked() {
    cases::embeds_are_links_and_broken_ones_are_marked().await;
}
