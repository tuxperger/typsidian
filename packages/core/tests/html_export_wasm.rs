//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `html_export_native.rs`.
//!
//! Оба прогоняют таблицу `common::html_export_cases` (фича 034).
#![cfg(target_arch = "wasm32")]

mod common;

use common::html_export_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn a_note_becomes_a_self_contained_page() {
    cases::a_note_becomes_a_self_contained_page().await;
}

#[wasm_bindgen_test]
async fn unsupported_elements_become_pictures() {
    cases::unsupported_elements_become_pictures().await;
}

#[wasm_bindgen_test]
async fn a_folder_becomes_a_site() {
    cases::a_folder_becomes_a_site().await;
}

#[wasm_bindgen_test]
async fn a_broken_note_does_not_stop_the_site() {
    cases::a_broken_note_does_not_stop_the_site().await;
}
