//! cargo test — только native (принцип II). Парный файл — `research_wasm.rs`. Исследование
//! (фича 056) вынесено из `ai_native`/`ai_wasm`: тесты на `MemoryVault` не отдают управление
//! странице, и общий wasm-набор упирался в предел ожидания chromedriver (см.
//! `knowledge_wasm.rs`).
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::research_cases as cases;

macro_rules! native_tests {
    ($($name:ident),* $(,)?) => {
        $(
            #[tokio::test]
            async fn $name() {
                cases::$name().await;
            }
        )*
    };
}

native_tests!(
    search_is_off_by_default_and_a_custom_endpoint_has_none,
    a_request_without_search_never_carries_the_search_tool,
    turning_search_off_mid_research_stops_the_continuation,
    web_research_ends_with_a_note_sources_and_cost,
    hostile_model_text_is_escaped_and_the_note_compiles,
    escaping_covers_markup_specials_and_line_starts,
    a_result_has_a_source_or_an_explicit_no_sources_mark,
    a_search_error_inside_a_200_reply_is_handled,
    pause_turn_continues_with_the_assistant_reply_and_is_bounded,
    a_continuation_body_is_limited,
    openai_research_uses_the_responses_api,
    the_stream_parser_is_chunk_independent_and_bounded,
    model_output_is_limited_everywhere,
    the_note_takes_a_free_name_and_never_goes_through_a_symlink,
    cancel_ends_the_research_and_sends_nothing_more,
    cancel_while_queued_and_before_sending_needs_no_network,
    the_queue_follows_research_parallel_in_order,
    unfinished_research_is_interrupted_and_events_fold_into_cards,
    a_dropped_call_does_not_leave_a_running_job,
    forged_steps_and_a_changed_connection_are_refused,
    research_respects_denied_lessons_and_disabled_learning,
    the_teacher_requests_research_and_gets_the_result_next_turn,
    teacher_research_calls_are_gated_and_limited,
    the_teacher_reads_pdf_only_when_allowed_and_through_access_check,
    the_estimate_shows_volume_pages_and_cost_and_respects_access,
    a_pdf_is_read_with_page_sources_and_a_denied_one_is_refused,
    a_long_pdf_is_truncated_in_the_request_and_the_estimate_says_so,
    research_settings_have_defaults_and_bounds,
    the_shared_job_list_has_both_kinds_and_events_round_trip,
);

#[tokio::test]
async fn the_stream_parser_is_linear_on_a_hostile_stream() {
    cases::the_stream_parser_is_linear_on_a_hostile_stream(1_024).await;
}
