//! Общие проверки сборки предпросмотра (фича 005, T054…T056, T073…T076, T091,
//! T093, T095, T099) — одна таблица для `cargo test` и `wasm-bindgen-test`
//! (принцип II). Vault — `MemoryVault`: сборка читает связанные файлы через
//! `VaultBackend`, а что под ним, ей безразлично.

#![allow(dead_code)]

use typsidian_core::messages::AppMessage;
use typsidian_core::preview::{
    compile_note, compile_note_resolved, compile_note_resolved_on, dependency_mark,
    CompileExecutor, CompileJob, CompileOutcome, CompileOutput, CompiledDocument,
    DiagnosticSeverity, ExecutorError, FontSet, PackageCache, ScrollAnchor, BUILD_TIME_LIMIT_SECS,
};

use super::MemoryVault;

const MAIN: &str = "заметка.typ";
const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"></svg>"#;

async fn build(vault: &MemoryVault, text: &str, revision: u64) -> CompiledDocument {
    compile_note_resolved(
        vault,
        &PackageCache::new(),
        MAIN,
        text,
        revision,
        FontSet::builtin(),
    )
    .await
    .unwrap()
}

/// T091, T095: импорт в два уровня и изображение собираются вместе с
/// заметкой, и каждый вошедший файл назван в зависимостях.
pub async fn a_two_level_import_and_an_image_build_with_the_note() {
    let vault = MemoryVault::new()
        .with_file(
            "оформление.typ",
            "#import \"цвета.typ\": акцент\n#let рамка = акцент\n",
        )
        .with_file("цвета.typ", "#let акцент = red\n")
        .with_file("рисунки/схема.svg", SVG);

    let doc = build(
        &vault,
        "#import \"оформление.typ\": рамка\n#image(\"рисунки/схема.svg\", width: 1cm)\n",
        1,
    )
    .await;

    assert_eq!(
        doc.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        doc.diagnostics
    );
    assert_eq!(
        doc.dependencies,
        ["оформление.typ", "рисунки/схема.svg", "цвета.typ"]
    );
}

/// FR-021: недостающий файл — тоже зависимость: его появление меняет
/// результат, и следить нужно именно за ним.
pub async fn a_missing_file_is_a_dependency_too() {
    let vault = MemoryVault::new();
    let doc = build(&vault, "#import \"нет.typ\": x\n", 1).await;

    assert_eq!(doc.outcome, CompileOutcome::Failed);
    assert_eq!(doc.dependencies, ["нет.typ"]);
}

/// T093, T099: два файла, импортирующие друг друга, — отказ с названной
/// причиной и местом, а не зависание и не переполнение стека.
pub async fn a_cyclic_import_fails_with_a_named_reason() {
    let vault = MemoryVault::new()
        .with_file("а.typ", "#import \"б.typ\": *\n#let из_а = 1\n")
        .with_file("б.typ", "#import \"а.typ\": *\n#let из_б = 2\n");

    let doc = build(&vault, "#import \"а.typ\": *\n", 1).await;

    assert_eq!(doc.outcome, CompileOutcome::Failed);
    let error = doc
        .diagnostics
        .iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
        .expect("циклическая связь обязана быть названа");
    assert!(error.message.contains("cyclic"), "{}", error.message);
    let place = error.location.as_ref().expect("место обязано быть указано");
    assert!(!place.is_open_note);
    // Оба участника цикла вошли в сборку — правка любого её перезапустит.
    assert_eq!(doc.dependencies, ["а.typ", "б.typ"]);
}

/// T056: собирается переданный текст, а файл заметки на диске не читается —
/// даже когда он есть и содержит другое.
pub async fn the_build_uses_the_given_text_and_never_reads_the_note_file() {
    let vault = MemoryVault::new().with_file(MAIN, "#let = сломано\n");

    let doc = build(&vault, "= Из буфера\n", 7).await;

    assert_eq!(
        doc.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        doc.diagnostics
    );
    assert_eq!(vault.read_count(MAIN), 0);
    assert!(doc.dependencies.is_empty());
}

/// T054, T055: результат помечен версией текста, из которого собран, — по
/// ней интерфейс отбрасывает опоздавший результат более старой сборки.
pub fn the_result_carries_the_revision_of_its_text() {
    let older = compile_note(MAIN, "= Раньше\n", 3, FontSet::builtin()).unwrap();
    let newer = compile_note(MAIN, "= Позже\n", 4, FontSet::builtin()).unwrap();

    assert_eq!((older.revision, newer.revision), (3, 4));
    assert_ne!(older.pages[0].digest, newer.pages[0].digest);
}

/// T073: причина словами и место — файл, строка и колонка от единицы.
pub fn a_diagnostic_names_its_reason_and_place() {
    let doc = compile_note(
        MAIN,
        "= Заголовок\n\nТекст #неизвестное\n",
        1,
        FontSet::builtin(),
    )
    .unwrap();

    let error = doc
        .diagnostics
        .iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
        .expect("ошибка обязана быть названа");
    assert!(
        error.message.contains("unknown variable"),
        "{}",
        error.message
    );
    let place = error.location.as_ref().expect("место обязано быть указано");
    assert_eq!(place.path, MAIN);
    assert!(place.is_open_note);
    // Место — само имя после `#`: восьмой знак третьей строки. Счёт идёт в
    // знаках, а не в байтах: кириллица перед ним занимает по два.
    assert_eq!((place.line, place.column), (3, 8));
}

/// T074: несколько замечаний идут по месту в тексте.
pub fn diagnostics_are_ordered_by_place() {
    let doc = compile_note(
        MAIN,
        "#set text(font: \"Нет такого шрифта\")\nтекст\n\n#let = 1\n",
        1,
        FontSet::builtin(),
    )
    .unwrap();

    let starts: Vec<u32> = doc
        .diagnostics
        .iter()
        .filter_map(|d| d.location.as_ref())
        .map(|place| place.byte_start)
        .collect();
    let mut sorted = starts.clone();
    sorted.sort_unstable();
    assert_eq!(starts, sorted);
}

/// T075: предупреждение приходит предупреждением и сборку не ломает.
pub fn a_warning_does_not_fail_the_build() {
    let doc = compile_note(
        MAIN,
        "#set text(font: \"Нет такого шрифта\")\nтекст\n",
        1,
        FontSet::builtin(),
    )
    .unwrap();

    assert_eq!(doc.outcome, CompileOutcome::Succeeded);
    assert!(!doc.pages.is_empty());
    assert!(doc
        .diagnostics
        .iter()
        .all(|d| d.severity == DiagnosticSeverity::Warning));
    assert!(
        !doc.diagnostics.is_empty(),
        "недостающий шрифт обязан быть назван"
    );
}

/// T076: ошибка в связанном файле несёт его путь и не выдаётся за ошибку
/// открытой заметки.
pub async fn a_diagnostic_in_a_linked_file_points_at_that_file() {
    let vault = MemoryVault::new().with_file("оформление.typ", "#let рамка = 1\n#let = 2\n");

    let doc = build(&vault, "#import \"оформление.typ\": рамка\n", 1).await;

    assert_eq!(doc.outcome, CompileOutcome::Failed);
    let place = doc
        .diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .find_map(|d| d.location.as_ref())
        .expect("место обязано быть указано");
    assert_eq!(place.path, "оформление.typ");
    assert!(!place.is_open_note);
    assert_eq!(place.line, 2);
}

// ── Удержание места при пересборке (T065, T066) ────────────────────────────

/// Документ, где у каждого абзаца свой текст и каждый занимает страницу.
fn paged(paragraphs: &[&str]) -> CompiledDocument {
    let body: Vec<String> = paragraphs.iter().map(|text| text.to_string()).collect();
    let source = format!(
        "#set page(width: 8cm, height: 4cm)\n{}\n",
        body.join("\n#pagebreak()\n")
    );
    compile_note(MAIN, &source, 1, FontSet::builtin()).unwrap()
}

fn anchor_at(document: &CompiledDocument, text: &str) -> ScrollAnchor {
    for page in &document.pages {
        if let Some(index) = page.text_runs.iter().position(|run| run.text == text) {
            return ScrollAnchor {
                page_index: page.index as u32,
                run_text: text.to_string(),
                run_index: index as u32,
            };
        }
    }
    panic!("в документе нет прогона «{text}»");
}

/// T065: место находится по тексту и после того, как переверстка унесла его
/// на другую страницу.
pub fn an_anchor_follows_its_text_to_another_page() {
    let before = paged(&["Первый", "Второй", "Третий"]);
    let anchor = anchor_at(&before, "Третий");
    assert_eq!(anchor.page_index, 2);

    let after =
        paged(&["Первый", "Вставка", "Ещё вставка", "Второй", "Третий"]).with_anchor(Some(&anchor));
    let resolved = after.anchor.expect("якорь обязан разрешиться");

    assert!(resolved.exact);
    assert_eq!(resolved.page_index, 4);
    let run = &after.pages[4].text_runs[resolved.run_index as usize];
    assert_eq!(run.text, "Третий");
}

/// Из одинаковых фрагментов выбирается ближайший к прежнему месту.
pub fn an_anchor_picks_the_nearest_of_equal_fragments() {
    let before = paged(&["Повтор", "Середина", "Повтор"]);
    let anchor = ScrollAnchor {
        page_index: 2,
        run_text: "Повтор".into(),
        run_index: 0,
    };
    let resolved = before.with_anchor(Some(&anchor)).anchor.unwrap();
    assert_eq!((resolved.page_index, resolved.exact), (2, true));
}

/// T066: текст исчез — возвращается ближайшее сохранившееся место, а не
/// начало документа.
pub fn a_vanished_anchor_resolves_nearby_and_never_to_the_start() {
    let before = paged(&["Первый", "Второй", "Третий", "Четвёртый"]);
    let anchor = anchor_at(&before, "Третий");

    let after =
        paged(&["Первый", "Второй", "Совсем другое", "Четвёртый"]).with_anchor(Some(&anchor));
    let resolved = after.anchor.expect("якорь обязан разрешиться");

    assert!(!resolved.exact);
    assert_eq!(resolved.page_index, 2);

    // Документ стал короче прежнего места — прижимаемся к его концу.
    let short = paged(&["Первый", "Второй"]).with_anchor(Some(&anchor));
    let resolved = short.anchor.unwrap();
    assert_eq!((resolved.page_index, resolved.exact), (1, false));
}

/// Без якоря и в документе без текста удерживать нечего.
pub fn no_anchor_without_a_request_or_without_text() {
    let document = paged(&["Первый"]);
    assert_eq!(document.clone().with_anchor(None).anchor, None);

    let empty = compile_note(MAIN, "#rect()\n", 1, FontSet::builtin()).unwrap();
    let anchor = ScrollAnchor {
        page_index: 0,
        run_text: "Первый".into(),
        run_index: 0,
    };
    assert_eq!(empty.with_anchor(Some(&anchor)).anchor, None);
}

// ── Исполнитель и предел времени (T017…T019, T023) ─────────────────────────

/// Исполнитель, у которого сборка не укладывается в предел.
struct OverTheLimit;

impl CompileExecutor for OverTheLimit {
    async fn run(&self, _job: CompileJob) -> Result<CompileOutput, ExecutorError> {
        Err(ExecutorError::TimedOut)
    }
}

/// Исполнитель, считающий проходы: всё остальное — обычная сборка.
#[derive(Default)]
struct Counting(std::cell::Cell<u32>);

impl CompileExecutor for Counting {
    async fn run(&self, job: CompileJob) -> Result<CompileOutput, ExecutorError> {
        self.0.set(self.0.get() + 1);
        Ok(job.run_inline())
    }
}

/// T023, T019: оборванная сборка называет причину, не показывает страниц и
/// не трогает ни одного файла vault.
pub async fn a_build_over_the_time_limit_is_named_and_changes_nothing() {
    let vault = MemoryVault::new()
        .with_file(MAIN, "= На диске\n")
        .with_file("оформление.typ", "#let рамка = 1\n");
    let before = vault.paths_under("");
    let note_before = vault.bytes_of(MAIN);

    let doc = compile_note_resolved_on(
        &OverTheLimit,
        &vault,
        &PackageCache::new(),
        MAIN,
        "#import \"оформление.typ\": рамка\n#while true {}\n",
        5,
        FontSet::builtin(),
    )
    .await
    .unwrap();

    assert_eq!(doc.outcome, CompileOutcome::Failed);
    assert!(doc.pages.is_empty());
    assert_eq!(doc.revision, 5);
    assert_eq!(
        doc.diagnostics[0].app,
        Some(AppMessage::BuildTimedOut {
            seconds: BUILD_TIME_LIMIT_SECS
        })
    );
    assert_eq!(doc.diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(vault.paths_under(""), before);
    assert_eq!(vault.bytes_of(MAIN), note_before);
}

/// T020: каждый проход компилятора идёт через исполнителя — и первый, и
/// повторные после догрузки зависимостей; мимо него сборка не ходит.
pub async fn every_compiler_pass_goes_through_the_executor() {
    let vault = MemoryVault::new()
        .with_file(
            "оформление.typ",
            "#import \"цвета.typ\": акцент\n#let рамка = акцент\n",
        )
        .with_file("цвета.typ", "#let акцент = red\n");
    let executor = Counting::default();

    let doc = compile_note_resolved_on(
        &executor,
        &vault,
        &PackageCache::new(),
        MAIN,
        "#import \"оформление.typ\": рамка\n",
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap();

    assert_eq!(
        doc.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        doc.diagnostics
    );
    // Заметка, затем каждый из двух уровней импорта.
    assert_eq!(executor.0.get(), 3);
}

// ── Отмена, недоступный vault, отметка зависимостей (T018, T110, T121) ─────

/// Исполнитель, у которого сборку отменили.
struct CancelledBuild;

impl CompileExecutor for CancelledBuild {
    async fn run(&self, _job: CompileJob) -> Result<CompileOutput, ExecutorError> {
        Err(ExecutorError::Cancelled)
    }
}

/// T018: отменённая сборка не приносит ни страниц, ни замечаний и не трогает
/// vault — показывать и сообщать после неё нечего.
pub async fn a_cancelled_build_yields_nothing_to_show() {
    let vault = MemoryVault::new().with_file("оформление.typ", "#let рамка = 1\n");
    let before = vault.paths_under("");

    let doc = compile_note_resolved_on(
        &CancelledBuild,
        &vault,
        &PackageCache::new(),
        MAIN,
        "#import \"оформление.typ\": рамка\n",
        9,
        FontSet::builtin(),
    )
    .await
    .unwrap();

    assert_eq!(doc.outcome, CompileOutcome::Failed);
    assert!(doc.pages.is_empty());
    assert!(doc.diagnostics.is_empty());
    assert_eq!(doc.revision, 9);
    assert_eq!(vault.paths_under(""), before);
}

/// T110: недоступная папка vault названа, а текст из буфера всё равно собран.
pub async fn an_unavailable_vault_is_named_and_the_text_still_builds() {
    let vault = MemoryVault::new().unavailable();
    let doc = build(&vault, "= Из буфера\n", 1).await;

    assert_eq!(doc.outcome, CompileOutcome::Succeeded);
    assert!(doc
        .diagnostics
        .iter()
        .any(|d| d.app == Some(AppMessage::VaultUnavailable)
            && d.severity == DiagnosticSeverity::Warning));

    // У доступного vault такого замечания нет.
    let fine = build(&MemoryVault::new(), "= Из буфера\n", 1).await;
    assert!(fine.diagnostics.is_empty());
}

/// T121: отметка меняется, когда связанный файл изменился, появился или
/// исчез, и не меняется, пока всё на месте.
pub async fn the_dependency_mark_follows_the_files() {
    let vault = MemoryVault::new().with_file("схема.svg", SVG);
    let paths = vec!["схема.svg".to_string(), "данные.csv".to_string()];

    let first = dependency_mark(&vault, &paths).await;
    assert_eq!(dependency_mark(&vault, &paths).await, first);

    vault
        .files_handle()
        .borrow_mut()
        .insert("схема.svg".into(), b"<svg/>".to_vec());
    let changed = dependency_mark(&vault, &paths).await;
    assert_ne!(changed, first);

    vault
        .files_handle()
        .borrow_mut()
        .insert("данные.csv".into(), b"a,b\n".to_vec());
    let appeared = dependency_mark(&vault, &paths).await;
    assert_ne!(appeared, changed);

    vault
        .files_handle()
        .borrow_mut()
        .remove(std::path::Path::new("схема.svg"));
    assert_ne!(dependency_mark(&vault, &paths).await, appeared);
}

// ── Кэш связанных файлов между сборками (T122) ─────────────────────────────

/// T122: связанный файл читается один раз, пока не изменится; изменившийся
/// перечитывается, и в сборку идёт новое содержимое; импорт, убранный из
/// текста, перестаёт быть зависимостью.
pub async fn a_linked_file_is_read_once_until_it_changes() {
    let vault = MemoryVault::new()
        .with_file("оформление.typ", "#let метка = [ОДИН]\n")
        .with_file("другое.typ", "#let иное = [ИНОЕ]\n");
    let packages = PackageCache::new();
    let text = "#import \"оформление.typ\": метка\n#import \"другое.typ\": иное\n#метка #иное\n";
    let build = |text: &'static str, revision| {
        compile_note_resolved(&vault, &packages, MAIN, text, revision, FontSet::builtin())
    };
    let words = |doc: &CompiledDocument| -> String {
        doc.pages
            .iter()
            .flat_map(|page| page.text_runs.iter().map(|run| run.text.clone()))
            .collect::<Vec<_>>()
            .join(" ")
    };

    for revision in 1..=3 {
        let doc = build(text, revision).await.unwrap();
        assert_eq!(
            doc.outcome,
            CompileOutcome::Succeeded,
            "{:?}",
            doc.diagnostics
        );
        assert!(words(&doc).contains("ОДИН"));
    }
    assert_eq!(vault.read_count("оформление.typ"), 1);
    assert_eq!(vault.read_count("другое.typ"), 1);

    vault
        .files_handle()
        .borrow_mut()
        .insert("оформление.typ".into(), "#let метка = [ДВА-НОВОЕ]\n".into());
    let doc = build(text, 4).await.unwrap();
    assert!(words(&doc).contains("ДВА-НОВОЕ"), "{}", words(&doc));
    assert_eq!(vault.read_count("оформление.typ"), 2);
    assert_eq!(vault.read_count("другое.typ"), 1);

    // Импорт убран из текста: файл уходит и из зависимостей, и из кэша.
    let shorter = "#import \"оформление.typ\": метка\n#метка\n";
    build(shorter, 5).await.unwrap();
    let doc = build(shorter, 6).await.unwrap();
    assert_eq!(doc.dependencies, ["оформление.typ"]);

    // Файл удалён — сборка называет его, а не собирает из кэша.
    vault
        .files_handle()
        .borrow_mut()
        .remove(std::path::Path::new("оформление.typ"));
    let doc = build(shorter, 7).await.unwrap();
    assert_eq!(doc.outcome, CompileOutcome::Failed);
}
