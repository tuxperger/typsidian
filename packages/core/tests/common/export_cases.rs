//! Экспорт в PDF (фича 009): общие случаи для `export_native.rs` и
//! `export_wasm.rs`.
//!
//! Сборка и запись идут поверх трейта `VaultBackend`, поэтому проверяются на
//! бэкенде в памяти одинаково на обоих таргетах (принцип II). Атомарность
//! настоящей записи на диск — свойство бэкенда, её проверяет `export_native.rs`
//! отдельно.

use typsidian_core::export::{
    self, batch, BatchPhase, ExportOutcome, ExportReport, ExportWriteProblem, JobStatus,
};
use typsidian_core::messages::{AppMessage, ResolveReason};
use typsidian_core::preview::{compile_note_resolved, DiagnosticSeverity, FontSet, PackageCache};
use typsidian_core::vault::vault_settings::VaultOverrides;

use super::MemoryVault;

const NOTE: &str = "Диссертация/Введение.typ";
const PDF: &str = "export/Диссертация/Введение.pdf";

const TWO_PAGES: &str = "#set document(title: \"Введение\")\n= Введение\n\nПервая страница.\n\n#pagebreak()\n\nВторая страница с формулой $E = m c^2$.\n";

const PICTURE: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"20\" height=\"10\"><rect width=\"20\" height=\"10\" fill=\"red\"/></svg>";

async fn export(
    vault: &mut MemoryVault,
    note: &str,
    text: Option<&str>,
    overrides: &VaultOverrides,
) -> ExportReport {
    export::export_note_pdf(
        vault,
        &PackageCache::new(),
        note,
        text,
        FontSet::builtin(),
        overrides,
    )
    .await
}

fn offline() -> VaultOverrides {
    VaultOverrides {
        auto_download_templates: Some(false),
        ..VaultOverrides::default()
    }
}

fn written(report: &ExportReport) -> u64 {
    match report.outcome {
        ExportOutcome::Written { bytes } => bytes,
        other => panic!(
            "ожидался записанный файл, а не {other:?}: {:?}",
            report.diagnostics
        ),
    }
}

/// Число страниц PDF — по записям `/Type /Page` (не `/Pages`).
///
/// Грубо, но достаточно: PDF от Typst пишет словари страниц несжатыми, а
/// разбирать PDF ради одного числа значило бы тянуть в тесты читатель PDF.
fn page_count(pdf: &[u8]) -> usize {
    let text = String::from_utf8_lossy(pdf);
    let re = regex::Regex::new(r"/Type\s*/Page\b").unwrap();
    re.find_iter(&text).count()
}

pub async fn a_note_exports_to_a_pdf_with_the_preview_page_count() {
    let mut vault = MemoryVault::new()
        .with_file(NOTE, TWO_PAGES)
        .with_file("Диссертация/схема.svg", PICTURE);
    let text = format!("{TWO_PAGES}\n#image(\"схема.svg\", width: 2cm)\n");

    let report = export(&mut vault, NOTE, Some(&text), &offline()).await;
    let bytes = written(&report);
    assert_eq!(report.pdf_path, PDF);

    let pdf = vault.bytes_of(PDF).expect("файл записан");
    assert_eq!(pdf.len() as u64, bytes);
    assert!(pdf.starts_with(b"%PDF-"), "это PDF");

    let preview = compile_note_resolved(
        &vault,
        &PackageCache::new(),
        NOTE,
        &text,
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap();
    assert_eq!(
        page_count(&pdf),
        preview.pages.len(),
        "страниц столько же, сколько в предпросмотре (FR-001)"
    );
}

pub async fn fonts_and_metadata_are_embedded() {
    let mut vault = MemoryVault::new();
    written(&export(&mut vault, NOTE, Some(TWO_PAGES), &offline()).await);
    let pdf = vault.bytes_of(PDF).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    assert!(
        text.contains("/FontFile"),
        "шрифты встроены (FR-002, SC-003)"
    );
    assert!(text.contains("Typsidian"), "метка создателя (FR-004)");
}

pub async fn the_editor_text_wins_over_the_disk() {
    let mut vault = MemoryVault::new().with_file(NOTE, "#panic(\"на диске сломано\")");
    // Текст редактора собирается, текст на диске — нет: экспорт обязан взять
    // первый (FR-006).
    written(&export(&mut vault, NOTE, Some(TWO_PAGES), &offline()).await);
}

pub async fn without_editor_text_the_note_is_read_from_disk() {
    let mut vault = MemoryVault::new().with_file(NOTE, TWO_PAGES);
    written(&export(&mut vault, NOTE, None, &offline()).await);
    assert!(vault.bytes_of(PDF).is_some());
}

pub async fn a_missing_note_is_reported_as_missing() {
    let mut vault = MemoryVault::new();
    let report = export(&mut vault, NOTE, None, &offline()).await;
    assert_eq!(report.outcome, ExportOutcome::NoteMissing);
    assert!(vault.bytes_of(PDF).is_none());
}

pub async fn a_broken_note_writes_nothing_and_keeps_the_old_pdf() {
    let mut vault = MemoryVault::new().with_file(PDF, b"old".to_vec());
    let report = export(&mut vault, NOTE, Some("#let x = \n= Ошибка"), &offline()).await;
    assert_eq!(report.outcome, ExportOutcome::CompileFailed);
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.severity == typsidian_core::preview::DiagnosticSeverity::Error));
    assert_eq!(
        vault.bytes_of(PDF).unwrap(),
        b"old",
        "прежний файл не тронут (FR-008)"
    );
}

pub async fn a_second_export_replaces_the_first() {
    let mut vault = MemoryVault::new();
    written(&export(&mut vault, NOTE, Some("Первая версия"), &offline()).await);
    let first = vault.bytes_of(PDF).unwrap();
    written(
        &export(
            &mut vault,
            NOTE,
            Some("Вторая версия, заметно длиннее первой"),
            &offline(),
        )
        .await,
    );
    let second = vault.bytes_of(PDF).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        vault.paths_under("export").len(),
        1,
        "рядом не появилось второго файла (FR-030)"
    );
}

pub async fn the_same_note_exports_to_the_same_bytes() {
    let mut vault = MemoryVault::new();
    written(&export(&mut vault, NOTE, Some(TWO_PAGES), &offline()).await);
    let first = vault.bytes_of(PDF).unwrap();
    written(&export(&mut vault, NOTE, Some(TWO_PAGES), &offline()).await);
    assert_eq!(
        first,
        vault.bytes_of(PDF).unwrap(),
        "без часов в метаданных результат детерминирован"
    );
}

pub async fn a_folder_in_place_of_the_pdf_is_not_touched() {
    let mut vault = MemoryVault::new().with_file(&format!("{PDF}/внутри.txt"), b"x".to_vec());
    let report = export(&mut vault, NOTE, Some(TWO_PAGES), &offline()).await;
    assert_eq!(
        report.outcome,
        ExportOutcome::WriteFailed {
            reason: ExportWriteProblem::TargetIsFolder
        }
    );
    assert!(vault.bytes_of(&format!("{PDF}/внутри.txt")).is_some());
}

pub async fn a_read_only_vault_refuses_the_write() {
    let mut vault = MemoryVault::new().read_only();
    let report = export(&mut vault, NOTE, Some(TWO_PAGES), &offline()).await;
    assert_eq!(
        report.outcome,
        ExportOutcome::WriteFailed {
            reason: ExportWriteProblem::AccessDenied
        }
    );
}

pub async fn the_export_folder_setting_is_honoured() {
    let mut vault = MemoryVault::new();
    let overrides = VaultOverrides {
        export_folder: Some("Выгрузка/2026".into()),
        ..offline()
    };
    let report = export(&mut vault, "Идея.typ", Some(TWO_PAGES), &overrides).await;
    written(&report);
    assert_eq!(report.pdf_path, "Выгрузка/2026/Идея.pdf");
    assert!(vault.bytes_of("Выгрузка/2026/Идея.pdf").is_some());
}

pub async fn nothing_but_the_pdf_is_written() {
    let mut vault = MemoryVault::new().with_file(NOTE, TWO_PAGES);
    let before = vault.paths_under("");
    written(&export(&mut vault, NOTE, None, &offline()).await);
    let mut after = vault.paths_under("");
    after.retain(|path| !before.contains(path));
    assert_eq!(after, vec![PDF.to_string()], "FR-032");
}

// --- Пакет ------------------------------------------------------------------

fn report_for(path: &str, outcome: ExportOutcome) -> ExportReport {
    ExportReport {
        note_path: path.into(),
        pdf_path: format!("export/{path}"),
        outcome,
        diagnostics: Vec::new(),
    }
}

fn ok(path: &str) -> ExportReport {
    report_for(path, ExportOutcome::Written { bytes: 10 })
}

fn paths(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn statuses(state: &batch::BatchState) -> Vec<JobStatus> {
    state.jobs.iter().map(|job| job.status).collect()
}

pub fn a_batch_runs_in_list_order_one_at_a_time() {
    let state = batch::start(
        paths(&["a.typ", "b.typ", "a.typ"]),
        false,
        &VaultOverrides::default(),
    );
    assert_eq!(state.jobs.len(), 2, "повтор пути сворачивается");

    let (state, first) = batch::next(state);
    assert_eq!(first.as_deref(), Some("a.typ"));
    let (state, second) = batch::next(state);
    assert_eq!(second, None, "второго задания, пока первое в работе, нет");

    let state = batch::record(state, ok("a.typ"), 100);
    let (state, second) = batch::next(state);
    assert_eq!(second.as_deref(), Some("b.typ"));
    let state = batch::record(state, ok("b.typ"), 300);
    let (state, none) = batch::next(state);
    assert_eq!(none, None);
    assert_eq!(state.phase, BatchPhase::Finished);

    let view = batch::view(state);
    assert_eq!((view.processed, view.total), (2, 2));
    assert_eq!(view.summary.done, 2);
    assert!(view.is_over);
}

pub fn the_estimate_appears_after_the_first_note() {
    let state = batch::start(paths(&["a", "b", "c"]), false, &VaultOverrides::default());
    let (state, _) = batch::next(state);
    assert_eq!(
        batch::view(state.clone()).remaining_estimate_ms,
        None,
        "оценивать не по чему (FR-017)"
    );
    let state = batch::record(state, ok("a"), 200);
    let (state, _) = batch::next(state);
    // Осталось: одна в работе и одна в очереди, по 200 мс.
    assert_eq!(batch::view(state).remaining_estimate_ms, Some(400));
}

pub fn a_failure_does_not_stop_the_batch_by_default() {
    let state = batch::start(paths(&["a", "b"]), false, &VaultOverrides::default());
    let (state, _) = batch::next(state);
    let state = batch::record(state, report_for("a", ExportOutcome::CompileFailed), 5);
    let (state, next) = batch::next(state);
    assert_eq!(next.as_deref(), Some("b"), "FR-021");
    assert_eq!(
        statuses(&state),
        vec![JobStatus::Failed, JobStatus::Exporting]
    );
}

pub fn stop_on_error_cancels_the_rest() {
    let state = batch::start(paths(&["a", "b", "c"]), true, &VaultOverrides::default());
    let (state, _) = batch::next(state);
    let state = batch::record(state, report_for("a", ExportOutcome::CompileFailed), 5);
    assert_eq!(
        state.phase,
        BatchPhase::StoppedOnError {
            note_path: "a".into()
        }
    );
    assert_eq!(
        statuses(&state),
        vec![
            JobStatus::Failed,
            JobStatus::Cancelled,
            JobStatus::Cancelled
        ]
    );
    assert_eq!(batch::next(state).1, None);
}

pub fn cancel_lets_the_current_note_finish() {
    let state = batch::start(paths(&["a", "b", "c"]), false, &VaultOverrides::default());
    let (state, _) = batch::next(state);
    let state = batch::cancel(state);
    assert_eq!(state.phase, BatchPhase::Cancelled);
    assert_eq!(
        statuses(&state),
        vec![
            JobStatus::Exporting,
            JobStatus::Cancelled,
            JobStatus::Cancelled
        ]
    );
    assert!(!batch::view(state.clone()).is_over, "текущая ещё пишется");
    assert_eq!(
        batch::next(state.clone()).1,
        None,
        "отмена побеждает очередь (FR-019)"
    );

    let state = batch::record(state, ok("a"), 10);
    let view = batch::view(state);
    assert_eq!(view.summary.done, 1, "готовый файл остаётся");
    assert_eq!(view.summary.not_exported, 2);
    assert!(view.is_over);
}

pub fn only_a_queued_note_can_be_skipped() {
    let state = batch::start(paths(&["a", "b"]), false, &VaultOverrides::default());
    let (state, _) = batch::next(state);
    let state = batch::skip(state, "a");
    let state = batch::skip(state, "b");
    assert_eq!(
        statuses(&state),
        vec![JobStatus::Exporting, JobStatus::Skipped]
    );
    let state = batch::record(state, ok("a"), 1);
    let (state, next) = batch::next(state);
    assert_eq!(next, None);
    assert_eq!(state.phase, BatchPhase::Finished);
}

pub fn a_stray_report_changes_nothing() {
    let state = batch::start(paths(&["a", "b"]), false, &VaultOverrides::default());
    let (state, _) = batch::next(state);
    let before = state.clone();
    let after = batch::record(state, ok("b"), 1);
    assert_eq!(before, after, "итог для заметки не в работе игнорируется");
}

pub fn warnings_mark_the_row() {
    let state = batch::start(paths(&["a"]), false, &VaultOverrides::default());
    let (state, _) = batch::next(state);
    let mut report = ok("a");
    report
        .diagnostics
        .push(typsidian_core::preview::Diagnostic {
            severity: typsidian_core::preview::DiagnosticSeverity::Warning,
            message: "шрифт заменён".into(),
            location: None,
            template: None,
            app: None,
        });
    let state = batch::record(state, report, 1);
    assert_eq!(statuses(&state), vec![JobStatus::DoneWithWarnings]);
}

pub fn an_empty_batch_is_already_over() {
    let state = batch::start(Vec::new(), false, &VaultOverrides::default());
    assert_eq!(state.phase, BatchPhase::Finished);
    assert!(batch::view(state).is_over);
}

pub fn the_batch_remembers_the_export_folder() {
    let overrides = VaultOverrides {
        export_folder: Some("pdf".into()),
        ..VaultOverrides::default()
    };
    assert_eq!(
        batch::start(paths(&["a"]), false, &overrides).export_folder,
        "pdf"
    );
}

// Фича 010 (T042/T043): отказ экспорта несёт вид сообщения, если его
// сформировало приложение, и не несёт — если это текст Typst.

/// Ошибка в самом тексте — сообщение компилятора, показывается как есть.
pub async fn a_typst_error_carries_no_app_message() {
    let mut vault = MemoryVault::new();
    let report = export(&mut vault, NOTE, Some("#let x = \n= Ошибка"), &offline()).await;
    assert_eq!(report.outcome, ExportOutcome::CompileFailed);
    let error = report
        .diagnostics
        .iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
        .expect("ошибка сборки названа");
    assert_eq!(error.app, None, "{error:?}");
}

/// Связанный файл не прочитался — это слова приложения, путь дословно.
pub async fn an_unreadable_dependency_is_an_app_message() {
    let mut vault = MemoryVault::new();
    let report = export(
        &mut vault,
        NOTE,
        Some("#image(\"рисунки/нет-такого.svg\")\n"),
        &offline(),
    )
    .await;
    assert_eq!(report.outcome, ExportOutcome::CompileFailed);
    assert!(
        report.diagnostics.iter().any(|d| matches!(
            &d.app,
            Some(AppMessage::DependencyUnreadable { path, reason: ResolveReason::NotFound })
                if path.ends_with("рисунки/нет-такого.svg")
        )),
        "{:?}",
        report.diagnostics
    );
    assert!(vault.bytes_of(PDF).is_none());
}

/// Путь заметки вне vault — отказ приложения, а не текст Typst.
pub async fn a_note_outside_the_vault_is_an_app_message() {
    let mut vault = MemoryVault::new();
    let report = export(
        &mut vault,
        "../снаружи.typ",
        Some("= Снаружи\n"),
        &offline(),
    )
    .await;
    assert_eq!(report.outcome, ExportOutcome::CompileFailed);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| matches!(&d.app, Some(AppMessage::NoteOutsideVault { path }) if path.contains("снаружи.typ"))),
        "{:?}",
        report.diagnostics
    );
}
