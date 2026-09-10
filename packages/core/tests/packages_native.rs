//! cargo test — только native (принцип II). Парный файл — `packages_wasm.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::package_cases`
//! (contracts/package-resolve.md → «Проверяемые сценарии»): отображение
//! спецификации пакета в путь платформенно нейтрально, и расхождение между
//! таргетами является дефектом.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::package_cases as cases;

#[test]
fn the_layout_mirrors_the_typst_package_cache() {
    cases::the_layout_mirrors_the_typst_package_cache();
}

#[test]
fn the_namespace_is_not_hardcoded() {
    cases::the_namespace_is_not_hardcoded();
}

#[tokio::test]
async fn an_installed_template_is_read() {
    cases::an_installed_template_is_read().await;
}

#[tokio::test]
async fn a_missing_template_is_named() {
    cases::a_missing_template_is_named().await;
}

#[tokio::test]
async fn a_different_version_is_not_substituted() {
    cases::a_different_version_is_not_substituted().await;
}

#[tokio::test]
async fn a_missing_file_inside_an_installed_template_is_a_different_reason() {
    cases::a_missing_file_inside_an_installed_template_is_a_different_reason().await;
}

#[tokio::test]
async fn a_document_without_imports_is_unaffected() {
    cases::a_document_without_imports_is_unaffected().await;
}
