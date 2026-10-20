//! cargo test — только native (принцип II). Парный файл — `completion_wasm.rs`.
//!
//! Обе стороны гоняют одну таблицу `common::completion_cases` (фича 018).
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::completion_cases as cases;

#[test]
fn context_offers_in_markup_and_content_blocks() {
    cases::context_offers_in_markup_and_content_blocks();
}

#[test]
fn context_refuses_code_strings_comments_raw_math_and_addresses() {
    cases::context_refuses_code_strings_comments_raw_math_and_addresses();
}

#[test]
fn context_counts_utf16_after_cyrillic_and_emoji() {
    cases::context_counts_utf16_after_cyrillic_and_emoji();
}

#[test]
fn manual_call_without_trigger_says_why() {
    cases::manual_call_without_trigger_says_why();
}

#[test]
fn label_candidates_carry_kind_text_and_file() {
    cases::label_candidates_carry_kind_text_and_file();
}

#[test]
fn label_candidates_filter_by_any_part_and_rank_by_tier() {
    cases::label_candidates_filter_by_any_part_and_rank_by_tier();
}

#[test]
fn label_candidates_cap_rows_and_fold_duplicates() {
    cases::label_candidates_cap_rows_and_fold_duplicates();
}

#[test]
fn build_evidence_names_labels_and_references() {
    cases::build_evidence_names_labels_and_references();
}

#[tokio::test]
async fn linked_index_reads_typ_dependencies() {
    cases::linked_index_reads_typ_dependencies().await;
}

#[tokio::test]
async fn a_failed_read_gives_an_unreadable_path() {
    cases::a_failed_read_gives_an_unreadable_path().await;
}

#[tokio::test]
async fn note_candidates_prefer_the_folder_and_insert_what_resolves() {
    cases::note_candidates_prefer_the_folder_and_insert_what_resolves().await;
}

#[tokio::test]
async fn completions_explain_nothing_and_use_the_buffer() {
    cases::completions_explain_nothing_and_use_the_buffer().await;
}

#[tokio::test]
async fn definition_reaches_labels_files_notes_and_sources() {
    cases::definition_reaches_labels_files_notes_and_sources().await;
}

#[tokio::test]
async fn definition_names_why_there_is_nowhere_to_go() {
    cases::definition_names_why_there_is_nowhere_to_go().await;
}

#[tokio::test]
async fn missing_labels_are_marked_unless_known_somewhere() {
    cases::missing_labels_are_marked_unless_known_somewhere().await;
}

#[tokio::test]
async fn a_document_that_builds_gets_no_missing_labels() {
    cases::a_document_that_builds_gets_no_missing_labels().await;
}

#[tokio::test]
async fn held_marks_wait_for_the_build_and_duplicates_need_proof() {
    cases::held_marks_wait_for_the_build_and_duplicates_need_proof().await;
}

#[tokio::test]
async fn note_links_are_marked_when_they_lead_nowhere() {
    cases::note_links_are_marked_when_they_lead_nowhere().await;
}
