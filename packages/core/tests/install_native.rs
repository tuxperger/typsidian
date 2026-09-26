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

#[tokio::test]
async fn a_neighbouring_version_is_left_alone() {
    cases::a_neighbouring_version_is_left_alone().await;
}

#[tokio::test]
async fn tampering_is_noticed_without_reinstalling() {
    cases::tampering_is_noticed_without_reinstalling().await;
}

#[tokio::test]
async fn a_hand_placed_template_cannot_be_verified() {
    cases::a_hand_placed_template_cannot_be_verified().await;
}

#[tokio::test]
async fn the_registry_lists_installed_templates() {
    cases::the_registry_lists_installed_templates().await;
}

#[tokio::test]
async fn an_installed_template_is_not_fetched_again() {
    cases::an_installed_template_is_not_fetched_again().await;
}

#[tokio::test]
async fn a_note_without_imports_never_reaches_for_the_network() {
    cases::a_note_without_imports_never_reaches_for_the_network().await;
}

#[tokio::test]
async fn the_fetch_path_sends_nothing_about_the_note() {
    cases::the_fetch_path_sends_nothing_about_the_note().await;
}

#[tokio::test]
async fn disabled_auto_download_makes_no_attempt() {
    cases::disabled_auto_download_makes_no_attempt().await;
}

#[tokio::test]
async fn enabled_auto_download_makes_an_attempt() {
    cases::enabled_auto_download_makes_an_attempt().await;
}

#[tokio::test]
async fn an_explicit_install_ignores_the_switch() {
    cases::an_explicit_install_ignores_the_switch().await;
}

#[tokio::test]
async fn a_user_url_is_never_replaced_by_the_registry() {
    cases::a_user_url_is_never_replaced_by_the_registry().await;
}

#[tokio::test]
async fn a_local_namespace_is_only_reported_missing() {
    cases::a_local_namespace_is_only_reported_missing().await;
}

#[tokio::test]
async fn compiling_reports_a_tampered_template_without_reinstalling() {
    cases::compiling_reports_a_tampered_template_without_reinstalling().await;
}
