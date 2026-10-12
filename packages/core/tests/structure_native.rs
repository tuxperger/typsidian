//! cargo test — только native (принцип II). Парный файл — `structure_wasm.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::structure_cases`: разбор
//! текста и оглавление платформенно нейтральны, и расхождение между таргетами
//! является дефектом.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::structure_cases as cases;

#[test]
fn headings_by_signs_and_by_call() {
    cases::headings_by_signs_and_by_call();
}

#[test]
fn call_levels_follow_the_documented_rule() {
    cases::call_levels_follow_the_documented_rule();
}

#[test]
fn equals_inside_code_comments_and_strings_is_not_a_heading() {
    cases::equals_inside_code_comments_and_strings_is_not_a_heading();
}

#[test]
fn computed_and_foreign_headings_are_skipped() {
    cases::computed_and_foreign_headings_are_skipped();
}

#[test]
fn headings_inside_content_blocks_and_branches_are_found() {
    cases::headings_inside_content_blocks_and_branches_are_found();
}

#[test]
fn readable_text_drops_markup() {
    cases::readable_text_drops_markup();
}

#[test]
fn figures_and_tables_carry_kind_and_caption() {
    cases::figures_and_tables_carry_kind_and_caption();
}

#[test]
fn labels_attach_to_the_previous_neighbour_and_code_uses_are_not_definitions() {
    cases::labels_attach_to_the_previous_neighbour_and_code_uses_are_not_definitions();
}

#[test]
fn references_carry_target_and_place() {
    cases::references_carry_target_and_place();
}

#[test]
fn places_count_characters_and_utf16_units() {
    cases::places_count_characters_and_utf16_units();
}

#[test]
fn broken_text_never_panics() {
    cases::broken_text_never_panics();
}

#[test]
fn outline_follows_text_order_with_levels_and_parents() {
    cases::outline_follows_text_order_with_levels_and_parents();
}

#[test]
fn outline_keys_are_unique_and_places_point_at_the_record() {
    cases::outline_keys_are_unique_and_places_point_at_the_record();
}

#[test]
fn view_shows_headings_with_counts_and_states() {
    cases::view_shows_headings_with_counts_and_states();
}

#[test]
fn outline_matches_the_compiled_document() {
    cases::outline_matches_the_compiled_document();
}

#[test]
fn heading_probe_numbers_equal_headings() {
    cases::heading_probe_numbers_equal_headings();
}

#[test]
fn locate_heading_on_hand_made_runs() {
    cases::locate_heading_on_hand_made_runs();
}

#[test]
fn locate_heading_on_a_compiled_document() {
    cases::locate_heading_on_a_compiled_document();
}

#[test]
fn the_active_section_follows_the_caret() {
    cases::the_active_section_follows_the_caret();
}

#[test]
fn collapsed_branches_hide_their_descendants() {
    cases::collapsed_branches_hide_their_descendants();
}

#[test]
fn the_filter_keeps_matches_with_their_ancestors() {
    cases::the_filter_keeps_matches_with_their_ancestors();
}

#[test]
fn collapsing_is_ignored_while_filtering() {
    cases::collapsing_is_ignored_while_filtering();
}

#[test]
fn the_active_section_climbs_to_a_visible_ancestor() {
    cases::the_active_section_climbs_to_a_visible_ancestor();
}

#[test]
fn carry_collapsed_follows_edits() {
    cases::carry_collapsed_follows_edits();
}

#[test]
fn kinds_are_chosen_per_view() {
    cases::kinds_are_chosen_per_view();
}

#[test]
fn counts_cover_the_whole_note_and_children_follow_the_kinds() {
    cases::counts_cover_the_whole_note_and_children_follow_the_kinds();
}

#[test]
fn filter_reaches_label_names_and_the_active_section_is_always_a_heading() {
    cases::filter_reaches_label_names_and_the_active_section_is_always_a_heading();
}
