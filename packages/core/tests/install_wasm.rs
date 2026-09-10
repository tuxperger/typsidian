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
