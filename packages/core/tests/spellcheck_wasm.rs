//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `spellcheck_native.rs`.
//!
//! Оба прогоняют таблицу `common::spellcheck_cases` (фича 030).
#![cfg(target_arch = "wasm32")]

mod common;

use common::spellcheck_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
fn reference_note_underlines_only_prose() {
    cases::reference_note_underlines_only_prose();
}

#[wasm_bindgen_test]
fn languages_follow_the_alphabet_and_settings() {
    cases::languages_follow_the_alphabet_and_settings();
}

#[wasm_bindgen_test]
fn edge_cases_are_not_false_errors() {
    cases::edge_cases_are_not_false_errors();
}

#[wasm_bindgen_test]
fn suggestions_contain_the_fix() {
    cases::suggestions_contain_the_fix();
}

#[wasm_bindgen_test]
fn only_windows_are_checked() {
    cases::only_windows_are_checked();
}

#[wasm_bindgen_test]
fn most_deliberate_typos_are_found() {
    cases::most_deliberate_typos_are_found();
}

#[wasm_bindgen_test]
async fn vault_dictionary_adds_and_removes_words() {
    cases::vault_dictionary_adds_and_removes_words().await;
}

#[wasm_bindgen_test]
async fn configure_reads_dictionary_and_settings() {
    cases::configure_reads_dictionary_and_settings().await;
}
