//! Экспорт заметок в PDF (фича 009).
//!
//! Сборка — тот же цикл резолва и доустановки шаблонов, что у предпросмотра
//! (`preview::compile`); отличается только последний шаг: из свёрстанного
//! документа получается PDF, а не страницы SVG (research.md §3). Поэтому то,
//! что собирается в предпросмотре, собирается и в экспорте, и наоборот.
//!
//! Запись — через `VaultBackend::write_bytes`, атомарную на обоих таргетах
//! (research.md §4): на месте PDF в любой момент лежит либо прежний файл
//! целиком, либо новый целиком (FR-030), и отмена пакета не оставляет
//! полузаписанных файлов (FR-019).

pub mod batch;
pub mod candidates;
pub mod folder;
pub mod html;

#[cfg(target_arch = "wasm32")]
pub mod web;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::messages::AppMessage;
use crate::preview::compile::{build_with_templates, InlineExecutor, PdfMeta, Render};
use crate::preview::snapshot::SnapshotError;
use crate::preview::{Diagnostic, DiagnosticSeverity, FontSet, PackageCache};
use crate::vault::backend::{ReadError, VaultBackend, WriteError};
use crate::vault::vault_settings::VaultOverrides;
use crate::vault::FolderAccess;

pub use batch::{
    BatchJob, BatchPhase, BatchState, BatchStep, BatchSummary, BatchView, ExportFormat, JobStatus,
};
pub use candidates::{candidates, ExportCandidate, NotExportable};
pub use folder::{export_folder, pdf_path, validate_folder, ExportFolderProblem};
pub use html::{export_note_html, export_site_archive, export_site_index, html_path, SiteLabels};

/// Чем закончился экспорт одной заметки (data-model.md «ExportReport»).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ExportReport {
    pub note_path: String,
    /// Путь результата внутри vault — и тогда, когда файл не записан: его
    /// называют в сообщении об отказе.
    pub pdf_path: String,
    pub outcome: ExportOutcome,
    /// Те же диагностики, что показывает предпросмотр (FR-008, FR-016). При
    /// `Written` — только предупреждения.
    pub diagnostics: Vec<Diagnostic>,
    /// HTML (фича 034): элементы, выведенные рисунком, потому что HTML-вывод их
    /// пропускает (FR-003). У PDF пусто.
    #[serde(default)]
    pub as_images: Vec<String>,
}

/// Исход экспорта. Отказ — вариант значения, а не ошибка вызова: в пакете
/// каждый исход становится состоянием строки, и различать их обязан UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ExportOutcome {
    /// Файл записан целиком. `bytes` — его размер (FR-016).
    Written { bytes: u64 },
    /// Документ не собрался; файл не создан и не изменён (FR-008).
    CompileFailed,
    /// Документ собран, но записать его не удалось; прежний файл не тронут
    /// (FR-031).
    WriteFailed { reason: ExportWriteProblem },
    /// Заметки нет: её удалили или переименовали, пока она ждала очереди.
    NoteMissing,
    /// Заметка есть, но не читается.
    NoteUnreadable,
}

/// Почему PDF не записан (FR-031).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ExportWriteProblem {
    #[error("нет прав на запись в папку экспорта")]
    AccessDenied,
    /// На месте файла лежит папка с тем же именем. Её не трогаем.
    #[error("на месте файла лежит папка с тем же именем")]
    TargetIsFolder,
    #[error("не удалось записать файл на диск")]
    Io,
}

impl ExportReport {
    /// Собран ли и записан ли файл.
    pub fn is_written(&self) -> bool {
        matches!(self.outcome, ExportOutcome::Written { .. })
    }

    /// Записан, но сборка оставила предупреждения (FR-015, «готово с
    /// предупреждениями»).
    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Warning)
    }
}

/// Экспортирует заметку в PDF (contracts/core-export-api.md).
///
/// `source_text` — текст из редактора, если заметка открыта: в PDF попадает
/// то, что пользователь видит сейчас, включая незаписанные правки (FR-006,
/// FR-022). `None` — заметка читается с диска.
///
/// Вне папки экспорта vault не меняется (FR-032); единственное исключение —
/// установка недостающего внешнего шаблона, по тем же правилам, что у
/// предпросмотра (фича 007).
pub async fn export_note_pdf<B: VaultBackend>(
    backend: &mut B,
    packages: &PackageCache,
    note_path: &str,
    source_text: Option<&str>,
    fonts: FontSet,
    overrides: &VaultOverrides,
) -> ExportReport {
    let folder = export_folder(overrides);
    let target = pdf_path(Path::new(note_path), &folder);
    let report = |outcome, diagnostics| ExportReport {
        note_path: note_path.to_owned(),
        pdf_path: target.to_string_lossy().replace('\\', "/"),
        outcome,
        diagnostics,
        as_images: Vec::new(),
    };

    let owned;
    let text = match source_text {
        Some(text) => text,
        None => match backend.read(Path::new(note_path)).await {
            Ok(text) => {
                owned = text;
                owned.as_str()
            }
            Err(ReadError::NotFound) => return report(ExportOutcome::NoteMissing, Vec::new()),
            Err(ReadError::Io | ReadError::TooLarge) => {
                return report(ExportOutcome::NoteUnreadable, Vec::new())
            }
        },
    };

    let render = Render::Pdf(PdfMeta {
        ident: note_path.to_owned(),
    });
    let built = build_with_templates(
        &InlineExecutor,
        backend,
        packages,
        note_path,
        text,
        0,
        fonts,
        &overrides.template_policy(),
        &render,
    )
    .await;
    let (document, pdf) = match built {
        Ok(built) => built,
        // Путь заметки не выражается относительно vault — тот же отказ, что
        // увидел бы предпросмотр, названный словами, а не кодом.
        Err(SnapshotError::OutsideVault { path }) => {
            return report(
                ExportOutcome::CompileFailed,
                vec![Diagnostic::app(
                    DiagnosticSeverity::Error,
                    AppMessage::NoteOutsideVault { path },
                )],
            )
        }
    };
    let Some(bytes) = pdf else {
        return report(ExportOutcome::CompileFailed, document.diagnostics);
    };

    // Папка на месте файла: запись поверх неё либо упала бы невнятным `Io`,
    // либо — на некоторых бэкендах — снесла бы её содержимое.
    if matches!(
        backend.folder_access(&target).await,
        FolderAccess::Present | FolderAccess::NotWritable { .. }
    ) {
        return report(
            ExportOutcome::WriteFailed {
                reason: ExportWriteProblem::TargetIsFolder,
            },
            document.diagnostics,
        );
    }

    let written = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => backend.ensure_folder(parent).await,
        _ => Ok(()),
    };
    let written = match written {
        Ok(()) => backend.write_bytes(&target, &bytes).await,
        Err(error) => Err(error),
    };
    match written {
        Ok(()) => report(
            ExportOutcome::Written {
                bytes: bytes.len() as u64,
            },
            document.diagnostics,
        ),
        Err(error) => report(
            ExportOutcome::WriteFailed {
                reason: match error {
                    WriteError::AccessDenied => ExportWriteProblem::AccessDenied,
                    WriteError::Conflict | WriteError::Io => ExportWriteProblem::Io,
                },
            },
            document.diagnostics,
        ),
    }
}
