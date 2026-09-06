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
    while snapshot.begin_iteration() {
        allowed += 1;
        assert!(allowed <= 64, "предел итераций не сработал");
    }

    assert_eq!(allowed, typsidian_core::preview::MAX_RESOLVE_ITERATIONS);
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
