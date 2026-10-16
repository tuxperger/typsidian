//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `capture_native.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::capture_cases`: правила
//! захвата платформенно нейтральны, и расхождение между таргетами является
//! дефектом.
#![cfg(target_arch = "wasm32")]

mod common;

use common::capture_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn defaults_apply_when_nothing_is_set() {
    cases::defaults_apply_when_nothing_is_set();
}

#[wasm_bindgen_test]
fn unknown_values_fall_back_to_defaults() {
    cases::unknown_values_fall_back_to_defaults();
}

#[wasm_bindgen_test]
fn settings_survive_the_round_trip() {
    cases::settings_survive_the_round_trip();
}

#[wasm_bindgen_test]
fn writing_capture_keeps_other_keys() {
    cases::writing_capture_keeps_other_keys();
}

#[wasm_bindgen_test]
async fn writing_settings_normalizes_and_refuses_bad_paths() {
    cases::writing_settings_normalizes_and_refuses_bad_paths().await;
}

#[wasm_bindgen_test]
fn daily_path_follows_the_three_name_forms() {
    cases::daily_path_follows_the_three_name_forms();
}

#[wasm_bindgen_test]
fn impossible_days_have_no_path() {
    cases::impossible_days_have_no_path();
}

#[wasm_bindgen_test]
fn the_date_of_a_path_is_the_inverse_of_the_path() {
    cases::the_date_of_a_path_is_the_inverse_of_the_path();
}

#[wasm_bindgen_test]
async fn today_creates_the_note_without_a_template() {
    cases::today_creates_the_note_without_a_template().await;
}

#[wasm_bindgen_test]
async fn today_creates_the_note_from_the_template() {
    cases::today_creates_the_note_from_the_template().await;
}

#[wasm_bindgen_test]
async fn a_missing_template_does_not_block_today() {
    cases::a_missing_template_does_not_block_today().await;
}

#[wasm_bindgen_test]
async fn today_twice_is_one_note_and_a_new_day_is_a_new_one() {
    cases::today_twice_is_one_note_and_a_new_day_is_a_new_one().await;
}

#[wasm_bindgen_test]
async fn today_in_a_read_only_vault() {
    cases::today_in_a_read_only_vault().await;
}

#[wasm_bindgen_test]
async fn neighbours_skip_the_gaps() {
    cases::neighbours_skip_the_gaps().await;
}

#[wasm_bindgen_test]
async fn neighbours_work_with_nested_folders() {
    cases::neighbours_work_with_nested_folders().await;
}

#[wasm_bindgen_test]
fn entry_styles_and_time() {
    cases::entry_styles_and_time();
}

#[wasm_bindgen_test]
fn entry_kinds_and_the_note() {
    cases::entry_kinds_and_the_note();
}

#[wasm_bindgen_test]
fn entry_escapes_markup() {
    cases::entry_escapes_markup();
}

#[wasm_bindgen_test]
fn entry_keeps_multiline_text_together() {
    cases::entry_keeps_multiline_text_together();
}

#[wasm_bindgen_test]
fn append_never_changes_the_previous_text() {
    cases::append_never_changes_the_previous_text();
}

#[wasm_bindgen_test]
fn append_edit_is_in_utf16_units() {
    cases::append_edit_is_in_utf16_units();
}

#[wasm_bindgen_test]
fn target_paths() {
    cases::target_paths();
}

#[wasm_bindgen_test]
async fn a_capture_creates_the_inbox_and_keeps_the_order() {
    cases::a_capture_creates_the_inbox_and_keeps_the_order().await;
}

#[wasm_bindgen_test]
async fn a_capture_keeps_the_previous_text() {
    cases::a_capture_keeps_the_previous_text().await;
}

#[wasm_bindgen_test]
async fn a_capture_to_a_chosen_note() {
    cases::a_capture_to_a_chosen_note().await;
}

#[wasm_bindgen_test]
async fn a_capture_to_the_daily_note_creates_it_from_the_template() {
    cases::a_capture_to_the_daily_note_creates_it_from_the_template().await;
}

#[wasm_bindgen_test]
async fn vault_settings_drive_the_entry() {
    cases::vault_settings_drive_the_entry().await;
}

#[wasm_bindgen_test]
async fn a_broken_settings_file_does_not_stop_a_capture() {
    cases::a_broken_settings_file_does_not_stop_a_capture().await;
}

#[wasm_bindgen_test]
async fn an_open_note_gets_a_buffer_edit_and_no_file_write() {
    cases::an_open_note_gets_a_buffer_edit_and_no_file_write().await;
}

#[wasm_bindgen_test]
async fn unsaved_edits_elsewhere_refuse_the_capture() {
    cases::unsaved_edits_elsewhere_refuse_the_capture().await;
}

#[wasm_bindgen_test]
async fn a_conflict_retries_on_the_fresh_text() {
    cases::a_conflict_retries_on_the_fresh_text().await;
}

#[wasm_bindgen_test]
async fn a_persistent_conflict_is_refused_after_three_attempts() {
    cases::a_persistent_conflict_is_refused_after_three_attempts().await;
}

#[wasm_bindgen_test]
async fn a_capture_over_the_size_limit_is_refused() {
    cases::a_capture_over_the_size_limit_is_refused().await;
}

#[wasm_bindgen_test]
async fn read_only_and_unavailable_vaults_refuse_with_a_reason() {
    cases::read_only_and_unavailable_vaults_refuse_with_a_reason().await;
}

#[wasm_bindgen_test]
async fn an_empty_capture_is_refused() {
    cases::an_empty_capture_is_refused().await;
}

#[wasm_bindgen_test]
async fn an_image_becomes_an_attachment_next_to_the_note() {
    cases::an_image_becomes_an_attachment_next_to_the_note().await;
}

#[wasm_bindgen_test]
async fn the_attachment_follows_the_vault_placement_policy() {
    cases::the_attachment_follows_the_vault_placement_policy().await;
}

#[wasm_bindgen_test]
async fn an_image_into_an_open_note_returns_the_markup_edit() {
    cases::an_image_into_an_open_note_returns_the_markup_edit().await;
}

#[wasm_bindgen_test]
async fn attachments_are_not_duplicated_and_names_do_not_collide() {
    cases::attachments_are_not_duplicated_and_names_do_not_collide().await;
}

#[wasm_bindgen_test]
async fn an_unsupported_image_is_refused() {
    cases::an_unsupported_image_is_refused().await;
}

#[wasm_bindgen_test]
fn queue_json_and_outcomes_cross_the_boundary() {
    cases::queue_json_and_outcomes_cross_the_boundary();
}

/// Пустая папка OPFS под уникальным именем (образец — `relocate_wasm.rs`).
async fn fresh_opfs_vault(name: &str) -> wasm_bindgen::JsValue {
    use wasm_bindgen::JsCast;
    let make: js_sys::Function = js_sys::eval(
        "(async (name) => { const root = await navigator.storage.getDirectory(); \
         try { await root.removeEntry(name, { recursive: true }); } catch (e) {} \
         return await root.getDirectoryHandle(name, { create: true }); })",
    )
    .expect("вспомогательный код JS должен собираться")
    .dyn_into()
    .expect("это функция");
    let promise = make
        .call1(
            &wasm_bindgen::JsValue::NULL,
            &wasm_bindgen::JsValue::from_str(name),
        )
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
        .await
        .unwrap()
}

/// Свежий vault в браузере: папки ежедневных заметок нет. Раньше `create` в web
/// папок не заводил (в отличие от native), и «Сегодня» отказывало с `Io` —
/// задача T020. Теперь и `create` в несуществующей папке, и «Сегодня» работают.
#[wasm_bindgen_test]
async fn today_and_create_work_in_a_fresh_browser_vault() {
    use std::path::Path;
    use typsidian_core::capture::{open_daily, CaptureSettings, DailyDate};
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::web::WebBackend;

    let handle = fresh_opfs_vault("capture-fresh").await;
    let mut backend = WebBackend::from_handle("vault".into(), handle)
        .await
        .unwrap();

    backend
        .create(Path::new("папка/вложенная/заметка.typ"))
        .await
        .expect("create заводит недостающие папки, как на native");
    assert_eq!(
        backend
            .read(Path::new("папка/вложенная/заметка.typ"))
            .await
            .unwrap(),
        ""
    );

    let opened = open_daily(
        &mut backend,
        &CaptureSettings::default(),
        &DailyDate {
            year: 2026,
            month: 10,
            day: 16,
        },
    )
    .await
    .expect("«Сегодня» в свежем vault создаёт заметку");
    assert!(opened.created);
    assert!(backend.read(Path::new(&opened.path)).await.is_ok());
}
