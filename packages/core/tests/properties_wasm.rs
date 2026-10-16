//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `properties_native.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::properties_cases`: разбор,
//! правка, отбор и блок выборки платформенно нейтральны, и расхождение между
//! таргетами является дефектом. Кэш свойств здесь — `MemoryCache`, тот же
//! трейт `IndexCache`, что у SQLite на native.
#![cfg(target_arch = "wasm32")]

mod common;

use common::properties_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn all_value_kinds_are_read() {
    cases::all_value_kinds_are_read();
}

#[wasm_bindgen_test]
fn cyrillic_and_string_keys_keep_their_order() {
    cases::cyrillic_and_string_keys_keep_their_order();
}

#[wasm_bindgen_test]
fn a_non_literal_value_is_unknown_and_blocks_editing() {
    cases::a_non_literal_value_is_unknown_and_blocks_editing();
}

#[wasm_bindgen_test]
fn a_repeated_key_is_ambiguous() {
    cases::a_repeated_key_is_ambiguous();
}

#[wasm_bindgen_test]
fn a_second_record_is_ambiguous() {
    cases::a_second_record_is_ambiguous();
}

#[wasm_bindgen_test]
fn a_non_dictionary_argument_is_ambiguous() {
    cases::a_non_dictionary_argument_is_ambiguous();
}

#[wasm_bindgen_test]
fn a_spread_is_ambiguous() {
    cases::a_spread_is_ambiguous();
}

#[wasm_bindgen_test]
fn the_record_is_found_anywhere_on_the_top_level_only() {
    cases::the_record_is_found_anywhere_on_the_top_level_only();
}

#[wasm_bindgen_test]
fn places_count_utf16_units() {
    cases::places_count_utf16_units();
}

#[wasm_bindgen_test]
fn broken_text_never_panics() {
    cases::broken_text_never_panics();
}

#[wasm_bindgen_test]
fn creating_and_removing_the_last_property_round_trip() {
    cases::creating_and_removing_the_last_property_round_trip();
}

#[wasm_bindgen_test]
fn add_change_and_remove_touch_only_the_record() {
    cases::add_change_and_remove_touch_only_the_record();
}

#[wasm_bindgen_test]
fn multiline_records_keep_their_shape() {
    cases::multiline_records_keep_their_shape();
}

#[wasm_bindgen_test]
fn written_values_read_back_equal() {
    cases::written_values_read_back_equal();
}

#[wasm_bindgen_test]
fn awkward_keys_are_quoted() {
    cases::awkward_keys_are_quoted();
}

#[wasm_bindgen_test]
fn rename_replaces_only_the_key() {
    cases::rename_replaces_only_the_key();
}

#[wasm_bindgen_test]
fn a_no_op_edit_plans_nothing() {
    cases::a_no_op_edit_plans_nothing();
}

#[wasm_bindgen_test]
fn an_ambiguous_record_is_never_rewritten() {
    cases::an_ambiguous_record_is_never_rewritten();
}

#[wasm_bindgen_test]
fn unwritable_values_are_refused_with_the_key() {
    cases::unwritable_values_are_refused_with_the_key();
}

#[wasm_bindgen_test]
fn edits_use_utf16_offsets() {
    cases::edits_use_utf16_offsets();
}

#[wasm_bindgen_test]
fn the_record_builds_and_prints_nothing() {
    cases::the_record_builds_and_prints_nothing();
}

#[wasm_bindgen_test]
fn a_tag_selects_its_notes() {
    cases::a_tag_selects_its_notes();
}

#[wasm_bindgen_test]
fn conditions_combine_with_and() {
    cases::conditions_combine_with_and();
}

#[wasm_bindgen_test]
fn comparisons_work_per_kind() {
    cases::comparisons_work_per_kind();
}

#[wasm_bindgen_test]
fn incomparable_values_do_not_pass() {
    cases::incomparable_values_do_not_pass();
}

#[wasm_bindgen_test]
fn built_in_facts_work_without_properties() {
    cases::built_in_facts_work_without_properties();
}

#[wasm_bindgen_test]
fn sorting_puts_empty_and_incomparable_last() {
    cases::sorting_puts_empty_and_incomparable_last();
}

#[wasm_bindgen_test]
fn limit_cuts_rows_and_reports_truncation() {
    cases::limit_cuts_rows_and_reports_truncation();
}

#[wasm_bindgen_test]
fn typed_words_become_conditions() {
    cases::typed_words_become_conditions();
}

#[wasm_bindgen_test]
fn blocks_are_found_with_an_order_independent_key() {
    cases::blocks_are_found_with_an_order_independent_key();
}

#[wasm_bindgen_test]
fn a_computed_block_is_not_editable() {
    cases::a_computed_block_is_not_editable();
}

#[wasm_bindgen_test]
fn a_rendered_call_reads_back() {
    cases::a_rendered_call_reads_back();
}

#[wasm_bindgen_test]
fn the_import_line_is_added_once() {
    cases::the_import_line_is_added_once();
}

#[wasm_bindgen_test]
fn inputs_carry_rows_by_block_key() {
    cases::inputs_carry_rows_by_block_key();
}

#[wasm_bindgen_test]
fn without_inputs_the_block_prints_a_placeholder() {
    cases::without_inputs_the_block_prints_a_placeholder();
}

#[wasm_bindgen_test]
fn error_messages_never_carry_property_values() {
    cases::error_messages_never_carry_property_values();
}

#[wasm_bindgen_test]
fn indexed_note_paths_use_forward_slashes() {
    cases::indexed_note_paths_use_forward_slashes();
}

#[wasm_bindgen_test]
fn the_select_element_has_a_form_schema() {
    cases::the_select_element_has_a_form_schema();
}

#[wasm_bindgen_test]
fn the_form_writes_reads_and_edits_a_block() {
    cases::the_form_writes_reads_and_edits_a_block();
}

#[wasm_bindgen_test]
fn a_computed_argument_stays_an_expression_in_the_form() {
    cases::a_computed_argument_stays_an_expression_in_the_form();
}

#[wasm_bindgen_test]
async fn the_vault_selection_follows_note_edits() {
    cases::the_vault_selection_follows_note_edits().await;
}

#[wasm_bindgen_test]
async fn search_filters_by_properties() {
    cases::search_filters_by_properties().await;
}

#[wasm_bindgen_test]
async fn reindex_runs_in_steps_and_can_be_cancelled() {
    cases::reindex_runs_in_steps_and_can_be_cancelled().await;
}

#[wasm_bindgen_test]
async fn the_block_builds_into_a_table() {
    cases::the_block_builds_into_a_table().await;
}

#[wasm_bindgen_test]
async fn the_table_follows_the_vault() {
    cases::the_table_follows_the_vault().await;
}

#[wasm_bindgen_test]
async fn an_empty_result_explains_itself() {
    cases::an_empty_result_explains_itself().await;
}

#[wasm_bindgen_test]
async fn the_list_view_and_the_truncation_note() {
    cases::the_list_view_and_the_truncation_note().await;
}

#[wasm_bindgen_test]
async fn a_computed_block_falls_back_to_the_placeholder() {
    cases::a_computed_block_falls_back_to_the_placeholder().await;
}

#[wasm_bindgen_test]
async fn export_builds_with_the_same_inputs() {
    cases::export_builds_with_the_same_inputs().await;
}

#[wasm_bindgen_test]
async fn the_library_is_written_once_and_never_overwrites_edits() {
    cases::the_library_is_written_once_and_never_overwrites_edits().await;
}

#[wasm_bindgen_test]
async fn the_cache_stores_properties() {
    cases::the_cache_stores_properties(typsidian_core::vault::cache::MemoryCache::new()).await;
}
