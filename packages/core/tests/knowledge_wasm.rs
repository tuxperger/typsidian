//! wasm-bindgen-test графа знаний (фича 054; импорт 055 — `knowledge_import_wasm.rs`) — только wasm32 (принцип II). Парные
//! тесты — в `ai_native.rs`. Отдельный бинарник: тесты на `MemoryVault` не отдают
//! управление браузеру, и общий `ai_wasm` с тяжёлыми тестами импорта выходил за предел
//! ожидания chromedriver (30 с на ответ страницы).
#![cfg(target_arch = "wasm32")]

mod common;

use common::knowledge_cases as knowledge;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

async fn graph_files_are_other_kind_and_projection_applies_tombstones() {
    knowledge::graph_files_are_other_kind_and_projection_applies_tombstones().await;
}

#[wasm_bindgen_test]
async fn moved_lines_make_the_source_follow_a_rename() {
    knowledge::moved_lines_make_the_source_follow_a_rename().await;
}

#[wasm_bindgen_test]
async fn hostile_graph_files_are_read_with_limits() {
    knowledge::hostile_graph_files_are_read_with_limits().await;
}

#[wasm_bindgen_test]
async fn graph_parsing_is_linear_on_a_hostile_file() {
    knowledge::graph_parsing_is_linear_on_a_hostile_file().await;
}

#[wasm_bindgen_test]
async fn append_is_atomic_and_never_goes_through_a_symlink() {
    knowledge::append_is_atomic_and_never_goes_through_a_symlink().await;
}

// Ревью безопасности 054 (T020–T030).

#[wasm_bindgen_test]
async fn a_recreated_shorter_lesson_is_covered_again_and_a_future_move_cannot_stall_it() {
    knowledge::a_recreated_shorter_lesson_is_covered_again_and_a_future_move_cannot_stall_it()
        .await;
}

#[wasm_bindgen_test]
async fn the_core_stops_a_job_after_the_batch_limit() {
    knowledge::the_core_stops_a_job_after_the_batch_limit().await;
}

#[wasm_bindgen_test]
async fn statements_from_closed_lessons_never_reach_the_model() {
    knowledge::statements_from_closed_lessons_never_reach_the_model().await;
}

#[wasm_bindgen_test]
async fn moves_are_written_only_for_known_sources_and_the_latest_are_kept() {
    knowledge::moves_are_written_only_for_known_sources_and_the_latest_are_kept().await;
}

#[wasm_bindgen_test]
async fn resolving_moves_is_not_quadratic_on_a_hostile_sources_file() {
    let (with_moves, plain) =
        knowledge::resolving_moves_is_not_quadratic_on_a_hostile_sources_file().await;
    assert!(
        with_moves <= plain * 3 + 5_000,
        "{with_moves} мс против {plain} мс"
    );
}

#[wasm_bindgen_test]
async fn tombstones_are_later_than_every_line_of_their_id_and_source() {
    knowledge::tombstones_are_later_than_every_line_of_their_id_and_source().await;
}

#[wasm_bindgen_test]
async fn deleting_a_lesson_is_scoped_to_its_vault_and_cancels_the_extraction() {
    knowledge::deleting_a_lesson_is_scoped_to_its_vault_and_cancels_the_extraction().await;
}

#[wasm_bindgen_test]
async fn a_forged_or_stale_batch_is_not_written_and_end_removes_only_its_own_flag() {
    knowledge::a_forged_or_stale_batch_is_not_written_and_end_removes_only_its_own_flag().await;
}

#[wasm_bindgen_test]
async fn model_refs_go_only_to_sent_ids_and_tombstones_fit_in_a_full_file() {
    knowledge::model_refs_go_only_to_sent_ids_and_tombstones_fit_in_a_full_file().await;
}

#[wasm_bindgen_test]
async fn coverage_and_job_conflict_use_the_folded_path() {
    knowledge::coverage_and_job_conflict_use_the_folded_path().await;
}

#[wasm_bindgen_test]
async fn knowledge_settings_have_defaults_and_old_files_still_read() {
    knowledge::knowledge_settings_have_defaults_and_old_files_still_read().await;
}

#[wasm_bindgen_test]
async fn batches_take_only_complete_teach_phase_explanations() {
    knowledge::batches_take_only_complete_teach_phase_explanations().await;
}

#[wasm_bindgen_test]
async fn request_has_only_teacher_explanations_and_is_journaled_before_the_network() {
    knowledge::request_has_only_teacher_explanations_and_is_journaled_before_the_network().await;
}

#[wasm_bindgen_test]
async fn extraction_of_a_practice_or_probe_sends_nothing() {
    knowledge::extraction_of_a_practice_or_probe_sends_nothing().await;
}

#[wasm_bindgen_test]
async fn findings_parse_has_limits_and_accepts_both_spellings() {
    knowledge::findings_parse_has_limits_and_accepts_both_spellings().await;
}

#[wasm_bindgen_test]
async fn ids_come_from_normalized_text() {
    knowledge::ids_come_from_normalized_text().await;
}
