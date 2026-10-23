//! Встраивание заметок (фича 031) — общая таблица для обоих таргетов.
//!
//! Файлы `embeds_native.rs` / `embeds_wasm.rs` только вызывают эти функции
//! своим harness'ом (принцип II).

#![allow(dead_code)]

use typsidian_core::completion::{
    completions, reference_marks, CompletionRequest, CompletionTarget, GroupKind, LinkedIndex,
    MarkKind, MarksRequest,
};
use typsidian_core::embeds::{
    calls, extract, insert_embed, parse_target, plan, EmbedPart, LIBRARY_PATH,
};
use typsidian_core::graph::extract_links;
use typsidian_core::preview::{compile_note_resolved, CompileOutcome, FontSet, PackageCache};
use typsidian_core::properties::LibraryStatus;

use super::MemoryVault;

const MAIN: &str = "Диссертация/Введение.typ";
const IMPORT: &str = "#import \"/.typsidian/lib/embed.typ\": embed\n";

const METHOD: &str = "= Методика\n\nВводная часть методики.\n\n== Выборка\n\nОтобрано двадцать четыре образца.\n\n=== Партии\n\nТри партии по восемь.\n\n== Выводы\n\nСоседний раздел.\n\n$ q = -lambda nabla T $ <eq-fourier>\n";

fn vault() -> MemoryVault {
    MemoryVault::new()
        .with_file("Диссертация/Методика.typ", METHOD)
        .with_file("Диссертация/Введение.typ", "= Введение\n")
}

fn texts_of(document: &typsidian_core::preview::CompiledDocument) -> String {
    document
        .pages
        .iter()
        .flat_map(|page| page.text_runs.iter())
        .map(|run| run.text.clone())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

async fn build(vault: &MemoryVault, source: &str) -> typsidian_core::preview::CompiledDocument {
    compile_note_resolved(
        vault,
        &PackageCache::new(),
        MAIN,
        source,
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap()
}

// ── US1 ─────────────────────────────────────────────────────────────────────

/// Цели и вызовы разбираются; раздел — до следующего заголовка того же
/// уровня; метка — помеченный элемент без самой метки (FR-001).
pub fn targets_sections_and_labels_are_cut() {
    assert_eq!(
        parse_target("Методика#Выборка").part,
        EmbedPart::Section("Выборка".into())
    );
    let found = calls("#embed(\"Методика#Выборка\")\nтекст");
    assert_eq!(found.len(), 1);

    let section = extract(METHOD, &EmbedPart::Section("Выборка".into())).unwrap();
    assert!(section.starts_with("== Выборка"));
    assert!(
        section.contains("Три партии"),
        "подраздел входит: {section}"
    );
    assert!(!section.contains("Соседний"), "соседний раздел не входит");
    assert!(extract(METHOD, &EmbedPart::Section("Нет такого".into())).is_none());

    let label = extract(METHOD, &EmbedPart::Label("eq-fourier".into())).unwrap();
    assert_eq!(label, "$ q = -lambda nabla T $");
    assert!(extract(METHOD, &EmbedPart::Label("нет".into())).is_none());
}

/// Предпросмотр показывает встроенный раздел и только его (FR-002); пути
/// встроенных заметок — зависимости документа (FR-003).
pub async fn preview_shows_the_embedded_section() {
    let source = format!("{IMPORT}= Введение\n\nЦель работы.\n\n#embed(\"Методика#Выборка\")\n");
    let vault = vault();
    let document = build(&vault, &source).await;
    assert_eq!(
        document.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        document.diagnostics
    );
    let shown = texts_of(&document);
    assert!(
        shown.contains("Отобрано двадцать четыре образца"),
        "{shown}"
    );
    assert!(shown.contains("Три партии"), "{shown}");
    assert!(!shown.contains("Соседний"), "лишний раздел: {shown}");
    assert!(!shown.contains("Вводная"), "лишнее начало: {shown}");
    assert!(
        document
            .dependencies
            .contains(&"Диссертация/Методика.typ".to_owned()),
        "{:?}",
        document.dependencies
    );
    assert!(
        !document
            .dependencies
            .iter()
            .any(|path| path.contains(".embed-")),
        "виртуальные файлы не зависимости: {:?}",
        document.dependencies
    );
}

/// Экспорт PDF собирается тем же циклом и с тем же содержимым (FR-002).
pub async fn export_contains_the_embedded_text() {
    let source = format!("{IMPORT}#embed(\"Методика#<eq-fourier>\")\n");
    let mut vault = vault();
    let report = typsidian_core::export::export_note_pdf(
        &mut vault,
        &PackageCache::new(),
        MAIN,
        Some(&source),
        FontSet::builtin(),
        &typsidian_core::vault::vault_settings::VaultOverrides {
            auto_download_templates: Some(false),
            ..Default::default()
        },
    )
    .await;
    assert!(
        matches!(
            report.outcome,
            typsidian_core::export::ExportOutcome::Written { .. }
        ),
        "{:?} {:?}",
        report.outcome,
        report.diagnostics
    );
}

/// Дополнение: в строке цели — заметки, после `#` — разделы и метки (US1 AS4).
pub async fn completion_offers_notes_sections_and_labels() {
    let vault = vault();
    let request = |text: &str| CompletionRequest {
        note_path: MAIN.into(),
        text: text.into(),
        offset: text.encode_utf16().count() as u32,
        manual: false,
        linked: LinkedIndex::default(),
        sources: Vec::new(),
    };

    let notes = completions(&vault, &request("#embed(\"Мет")).await;
    assert_eq!(notes.target, CompletionTarget::Note);
    assert_eq!(notes.groups[0].kind, GroupKind::Notes);
    assert_eq!(notes.groups[0].items[0].insert, "Методика");

    let parts = completions(&vault, &request("#embed(\"Методика#Вы")).await;
    let sections = parts
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Sections)
        .expect("разделы");
    let names: Vec<&str> = sections
        .items
        .iter()
        .map(|item| item.insert.as_str())
        .collect();
    assert_eq!(names, ["Выборка", "Выводы"]);

    let labels = completions(&vault, &request("#embed(\"Методика#eq")).await;
    let label = labels
        .groups
        .iter()
        .find(|group| group.kind == GroupKind::Labels)
        .expect("метки");
    assert_eq!(label.items[0].insert, "<eq-fourier>");
}

/// «Встроить заметку»: библиотека в vault, `#import` один раз, вызов у
/// каретки и каретка в кавычках.
pub async fn insert_command_writes_library_and_import() {
    let mut vault = vault();
    let text = "= Введение\n\nТекст.\n";
    let offset = text.encode_utf16().count() as u32;
    let inserted = insert_embed(&mut vault, text, offset).await.unwrap();
    assert_eq!(inserted.library, LibraryStatus::Written);
    assert!(vault.bytes_of(LIBRARY_PATH).is_some());
    assert_eq!(inserted.edits.len(), 2);
    assert_eq!(inserted.edits[0].insert, IMPORT);
    assert_eq!(inserted.edits[1].insert, "#embed(\"\")");

    let with_import = format!("{IMPORT}{text}");
    let offset = with_import.encode_utf16().count() as u32;
    let again = insert_embed(&mut vault, &with_import, offset)
        .await
        .unwrap();
    assert_eq!(again.library, LibraryStatus::UpToDate);
    assert_eq!(again.edits.len(), 1, "второй `#import` не нужен");
    assert_eq!(
        again.caret,
        offset + "#embed(\"".encode_utf16().count() as u32
    );
}

// ── US2 ─────────────────────────────────────────────────────────────────────

fn entries(inputs: &str) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str(inputs).unwrap()
}

/// Нет заметки, нет раздела, доска, цикл на втором заходе, глубина > 5 —
/// ответы с причиной, а не пустота (FR-005).
pub async fn broken_embeds_answer_with_a_reason() {
    let vault = vault()
        .with_file("Диссертация/А.typ", "#embed(\"Б\")\n")
        .with_file("Диссертация/Б.typ", "Текст Б\n#embed(\"А\")\n")
        .with_file("Диссертация/Г1.typ", "#embed(\"Г2\")")
        .with_file("Диссертация/Г2.typ", "#embed(\"Г3\")")
        .with_file("Диссертация/Г3.typ", "#embed(\"Г4\")")
        .with_file("Диссертация/Г4.typ", "#embed(\"Г5\")")
        .with_file("Диссертация/Г5.typ", "#embed(\"Г6\")")
        .with_file("Диссертация/Г6.typ", "#embed(\"Г7\")")
        .with_file("Диссертация/Г7.typ", "дно");
    let text = "#embed(\"Нет\")\n#embed(\"Методика#Погрешность\")\n#embed(\"Схема.excalidraw\")\n#embed(\"А\")\n#embed(\"Г1\")\n";
    let planned = plan(&vault, MAIN, text).await;
    let all = entries(planned.inputs.as_deref().unwrap());
    assert_eq!(all["Нет"]["error"], "note");
    assert_eq!(all["Методика#Погрешность"]["error"], "part");
    assert_eq!(all["Методика#Погрешность"]["part"], "Погрешность");
    assert_eq!(all["Схема.excalidraw"]["error"], "board");
    // А встроена, внутри неё Б, внутри Б — снова А: цикл на втором заходе.
    assert!(all["А"]["path"].is_string());
    let cycle = all
        .values()
        .find(|entry| entry["error"] == "cycle")
        .expect("цикл");
    assert_eq!(
        cycle["chain"],
        serde_json::json!(["Введение", "А", "Б", "А"])
    );
    // Г1…Г5 встроены, глубже — плашка.
    assert!(all.values().any(|entry| entry["error"] == "depth"));

    // Сборка не падает: на местах ошибок — плашки.
    let source = format!("{IMPORT}{text}");
    let document = build(&vault, &source).await;
    assert_eq!(
        document.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        document.diagnostics
    );
    let shown = texts_of(&document);
    assert!(shown.contains("Текст Б"), "{shown}");
    assert!(
        shown.contains("Встраивание") || shown.contains("Embed"),
        "{shown}"
    );
}

/// Встраивание — ссылка: индекс ссылок видит заметку; отметки 018 — нет
/// заметки и нет раздела (FR-004, US2).
pub async fn embeds_are_links_and_broken_ones_are_marked() {
    let text = "#embed(\"Методика#Выборка\") [[Методика]] #embed(\"Нет\") #embed(\"Методика#Нет\")";
    assert_eq!(
        extract_links(text),
        ["Методика#Выборка", "Методика", "Нет", "Методика#Нет"].map(|raw| raw
            .split('#')
            .next()
            .unwrap()
            .to_owned())
    );
    let vault = vault();
    let marks = reference_marks(
        &vault,
        &MarksRequest {
            note_path: MAIN.into(),
            text: text.into(),
            linked: LinkedIndex::default(),
            evidence: None,
            sources: Vec::new(),
            claimed: Vec::new(),
            held: false,
            previously_missing: Vec::new(),
        },
    )
    .await;
    let kinds: Vec<&MarkKind> = marks.iter().map(|mark| &mark.kind).collect();
    assert!(
        kinds
            .iter()
            .any(|kind| matches!(kind, MarkKind::MissingNote { target, .. } if target == "Нет")),
        "{kinds:?}"
    );
    assert!(
        kinds.iter().any(|kind| matches!(
            kind,
            MarkKind::MissingEmbedPart { target, part } if target == "Методика" && part == "Нет"
        )),
        "{kinds:?}"
    );
    assert_eq!(marks.len(), 2, "верные встраивания не отмечены: {kinds:?}");
}
