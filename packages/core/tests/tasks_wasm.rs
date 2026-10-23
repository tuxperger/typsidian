//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `tasks_native.rs`.
//!
//! Оба прогоняют таблицу `common::tasks_cases` (фича 033).
#![cfg(target_arch = "wasm32")]

mod common;

use common::tasks_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn tasks_are_parsed_from_markup_only() {
    cases::tasks_are_parsed_from_markup_only();
}

#[wasm_bindgen_test]
async fn view_collects_all_notes() {
    cases::view_collects_all_notes().await;
}

#[wasm_bindgen_test]
async fn index_rereads_only_changed_notes() {
    cases::index_rereads_only_changed_notes().await;
}

#[wasm_bindgen_test]
async fn toggling_changes_exactly_one_line() {
    cases::toggling_changes_exactly_one_line().await;
}

#[wasm_bindgen_test]
async fn filters_and_due_groups() {
    cases::filters_and_due_groups().await;
}

#[wasm_bindgen_test]
fn toggle_at_caret() {
    cases::toggle_at_caret();
}
