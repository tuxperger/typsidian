//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `ai_native.rs`.
#![cfg(target_arch = "wasm32")]

mod common;

use common::ai_cases as cases;
use common::lesson_cases as lesson;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn anthropic_gets_a_default_model() {
    cases::anthropic_gets_a_default_model().await;
}

#[wasm_bindgen_test]
async fn missing_or_broken_settings_mean_disabled() {
    cases::missing_or_broken_settings_mean_disabled().await;
}

#[wasm_bindgen_test]
async fn disabled_makes_no_requests() {
    cases::disabled_makes_no_requests().await;
}

#[wasm_bindgen_test]
async fn journal_gets_request_before_sending_and_never_the_key() {
    cases::journal_gets_request_before_sending_and_never_the_key().await;
}

#[wasm_bindgen_test]
async fn journal_failure_stops_the_request() {
    cases::journal_failure_stops_the_request().await;
}

#[wasm_bindgen_test]
async fn unknown_price_is_not_zero() {
    cases::unknown_price_is_not_zero().await;
}

#[wasm_bindgen_test]
async fn failure_is_recorded_and_carries_no_server_text() {
    cases::failure_is_recorded_and_carries_no_server_text().await;
}

#[wasm_bindgen_test]
async fn statuses_map_to_failures() {
    cases::statuses_map_to_failures().await;
}

#[wasm_bindgen_test]
async fn journal_keeps_last_entries_and_clears() {
    cases::journal_keeps_last_entries_and_clears().await;
}

#[wasm_bindgen_test]
async fn journal_stays_out_of_sync_and_settings_travel() {
    cases::journal_stays_out_of_sync_and_settings_travel().await;
}

#[wasm_bindgen_test]
async fn endpoints_are_guarded() {
    cases::endpoints_are_guarded().await;
}

#[wasm_bindgen_test]
async fn cloud_key_never_goes_to_custom_endpoint() {
    cases::cloud_key_never_goes_to_custom_endpoint().await;
}

#[wasm_bindgen_test]
async fn models_are_listed_sorted() {
    cases::models_are_listed_sorted().await;
}

#[wasm_bindgen_test]
async fn access_stays_inside_the_vault() {
    cases::access_stays_inside_the_vault().await;
}

#[wasm_bindgen_test]
async fn denied_folder_follows_the_move() {
    cases::denied_folder_follows_the_move().await;
}

#[wasm_bindgen_test]
async fn cancel_stops_waiting_at_once() {
    cases::cancel_stops_waiting_at_once().await;
}

#[wasm_bindgen_test]
async fn access_ignores_unicode_form() {
    cases::access_ignores_unicode_form().await;
}

#[wasm_bindgen_test]
async fn duplicate_request_id_keeps_the_first_cancellable() {
    cases::duplicate_request_id_keeps_the_first_cancellable().await;
}

#[wasm_bindgen_test]
async fn custom_endpoint_needs_approval_on_this_device() {
    cases::custom_endpoint_needs_approval_on_this_device().await;
}

// ── Фича 051: сессия урока ──

#[wasm_bindgen_test]
async fn lesson_is_a_listed_kind_and_partial_stays_on_device() {
    lesson::lesson_is_a_listed_kind_and_partial_stays_on_device().await;
}

#[wasm_bindgen_test]
async fn file_skips_broken_lines_and_appends_after_torn_line() {
    lesson::file_skips_broken_lines_and_appends_after_torn_line().await;
}

#[wasm_bindgen_test]
async fn goal_name_and_folder_rules() {
    lesson::goal_name_and_folder_rules().await;
}

#[wasm_bindgen_test]
async fn parser_assembles_anthropic_stream_from_arbitrary_chunks() {
    lesson::parser_assembles_anthropic_stream_from_arbitrary_chunks().await;
}

#[wasm_bindgen_test]
async fn parser_assembles_compatible_stream_with_usage() {
    lesson::parser_assembles_compatible_stream_with_usage().await;
}

#[wasm_bindgen_test]
async fn parser_does_not_take_a_cut_stream_for_a_reply() {
    lesson::parser_does_not_take_a_cut_stream_for_a_reply().await;
}

#[wasm_bindgen_test]
async fn answer_is_on_disk_before_the_network() {
    lesson::answer_is_on_disk_before_the_network().await;
}

#[wasm_bindgen_test]
async fn disabled_lesson_makes_no_requests() {
    lesson::disabled_lesson_makes_no_requests().await;
}

#[wasm_bindgen_test]
async fn question_with_choices_and_open_question() {
    lesson::question_with_choices_and_open_question().await;
}

#[wasm_bindgen_test]
async fn reply_without_tools_is_a_teacher_line() {
    lesson::reply_without_tools_is_a_teacher_line().await;
}

#[wasm_bindgen_test]
async fn changed_lesson_is_refused_and_nothing_is_written() {
    lesson::changed_lesson_is_refused_and_nothing_is_written().await;
}

#[wasm_bindgen_test]
async fn journal_failure_keeps_the_answer_and_stops_the_request() {
    lesson::journal_failure_keeps_the_answer_and_stops_the_request().await;
}

#[wasm_bindgen_test]
async fn status_failure_is_a_refusal_without_server_text() {
    lesson::status_failure_is_a_refusal_without_server_text().await;
}

#[wasm_bindgen_test]
async fn cancel_keeps_the_received_text_and_retry_needs_no_new_answer() {
    lesson::cancel_keeps_the_received_text_and_retry_needs_no_new_answer().await;
}

#[wasm_bindgen_test]
async fn network_break_saves_what_arrived() {
    lesson::network_break_saves_what_arrived().await;
}

#[wasm_bindgen_test]
async fn compatible_protocol_gets_stream_options() {
    lesson::compatible_protocol_gets_stream_options().await;
}

#[wasm_bindgen_test]
async fn language_and_state_are_events() {
    lesson::language_and_state_are_events().await;
}

#[wasm_bindgen_test]
async fn partial_file_restores_an_interrupted_reply() {
    lesson::partial_file_restores_an_interrupted_reply().await;
}

#[wasm_bindgen_test]
async fn compaction_summarises_and_keeps_the_full_file() {
    lesson::compaction_summarises_and_keeps_the_full_file().await;
}

#[wasm_bindgen_test]
async fn list_shows_all_lessons_with_states() {
    lesson::list_shows_all_lessons_with_states().await;
}

#[wasm_bindgen_test]
async fn attached_note_goes_into_request_and_log() {
    lesson::attached_note_goes_into_request_and_log().await;
}

#[wasm_bindgen_test]
async fn denied_missing_and_outside_notes_are_refused_without_text() {
    lesson::denied_missing_and_outside_notes_are_refused_without_text().await;
}

#[wasm_bindgen_test]
async fn read_note_tool_continues_and_result_goes_next() {
    lesson::read_note_tool_continues_and_result_goes_next().await;
}

#[wasm_bindgen_test]
async fn auto_continue_stops_after_four() {
    lesson::auto_continue_stops_after_four().await;
}

#[wasm_bindgen_test]
async fn lesson_cost_equals_the_journal_sum() {
    lesson::lesson_cost_equals_the_journal_sum().await;
}

#[wasm_bindgen_test]
async fn unknown_price_makes_the_lesson_cost_unknown() {
    lesson::unknown_price_makes_the_lesson_cost_unknown().await;
}

#[wasm_bindgen_test]
async fn changed_connection_never_gets_the_old_key() {
    lesson::changed_connection_never_gets_the_old_key().await;
}

#[wasm_bindgen_test]
async fn lesson_writes_never_go_through_symlinks() {
    lesson::lesson_writes_never_go_through_symlinks().await;
}

#[wasm_bindgen_test]
async fn lesson_settings_are_sanitized_on_read() {
    lesson::lesson_settings_are_sanitized_on_read().await;
}

#[wasm_bindgen_test]
async fn only_small_notes_are_read_and_lesson_files_have_a_limit() {
    lesson::only_small_notes_are_read_and_lesson_files_have_a_limit().await;
}

#[wasm_bindgen_test]
async fn attachments_are_limited_normalized_and_initial_only() {
    lesson::attachments_are_limited_normalized_and_initial_only().await;
}

#[wasm_bindgen_test]
async fn stream_parser_is_linear_and_refuses_a_huge_line() {
    lesson::stream_parser_is_linear_and_refuses_a_huge_line().await;
}

#[wasm_bindgen_test]
async fn progress_buffer_is_delta_guarded_and_swept() {
    lesson::progress_buffer_is_delta_guarded_and_swept().await;
}

#[wasm_bindgen_test]
async fn dropped_request_frees_its_slot_and_a_duplicate_keeps_the_buffer() {
    lesson::dropped_request_frees_its_slot_and_a_duplicate_keeps_the_buffer().await;
}

#[wasm_bindgen_test]
async fn cancel_between_turns_is_remembered_for_the_whole_cycle() {
    lesson::cancel_between_turns_is_remembered_for_the_whole_cycle().await;
}

#[wasm_bindgen_test]
async fn checkpoint_takes_the_path_from_the_turn_buffer() {
    lesson::checkpoint_takes_the_path_from_the_turn_buffer().await;
}

#[wasm_bindgen_test]
async fn lesson_commands_accept_only_lesson_files() {
    lesson::lesson_commands_accept_only_lesson_files().await;
}

#[wasm_bindgen_test]
async fn goal_name_avoids_reserved_names_and_format_characters() {
    lesson::goal_name_avoids_reserved_names_and_format_characters().await;
}

#[wasm_bindgen_test]
async fn question_is_limited_in_choices_and_length() {
    lesson::question_is_limited_in_choices_and_length().await;
}

#[wasm_bindgen_test]
async fn versions_folder_is_closed_to_the_agent() {
    lesson::versions_folder_is_closed_to_the_agent().await;
}

#[wasm_bindgen_test]
async fn stignore_report_includes_the_lesson_partial_folder() {
    lesson::stignore_report_includes_the_lesson_partial_folder().await;
}

#[wasm_bindgen_test]
async fn summary_tells_what_the_lesson_waits_for() {
    lesson::summary_tells_what_the_lesson_waits_for().await;
}

#[wasm_bindgen_test]
async fn probe_refuses_a_connection_that_changed_after_the_check() {
    cases::probe_refuses_a_connection_that_changed_after_the_check().await;
}

// ── Фича 052: замер и план ──

#[wasm_bindgen_test]
async fn probe_stage_asks_by_parts_takes_dont_know_and_goes_to_teach() {
    lesson::probe_stage_asks_by_parts_takes_dont_know_and_goes_to_teach().await;
}

#[wasm_bindgen_test]
async fn probe_question_limit_requires_the_result() {
    lesson::probe_question_limit_requires_the_result().await;
}

#[wasm_bindgen_test]
async fn skipping_the_probe_and_old_lessons_without_start() {
    lesson::skipping_the_probe_and_old_lessons_without_start().await;
}

#[wasm_bindgen_test]
async fn plan_is_accepted_laid_out_and_progress_marks_steps() {
    lesson::plan_is_accepted_laid_out_and_progress_marks_steps().await;
}

#[wasm_bindgen_test]
async fn broken_plans_are_refused_with_a_reason_and_never_shown() {
    lesson::broken_plans_are_refused_with_a_reason_and_never_shown().await;
}

#[wasm_bindgen_test]
async fn layout_has_no_overlaps_and_layers_follow_the_longest_path() {
    lesson::layout_has_no_overlaps_and_layers_follow_the_longest_path().await;
}

#[wasm_bindgen_test]
async fn plan_rebuild_makes_versions_with_added_marks_and_history() {
    lesson::plan_rebuild_makes_versions_with_added_marks_and_history().await;
}

#[wasm_bindgen_test]
async fn plan_view_of_a_huge_progress_log_is_linear() {
    lesson::plan_view_of_a_huge_progress_log_is_linear().await;
}

#[wasm_bindgen_test]
async fn plan_set_after_over_the_limit_is_malformed() {
    lesson::plan_set_after_over_the_limit_is_malformed().await;
}

#[wasm_bindgen_test]
async fn unknown_reference_is_clipped_in_the_rejection() {
    lesson::unknown_reference_is_clipped_in_the_rejection().await;
}

#[wasm_bindgen_test]
async fn hostile_plan_and_probe_blocks_are_bounded_and_marked() {
    lesson::hostile_plan_and_probe_blocks_are_bounded_and_marked().await;
}
