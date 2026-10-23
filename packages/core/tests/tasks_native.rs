//! cargo test — только native (принцип II). Парный файл — `tasks_wasm.rs`.
//!
//! Оба прогоняют таблицу `common::tasks_cases` (фича 033).
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::tasks_cases as cases;

#[test]
fn tasks_are_parsed_from_markup_only() {
    cases::tasks_are_parsed_from_markup_only();
}

#[tokio::test]
async fn view_collects_all_notes() {
    cases::view_collects_all_notes().await;
}

#[tokio::test]
async fn index_rereads_only_changed_notes() {
    cases::index_rereads_only_changed_notes().await;
}

#[tokio::test]
async fn toggling_changes_exactly_one_line() {
    cases::toggling_changes_exactly_one_line().await;
}

#[tokio::test]
async fn filters_and_due_groups() {
    cases::filters_and_due_groups().await;
}

#[test]
fn toggle_at_caret() {
    cases::toggle_at_caret();
}
