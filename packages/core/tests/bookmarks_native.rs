//! cargo test — только native (принцип II). Парный файл — `bookmarks_wasm.rs`.
//!
//! Оба прогоняют таблицу `common::bookmarks_cases` (фича 032).
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::bookmarks_cases as cases;

#[tokio::test]
async fn four_kinds_are_added_once() {
    cases::four_kinds_are_added_once().await;
}

#[tokio::test]
async fn order_and_groups_are_kept() {
    cases::order_and_groups_are_kept().await;
}

#[tokio::test]
async fn renames_follow_and_missing_targets_are_marked() {
    cases::renames_follow_and_missing_targets_are_marked().await;
}

#[tokio::test]
async fn a_long_list_is_shown_quickly() {
    // SC-001: вид из 200 закладок (100 разделов читают свои заметки) ≤ 50 мс в
    // выпуске; отладочная сборка тестов — с десятикратным запасом.
    let started = std::time::Instant::now();
    cases::a_long_list_is_shown_quickly().await;
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_millis(500), "{took:?}");
}
