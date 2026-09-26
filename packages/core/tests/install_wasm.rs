//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `install_native.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::install_cases`: разбор
//! архива, проверка путей и отпечаток платформенно нейтральны, и расхождение
//! между таргетами является дефектом.
#![cfg(target_arch = "wasm32")]

mod common;

use common::install_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn a_plain_archive_unpacks() {
    cases::a_plain_archive_unpacks();
}

#[wasm_bindgen_test]
fn a_path_traversal_rejects_the_whole_archive() {
    cases::a_path_traversal_rejects_the_whole_archive();
}

#[wasm_bindgen_test]
fn an_archive_without_a_manifest_is_rejected() {
    cases::an_archive_without_a_manifest_is_rejected();
}

#[wasm_bindgen_test]
fn garbage_is_not_an_archive() {
    cases::garbage_is_not_an_archive();
}

#[wasm_bindgen_test]
fn a_wrapper_folder_is_stripped() {
    cases::a_wrapper_folder_is_stripped();
}

#[wasm_bindgen_test]
fn the_fingerprint_is_deterministic() {
    cases::the_fingerprint_is_deterministic();
}

#[wasm_bindgen_test]
fn only_the_registry_namespace_has_a_registry_url() {
    cases::only_the_registry_namespace_has_a_registry_url();
}

#[wasm_bindgen_test]
fn the_request_carries_nothing_but_the_template_name() {
    cases::the_request_carries_nothing_but_the_template_name();
}

#[wasm_bindgen_test]
fn the_fingerprint_file_is_excluded() {
    cases::the_fingerprint_file_is_excluded();
}

#[wasm_bindgen_test]
async fn install_writes_the_manifest_last() {
    cases::install_writes_the_manifest_last().await;
}

#[wasm_bindgen_test]
async fn a_read_only_vault_refuses_installation() {
    cases::a_read_only_vault_refuses_installation().await;
}

#[wasm_bindgen_test]
async fn a_changed_template_diverges_from_its_fingerprint() {
    cases::a_changed_template_diverges_from_its_fingerprint().await;
}

#[wasm_bindgen_test]
async fn a_template_with_binary_files_installs() {
    cases::a_template_with_binary_files_installs().await;
}

#[wasm_bindgen_test]
async fn a_neighbouring_version_is_left_alone() {
    cases::a_neighbouring_version_is_left_alone().await;
}

#[wasm_bindgen_test]
async fn tampering_is_noticed_without_reinstalling() {
    cases::tampering_is_noticed_without_reinstalling().await;
}

#[wasm_bindgen_test]
async fn a_hand_placed_template_cannot_be_verified() {
    cases::a_hand_placed_template_cannot_be_verified().await;
}

#[wasm_bindgen_test]
async fn the_registry_lists_installed_templates() {
    cases::the_registry_lists_installed_templates().await;
}

#[wasm_bindgen_test]
async fn an_installed_template_is_not_fetched_again() {
    cases::an_installed_template_is_not_fetched_again().await;
}

#[wasm_bindgen_test]
async fn a_note_without_imports_never_reaches_for_the_network() {
    cases::a_note_without_imports_never_reaches_for_the_network().await;
}

#[wasm_bindgen_test]
async fn the_fetch_path_sends_nothing_about_the_note() {
    cases::the_fetch_path_sends_nothing_about_the_note().await;
}

#[wasm_bindgen_test]
async fn disabled_auto_download_makes_no_attempt() {
    cases::disabled_auto_download_makes_no_attempt().await;
}

#[wasm_bindgen_test]
async fn enabled_auto_download_makes_an_attempt() {
    cases::enabled_auto_download_makes_an_attempt().await;
}

#[wasm_bindgen_test]
async fn an_explicit_install_ignores_the_switch() {
    cases::an_explicit_install_ignores_the_switch().await;
}

#[wasm_bindgen_test]
async fn a_user_url_is_never_replaced_by_the_registry() {
    cases::a_user_url_is_never_replaced_by_the_registry().await;
}

#[wasm_bindgen_test]
async fn a_local_namespace_is_only_reported_missing() {
    cases::a_local_namespace_is_only_reported_missing().await;
}

#[wasm_bindgen_test]
async fn compiling_reports_a_tampered_template_without_reinstalling() {
    cases::compiling_reports_a_tampered_template_without_reinstalling().await;
}
