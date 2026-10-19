//! cargo test — только native (принцип II). Парный файл — `sources_wasm.rs`.
//!
//! Обе стороны гоняют одну таблицу `common::sources_cases`; здесь же —
//! обнаружение изменений файлов источников вотчером на настоящем диске.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::sources_cases as cases;

#[test]
fn the_reading_crates_behave_as_researched() {
    cases::the_reading_crates_behave_as_researched();
}

#[test]
fn structure_finds_cites_and_bibliographies() {
    cases::structure_finds_cites_and_bibliographies();
}

#[test]
fn mode_at_tells_markup_math_code_string_and_comment() {
    cases::mode_at_tells_markup_math_code_string_and_comment();
}

#[test]
fn bib_and_yaml_give_the_same_source() {
    cases::bib_and_yaml_give_the_same_source();
}

#[test]
fn a_broken_bib_entry_keeps_the_rest_and_is_named() {
    cases::a_broken_bib_entry_keeps_the_rest_and_is_named();
}

#[test]
fn too_many_problems_are_folded() {
    cases::too_many_problems_are_folded();
}

#[test]
fn reading_never_panics_on_arbitrary_text() {
    cases::reading_never_panics_on_arbitrary_text();
}

#[tokio::test]
async fn the_library_recognizes_files_and_rereads_only_changes() {
    cases::the_library_recognizes_files_and_rereads_only_changes().await;
}

#[tokio::test]
async fn a_removed_file_leaves_the_summary() {
    cases::a_removed_file_leaves_the_summary().await;
}

#[tokio::test]
async fn search_matches_every_word_and_orders_by_key_then_family() {
    cases::search_matches_every_word_and_orders_by_key_then_family().await;
}

#[tokio::test]
async fn search_over_ten_thousand_sources_keeps_its_order() {
    cases::search_over_ten_thousand_sources_keeps_its_order().await;
}

#[test]
fn every_citation_form_reads_back_as_one_citation() {
    cases::every_citation_form_reads_back_as_one_citation();
}

#[test]
fn strings_comments_and_unknown_keys_are_refused() {
    cases::strings_comments_and_unknown_keys_are_refused();
}

#[test]
fn the_bibliography_offer_adds_extends_or_stays_quiet() {
    cases::the_bibliography_offer_adds_extends_or_stays_quiet();
}

#[tokio::test]
async fn creating_the_source_file_refuses_busy_names_and_read_only() {
    cases::creating_the_source_file_refuses_busy_names_and_read_only().await;
}

#[tokio::test]
async fn a_bad_source_file_setting_is_refused_before_writing() {
    cases::a_bad_source_file_setting_is_refused_before_writing().await;
}

#[tokio::test]
async fn the_citation_index_scans_in_portions_and_answers() {
    cases::the_citation_index_scans_in_portions_and_answers().await;
}

#[tokio::test]
async fn cancel_keeps_what_was_scanned_and_a_rescan_skips_it() {
    cases::cancel_keeps_what_was_scanned_and_a_rescan_skips_it().await;
}

#[tokio::test]
async fn single_notes_follow_vault_events() {
    cases::single_notes_follow_vault_events().await;
}

#[tokio::test]
async fn citation_marks_follow_the_research_table() {
    cases::citation_marks_follow_the_research_table().await;
}

#[tokio::test]
async fn writing_a_source_file_is_announced() {
    cases::writing_a_source_file_is_announced().await;
}

#[tokio::test]
async fn the_whole_path_runs_without_a_network() {
    cases::the_whole_path_runs_without_a_network().await;
}

// ── Обнаружение изменений на диске (T047) ──────────────────────────────────

#[tokio::test]
async fn source_file_changes_on_disk_are_announced_and_others_are_not() {
    use std::time::Duration;
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::events::VaultEvent;
    use typsidian_core::vault::native::NativeBackend;

    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("note.typ"), "= Заметка\n").unwrap();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    async fn drain(backend: &mut NativeBackend) -> Vec<VaultEvent> {
        let mut seen = Vec::new();
        for _ in 0..40 {
            seen.extend(backend.poll_events());
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        seen
    }
    let _ = drain(&mut backend).await;

    std::fs::write(dir.path().join("notes.txt"), "посторонний").unwrap();
    let foreign = drain(&mut backend).await;
    assert!(
        !foreign
            .iter()
            .any(|event| matches!(event, VaultEvent::SourceFilesChanged)),
        "{foreign:?}"
    );

    for step in ["create", "modify", "rename", "remove"] {
        match step {
            "create" => {
                std::fs::write(dir.path().join("refs.bib"), "@misc{a, title={x}}\n").unwrap()
            }
            "modify" => {
                std::fs::write(dir.path().join("refs.bib"), "@misc{b, title={y}}\n").unwrap()
            }
            "rename" => {
                std::fs::rename(dir.path().join("refs.bib"), dir.path().join("lit.bib")).unwrap()
            }
            _ => std::fs::remove_file(dir.path().join("lit.bib")).unwrap(),
        }
        let events = drain(&mut backend).await;
        assert!(
            events
                .iter()
                .any(|event| matches!(event, VaultEvent::SourceFilesChanged)),
            "{step}: {events:?}"
        );
    }

    // Запись самим приложением — то же событие.
    backend
        .write_bytes(std::path::Path::new("own.bib"), b"")
        .await
        .unwrap();
    assert!(backend
        .poll_events()
        .iter()
        .any(|event| matches!(event, VaultEvent::SourceFilesChanged)));
}
