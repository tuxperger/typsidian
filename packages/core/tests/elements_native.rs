//! cargo test — только native (принцип II). Парный файл — `elements_wasm.rs`.
//!
//! Оба прогоняют одну таблицу из `common::element_cases`: функции параметров
//! элементов чистые, и расхождение между таргетами — дефект.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::element_cases as cases;

#[test]
fn every_parameter_comes_from_typst() {
    cases::every_parameter_comes_from_typst();
}

#[test]
fn titles_and_hints_come_from_typst() {
    cases::titles_and_hints_come_from_typst();
}

#[test]
fn every_catalog_element_has_a_main_parameter() {
    cases::every_catalog_element_has_a_main_parameter();
}

#[test]
fn a_heading_inserts_without_a_form_and_a_code_block_with_one() {
    cases::a_heading_inserts_without_a_form_and_a_code_block_with_one();
}

#[test]
fn every_element_round_trips() {
    cases::every_element_round_trips();
}

#[test]
fn defaults_are_not_written() {
    cases::defaults_are_not_written();
}

#[test]
fn strings_and_content_are_quoted_by_the_core() {
    cases::strings_and_content_are_quoted_by_the_core();
}

#[test]
fn required_and_wrong_values_are_named() {
    cases::required_and_wrong_values_are_named();
}

#[test]
fn labels_are_written_and_checked() {
    cases::labels_are_written_and_checked();
}

#[test]
fn values_are_read_from_the_text() {
    cases::values_are_read_from_the_text();
}

#[test]
fn the_caret_on_the_boundary_is_inside() {
    cases::the_caret_on_the_boundary_is_inside();
}

#[test]
fn offsets_are_utf16() {
    cases::offsets_are_utf16();
}

#[test]
fn old_snippet_text_reads_as_ordinary_values() {
    cases::old_snippet_text_reads_as_ordinary_values();
}

#[test]
fn an_unchanged_form_changes_nothing() {
    cases::an_unchanged_form_changes_nothing();
}

#[test]
fn a_changed_value_is_replaced_in_place() {
    cases::a_changed_value_is_replaced_in_place();
}

#[test]
fn a_new_named_argument_follows_the_call_style() {
    cases::a_new_named_argument_follows_the_call_style();
}

#[test]
fn a_value_reset_to_default_is_removed_with_its_comma() {
    cases::a_value_reset_to_default_is_removed_with_its_comma();
}

#[test]
fn the_inner_element_is_edited_through_the_outer_form() {
    cases::the_inner_element_is_edited_through_the_outer_form();
}

#[test]
fn labels_are_edited() {
    cases::labels_are_edited();
}

#[test]
fn markup_elements_are_edited_in_their_own_syntax() {
    cases::markup_elements_are_edited_in_their_own_syntax();
}

#[test]
fn a_changed_text_is_not_overwritten() {
    cases::a_changed_text_is_not_overwritten();
}

#[test]
fn unknown_arguments_and_comments_survive() {
    cases::unknown_arguments_and_comments_survive();
}

#[test]
fn expressions_are_kept_until_replaced() {
    cases::expressions_are_kept_until_replaced();
}

#[test]
fn ambiguous_markup_is_refused_with_a_reason() {
    cases::ambiguous_markup_is_refused_with_a_reason();
}

#[test]
fn the_innermost_element_is_chosen() {
    cases::the_innermost_element_is_chosen();
}

#[test]
fn typst_description_gaps_are_corrected() {
    cases::typst_description_gaps_are_corrected();
}

#[test]
fn math_calls_are_found_inside_formulas() {
    cases::math_calls_are_found_inside_formulas();
}

#[test]
fn initial_values_come_ready_from_the_core() {
    cases::initial_values_come_ready_from_the_core();
}
