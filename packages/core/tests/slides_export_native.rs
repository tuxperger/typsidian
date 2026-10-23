//! cargo test — только native (принцип II). Парный файл — `slides_export_wasm.rs`.
//!
//! Оба прогоняют таблицу `common::slides_export_cases` (фича 035).
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::slides_export_cases as cases;

#[tokio::test]
async fn pdf_has_a_page_per_slide_or_step() {
    cases::pdf_has_a_page_per_slide_or_step().await;
}

#[tokio::test]
async fn pdf_with_notes_has_slides_and_notes() {
    cases::pdf_with_notes_has_slides_and_notes().await;
}

#[tokio::test]
async fn pdfpc_file_lies_next_to_the_pdf() {
    cases::pdfpc_file_lies_next_to_the_pdf().await;
}

#[tokio::test]
async fn a_broken_deck_writes_nothing() {
    cases::a_broken_deck_writes_nothing().await;
}

#[tokio::test]
async fn images_are_numbered_files() {
    cases::images_are_numbered_files().await;
}
