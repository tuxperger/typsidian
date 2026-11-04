//! wasm-bindgen-test графа знаний (фича 054), вторая половина — только wasm32 (принцип II).
//! Парные тесты — в `ai_native.rs`. Граф поделён на два бинарника: тесты на враждебных
//! файлах в сотни тысяч строк не отдают управление браузеру, и вместе страница не
//! укладывалась в предел ожидания chromedriver с запасом.
#![cfg(target_arch = "wasm32")]

mod common;

use common::knowledge_cases as knowledge;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn merge_gives_no_duplicates_same_as_and_rejects_dangling_chains() {
    knowledge::merge_gives_no_duplicates_same_as_and_rejects_dangling_chains().await;
}

#[wasm_bindgen_test]
async fn three_explanations_become_propositions_with_sources_and_repeat_adds_nothing() {
    knowledge::three_explanations_become_propositions_with_sources_and_repeat_adds_nothing().await;
}

#[wasm_bindgen_test]
async fn existing_statements_are_found_locally_and_same_as_merges_them() {
    knowledge::existing_statements_are_found_locally_and_same_as_merges_them().await;
}

#[wasm_bindgen_test]
async fn dangling_chain_is_rejected_and_counted() {
    knowledge::dangling_chain_is_rejected_and_counted().await;
}

#[wasm_bindgen_test]
async fn a_cut_between_batches_keeps_what_is_done_and_a_retry_takes_the_rest() {
    knowledge::a_cut_between_batches_keeps_what_is_done_and_a_retry_takes_the_rest().await;
}

#[wasm_bindgen_test]
async fn a_bad_reply_leaves_the_batch_uncovered() {
    knowledge::a_bad_reply_leaves_the_batch_uncovered().await;
}

#[wasm_bindgen_test]
async fn cancel_stops_the_request_and_the_remaining_batches() {
    knowledge::cancel_stops_the_request_and_the_remaining_batches().await;
}

#[wasm_bindgen_test]
async fn extraction_keeps_the_safety_guarantees_of_lessons() {
    knowledge::extraction_keeps_the_safety_guarantees_of_lessons().await;
}

#[wasm_bindgen_test]
async fn auto_start_flag_follows_the_threshold_and_the_settings() {
    knowledge::auto_start_flag_follows_the_threshold_and_the_settings().await;
}

#[wasm_bindgen_test]
async fn jobs_registry_is_per_vault_and_lists_running_first() {
    knowledge::jobs_registry_is_per_vault_and_lists_running_first().await;
}

#[wasm_bindgen_test]
async fn graph_view_has_the_shape_of_the_notes_graph() {
    knowledge::graph_view_has_the_shape_of_the_notes_graph().await;
}

#[wasm_bindgen_test]
async fn proposition_view_has_chains_and_sources_with_the_removed_mark() {
    knowledge::proposition_view_has_chains_and_sources_with_the_removed_mark().await;
}

#[wasm_bindgen_test]
async fn read_knowledge_is_absent_unless_allowed_and_reads_up_to_thirty() {
    knowledge::read_knowledge_is_absent_unless_allowed_and_reads_up_to_thirty().await;
}

#[wasm_bindgen_test]
async fn read_knowledge_works_in_probe_and_practice_too() {
    knowledge::read_knowledge_works_in_probe_and_practice_too().await;
}

#[wasm_bindgen_test]
async fn selected_statements_go_into_material_and_the_request() {
    knowledge::selected_statements_go_into_material_and_the_request().await;
}

#[wasm_bindgen_test]
async fn removing_a_proposition_hides_it_and_marks_chains() {
    knowledge::removing_a_proposition_hides_it_and_marks_chains().await;
}

#[wasm_bindgen_test]
async fn removing_a_source_contribution_keeps_shared_statements() {
    knowledge::removing_a_source_contribution_keeps_shared_statements().await;
}

#[wasm_bindgen_test]
async fn deleting_a_lesson_decides_the_fate_of_its_contribution() {
    knowledge::deleting_a_lesson_decides_the_fate_of_its_contribution().await;
}

#[wasm_bindgen_test]
async fn related_search_is_local_stable_and_capped() {
    knowledge::related_search_is_local_stable_and_capped().await;
}

#[wasm_bindgen_test]
async fn graph_of_2000_propositions_opens_and_searches_in_under_a_second() {
    let elapsed =
        knowledge::graph_of_2000_propositions_opens_and_searches_in_under_a_second().await;
    assert!(elapsed < 3_000, "{elapsed} мс");
}
