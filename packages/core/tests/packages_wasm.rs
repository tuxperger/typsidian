//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `packages_native.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::package_cases`
//! (contracts/package-resolve.md → «Проверяемые сценарии»): отображение
//! спецификации пакета в путь платформенно нейтрально, и расхождение между
//! таргетами является дефектом.
#![cfg(target_arch = "wasm32")]

mod common;

use common::package_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn the_layout_mirrors_the_typst_package_cache() {
    cases::the_layout_mirrors_the_typst_package_cache();
}

#[wasm_bindgen_test]
fn the_namespace_is_not_hardcoded() {
    cases::the_namespace_is_not_hardcoded();
}

#[wasm_bindgen_test]
async fn an_installed_template_is_read() {
    cases::an_installed_template_is_read().await;
}

#[wasm_bindgen_test]
async fn a_missing_template_is_named() {
    cases::a_missing_template_is_named().await;
}

#[wasm_bindgen_test]
async fn a_different_version_is_not_substituted() {
    cases::a_different_version_is_not_substituted().await;
}

#[wasm_bindgen_test]
async fn a_missing_file_inside_an_installed_template_is_a_different_reason() {
    cases::a_missing_file_inside_an_installed_template_is_a_different_reason().await;
}

#[wasm_bindgen_test]
async fn a_document_without_imports_is_unaffected() {
    cases::a_document_without_imports_is_unaffected().await;
}
