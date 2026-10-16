//! cargo test — только native (принцип II). Парный файл — `properties_wasm.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::properties_cases`: разбор,
//! правка, отбор и блок выборки платформенно нейтральны, и расхождение между
//! таргетами является дефектом. Кэш свойств здесь — SQLite, тот же трейт
//! `IndexCache`, что у `MemoryCache` на wasm.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::properties_cases as cases;

#[test]
fn all_value_kinds_are_read() {
    cases::all_value_kinds_are_read();
}

#[test]
fn cyrillic_and_string_keys_keep_their_order() {
    cases::cyrillic_and_string_keys_keep_their_order();
}

#[test]
fn a_non_literal_value_is_unknown_and_blocks_editing() {
    cases::a_non_literal_value_is_unknown_and_blocks_editing();
}

#[test]
fn a_repeated_key_is_ambiguous() {
    cases::a_repeated_key_is_ambiguous();
}

#[test]
fn a_second_record_is_ambiguous() {
    cases::a_second_record_is_ambiguous();
}

#[test]
fn a_non_dictionary_argument_is_ambiguous() {
    cases::a_non_dictionary_argument_is_ambiguous();
}

#[test]
fn a_spread_is_ambiguous() {
    cases::a_spread_is_ambiguous();
}

#[test]
fn the_record_is_found_anywhere_on_the_top_level_only() {
    cases::the_record_is_found_anywhere_on_the_top_level_only();
}

#[test]
fn places_count_utf16_units() {
    cases::places_count_utf16_units();
}

#[test]
fn broken_text_never_panics() {
    cases::broken_text_never_panics();
}

#[test]
fn creating_and_removing_the_last_property_round_trip() {
    cases::creating_and_removing_the_last_property_round_trip();
}

#[test]
fn add_change_and_remove_touch_only_the_record() {
    cases::add_change_and_remove_touch_only_the_record();
}

#[test]
fn multiline_records_keep_their_shape() {
    cases::multiline_records_keep_their_shape();
}

#[test]
fn written_values_read_back_equal() {
    cases::written_values_read_back_equal();
}

#[test]
fn awkward_keys_are_quoted() {
    cases::awkward_keys_are_quoted();
}

#[test]
fn rename_replaces_only_the_key() {
    cases::rename_replaces_only_the_key();
}

#[test]
fn a_no_op_edit_plans_nothing() {
    cases::a_no_op_edit_plans_nothing();
}

#[test]
fn an_ambiguous_record_is_never_rewritten() {
    cases::an_ambiguous_record_is_never_rewritten();
}

#[test]
fn unwritable_values_are_refused_with_the_key() {
    cases::unwritable_values_are_refused_with_the_key();
}

#[test]
fn edits_use_utf16_offsets() {
    cases::edits_use_utf16_offsets();
}

#[test]
fn the_record_builds_and_prints_nothing() {
    cases::the_record_builds_and_prints_nothing();
}

#[test]
fn a_tag_selects_its_notes() {
    cases::a_tag_selects_its_notes();
}

#[test]
fn conditions_combine_with_and() {
    cases::conditions_combine_with_and();
}

#[test]
fn comparisons_work_per_kind() {
    cases::comparisons_work_per_kind();
}

#[test]
fn incomparable_values_do_not_pass() {
    cases::incomparable_values_do_not_pass();
}

#[test]
fn built_in_facts_work_without_properties() {
    cases::built_in_facts_work_without_properties();
}

#[test]
fn sorting_puts_empty_and_incomparable_last() {
    cases::sorting_puts_empty_and_incomparable_last();
}

#[test]
fn limit_cuts_rows_and_reports_truncation() {
    cases::limit_cuts_rows_and_reports_truncation();
}

#[test]
fn typed_words_become_conditions() {
    cases::typed_words_become_conditions();
}

#[test]
fn blocks_are_found_with_an_order_independent_key() {
    cases::blocks_are_found_with_an_order_independent_key();
}

#[test]
fn a_computed_block_is_not_editable() {
    cases::a_computed_block_is_not_editable();
}

#[test]
fn a_rendered_call_reads_back() {
    cases::a_rendered_call_reads_back();
}

#[test]
fn the_import_line_is_added_once() {
    cases::the_import_line_is_added_once();
}

#[test]
fn inputs_carry_rows_by_block_key() {
    cases::inputs_carry_rows_by_block_key();
}

#[test]
fn without_inputs_the_block_prints_a_placeholder() {
    cases::without_inputs_the_block_prints_a_placeholder();
}

#[test]
fn error_messages_never_carry_property_values() {
    cases::error_messages_never_carry_property_values();
}

#[test]
fn indexed_note_paths_use_forward_slashes() {
    cases::indexed_note_paths_use_forward_slashes();
}

#[test]
fn the_select_element_has_a_form_schema() {
    cases::the_select_element_has_a_form_schema();
}

#[test]
fn the_form_writes_reads_and_edits_a_block() {
    cases::the_form_writes_reads_and_edits_a_block();
}

#[test]
fn a_computed_argument_stays_an_expression_in_the_form() {
    cases::a_computed_argument_stays_an_expression_in_the_form();
}

#[tokio::test]
async fn the_vault_selection_follows_note_edits() {
    cases::the_vault_selection_follows_note_edits().await;
}

#[tokio::test]
async fn search_filters_by_properties() {
    cases::search_filters_by_properties().await;
}

#[tokio::test]
async fn reindex_runs_in_steps_and_can_be_cancelled() {
    cases::reindex_runs_in_steps_and_can_be_cancelled().await;
}

#[tokio::test]
async fn the_block_builds_into_a_table() {
    cases::the_block_builds_into_a_table().await;
}

#[tokio::test]
async fn the_table_follows_the_vault() {
    cases::the_table_follows_the_vault().await;
}

#[tokio::test]
async fn an_empty_result_explains_itself() {
    cases::an_empty_result_explains_itself().await;
}

#[tokio::test]
async fn the_list_view_and_the_truncation_note() {
    cases::the_list_view_and_the_truncation_note().await;
}

#[tokio::test]
async fn a_computed_block_falls_back_to_the_placeholder() {
    cases::a_computed_block_falls_back_to_the_placeholder().await;
}

#[tokio::test]
async fn export_builds_with_the_same_inputs() {
    cases::export_builds_with_the_same_inputs().await;
}

#[tokio::test]
async fn the_library_is_written_once_and_never_overwrites_edits() {
    cases::the_library_is_written_once_and_never_overwrites_edits().await;
}

#[tokio::test]
async fn the_cache_stores_properties() {
    cases::the_cache_stores_properties(
        typsidian_core::vault::cache::SqliteCache::in_memory()
            .await
            .unwrap(),
    )
    .await;
}
