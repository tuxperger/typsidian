//! cargo test — только native (принцип II). Парный файл — `install_wasm.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::install_cases`: разбор
//! архива, проверка путей и отпечаток платформенно нейтральны, и расхождение
//! между таргетами является дефектом.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::install_cases as cases;

#[test]
fn a_plain_archive_unpacks() {
    cases::a_plain_archive_unpacks();
}

#[test]
fn a_path_traversal_rejects_the_whole_archive() {
    cases::a_path_traversal_rejects_the_whole_archive();
}

#[test]
fn an_archive_without_a_manifest_is_rejected() {
    cases::an_archive_without_a_manifest_is_rejected();
}

#[test]
fn garbage_is_not_an_archive() {
    cases::garbage_is_not_an_archive();
}

#[test]
fn a_wrapper_folder_is_stripped() {
    cases::a_wrapper_folder_is_stripped();
}

#[test]
fn the_fingerprint_is_deterministic() {
    cases::the_fingerprint_is_deterministic();
}

#[test]
fn only_the_registry_namespace_has_a_registry_url() {
    cases::only_the_registry_namespace_has_a_registry_url();
}

#[test]
fn the_request_carries_nothing_but_the_template_name() {
    cases::the_request_carries_nothing_but_the_template_name();
}

#[test]
fn the_fingerprint_file_is_excluded() {
    cases::the_fingerprint_file_is_excluded();
}

#[tokio::test]
async fn install_writes_the_manifest_last() {
    cases::install_writes_the_manifest_last().await;
}

#[tokio::test]
async fn a_read_only_vault_refuses_installation() {
    cases::a_read_only_vault_refuses_installation().await;
}

#[tokio::test]
async fn a_changed_template_diverges_from_its_fingerprint() {
    cases::a_changed_template_diverges_from_its_fingerprint().await;
}

#[tokio::test]
async fn a_template_with_binary_files_installs() {
    cases::a_template_with_binary_files_installs().await;
}
