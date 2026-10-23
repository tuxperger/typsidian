//! Экспорт презентации (фича 035) — общая таблица для обоих таргетов.
//!
//! Файлы `slides_export_native.rs` / `slides_export_wasm.rs` только вызывают
//! эти функции своим harness'ом (принцип II). Разметку `<pdfpc>` тесты пишут
//! руками, как в 019.

#![allow(dead_code)]

use typsidian_core::preview::{FontSet, PackageCache};
use typsidian_core::slides::export::{
    export_slides, SlidesExportOutcome, SlidesExportReport, SlidesExportRequest, SlidesFormat,
    StepChoice,
};
use typsidian_core::vault::vault_settings::VaultOverrides;

use super::MemoryVault;

const NOTE: &str = "Доклад.typ";

/// 4 страницы: слайд 1 с заметкой, слайд 2 в два шага, слайд 3.
fn deck() -> String {
    let mut text = String::from("#set page(width: 16cm, height: 9cm)\n");
    for (i, number) in [1, 2, 2, 3].into_iter().enumerate() {
        if i > 0 {
            text.push_str("#pagebreak()\n");
        }
        text.push_str(&format!(
            "Страница {i}\n#metadata((t: \"LogicalSlide\", v: {number})) <pdfpc>\n"
        ));
        if i == 0 {
            text.push_str("#metadata((t: \"Note\", v: \"Сказать про стенд\")) <pdfpc>\n");
        }
    }
    text
}

fn overrides() -> VaultOverrides {
    VaultOverrides {
        auto_download_templates: Some(false),
        ..VaultOverrides::default()
    }
}

async fn export(
    vault: &mut MemoryVault,
    format: SlidesFormat,
    steps: StepChoice,
) -> SlidesExportReport {
    export_slides(
        vault,
        &PackageCache::new(),
        NOTE,
        None,
        FontSet::builtin(),
        &overrides(),
        &SlidesExportRequest {
            format,
            steps,
            png_scale: 2,
        },
    )
    .await
}

fn written(report: &SlidesExportReport) -> (Vec<String>, u32) {
    match &report.outcome {
        SlidesExportOutcome::Written { files, pages } => (files.clone(), *pages),
        other => panic!("ждали Written: {other:?} {:?}", report.diagnostics),
    }
}

/// Число страниц PDF по словарю `/Pages`.
fn pdf_pages(bytes: &[u8]) -> usize {
    let text = String::from_utf8_lossy(bytes);
    text.matches("/Type /Page\n")
        .count()
        .max(text.matches("/Type /Page ").count())
        .max(text.matches("/Type/Page/").count())
}

// ── US1 ─────────────────────────────────────────────────────────────────────

/// PDF слайдов: последний шаг — страница на слайд, все шаги — на шаг (SC-001).
pub async fn pdf_has_a_page_per_slide_or_step() {
    let mut vault = MemoryVault::new().with_file(NOTE, deck());
    let (files, pages) =
        written(&export(&mut vault, SlidesFormat::Pdf, StepChoice::LastStep).await);
    assert_eq!(files, ["export/Доклад.pdf"]);
    assert_eq!(pages, 3);
    let last = vault.bytes_of("export/Доклад.pdf").unwrap();
    let (_, all_pages) =
        written(&export(&mut vault, SlidesFormat::Pdf, StepChoice::AllSteps).await);
    assert_eq!(all_pages, 4);
    let all = vault.bytes_of("export/Доклад.pdf").unwrap();
    assert!(all.len() > last.len(), "все шаги — больше страниц");
    assert_eq!((pdf_pages(&last), pdf_pages(&all)), (3, 4), "SC-001");
}

/// PDF со страницами заметок: страница на слайд, заметки под слайдом.
pub async fn pdf_with_notes_has_slides_and_notes() {
    let mut vault = MemoryVault::new().with_file(NOTE, deck());
    let (files, pages) =
        written(&export(&mut vault, SlidesFormat::PdfWithNotes, StepChoice::LastStep).await);
    assert_eq!(files, ["export/Доклад.notes.pdf"]);
    assert_eq!(pages, 3);
    let bytes = vault.bytes_of("export/Доклад.notes.pdf").unwrap();
    assert!(bytes.starts_with(b"%PDF"));
}

/// PDF для pdfpc: все шаги и `.pdfpc` рядом с заметками и шагами (SC-002).
pub async fn pdfpc_file_lies_next_to_the_pdf() {
    let mut vault = MemoryVault::new().with_file(NOTE, deck());
    let (files, pages) =
        written(&export(&mut vault, SlidesFormat::PdfPc, StepChoice::LastStep).await);
    assert_eq!(files, ["export/Доклад.pdf", "export/Доклад.pdfpc"]);
    assert_eq!(pages, 4, "pdfpc — всегда все шаги");
    let json: serde_json::Value =
        serde_json::from_slice(&vault.bytes_of("export/Доклад.pdfpc").unwrap()).unwrap();
    assert_eq!(json["pdfpcFormat"], 2);
    assert_eq!(json["pages"].as_array().unwrap().len(), 4);
    assert_eq!(json["pages"][0]["note"], "Сказать про стенд");
    assert_eq!(json["pages"][2]["label"], "2");
    assert_eq!(json["pages"][2]["overlay"], 1);
}

/// Ошибка сборки — ничего не записано, причина названа.
pub async fn a_broken_deck_writes_nothing() {
    let mut vault = MemoryVault::new().with_file(NOTE, "#let x = (\n");
    let report = export(&mut vault, SlidesFormat::Pdf, StepChoice::LastStep).await;
    assert_eq!(report.outcome, SlidesExportOutcome::CompileFailed);
    assert!(!report.diagnostics.is_empty());
    assert!(vault.bytes_of("export/Доклад.pdf").is_none());
}

// ── US2 ─────────────────────────────────────────────────────────────────────

/// Картинки: PNG и SVG по файлу на слайд или шаг, номер в имени (FR-002).
pub async fn images_are_numbered_files() {
    let mut vault = MemoryVault::new().with_file(NOTE, deck());
    let (files, _) = written(&export(&mut vault, SlidesFormat::Png, StepChoice::LastStep).await);
    assert_eq!(
        files,
        [
            "export/Доклад.slides/01.png",
            "export/Доклад.slides/02.png",
            "export/Доклад.slides/03.png"
        ]
    );
    let png = vault.bytes_of("export/Доклад.slides/01.png").unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    // Масштаб 2×: ширина 16 см ≈ 453.5 пт → 907 точек.
    let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
    assert!((900..=910).contains(&width), "{width}");

    let (files, _) = written(&export(&mut vault, SlidesFormat::Svg, StepChoice::AllSteps).await);
    assert_eq!(
        files,
        [
            "export/Доклад.slides/01.svg",
            "export/Доклад.slides/02-1.svg",
            "export/Доклад.slides/02-2.svg",
            "export/Доклад.slides/03.svg"
        ]
    );
    let svg = String::from_utf8(vault.bytes_of("export/Доклад.slides/02-2.svg").unwrap()).unwrap();
    assert!(svg.contains("<svg"));
}
