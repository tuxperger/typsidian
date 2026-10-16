//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `slides_native.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::slides_cases`: сбор сведений
//! о страницах и правила колоды платформенно нейтральны, и расхождение между
//! таргетами является дефектом.
#![cfg(target_arch = "wasm32")]

mod common;

use common::slides_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn three_pages_give_rising_source_starts() {
    cases::three_pages_give_rising_source_starts();
}

#[wasm_bindgen_test]
fn source_start_counts_utf16_units() {
    cases::source_start_counts_utf16_units();
}

#[wasm_bindgen_test]
fn pages_without_own_text_have_no_source_start() {
    cases::pages_without_own_text_have_no_source_start();
}

#[wasm_bindgen_test]
fn notes_are_attached_to_their_pages() {
    cases::notes_are_attached_to_their_pages();
}

#[wasm_bindgen_test]
fn logical_slides_come_from_the_markup() {
    cases::logical_slides_come_from_the_markup();
}

#[wasm_bindgen_test]
fn a_document_without_markup_is_numbered_by_pages() {
    cases::a_document_without_markup_is_numbered_by_pages();
}

#[wasm_bindgen_test]
fn collecting_does_not_change_pages() {
    cases::collecting_does_not_change_pages();
}

#[wasm_bindgen_test]
fn start_picks_the_slide_under_the_caret() {
    cases::start_picks_the_slide_under_the_caret();
}

#[wasm_bindgen_test]
fn a_stepped_slide_starts_at_its_first_step() {
    cases::a_stepped_slide_starts_at_its_first_step();
}

#[wasm_bindgen_test]
fn an_empty_deck_is_an_error() {
    cases::an_empty_deck_is_an_error();
}

#[wasm_bindgen_test]
fn caret_before_any_text_and_pages_without_text() {
    cases::caret_before_any_text_and_pages_without_text();
}

#[wasm_bindgen_test]
fn navigate_stops_at_the_borders() {
    cases::navigate_stops_at_the_borders();
}

#[wasm_bindgen_test]
fn go_to_slide_without_logical_numbers_counts_pages() {
    cases::go_to_slide_without_logical_numbers_counts_pages();
}

#[wasm_bindgen_test]
fn go_to_slide_with_logical_numbers_finds_the_first_page() {
    cases::go_to_slide_with_logical_numbers_finds_the_first_page();
}

#[wasm_bindgen_test]
fn broken_logical_numbers_fall_back_to_pages() {
    cases::broken_logical_numbers_fall_back_to_pages();
}

#[wasm_bindgen_test]
fn labels_show_steps_of_logical_slides() {
    cases::labels_show_steps_of_logical_slides();
}

#[wasm_bindgen_test]
fn labels_without_markup_count_pages() {
    cases::labels_without_markup_count_pages();
}

#[wasm_bindgen_test]
fn carry_keeps_the_page_and_clamps_to_the_last() {
    cases::carry_keeps_the_page_and_clamps_to_the_last();
}

#[wasm_bindgen_test]
fn carry_to_no_pages_is_an_error() {
    cases::carry_to_no_pages_is_an_error();
}

#[wasm_bindgen_test]
fn caret_at_page_falls_back_to_the_previous_text() {
    cases::caret_at_page_falls_back_to_the_previous_text();
}

#[wasm_bindgen_test]
fn caret_at_an_image_page_is_the_previous_start() {
    cases::caret_at_an_image_page_is_the_previous_start();
}

#[wasm_bindgen_test]
fn the_keymap_has_clicker_and_arrow_keys() {
    cases::the_keymap_has_clicker_and_arrow_keys();
}

#[wasm_bindgen_test]
fn actions_cross_the_boundary_as_tagged_objects() {
    cases::actions_cross_the_boundary_as_tagged_objects();
}

#[wasm_bindgen_test]
async fn an_included_page_has_no_source_start() {
    cases::an_included_page_has_no_source_start().await;
}
