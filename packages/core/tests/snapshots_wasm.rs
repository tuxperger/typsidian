//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `snapshots_native.rs`.
//!
//! Оба прогоняют таблицу `common::snapshots_cases` (фича 029).
#![cfg(target_arch = "wasm32")]

mod common;

use common::snapshots_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn snapshots_follow_interval_and_skip_duplicates() {
    cases::snapshots_follow_interval_and_skip_duplicates().await;
}

#[wasm_bindgen_test]
async fn history_is_newest_first_with_line_diff() {
    cases::history_is_newest_first_with_line_diff().await;
}

#[wasm_bindgen_test]
async fn restore_goes_to_the_editor_or_the_file_byte_for_byte() {
    cases::restore_goes_to_the_editor_or_the_file_byte_for_byte().await;
}

#[wasm_bindgen_test]
async fn expired_snapshots_are_removed_with_their_files() {
    cases::expired_snapshots_are_removed_with_their_files().await;
}

#[wasm_bindgen_test]
async fn history_follows_renames() {
    cases::history_follows_renames().await;
}

#[wasm_bindgen_test]
async fn deleted_notes_are_recovered_or_renamed() {
    cases::deleted_notes_are_recovered_or_renamed().await;
}

#[wasm_bindgen_test]
async fn usage_and_clear() {
    cases::usage_and_clear().await;
}
