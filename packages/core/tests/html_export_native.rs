//! cargo test — только native (принцип II). Парный файл — `html_export_wasm.rs`.
//!
//! Оба прогоняют таблицу `common::html_export_cases` (фича 034).
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::html_export_cases as cases;

#[tokio::test]
async fn a_note_becomes_a_self_contained_page() {
    cases::a_note_becomes_a_self_contained_page().await;
}

#[tokio::test]
async fn unsupported_elements_become_pictures() {
    cases::unsupported_elements_become_pictures().await;
}

#[tokio::test]
async fn a_folder_becomes_a_site() {
    cases::a_folder_becomes_a_site().await;
}

#[tokio::test]
async fn a_broken_note_does_not_stop_the_site() {
    cases::a_broken_note_does_not_stop_the_site().await;
}
