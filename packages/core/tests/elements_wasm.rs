//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `elements_native.rs`.
#![cfg(target_arch = "wasm32")]

mod common;

use common::element_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn the_schema_matches_typst() {
    cases::the_schema_matches_typst();
}

#[wasm_bindgen_test]
fn every_element_has_a_titled_main_parameter() {
    cases::every_element_has_a_titled_main_parameter();
}

#[wasm_bindgen_test]
fn a_heading_inserts_without_a_form_and_a_code_block_with_one() {
    cases::a_heading_inserts_without_a_form_and_a_code_block_with_one();
}

#[wasm_bindgen_test]
fn every_element_round_trips() {
    cases::every_element_round_trips();
}

#[wasm_bindgen_test]
fn defaults_are_not_written() {
    cases::defaults_are_not_written();
}

#[wasm_bindgen_test]
fn strings_and_content_are_quoted_by_the_core() {
    cases::strings_and_content_are_quoted_by_the_core();
}

#[wasm_bindgen_test]
fn required_and_wrong_values_are_named() {
    cases::required_and_wrong_values_are_named();
}

#[wasm_bindgen_test]
fn labels_are_written_and_checked() {
    cases::labels_are_written_and_checked();
}

#[wasm_bindgen_test]
fn values_are_read_from_the_text() {
    cases::values_are_read_from_the_text();
}

#[wasm_bindgen_test]
fn the_caret_on_the_boundary_is_inside() {
    cases::the_caret_on_the_boundary_is_inside();
}

#[wasm_bindgen_test]
fn offsets_are_utf16() {
    cases::offsets_are_utf16();
}

#[wasm_bindgen_test]
fn old_snippet_text_reads_as_ordinary_values() {
    cases::old_snippet_text_reads_as_ordinary_values();
}

#[wasm_bindgen_test]
fn an_unchanged_form_changes_nothing() {
    cases::an_unchanged_form_changes_nothing();
}

#[wasm_bindgen_test]
fn a_changed_value_is_replaced_in_place() {
    cases::a_changed_value_is_replaced_in_place();
}

#[wasm_bindgen_test]
fn a_new_named_argument_follows_the_call_style() {
    cases::a_new_named_argument_follows_the_call_style();
}

#[wasm_bindgen_test]
fn a_value_reset_to_default_is_removed_with_its_comma() {
    cases::a_value_reset_to_default_is_removed_with_its_comma();
}

#[wasm_bindgen_test]
fn the_inner_element_is_edited_through_the_outer_form() {
    cases::the_inner_element_is_edited_through_the_outer_form();
}

#[wasm_bindgen_test]
fn labels_are_edited() {
    cases::labels_are_edited();
}

#[wasm_bindgen_test]
fn markup_elements_are_edited_in_their_own_syntax() {
    cases::markup_elements_are_edited_in_their_own_syntax();
}

#[wasm_bindgen_test]
fn a_changed_text_is_not_overwritten() {
    cases::a_changed_text_is_not_overwritten();
}

#[wasm_bindgen_test]
fn unknown_arguments_and_comments_survive() {
    cases::unknown_arguments_and_comments_survive();
}

#[wasm_bindgen_test]
fn expressions_are_kept_until_replaced() {
    cases::expressions_are_kept_until_replaced();
}

#[wasm_bindgen_test]
fn ambiguous_markup_is_refused_with_a_reason() {
    cases::ambiguous_markup_is_refused_with_a_reason();
}

#[wasm_bindgen_test]
fn the_innermost_element_is_chosen() {
    cases::the_innermost_element_is_chosen();
}

#[wasm_bindgen_test]
fn math_calls_are_found_inside_formulas() {
    cases::math_calls_are_found_inside_formulas();
}

#[wasm_bindgen_test]
fn initial_values_come_ready_from_the_core() {
    cases::initial_values_come_ready_from_the_core();
}
