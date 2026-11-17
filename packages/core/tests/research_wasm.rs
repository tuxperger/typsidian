//! wasm-bindgen-test исследования в уроке (фича 056) — только wasm32 (принцип II). Парные
//! тесты — в `research_native.rs`. Отдельный бинарник по той же причине, что
//! `knowledge_wasm.rs`: тесты на `MemoryVault` не отдают управление браузеру, и общий
//! `ai_wasm` выходил бы за предел ожидания chromedriver (30 с на ответ страницы). Тяжёлые
//! проверки идут на меньшем объёме; чтение PDF в сотни тысяч символов — только native.
#![cfg(target_arch = "wasm32")]

mod common;

use common::research_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn search_is_off_by_default_and_a_custom_endpoint_has_none() {
    cases::search_is_off_by_default_and_a_custom_endpoint_has_none().await;
}

#[wasm_bindgen_test]
async fn a_request_without_search_never_carries_the_search_tool() {
    cases::a_request_without_search_never_carries_the_search_tool().await;
}

#[wasm_bindgen_test]
async fn turning_search_off_mid_research_stops_the_continuation() {
    cases::turning_search_off_mid_research_stops_the_continuation().await;
}

#[wasm_bindgen_test]
async fn web_research_ends_with_a_note_sources_and_cost() {
    cases::web_research_ends_with_a_note_sources_and_cost().await;
}

#[wasm_bindgen_test]
async fn hostile_model_text_is_escaped_and_the_note_compiles() {
    cases::hostile_model_text_is_escaped_and_the_note_compiles().await;
}

#[wasm_bindgen_test]
async fn escaping_covers_markup_specials_and_line_starts() {
    cases::escaping_covers_markup_specials_and_line_starts().await;
}

#[wasm_bindgen_test]
async fn a_result_has_a_source_or_an_explicit_no_sources_mark() {
    cases::a_result_has_a_source_or_an_explicit_no_sources_mark().await;
}

#[wasm_bindgen_test]
async fn a_search_error_inside_a_200_reply_is_handled() {
    cases::a_search_error_inside_a_200_reply_is_handled().await;
}

#[wasm_bindgen_test]
async fn pause_turn_continues_with_the_assistant_reply_and_is_bounded() {
    cases::pause_turn_continues_with_the_assistant_reply_and_is_bounded().await;
}

#[wasm_bindgen_test]
async fn a_continuation_body_is_limited() {
    cases::a_continuation_body_is_limited().await;
}

#[wasm_bindgen_test]
async fn openai_research_uses_the_responses_api() {
    cases::openai_research_uses_the_responses_api().await;
}

#[wasm_bindgen_test]
async fn the_stream_parser_is_chunk_independent_and_bounded() {
    cases::the_stream_parser_is_chunk_independent_and_bounded().await;
}

#[wasm_bindgen_test]
async fn model_output_is_limited_everywhere() {
    cases::model_output_is_limited_everywhere().await;
}

#[wasm_bindgen_test]
async fn the_note_takes_a_free_name_and_never_goes_through_a_symlink() {
    cases::the_note_takes_a_free_name_and_never_goes_through_a_symlink().await;
}

#[wasm_bindgen_test]
async fn cancel_ends_the_research_and_sends_nothing_more() {
    cases::cancel_ends_the_research_and_sends_nothing_more().await;
}

#[wasm_bindgen_test]
async fn cancel_while_queued_and_before_sending_needs_no_network() {
    cases::cancel_while_queued_and_before_sending_needs_no_network().await;
}

#[wasm_bindgen_test]
async fn the_queue_follows_research_parallel_in_order() {
    cases::the_queue_follows_research_parallel_in_order().await;
}

#[wasm_bindgen_test]
async fn unfinished_research_is_interrupted_and_events_fold_into_cards() {
    cases::unfinished_research_is_interrupted_and_events_fold_into_cards().await;
}

#[wasm_bindgen_test]
async fn a_dropped_call_does_not_leave_a_running_job() {
    cases::a_dropped_call_does_not_leave_a_running_job().await;
}

#[wasm_bindgen_test]
async fn forged_steps_and_a_changed_connection_are_refused() {
    cases::forged_steps_and_a_changed_connection_are_refused().await;
}

#[wasm_bindgen_test]
async fn research_respects_denied_lessons_and_disabled_learning() {
    cases::research_respects_denied_lessons_and_disabled_learning().await;
}

#[wasm_bindgen_test]
async fn the_teacher_requests_research_and_gets_the_result_next_turn() {
    cases::the_teacher_requests_research_and_gets_the_result_next_turn().await;
}

#[wasm_bindgen_test]
async fn teacher_research_calls_are_gated_and_limited() {
    cases::teacher_research_calls_are_gated_and_limited().await;
}

#[wasm_bindgen_test]
async fn the_teacher_reads_pdf_only_when_allowed_and_through_access_check() {
    cases::the_teacher_reads_pdf_only_when_allowed_and_through_access_check().await;
}

#[wasm_bindgen_test]
async fn the_estimate_shows_volume_pages_and_cost_and_respects_access() {
    cases::the_estimate_shows_volume_pages_and_cost_and_respects_access().await;
}

#[wasm_bindgen_test]
async fn a_pdf_is_read_with_page_sources_and_a_denied_one_is_refused() {
    cases::a_pdf_is_read_with_page_sources_and_a_denied_one_is_refused().await;
}

#[wasm_bindgen_test]
async fn research_settings_have_defaults_and_bounds() {
    cases::research_settings_have_defaults_and_bounds().await;
}

#[wasm_bindgen_test]
async fn the_shared_job_list_has_both_kinds_and_events_round_trip() {
    cases::the_shared_job_list_has_both_kinds_and_events_round_trip().await;
}

#[wasm_bindgen_test]
async fn the_stream_parser_is_linear_on_a_hostile_stream() {
    cases::the_stream_parser_is_linear_on_a_hostile_stream(64).await;
}
