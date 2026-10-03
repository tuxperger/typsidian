//! cargo test — только native (принцип II). Парный файл — `export_wasm.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::export_cases`. Здесь же —
//! проверки атомарной записи на настоящем диске: на wasm32 её даёт браузер
//! (`FileSystemWritableFileStream`), а своего кода записи там нет.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::export_cases as cases;
use std::path::Path;
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::native::NativeBackend;

#[tokio::test]
async fn a_note_exports_to_a_pdf_with_the_preview_page_count() {
    cases::a_note_exports_to_a_pdf_with_the_preview_page_count().await;
}

#[tokio::test]
async fn fonts_and_metadata_are_embedded() {
    cases::fonts_and_metadata_are_embedded().await;
}

#[tokio::test]
async fn the_editor_text_wins_over_the_disk() {
    cases::the_editor_text_wins_over_the_disk().await;
}

#[tokio::test]
async fn without_editor_text_the_note_is_read_from_disk() {
    cases::without_editor_text_the_note_is_read_from_disk().await;
}

#[tokio::test]
async fn a_missing_note_is_reported_as_missing() {
    cases::a_missing_note_is_reported_as_missing().await;
}

#[tokio::test]
async fn a_broken_note_writes_nothing_and_keeps_the_old_pdf() {
    cases::a_broken_note_writes_nothing_and_keeps_the_old_pdf().await;
}

#[tokio::test]
async fn a_second_export_replaces_the_first() {
    cases::a_second_export_replaces_the_first().await;
}

#[tokio::test]
async fn the_same_note_exports_to_the_same_bytes() {
    cases::the_same_note_exports_to_the_same_bytes().await;
}

#[tokio::test]
async fn a_folder_in_place_of_the_pdf_is_not_touched() {
    cases::a_folder_in_place_of_the_pdf_is_not_touched().await;
}

#[tokio::test]
async fn a_read_only_vault_refuses_the_write() {
    cases::a_read_only_vault_refuses_the_write().await;
}

#[tokio::test]
async fn the_export_folder_setting_is_honoured() {
    cases::the_export_folder_setting_is_honoured().await;
}

#[tokio::test]
async fn nothing_but_the_pdf_is_written() {
    cases::nothing_but_the_pdf_is_written().await;
}

#[test]
fn a_batch_runs_in_list_order_one_at_a_time() {
    cases::a_batch_runs_in_list_order_one_at_a_time();
}

#[test]
fn the_estimate_appears_after_the_first_note() {
    cases::the_estimate_appears_after_the_first_note();
}

#[test]
fn a_failure_does_not_stop_the_batch_by_default() {
    cases::a_failure_does_not_stop_the_batch_by_default();
}

#[test]
fn stop_on_error_cancels_the_rest() {
    cases::stop_on_error_cancels_the_rest();
}

#[test]
fn cancel_lets_the_current_note_finish() {
    cases::cancel_lets_the_current_note_finish();
}

#[test]
fn only_a_queued_note_can_be_skipped() {
    cases::only_a_queued_note_can_be_skipped();
}

#[test]
fn a_stray_report_changes_nothing() {
    cases::a_stray_report_changes_nothing();
}

#[test]
fn warnings_mark_the_row() {
    cases::warnings_mark_the_row();
}

#[test]
fn an_empty_batch_is_already_over() {
    cases::an_empty_batch_is_already_over();
}

#[test]
fn the_batch_remembers_the_export_folder() {
    cases::the_batch_remembers_the_export_folder();
}

// --- Атомарная запись на диск (research.md §4, SC-006) ----------------------

#[tokio::test]
async fn write_bytes_replaces_the_file_and_leaves_no_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    std::fs::create_dir_all(dir.path().join("export")).unwrap();
    std::fs::write(dir.path().join("export/a.pdf"), b"old").unwrap();

    backend
        .write_bytes(Path::new("export/a.pdf"), b"new content")
        .await
        .unwrap();

    assert_eq!(
        std::fs::read(dir.path().join("export/a.pdf")).unwrap(),
        b"new content"
    );
    let leftovers = std::fs::read_dir(dir.path().join(".typsidian/tmp"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(
        leftovers, 0,
        "временный файл переименован на место, а не оставлен"
    );
    let beside: Vec<_> = std::fs::read_dir(dir.path().join("export"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(beside, vec![std::ffi::OsString::from("a.pdf")]);
}

#[tokio::test]
async fn an_interrupted_write_leaves_the_old_file_and_is_cleaned_on_open() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("export")).unwrap();
    std::fs::write(dir.path().join("export/a.pdf"), b"old").unwrap();
    // Процесс убит между записью временного файла и переименованием: так
    // выглядит vault после сбоя.
    std::fs::create_dir_all(dir.path().join(".typsidian/tmp")).unwrap();
    std::fs::write(
        dir.path().join(".typsidian/tmp/1-0.typsidian-tmp"),
        b"new, half",
    )
    .unwrap();

    let _backend = NativeBackend::open(dir.path()).await.unwrap();

    assert_eq!(
        std::fs::read(dir.path().join("export/a.pdf")).unwrap(),
        b"old"
    );
    assert!(
        !dir.path().join(".typsidian/tmp/1-0.typsidian-tmp").exists(),
        "остаток прерванной записи убран при открытии"
    );
}

#[tokio::test]
async fn a_failed_write_keeps_the_old_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    // Цель — папка: переименовать файл на её место нельзя.
    std::fs::create_dir_all(dir.path().join("export/a.pdf")).unwrap();
    std::fs::write(dir.path().join("export/a.pdf/inside"), b"x").unwrap();

    assert!(backend
        .write_bytes(Path::new("export/a.pdf"), b"new")
        .await
        .is_err());
    assert_eq!(
        std::fs::read(dir.path().join("export/a.pdf/inside")).unwrap(),
        b"x"
    );
    let leftovers = std::fs::read_dir(dir.path().join(".typsidian/tmp"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(leftovers, 0, "неудачная запись убирает свой временный файл");
}

#[tokio::test]
async fn a_typst_error_carries_no_app_message() {
    cases::a_typst_error_carries_no_app_message().await;
}

#[tokio::test]
async fn an_unreadable_dependency_is_an_app_message() {
    cases::an_unreadable_dependency_is_an_app_message().await;
}

#[tokio::test]
async fn a_note_outside_the_vault_is_an_app_message() {
    cases::a_note_outside_the_vault_is_an_app_message().await;
}
