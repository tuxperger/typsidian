//! Экспорт презентации (фича 035).
//!
//! Слайды и шаги — те же сведения о страницах, что у показа 019: шаги — страницы
//! сборки, слайд — подряд идущие страницы с одним логическим номером `<pdfpc>`
//! (research.md §1). Отсюда — PDF подмножества страниц, PDF со страницами
//! заметок (вторая сборка из SVG слайдов), PDF с файлом `.pdfpc` и картинки
//! PNG/SVG. Сборка — цепочка 009; запись — `write_bytes`, атомарно.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::PageSlideInfo;
use crate::export::{export_folder, pdf_path, ExportWriteProblem};
use crate::messages::AppMessage;
use crate::preview::compile::{
    build_with_templates, compile_composed, unpack_files, CompileOutcome, ImagesSpec,
    InlineExecutor, PdfMeta, Render,
};
use crate::preview::snapshot::SnapshotError;
use crate::preview::{Diagnostic, DiagnosticSeverity, FontSet, PackageCache};
use crate::vault::backend::{ReadError, VaultBackend, WriteError};
use crate::vault::vault_settings::VaultOverrides;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SlidesFormat {
    Pdf,
    PdfWithNotes,
    PdfPc,
    Png,
    Svg,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum StepChoice {
    #[default]
    LastStep,
    AllSteps,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SlidesExportRequest {
    pub format: SlidesFormat,
    pub steps: StepChoice,
    /// Масштаб PNG, 1…4; у прочих форматов не читается.
    pub png_scale: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum SlidesExportOutcome {
    /// Записаны файлы (пути в vault); `pages` — страниц или картинок.
    Written {
        files: Vec<String>,
        pages: u32,
    },
    /// Документ не собрался — ничего не записано.
    CompileFailed,
    /// В документе нет страниц.
    NoSlides,
    WriteFailed {
        reason: ExportWriteProblem,
    },
    NoteMissing,
    NoteUnreadable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SlidesExportReport {
    pub note_path: String,
    pub outcome: SlidesExportOutcome,
    pub diagnostics: Vec<Diagnostic>,
}

/// Выбранная страница: её индекс, номер слайда и шага (с 1), шагов в слайде.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedPage {
    pub page: u32,
    pub slide: u32,
    pub step: u32,
    pub steps: u32,
}

/// Слайды и шаги по сведениям о страницах (research.md §1, SC-001).
pub fn selected_pages(slides: &[PageSlideInfo], steps: StepChoice) -> Vec<SelectedPage> {
    // Группы подряд идущих страниц с одним логическим номером.
    let mut groups: Vec<Vec<u32>> = Vec::new();
    let mut last: Option<u32> = None;
    for info in slides {
        match (info.logical_slide, last) {
            (Some(number), Some(previous)) if number == previous => {
                if let Some(group) = groups.last_mut() {
                    group.push(info.page_index);
                }
            }
            _ => groups.push(vec![info.page_index]),
        }
        last = info.logical_slide;
    }
    let mut out = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let slide = index as u32 + 1;
        let total = group.len() as u32;
        match steps {
            StepChoice::LastStep => out.push(SelectedPage {
                page: *group.last().unwrap_or(&0),
                slide,
                step: total,
                steps: total,
            }),
            StepChoice::AllSteps => {
                out.extend(group.iter().enumerate().map(|(at, page)| SelectedPage {
                    page: *page,
                    slide,
                    step: at as u32 + 1,
                    steps: total,
                }))
            }
        }
    }
    out
}

/// Файл заметок pdfpc (JSON, `pdfpcFormat: 2`) для PDF со всеми шагами (SC-002).
pub fn pdfpc_json(slides: &[PageSlideInfo]) -> String {
    let pages: Vec<Value> = selected_pages(slides, StepChoice::AllSteps)
        .into_iter()
        .map(|selected| {
            let mut page = json!({
                "idx": selected.page,
                "label": selected.slide.to_string(),
                "overlay": selected.step - 1,
                "forcedOverlay": false,
                "hidden": false,
            });
            let note = slides
                .iter()
                .find(|info| info.page_index == selected.page)
                .and_then(|info| info.notes.clone())
                .filter(|note| !note.trim().is_empty());
            if let (Value::Object(map), Some(note)) = (&mut page, note) {
                map.insert("note".into(), Value::String(note));
            }
            page
        })
        .collect();
    let document = json!({
        "pdfpcFormat": 2,
        "disableMarkdown": true,
        "pages": pages,
    });
    serde_json::to_string_pretty(&document).unwrap_or_default()
}

/// Имя картинки: `NN` или `NN-K` при нескольких шагах слайда.
fn image_name(selected: &SelectedPage, steps: StepChoice, width: usize, ext: &str) -> String {
    if steps == StepChoice::AllSteps && selected.steps > 1 {
        format!("{:0width$}-{}.{ext}", selected.slide, selected.step)
    } else {
        format!("{:0width$}.{ext}", selected.slide)
    }
}

/// Строка Typst в кавычках.
fn quoted(text: &str) -> String {
    format!(
        "\"{}\"",
        text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "")
    )
}

/// Документ «слайд + заметки докладчика под ним» (research.md §3).
fn notes_document(pages: &[(String, f64, Option<String>)]) -> String {
    let mut out = String::from("#set text(size: 11pt)\n");
    for (index, (svg, width, notes)) in pages.iter().enumerate() {
        out.push_str(&format!(
            "#page(width: {width}pt, height: auto, margin: 18pt)[\n#image({}, width: 100%)\n",
            quoted(svg)
        ));
        if let Some(notes) = notes.as_ref().filter(|notes| !notes.trim().is_empty()) {
            out.push_str(&format!("#v(10pt)\n#text({})\n", quoted(notes)));
        }
        out.push_str("]\n");
        let _ = index;
    }
    out
}

async fn write_file<B: VaultBackend>(
    backend: &mut B,
    target: &Path,
    bytes: &[u8],
) -> Result<(), ExportWriteProblem> {
    let ensured = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => backend.ensure_folder(parent).await,
        _ => Ok(()),
    };
    match ensured {
        Ok(()) => backend.write_bytes(target, bytes).await,
        Err(error) => Err(error),
    }
    .map_err(|error| match error {
        WriteError::AccessDenied => ExportWriteProblem::AccessDenied,
        _ => ExportWriteProblem::Io,
    })
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Экспорт презентации (FR-001…FR-004).
#[allow(clippy::too_many_arguments)]
pub async fn export_slides<B: VaultBackend>(
    backend: &mut B,
    packages: &PackageCache,
    note_path: &str,
    source_text: Option<&str>,
    fonts: FontSet,
    overrides: &VaultOverrides,
    request: &SlidesExportRequest,
) -> SlidesExportReport {
    let report = |outcome, diagnostics| SlidesExportReport {
        note_path: note_path.to_owned(),
        outcome,
        diagnostics,
    };
    let text = match source_text {
        Some(text) => text.to_owned(),
        None => match backend.read(Path::new(note_path)).await {
            Ok(text) => text,
            Err(ReadError::NotFound) => {
                return report(SlidesExportOutcome::NoteMissing, Vec::new())
            }
            Err(_) => return report(SlidesExportOutcome::NoteUnreadable, Vec::new()),
        },
    };
    let policy = overrides.template_policy();
    let outside = |path: String| {
        report(
            SlidesExportOutcome::CompileFailed,
            vec![Diagnostic::app(
                DiagnosticSeverity::Error,
                AppMessage::NoteOutsideVault { path },
            )],
        )
    };

    // Сведения о страницах — та же сборка, что у показа.
    let pages_build = build(
        backend,
        packages,
        note_path,
        &text,
        &fonts,
        &policy,
        Render::Pages,
    )
    .await;
    let (document, _) = match pages_build {
        Ok(built) => built,
        Err(SnapshotError::OutsideVault { path }) => return outside(path),
    };
    if document.outcome != CompileOutcome::Succeeded {
        return report(SlidesExportOutcome::CompileFailed, document.diagnostics);
    }
    if document.slides.is_empty() {
        return report(SlidesExportOutcome::NoSlides, document.diagnostics);
    }
    let warnings: Vec<Diagnostic> = document
        .diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .cloned()
        .collect();
    let steps = if request.format == SlidesFormat::PdfPc {
        StepChoice::AllSteps
    } else {
        request.steps
    };
    let selected = selected_pages(&document.slides, steps);
    let page_list: Vec<u32> = selected.iter().map(|s| s.page).collect();

    let folder = export_folder(overrides);
    let pdf = pdf_path(Path::new(note_path), &folder);
    let mut files: Vec<(PathBuf, Vec<u8>)> = Vec::new();

    match request.format {
        SlidesFormat::Pdf | SlidesFormat::PdfPc => {
            let built = build(
                backend,
                packages,
                note_path,
                &text,
                &fonts,
                &policy,
                Render::Pdf(PdfMeta {
                    ident: note_path.to_owned(),
                    pages: Some(page_list.clone()),
                }),
            )
            .await;
            let bytes = match built {
                Ok((_, Some(bytes))) => bytes,
                Ok((failed, None)) => {
                    return report(SlidesExportOutcome::CompileFailed, failed.diagnostics)
                }
                Err(SnapshotError::OutsideVault { path }) => return outside(path),
            };
            files.push((pdf.clone(), bytes));
            if request.format == SlidesFormat::PdfPc {
                let mut pdfpc = pdf.clone().into_os_string();
                pdfpc.push("pc");
                files.push((
                    PathBuf::from(pdfpc),
                    pdfpc_json(&document_slides(&text_slides(&selected, &document.slides)))
                        .into_bytes(),
                ));
            }
        }
        SlidesFormat::PdfWithNotes => {
            let built = build(
                backend,
                packages,
                note_path,
                &text,
                &fonts,
                &policy,
                Render::Images(ImagesSpec {
                    png_scale: None,
                    pages: page_list.clone(),
                }),
            )
            .await;
            let packed = match built {
                Ok((_, Some(packed))) => packed,
                Ok((failed, None)) => {
                    return report(SlidesExportOutcome::CompileFailed, failed.diagnostics)
                }
                Err(SnapshotError::OutsideVault { path }) => return outside(path),
            };
            let svgs = unpack_files(&packed);
            let mut virtual_files = Vec::new();
            let mut pages = Vec::new();
            for (index, (selected, svg)) in selected.iter().zip(svgs).enumerate() {
                let name = format!("slide-{index}.svg");
                let width = document
                    .pages
                    .get(selected.page as usize)
                    .map_or(595.0, |page| page.width_pt);
                let notes = document
                    .slides
                    .iter()
                    .find(|info| info.page_index == selected.page)
                    .and_then(|info| info.notes.clone());
                pages.push((format!("/{name}"), width, notes));
                virtual_files.push((name, svg));
            }
            let composed = compile_composed(
                "slides-notes.typ",
                &notes_document(&pages),
                &virtual_files,
                fonts.clone(),
                &Render::Pdf(PdfMeta {
                    ident: format!("{note_path}#notes"),
                    pages: None,
                }),
            );
            let Ok((composed, Some(bytes))) = composed else {
                return report(SlidesExportOutcome::CompileFailed, document.diagnostics);
            };
            let _ = composed;
            files.push((pdf.with_extension("notes.pdf"), bytes));
        }
        SlidesFormat::Png | SlidesFormat::Svg => {
            let png = request.format == SlidesFormat::Png;
            let built = build(
                backend,
                packages,
                note_path,
                &text,
                &fonts,
                &policy,
                Render::Images(ImagesSpec {
                    png_scale: png.then(|| request.png_scale.clamp(1, 4) as f32),
                    pages: page_list.clone(),
                }),
            )
            .await;
            let packed = match built {
                Ok((_, Some(packed))) => packed,
                Ok((failed, None)) => {
                    return report(SlidesExportOutcome::CompileFailed, failed.diagnostics)
                }
                Err(SnapshotError::OutsideVault { path }) => return outside(path),
            };
            let dir = pdf.with_extension("slides");
            let width = selected
                .last()
                .map_or(2, |last| last.slide.to_string().len().max(2));
            let ext = if png { "png" } else { "svg" };
            for (selected_page, bytes) in selected.iter().zip(unpack_files(&packed)) {
                files.push((
                    dir.join(image_name(selected_page, steps, width, ext)),
                    bytes,
                ));
            }
        }
    }

    let mut written = Vec::new();
    for (path, bytes) in &files {
        if let Err(reason) = write_file(backend, path, bytes).await {
            return report(SlidesExportOutcome::WriteFailed { reason }, warnings);
        }
        written.push(slashed(path));
    }
    report(
        SlidesExportOutcome::Written {
            files: written,
            pages: selected.len() as u32,
        },
        warnings,
    )
}

/// Сведения о страницах в порядке выбранных — для `.pdfpc` (все шаги).
fn text_slides(selected: &[SelectedPage], slides: &[PageSlideInfo]) -> Vec<PageSlideInfo> {
    selected
        .iter()
        .filter_map(|s| {
            slides
                .iter()
                .find(|info| info.page_index == s.page)
                .cloned()
        })
        .collect()
}

/// Страницы PDF с подмножеством: номера страниц в новом файле — по порядку.
fn document_slides(slides: &[PageSlideInfo]) -> Vec<PageSlideInfo> {
    slides
        .iter()
        .enumerate()
        .map(|(index, info)| PageSlideInfo {
            page_index: index as u32,
            ..info.clone()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(page: u32, slide: Option<u32>, notes: Option<&str>) -> PageSlideInfo {
        PageSlideInfo {
            page_index: page,
            source_start: None,
            logical_slide: slide,
            notes: notes.map(str::to_owned),
        }
    }

    #[test]
    fn steps_group_by_logical_slide() {
        let slides = [
            info(0, Some(1), Some("вступление")),
            info(1, Some(2), None),
            info(2, Some(2), None),
            info(3, None, None),
        ];
        let last: Vec<u32> = selected_pages(&slides, StepChoice::LastStep)
            .iter()
            .map(|s| s.page)
            .collect();
        assert_eq!(last, [0, 2, 3]);
        let all = selected_pages(&slides, StepChoice::AllSteps);
        assert_eq!(all.len(), 4);
        assert_eq!((all[2].slide, all[2].step, all[2].steps), (2, 2, 2));
        assert_eq!(
            image_name(&all[1], StepChoice::AllSteps, 2, "png"),
            "02-1.png"
        );
        assert_eq!(
            image_name(&all[0], StepChoice::AllSteps, 2, "png"),
            "01.png"
        );
    }

    #[test]
    fn pdfpc_lists_pages_with_overlays_and_notes() {
        let slides = [
            info(0, Some(1), Some("вступление")),
            info(1, Some(2), None),
            info(2, Some(2), None),
        ];
        let json: Value = serde_json::from_str(&pdfpc_json(&slides)).unwrap();
        assert_eq!(json["pdfpcFormat"], 2);
        assert_eq!(json["pages"][0]["note"], "вступление");
        assert_eq!(json["pages"][2]["label"], "2");
        assert_eq!(json["pages"][2]["overlay"], 1);
    }
}

/// Одна сборка цепочкой 009 с выбранным выводом.
#[allow(clippy::too_many_arguments)]
async fn build<B: VaultBackend>(
    backend: &mut B,
    packages: &PackageCache,
    note_path: &str,
    text: &str,
    fonts: &FontSet,
    policy: &crate::preview::compile::TemplatePolicy,
    render: Render,
) -> Result<(crate::preview::CompiledDocument, Option<Vec<u8>>), SnapshotError> {
    build_with_templates(
        &InlineExecutor,
        backend,
        packages,
        note_path,
        text,
        0,
        fonts.clone(),
        policy,
        &render,
    )
    .await
}
