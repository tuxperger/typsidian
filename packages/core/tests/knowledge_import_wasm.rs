//! wasm-bindgen-test импорта заметок в граф знаний (фича 055) — только wasm32 (принцип II).
//! Парные тесты — в `ai_native.rs`. Отдельный бинарник по той же причине, что
//! `knowledge_wasm.rs`: тяжёлые тесты импорта в общей странице выходили за предел ожидания
//! chromedriver.
#![cfg(target_arch = "wasm32")]

mod common;

use common::knowledge_cases as knowledge;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn pdf_text_comes_by_pages_and_hostile_files_are_refused() {
    knowledge::pdf_text_comes_by_pages_and_hostile_files_are_refused().await;
}

#[wasm_bindgen_test]
async fn an_estimate_lists_files_skips_with_reasons_and_cost() {
    knowledge::an_estimate_lists_files_skips_with_reasons_and_cost().await;
}

#[wasm_bindgen_test]
async fn the_vault_root_is_refused_and_big_files_split_into_parts() {
    knowledge::the_vault_root_is_refused_and_big_files_split_into_parts().await;
}

#[wasm_bindgen_test]
async fn a_million_paths_and_huge_folders_are_bounded() {
    knowledge::a_million_paths_and_huge_folders_are_bounded().await;
}

#[wasm_bindgen_test]
async fn an_import_sends_allowed_notes_and_reports_by_file() {
    knowledge::an_import_sends_allowed_notes_and_reports_by_file().await;
}

#[wasm_bindgen_test]
async fn an_import_refuses_an_estimate_that_no_longer_matches() {
    knowledge::an_import_refuses_an_estimate_that_no_longer_matches().await;
}

#[wasm_bindgen_test]
async fn cancelling_an_import_keeps_done_files_and_sends_no_more() {
    knowledge::cancelling_an_import_keeps_done_files_and_sends_no_more().await;
}

#[wasm_bindgen_test]
async fn a_failed_part_makes_the_file_partial_and_a_retry_sends_only_unfinished_parts() {
    knowledge::a_failed_part_makes_the_file_partial_and_a_retry_sends_only_unfinished_parts().await;
}

#[wasm_bindgen_test]
async fn a_repeat_import_is_free_and_sends_only_changed_parts() {
    knowledge::a_repeat_import_is_free_and_sends_only_changed_parts().await;
}

#[wasm_bindgen_test]
async fn note_coverage_lines_follow_moves_and_removals() {
    knowledge::note_coverage_lines_follow_moves_and_removals().await;
}

#[wasm_bindgen_test]
async fn import_guards_hold_one_job_forged_parts_and_late_denials() {
    knowledge::import_guards_hold_one_job_forged_parts_and_late_denials().await;
}

/// SC-003 на wasm: те же 2000 утверждений; предел мягче — браузер медленнее ядра.
#[wasm_bindgen_test]
async fn ending_a_job_by_any_way_drops_its_texts() {
    knowledge::ending_a_job_by_any_way_drops_its_texts().await;
}

#[wasm_bindgen_test]
async fn a_selection_naming_every_root_entry_is_the_root() {
    knowledge::a_selection_naming_every_root_entry_is_the_root().await;
}

#[wasm_bindgen_test]
async fn a_limited_read_never_returns_more_than_the_limit() {
    knowledge::a_limited_read_never_returns_more_than_the_limit().await;
}
