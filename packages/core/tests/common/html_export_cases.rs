//! Экспорт в HTML (фича 034) — общая таблица для обоих таргетов.
//!
//! Файлы `html_export_native.rs` / `html_export_wasm.rs` только вызывают эти
//! функции своим harness'ом (принцип II).

#![allow(dead_code)]

use typsidian_core::export::{
    export_note_html, export_site_archive, export_site_index, ExportOutcome, SiteLabels,
};
use typsidian_core::preview::{FontSet, PackageCache};
use typsidian_core::vault::vault_settings::VaultOverrides;

use super::MemoryVault;

/// PNG 1×1.
const PIXEL: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

const INTRO: &str = "#set text(lang: \"ru\")\n= Введение\n\nМетодика описана в [[Методика|методике]], обзор — в [[Обзор]].\n\n$ q = -lambda nabla T $\n\n#image(\"стенд.png\", width: 2cm)\n";
const METHOD: &str =
    "= Методика\n\nСсылка назад: [[Введение]].\n\n#rect(width: 2cm, height: 1cm)\n";

fn vault() -> MemoryVault {
    MemoryVault::new()
        .with_file("Диссертация/Введение.typ", INTRO)
        .with_file("Диссертация/Методика.typ", METHOD)
        .with_file("Обзор.typ", "= Обзор\n")
        .with_file("Диссертация/стенд.png", PIXEL)
}

fn labels() -> SiteLabels {
    SiteLabels {
        contents: "Оглавление".into(),
        outside: "вне экспорта".into(),
    }
}

fn overrides() -> VaultOverrides {
    VaultOverrides {
        auto_download_templates: Some(false),
        ..VaultOverrides::default()
    }
}

fn site() -> Vec<String> {
    vec![
        "Диссертация/Введение.typ".into(),
        "Диссертация/Методика.typ".into(),
    ]
}

async fn page(vault: &mut MemoryVault, path: &str, site: &[String]) -> String {
    let report = export_note_html(
        vault,
        &PackageCache::new(),
        path,
        None,
        FontSet::builtin(),
        &overrides(),
        site,
        &labels(),
    )
    .await;
    assert!(
        matches!(report.outcome, ExportOutcome::Written { .. }),
        "{:?} {:?}",
        report.outcome,
        report.diagnostics
    );
    String::from_utf8(vault.bytes_of(&report.pdf_path).expect("страница записана")).unwrap()
}

// ── US1 ─────────────────────────────────────────────────────────────────────

/// Страница: заголовок, текст, формула MathML, картинка встроена — внешних
/// адресов нет (FR-001, FR-004).
pub async fn a_note_becomes_a_self_contained_page() {
    let mut vault = vault();
    let path = "Диссертация/Введение.typ";
    let html = page(&mut vault, path, &[path.to_owned()]).await;
    assert!(html.contains("Введение"), "{html}");
    assert!(html.contains("<math"), "формула — MathML");
    assert!(html.contains("<img"), "картинка на месте");
    for (at, _) in html.match_indices("src=\"") {
        assert!(
            html[at + 5..].starts_with("data:"),
            "внешний адрес: {}",
            &html[at..(at + 60).min(html.len())]
        );
    }
    assert!(!html.contains("<nav"), "одна страница — без оглавления");
    assert!(vault.bytes_of("export/Диссертация/Введение.html").is_some());
}

/// Пропущенный HTML-выводом элемент — рисунком SVG и в итоге (FR-003).
pub async fn unsupported_elements_become_pictures() {
    let mut vault = vault();
    let path = "Диссертация/Методика.typ";
    let report = export_note_html(
        &mut vault,
        &PackageCache::new(),
        path,
        None,
        FontSet::builtin(),
        &overrides(),
        &[path.to_owned()],
        &labels(),
    )
    .await;
    assert_eq!(report.as_images, ["rect"]);
    let html = String::from_utf8(vault.bytes_of(&report.pdf_path).unwrap()).unwrap();
    assert!(html.contains("<svg"), "rect — рисунком");
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| d.message.contains("HTML export")),
        "служебных предупреждений нет: {:?}",
        report.diagnostics
    );
}

// ── US2 ─────────────────────────────────────────────────────────────────────

/// Ссылки внутри набора — на существующие страницы, вне набора — помечены;
/// оглавление и навигация; архив со всеми страницами (FR-002, FR-005, SC-001).
pub async fn a_folder_becomes_a_site() {
    let mut vault = vault();
    let site = site();
    let intro = page(&mut vault, &site[0], &site).await;
    let method = page(&mut vault, &site[1], &site).await;
    let index = export_site_index(&mut vault, &overrides(), &site, &labels())
        .await
        .unwrap();
    assert_eq!(index, "export/index.html");

    assert!(intro.contains("href=\"Методика.html\""), "{intro}");
    assert!(method.contains("href=\"Введение.html\""), "{method}");
    assert!(intro.contains("ts-outside"), "ссылка вне набора помечена");
    assert!(intro.contains("<nav class=\"ts-site-nav\"><a href=\"../index.html\">"));

    // SC-001: каждая ссылка на страницу ведёт на записанный файл.
    for (page_path, html) in [
        ("export/Диссертация/Введение.html", &intro),
        ("export/Диссертация/Методика.html", &method),
    ] {
        let dir = page_path.rsplit_once('/').unwrap().0;
        for (at, _) in html.match_indices("href=\"") {
            let href = &html[at + 6..];
            let href = &href[..href.find('"').unwrap()];
            if href.starts_with("http") || href.starts_with('#') {
                continue;
            }
            let mut parts: Vec<&str> = dir.split('/').collect();
            for part in href.split('/') {
                if part == ".." {
                    parts.pop();
                } else {
                    parts.push(part);
                }
            }
            let target = parts.join("/");
            assert!(
                vault.bytes_of(&target).is_some(),
                "битая ссылка {href} → {target}"
            );
        }
    }

    let index_html = String::from_utf8(vault.bytes_of("export/index.html").unwrap()).unwrap();
    assert!(index_html.contains("Диссертация/Введение.html"));

    let zip = export_site_archive(&vault, &overrides(), &site).await;
    assert_eq!(&zip[..4], b"PK\x03\x04");
    let text = String::from_utf8_lossy(&zip);
    assert!(text.contains("index.html"));
    assert!(text.contains("Диссертация/Методика.html"));
}

/// Ошибка сборки одной заметки не мешает остальным (Edge Cases).
pub async fn a_broken_note_does_not_stop_the_site() {
    let mut vault = vault().with_file("Сломанная.typ", "#let x = (\n");
    let report = export_note_html(
        &mut vault,
        &PackageCache::new(),
        "Сломанная.typ",
        None,
        FontSet::builtin(),
        &overrides(),
        &["Сломанная.typ".to_owned()],
        &labels(),
    )
    .await;
    assert_eq!(report.outcome, ExportOutcome::CompileFailed);
    assert!(!report.diagnostics.is_empty());
    let site = site();
    page(&mut vault, &site[0], &site).await;
}
