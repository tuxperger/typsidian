//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `ai_native.rs`.
#![cfg(target_arch = "wasm32")]

mod common;

use common::ai_cases as cases;
use common::knowledge_cases as knowledge;
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

// ── Фича 053: практика ──

#[wasm_bindgen_test]
async fn practice_is_a_lesson_file_with_a_kind_and_old_lessons_stay_lessons() {
    lesson::practice_is_a_lesson_file_with_a_kind_and_old_lessons_stay_lessons().await;
}

#[wasm_bindgen_test]
async fn practice_hides_the_solution_until_the_verdict_and_needs_next_problem() {
    lesson::practice_hides_the_solution_until_the_verdict_and_needs_next_problem().await;
}

#[wasm_bindgen_test]
async fn practice_actions_are_refused_when_they_do_not_apply() {
    lesson::practice_actions_are_refused_when_they_do_not_apply().await;
}

#[wasm_bindgen_test]
async fn problem_without_a_solution_is_refused_then_accepted_without_it() {
    lesson::problem_without_a_solution_is_refused_then_accepted_without_it().await;
}

#[wasm_bindgen_test]
async fn hints_keep_the_solution_hidden_and_give_up_reveals_it() {
    lesson::hints_keep_the_solution_hidden_and_give_up_reveals_it().await;
}

#[wasm_bindgen_test]
async fn practice_material_is_collected_once_from_chosen_sources() {
    lesson::practice_material_is_collected_once_from_chosen_sources().await;
}

#[wasm_bindgen_test]
async fn practice_material_is_limited() {
    lesson::practice_material_is_limited().await;
}

#[wasm_bindgen_test]
async fn practice_in_english_uses_english_instructions() {
    lesson::practice_in_english_uses_english_instructions().await;
}

#[wasm_bindgen_test]
async fn hostile_practice_file_never_leaks_the_solution_and_is_bounded() {
    lesson::hostile_practice_file_never_leaks_the_solution_and_is_bounded().await;
}

#[wasm_bindgen_test]
async fn practice_tool_arguments_are_limited() {
    lesson::practice_tool_arguments_are_limited().await;
}

#[wasm_bindgen_test]
async fn practice_drops_a_lesson_from_the_request_when_its_folder_is_denied_later() {
    lesson::practice_drops_a_lesson_from_the_request_when_its_folder_is_denied_later().await;
}

#[wasm_bindgen_test]
async fn practice_counts_lesson_attempts_and_records_every_refusal() {
    lesson::practice_counts_lesson_attempts_and_records_every_refusal().await;
}

#[wasm_bindgen_test]
async fn problem_overridden_by_a_later_one_with_the_same_number_is_closed() {
    lesson::problem_overridden_by_a_later_one_with_the_same_number_is_closed().await;
}

#[wasm_bindgen_test]
async fn hint_with_the_solution_is_refused_with_a_reason() {
    lesson::hint_with_the_solution_is_refused_with_a_reason().await;
}

#[wasm_bindgen_test]
async fn lesson_text_is_bounded_in_linear_time() {
    lesson::lesson_text_is_bounded_in_linear_time().await;
}

#[wasm_bindgen_test]
async fn lesson_goal_is_clipped_when_building_instructions() {
    lesson::lesson_goal_is_clipped_when_building_instructions().await;
}

// ── Фича 054: граф знаний ──

#[wasm_bindgen_test]
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

/// SC-003 на wasm: те же 2000 утверждений; предел мягче — браузер медленнее ядра.
#[wasm_bindgen_test]
async fn graph_of_2000_propositions_opens_and_searches_in_under_a_second() {
    let elapsed =
        knowledge::graph_of_2000_propositions_opens_and_searches_in_under_a_second().await;
    assert!(elapsed < 3_000, "{elapsed} мс");
}
