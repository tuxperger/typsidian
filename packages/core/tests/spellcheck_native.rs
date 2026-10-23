//! cargo test — только native (принцип II). Парный файл — `spellcheck_wasm.rs`.
//!
//! Оба прогоняют таблицу `common::spellcheck_cases` (фича 030).
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::spellcheck_cases as cases;

#[test]
fn reference_note_underlines_only_prose() {
    cases::reference_note_underlines_only_prose();
}

#[test]
fn languages_follow_the_alphabet_and_settings() {
    cases::languages_follow_the_alphabet_and_settings();
}

#[test]
fn edge_cases_are_not_false_errors() {
    cases::edge_cases_are_not_false_errors();
}

#[test]
fn suggestions_contain_the_fix() {
    cases::suggestions_contain_the_fix();
}

#[test]
fn only_windows_are_checked() {
    cases::only_windows_are_checked();
}

#[test]
fn most_deliberate_typos_are_found() {
    cases::most_deliberate_typos_are_found();
}

#[tokio::test]
async fn vault_dictionary_adds_and_removes_words() {
    cases::vault_dictionary_adds_and_removes_words().await;
}

#[tokio::test]
async fn configure_reads_dictionary_and_settings() {
    cases::configure_reads_dictionary_and_settings().await;
}
