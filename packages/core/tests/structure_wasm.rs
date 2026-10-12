//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `structure_native.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::structure_cases`: разбор
//! текста и оглавление платформенно нейтральны, и расхождение между таргетами
//! является дефектом.
#![cfg(target_arch = "wasm32")]

mod common;

use common::structure_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn headings_by_signs_and_by_call() {
    cases::headings_by_signs_and_by_call();
}

#[wasm_bindgen_test]
fn call_levels_follow_the_documented_rule() {
    cases::call_levels_follow_the_documented_rule();
}

#[wasm_bindgen_test]
fn equals_inside_code_comments_and_strings_is_not_a_heading() {
    cases::equals_inside_code_comments_and_strings_is_not_a_heading();
}

#[wasm_bindgen_test]
fn computed_and_foreign_headings_are_skipped() {
    cases::computed_and_foreign_headings_are_skipped();
}

#[wasm_bindgen_test]
fn headings_inside_content_blocks_and_branches_are_found() {
    cases::headings_inside_content_blocks_and_branches_are_found();
}

#[wasm_bindgen_test]
fn readable_text_drops_markup() {
    cases::readable_text_drops_markup();
}

#[wasm_bindgen_test]
fn figures_and_tables_carry_kind_and_caption() {
    cases::figures_and_tables_carry_kind_and_caption();
}

#[wasm_bindgen_test]
fn labels_attach_to_the_previous_neighbour_and_code_uses_are_not_definitions() {
    cases::labels_attach_to_the_previous_neighbour_and_code_uses_are_not_definitions();
}

#[wasm_bindgen_test]
fn references_carry_target_and_place() {
    cases::references_carry_target_and_place();
}

#[wasm_bindgen_test]
fn places_count_characters_and_utf16_units() {
    cases::places_count_characters_and_utf16_units();
}

#[wasm_bindgen_test]
fn broken_text_never_panics() {
    cases::broken_text_never_panics();
}

#[wasm_bindgen_test]
fn outline_follows_text_order_with_levels_and_parents() {
    cases::outline_follows_text_order_with_levels_and_parents();
}

#[wasm_bindgen_test]
fn outline_keys_are_unique_and_places_point_at_the_record() {
    cases::outline_keys_are_unique_and_places_point_at_the_record();
}

#[wasm_bindgen_test]
fn view_shows_headings_with_counts_and_states() {
    cases::view_shows_headings_with_counts_and_states();
}

#[wasm_bindgen_test]
fn outline_matches_the_compiled_document() {
    cases::outline_matches_the_compiled_document();
}

#[wasm_bindgen_test]
fn heading_probe_numbers_equal_headings() {
    cases::heading_probe_numbers_equal_headings();
}

#[wasm_bindgen_test]
fn locate_heading_on_hand_made_runs() {
    cases::locate_heading_on_hand_made_runs();
}

#[wasm_bindgen_test]
fn locate_heading_on_a_compiled_document() {
    cases::locate_heading_on_a_compiled_document();
}

#[wasm_bindgen_test]
fn the_active_section_follows_the_caret() {
    cases::the_active_section_follows_the_caret();
}

#[wasm_bindgen_test]
fn collapsed_branches_hide_their_descendants() {
    cases::collapsed_branches_hide_their_descendants();
}

#[wasm_bindgen_test]
fn the_filter_keeps_matches_with_their_ancestors() {
    cases::the_filter_keeps_matches_with_their_ancestors();
}

#[wasm_bindgen_test]
fn collapsing_is_ignored_while_filtering() {
    cases::collapsing_is_ignored_while_filtering();
}

#[wasm_bindgen_test]
fn the_active_section_climbs_to_a_visible_ancestor() {
    cases::the_active_section_climbs_to_a_visible_ancestor();
}

#[wasm_bindgen_test]
fn carry_collapsed_follows_edits() {
    cases::carry_collapsed_follows_edits();
}

#[wasm_bindgen_test]
fn kinds_are_chosen_per_view() {
    cases::kinds_are_chosen_per_view();
}

#[wasm_bindgen_test]
fn counts_cover_the_whole_note_and_children_follow_the_kinds() {
    cases::counts_cover_the_whole_note_and_children_follow_the_kinds();
}

#[wasm_bindgen_test]
fn filter_reaches_label_names_and_the_active_section_is_always_a_heading() {
    cases::filter_reaches_label_names_and_the_active_section_is_always_a_heading();
}
