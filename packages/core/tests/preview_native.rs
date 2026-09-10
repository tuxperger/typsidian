//! cargo test — только native (принцип II). Парный файл — `preview_wasm.rs`:
//! проверки совпадают дословно везде, где логика платформенно нейтральна.
//! Расхождение результата между таргетами является дефектом, а не особенностью
//! платформы.
#![cfg(not(target_arch = "wasm32"))]

use typsidian_core::preview::{DependencySnapshot, PreviewWorld, SnapshotError};
use typst::foundations::Bytes;
use typst::World;

const MAIN: &str = "заметка.typ";

/// Компиляционная проверка: `World` обязан быть `Send + Sync`
/// (`typst-library-0.15.1/src/lib.rs:60`). Парная ей проверка в
/// `preview_wasm.rs` — там же, где ограничение реально кусается.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PreviewWorld>();
    assert_send_sync::<DependencySnapshot>();
};

fn snapshot_with_main(text: &str) -> (DependencySnapshot, typst::syntax::FileId) {
    let id = DependencySnapshot::file_id(MAIN).unwrap();
    let mut snapshot = DependencySnapshot::new();
    snapshot.insert(id, Bytes::new(text.as_bytes().to_vec()));
    (snapshot, id)
}

#[test]
fn world_reads_only_from_the_snapshot() {
    let (snapshot, id) = snapshot_with_main("= Заголовок\n");
    let world = PreviewWorld::new(id, snapshot, Vec::new());

    assert_eq!(world.source(id).unwrap().text(), "= Заголовок\n");
}

#[test]
fn missing_file_is_recorded_instead_of_touching_the_filesystem() {
    let (snapshot, id) = snapshot_with_main("= Заголовок\n");
    let absent = DependencySnapshot::file_id("оформление.typ").unwrap();
    let world = PreviewWorld::new(id, snapshot, Vec::new());

    assert!(world.source(absent).is_err());
    assert_eq!(world.snapshot().missing(), vec![absent]);
}

#[test]
fn inserting_a_missing_file_clears_it_from_the_missing_set() {
    let (snapshot, id) = snapshot_with_main("= Заголовок\n");
    let absent = DependencySnapshot::file_id("оформление.typ").unwrap();
    let world = PreviewWorld::new(id, snapshot, Vec::new());
    let _ = world.source(absent);

    let mut snapshot = world.into_snapshot();
    assert!(snapshot.has_missing());
    snapshot.insert(absent, Bytes::new(b"#let x = 1\n".to_vec()));

    assert!(!snapshot.has_missing());
    assert!(snapshot.contains(absent));
}

#[test]
fn path_escaping_the_vault_is_rejected_before_any_read() {
    let escaped = DependencySnapshot::file_id("../снаружи.typ");

    assert_eq!(
        escaped,
        Err(SnapshotError::OutsideVault {
            path: "../снаружи.typ".to_owned()
        })
    );
}

#[test]
fn resolve_iterations_are_bounded() {
    let mut snapshot = DependencySnapshot::new();
    let mut allowed = 0;
    // Страховка от бесконечного цикла считается **от самого предела**, а не от
    // литерала: фича 007 подняла его с 8 до 512 (пакет, строящий словарь из
    // десятков `image(...)`, отдаёт по одному файлу за проход), и зашитое
    // здесь число сделало бы тест ложно красным при следующем таком уточнении.
    let guard = u32::from(typsidian_core::preview::MAX_RESOLVE_ITERATIONS) + 1;
    while snapshot.begin_iteration() {
        allowed += 1;
        assert!(allowed <= guard, "предел итераций не сработал");
    }

    assert_eq!(allowed, u32::from(typsidian_core::preview::MAX_RESOLVE_ITERATIONS));
}

#[test]
fn dependencies_list_every_file_that_entered_the_build() {
    let (mut snapshot, id) = snapshot_with_main("= Заголовок\n");
    let dep = DependencySnapshot::file_id("оформление.typ").unwrap();
    snapshot.insert(dep, Bytes::new(b"#let x = 1\n".to_vec()));

    // `FileId` не реализует `Ord`, поэтому сравниваем множествами, а не
    // отсортированными списками: порядок здесь и не является частью контракта.
    let deps: std::collections::HashSet<_> = snapshot.dependencies().into_iter().collect();
    let expected: std::collections::HashSet<_> = [id, dep].into_iter().collect();

    assert_eq!(deps, expected);
}

// ── Сборка документа (US1) ──────────────────────────────────────────────

use typsidian_core::preview::{compile_note, CompileOutcome, DiagnosticSeverity, FontSet};

#[test]
fn a_real_note_compiles_into_pages_with_svg() {
    let doc = compile_note(
        MAIN,
        "= Заголовок\n\nАбзац с формулой $x^2 + y^2 = z^2$.\n",
        1,
        FontSet::builtin(),
    )
    .unwrap();

    assert_eq!(doc.outcome, CompileOutcome::Succeeded);
    assert_eq!(doc.pages.len(), 1);
    assert!(doc.pages[0].svg.contains("<svg"), "ожидали настоящий SVG");
    assert!(doc.pages[0].width_pt > 0.0 && doc.pages[0].height_pt > 0.0);
    assert!(doc.diagnostics.iter().all(|d| d.severity != DiagnosticSeverity::Error));
}

#[test]
fn multi_page_note_yields_one_entry_per_page() {
    let doc = compile_note(MAIN, "Первая\n\n#pagebreak()\n\nВторая\n", 1, FontSet::builtin())
        .unwrap();

    assert_eq!(doc.pages.len(), 2);
    assert_eq!(doc.pages[0].index, 0);
    assert_eq!(doc.pages[1].index, 1);
    assert_ne!(
        doc.pages[0].digest, doc.pages[1].digest,
        "разные страницы обязаны отличаться отпечатком"
    );
}

#[test]
fn unchanged_text_yields_identical_page_digests() {
    let text = "= Заголовок\n\nАбзац.\n";
    let first = compile_note(MAIN, text, 1, FontSet::builtin()).unwrap();
    let second = compile_note(MAIN, text, 2, FontSet::builtin()).unwrap();

    assert_eq!(first.pages[0].digest, second.pages[0].digest);
}

#[test]
fn a_broken_note_fails_with_a_named_reason_and_a_place() {
    let doc = compile_note(MAIN, "#let = 1\n", 1, FontSet::builtin()).unwrap();

    assert_eq!(doc.outcome, CompileOutcome::Failed);
    assert!(doc.pages.is_empty(), "при отказе страниц быть не должно");

    let error = doc
        .diagnostics
        .iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
        .expect("ошибка обязана быть названа");
    assert!(!error.message.is_empty(), "общий текст без причины запрещён (FR-012)");

    let place = error.location.as_ref().expect("место обязано быть указано (FR-013)");
    assert!(place.is_open_note);
    assert_eq!(place.line, 1);
    assert!(place.byte_end >= place.byte_start);
}

// ── Фаза резолва: связанные файлы (T014-T015, FR-021) ───────────────────
//
// Парные проверки лежат в `preview_wasm.rs` дословно: цикл резолва
// платформенно нейтрален, и расхождение между таргетами — дефект (принцип II).

mod common;

use common::MemoryVault;
use typsidian_core::preview::{compile_note_resolved, PackageCache};
use typsidian_core::vault::backend::ReadError;

/// Однострочный SVG — годится как содержимое `#image`.
const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"></svg>"#;

#[tokio::test]
async fn image_from_vault_is_loaded_and_the_note_compiles() {
    let vault = MemoryVault::new().with_file("схема.svg", SVG);

    let doc = compile_note_resolved(
        &vault,
        &PackageCache::new(),
        MAIN,
        "#image(\"схема.svg\", width: 2cm)\n",
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap();

    assert_eq!(
        doc.outcome,
        CompileOutcome::Succeeded,
        "картинка из vault обязана догрузиться: {:?}",
        doc.diagnostics
    );
    assert!(!doc.pages.is_empty());
}

#[tokio::test]
async fn missing_image_is_named_with_its_reason_and_does_not_spin_the_loop() {
    let vault = MemoryVault::new();

    let doc = compile_note_resolved(
        &vault,
        &PackageCache::new(),
        MAIN,
        "#image(\"нет.svg\")\n",
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap();

    assert_eq!(doc.outcome, CompileOutcome::Failed);
    let named = doc
        .diagnostics
        .iter()
        .any(|d| d.severity == DiagnosticSeverity::Error && d.message.contains("нет.svg"));
    assert!(named, "имя файла обязано дойти до пользователя: {:?}", doc.diagnostics);

    // Отказ не меняется в пределах одной сборки, поэтому файл спрашивается
    // один раз, а не восемь — по разу на каждый проход резолва.
    assert_eq!(vault.read_count("нет.svg"), 1);
}

#[tokio::test]
async fn unreadable_dependency_reports_its_own_reason_not_a_generic_failure() {
    let vault = MemoryVault::new().with_failure("битая.svg", ReadError::TooLarge);

    let doc = compile_note_resolved(
        &vault,
        &PackageCache::new(),
        MAIN,
        "#image(\"битая.svg\")\n",
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap();

    assert_eq!(doc.outcome, CompileOutcome::Failed);
    let described = doc
        .diagnostics
        .iter()
        .any(|d| d.message.contains("битая.svg") && d.message.contains("размер"));
    assert!(
        described,
        "причина обязана отличаться от «файл не найден»: {:?}",
        doc.diagnostics
    );
}

#[tokio::test]
async fn a_note_without_dependencies_compiles_without_touching_the_vault() {
    let vault = MemoryVault::new();

    let doc = compile_note_resolved(
        &vault,
        &PackageCache::new(),
        MAIN,
        "Просто текст.\n",
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap();

    assert_eq!(doc.outcome, CompileOutcome::Succeeded);
    assert!(
        vault.reads.borrow().is_empty(),
        "документ без связей не обязан обращаться к vault"
    );
}

/// Vault с одним установленным шаблоном и заметкой, которая его подключает.
fn vault_with_template() -> MemoryVault {
    MemoryVault::new()
        .with_file(
            ".typsidian/packages/preview/demo/1.0.0/typst.toml",
            "[package]\nname = \"demo\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n",
        )
        .with_file(
            ".typsidian/packages/preview/demo/1.0.0/lib.typ",
            "#let привет = [Привет]\n",
        )
}

const NOTE_WITH_TEMPLATE: &str = "#import \"@preview/demo:1.0.0\": привет\n#привет\n";

/// Файлы шаблона переживают сборку и достаются следующей готовыми.
///
/// Это не про экономию чтения — читать дёшево. Каждый недостающий файл стоит
/// **целой пересборки**: Typst обрывает вычисление на первом промахе, поэтому
/// шаблон из 86 файлов поднимался бы 87 проходами на каждое нажатие клавиши
/// (`preview/cache.rs`).
#[tokio::test]
async fn template_files_survive_between_compiles() {
    let vault = vault_with_template();
    let packages = PackageCache::new();

    let first = compile_note_resolved(
        &vault,
        &packages,
        MAIN,
        NOTE_WITH_TEMPLATE,
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap();
    assert_eq!(
        first.outcome,
        CompileOutcome::Succeeded,
        "шаблон установлен, заметка обязана собраться: {:?}",
        first.diagnostics
    );
    let reads_after_first: u32 = vault.reads.borrow().values().sum();
    assert!(reads_after_first > 0, "первая сборка обязана прочитать шаблон");
    assert!(!packages.is_empty(), "прочитанные файлы шаблона обязаны осесть в кэше");

    let second = compile_note_resolved(
        &vault,
        &packages,
        MAIN,
        NOTE_WITH_TEMPLATE,
        2,
        FontSet::builtin(),
    )
    .await
    .unwrap();

    assert_eq!(second.outcome, CompileOutcome::Succeeded);
    assert_eq!(
        vault.reads.borrow().values().sum::<u32>(),
        reads_after_first,
        "вторая сборка обязана взять шаблон из кэша, не обращаясь к vault"
    );
    assert_eq!(
        second.pages.len(),
        first.pages.len(),
        "кэш меняет только путь к байтам, но не результат"
    );
}

/// Кэш общий на сеанс, но не общий на весь процесс: свой отдаёт свой шаблон,
/// чужой — не отдаёт ничего.
#[tokio::test]
async fn a_fresh_cache_does_not_see_another_sessions_templates() {
    let vault = vault_with_template();
    let packages = PackageCache::new();
    let _ = compile_note_resolved(
        &vault,
        &packages,
        MAIN,
        NOTE_WITH_TEMPLATE,
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap();

    let other = PackageCache::new();
    assert!(other.is_empty());

    let reads_before: u32 = vault.reads.borrow().values().sum();
    let doc = compile_note_resolved(
        &vault,
        &other,
        MAIN,
        NOTE_WITH_TEMPLATE,
        2,
        FontSet::builtin(),
    )
    .await
    .unwrap();

    assert_eq!(doc.outcome, CompileOutcome::Succeeded);
    assert!(
        vault.reads.borrow().values().sum::<u32>() > reads_before,
        "пустой кэш обязан пойти за шаблоном в vault"
    );
}
