//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `graph_native.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::graph_cases`: правила графа
//! платформенно нейтральны. Кэш ссылок и вставок здесь — `MemoryCache`, тот же
//! трейт `IndexCache`, что у SQLite на native.
#![cfg(target_arch = "wasm32")]

mod common;

use common::graph_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn nodes_and_edges_come_from_the_index() {
    cases::nodes_and_edges_come_from_the_index().await;
}

#[wasm_bindgen_test]
async fn loops_and_broken_links_are_dropped() {
    cases::loops_and_broken_links_are_dropped().await;
}

#[wasm_bindgen_test]
async fn a_pair_of_links_is_one_edge_with_both_ways() {
    cases::a_pair_of_links_is_one_edge_with_both_ways().await;
}

#[wasm_bindgen_test]
async fn a_link_with_text_points_at_the_same_note() {
    cases::a_link_with_text_points_at_the_same_note().await;
}

#[wasm_bindgen_test]
async fn orphans_can_be_hidden_and_degree_counts_the_visible_graph() {
    cases::orphans_can_be_hidden_and_degree_counts_the_visible_graph().await;
}

#[wasm_bindgen_test]
async fn an_empty_vault_is_an_empty_graph() {
    cases::an_empty_vault_is_an_empty_graph().await;
}

#[wasm_bindgen_test]
async fn a_rename_changes_the_id_and_keeps_the_links() {
    cases::a_rename_changes_the_id_and_keeps_the_links(
        typsidian_core::vault::cache::MemoryCache::new(),
    )
    .await;
}

#[wasm_bindgen_test]
async fn a_relocation_re_resolves_embeds() {
    cases::a_relocation_re_resolves_embeds(typsidian_core::vault::cache::MemoryCache::new()).await;
}

#[wasm_bindgen_test]
fn image_calls_are_read_with_the_typst_parser() {
    cases::image_calls_are_read_with_the_typst_parser();
}

#[wasm_bindgen_test]
fn board_json_has_no_links() {
    cases::board_json_has_no_links();
}

#[wasm_bindgen_test]
async fn attachments_are_shown_only_on_request() {
    cases::attachments_are_shown_only_on_request().await;
}

#[wasm_bindgen_test]
async fn tags_come_from_properties() {
    cases::tags_come_from_properties().await;
}

#[wasm_bindgen_test]
async fn the_filter_matches_text_in_name_and_path() {
    cases::the_filter_matches_text_in_name_and_path().await;
}

#[wasm_bindgen_test]
async fn the_filter_understands_folder_and_tag_conditions() {
    cases::the_filter_understands_folder_and_tag_conditions().await;
}

#[wasm_bindgen_test]
async fn the_filter_applies_to_attachments_and_tags() {
    cases::the_filter_applies_to_attachments_and_tags().await;
}

#[wasm_bindgen_test]
async fn the_first_matching_group_wins() {
    cases::the_first_matching_group_wins().await;
}

#[wasm_bindgen_test]
async fn groups_apply_to_tag_and_attachment_nodes() {
    cases::groups_apply_to_tag_and_attachment_nodes().await;
}

#[wasm_bindgen_test]
async fn local_graph_follows_links_by_depth() {
    cases::local_graph_follows_links_by_depth().await;
}

#[wasm_bindgen_test]
async fn local_graph_keeps_edges_between_reached_nodes() {
    cases::local_graph_keeps_edges_between_reached_nodes().await;
}

#[wasm_bindgen_test]
async fn a_center_without_links_is_one_node() {
    cases::a_center_without_links_is_one_node().await;
}

#[wasm_bindgen_test]
async fn a_center_outside_the_filter_stays() {
    cases::a_center_outside_the_filter_stays().await;
}

#[wasm_bindgen_test]
async fn an_unknown_center_is_an_empty_graph() {
    cases::an_unknown_center_is_an_empty_graph().await;
}

#[wasm_bindgen_test]
async fn a_tag_can_be_the_center() {
    cases::a_tag_can_be_the_center().await;
}

#[wasm_bindgen_test]
fn the_json_shape_is_the_contract() {
    cases::the_json_shape_is_the_contract();
}

#[wasm_bindgen_test]
async fn graph_settings_default_when_absent() {
    cases::graph_settings_default_when_absent().await;
}

#[wasm_bindgen_test]
async fn graph_settings_round_trip_through_the_file() {
    cases::graph_settings_round_trip_through_the_file().await;
}

#[wasm_bindgen_test]
fn graph_settings_tolerate_bad_values() {
    cases::graph_settings_tolerate_bad_values();
}

#[wasm_bindgen_test]
async fn writing_graph_settings_keeps_other_keys() {
    cases::writing_graph_settings_keeps_other_keys().await;
}

#[wasm_bindgen_test]
async fn graph_settings_are_normalized_on_write() {
    cases::graph_settings_are_normalized_on_write().await;
}
