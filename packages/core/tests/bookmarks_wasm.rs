//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `bookmarks_native.rs`.
//!
//! Оба прогоняют таблицу `common::bookmarks_cases` (фича 032).
#![cfg(target_arch = "wasm32")]

mod common;

use common::bookmarks_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn four_kinds_are_added_once() {
    cases::four_kinds_are_added_once().await;
}

#[wasm_bindgen_test]
async fn order_and_groups_are_kept() {
    cases::order_and_groups_are_kept().await;
}

#[wasm_bindgen_test]
async fn renames_follow_and_missing_targets_are_marked() {
    cases::renames_follow_and_missing_targets_are_marked().await;
}

#[wasm_bindgen_test]
async fn a_long_list_is_shown_quickly() {
    cases::a_long_list_is_shown_quickly().await;
}
