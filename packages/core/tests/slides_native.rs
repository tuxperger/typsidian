//! cargo test — только native (принцип II). Парный файл — `slides_wasm.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::slides_cases`: сбор сведений
//! о страницах и правила колоды платформенно нейтральны, и расхождение между
//! таргетами является дефектом.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::slides_cases as cases;

#[test]
fn three_pages_give_rising_source_starts() {
    cases::three_pages_give_rising_source_starts();
}

#[test]
fn source_start_counts_utf16_units() {
    cases::source_start_counts_utf16_units();
}

#[test]
fn pages_without_own_text_have_no_source_start() {
    cases::pages_without_own_text_have_no_source_start();
}

#[test]
fn notes_are_attached_to_their_pages() {
    cases::notes_are_attached_to_their_pages();
}

#[test]
fn logical_slides_come_from_the_markup() {
    cases::logical_slides_come_from_the_markup();
}

#[test]
fn a_document_without_markup_is_numbered_by_pages() {
    cases::a_document_without_markup_is_numbered_by_pages();
}

#[test]
fn collecting_does_not_change_pages() {
    cases::collecting_does_not_change_pages();
}

#[test]
fn start_picks_the_slide_under_the_caret() {
    cases::start_picks_the_slide_under_the_caret();
}

#[test]
fn a_stepped_slide_starts_at_its_first_step() {
    cases::a_stepped_slide_starts_at_its_first_step();
}

#[test]
fn an_empty_deck_is_an_error() {
    cases::an_empty_deck_is_an_error();
}

#[test]
fn caret_before_any_text_and_pages_without_text() {
    cases::caret_before_any_text_and_pages_without_text();
}

#[test]
fn navigate_stops_at_the_borders() {
    cases::navigate_stops_at_the_borders();
}

#[test]
fn go_to_slide_without_logical_numbers_counts_pages() {
    cases::go_to_slide_without_logical_numbers_counts_pages();
}

#[test]
fn go_to_slide_with_logical_numbers_finds_the_first_page() {
    cases::go_to_slide_with_logical_numbers_finds_the_first_page();
}

#[test]
fn broken_logical_numbers_fall_back_to_pages() {
    cases::broken_logical_numbers_fall_back_to_pages();
}

#[test]
fn labels_show_steps_of_logical_slides() {
    cases::labels_show_steps_of_logical_slides();
}

#[test]
fn labels_without_markup_count_pages() {
    cases::labels_without_markup_count_pages();
}

#[test]
fn carry_keeps_the_page_and_clamps_to_the_last() {
    cases::carry_keeps_the_page_and_clamps_to_the_last();
}

#[test]
fn carry_to_no_pages_is_an_error() {
    cases::carry_to_no_pages_is_an_error();
}

#[test]
fn caret_at_page_falls_back_to_the_previous_text() {
    cases::caret_at_page_falls_back_to_the_previous_text();
}

#[test]
fn caret_at_an_image_page_is_the_previous_start() {
    cases::caret_at_an_image_page_is_the_previous_start();
}

#[test]
fn the_keymap_has_clicker_and_arrow_keys() {
    cases::the_keymap_has_clicker_and_arrow_keys();
}

#[test]
fn actions_cross_the_boundary_as_tagged_objects() {
    cases::actions_cross_the_boundary_as_tagged_objects();
}

#[tokio::test]
async fn an_included_page_has_no_source_start() {
    cases::an_included_page_has_no_source_start().await;
}
