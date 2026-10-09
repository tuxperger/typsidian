//! wasm-bindgen-test — только wasm32-unknown-unknown (принцип II). Парный файл
//! — `preview_native.rs`: проверки совпадают дословно, потому что снимок и
//! `World` над ним платформенно нейтральны, а расхождение между таргетами
//! является дефектом.
//!
//! Отдельная ценность этого файла: он гоняется в **настоящем браузере**, то
//! есть заодно проверяет, что модуль с компилятором Typst там вообще
//! инстанцируется (T002, research.md §11).
#![cfg(target_arch = "wasm32")]

use typsidian_core::preview::{DependencySnapshot, PreviewWorld, SnapshotError};
use typst::foundations::Bytes;
use typst::World;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

const MAIN: &str = "заметка.typ";

/// Компиляционная проверка: `World` обязан быть `Send + Sync`
/// (`typst-library-0.15.1/src/lib.rs:60`), а значит в снимке не может оказаться
/// браузерного хендла — `JsValue` не `Send`.
///
/// Это не теоретическая страховка: первая версия `DependencySnapshot`
/// использовала `RefCell` и не собралась именно здесь (research.md §11).
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

#[wasm_bindgen_test]
fn world_reads_only_from_the_snapshot() {
    let (snapshot, id) = snapshot_with_main("= Заголовок\n");
    let world = PreviewWorld::new(id, snapshot, Vec::new());

    assert_eq!(world.source(id).unwrap().text(), "= Заголовок\n");
}

#[wasm_bindgen_test]
fn missing_file_is_recorded_instead_of_touching_the_filesystem() {
    let (snapshot, id) = snapshot_with_main("= Заголовок\n");
    let absent = DependencySnapshot::file_id("оформление.typ").unwrap();
    let world = PreviewWorld::new(id, snapshot, Vec::new());

    assert!(world.source(absent).is_err());
    assert_eq!(world.snapshot().missing(), vec![absent]);
}

#[wasm_bindgen_test]
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

#[wasm_bindgen_test]
fn path_escaping_the_vault_is_rejected_before_any_read() {
    let escaped = DependencySnapshot::file_id("../снаружи.typ");

    assert_eq!(
        escaped,
        Err(SnapshotError::OutsideVault {
            path: "../снаружи.typ".to_owned()
        })
    );
}

#[wasm_bindgen_test]
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

    assert_eq!(
        allowed,
        u32::from(typsidian_core::preview::MAX_RESOLVE_ITERATIONS)
    );
}

#[wasm_bindgen_test]
fn dependencies_list_every_file_that_entered_the_build() {
    let (mut snapshot, id) = snapshot_with_main("= Заголовок\n");
    let dep = DependencySnapshot::file_id("оформление.typ").unwrap();
    snapshot.insert(dep, Bytes::new(b"#let x = 1\n".to_vec()));

    let deps: std::collections::HashSet<_> = snapshot.dependencies().into_iter().collect();
    let expected: std::collections::HashSet<_> = [id, dep].into_iter().collect();

    assert_eq!(deps, expected);
}

// ── Сборка документа (US1) ──────────────────────────────────────────────
//
// Парные тесты к `preview_native.rs`. Здесь они гоняются в настоящем браузере,
// то есть проверяют не только логику, но и то, что компилятор Typst вообще
// работает под wasm32 — а это и есть предпосылка всей фичи (принцип II).

use typsidian_core::preview::{compile_note, CompileOutcome, DiagnosticSeverity, FontSet};

#[wasm_bindgen_test]
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
    assert!(doc
        .diagnostics
        .iter()
        .all(|d| d.severity != DiagnosticSeverity::Error));
}

#[wasm_bindgen_test]
fn multi_page_note_yields_one_entry_per_page() {
    let doc = compile_note(
        MAIN,
        "Первая\n\n#pagebreak()\n\nВторая\n",
        1,
        FontSet::builtin(),
    )
    .unwrap();

    assert_eq!(doc.pages.len(), 2);
    assert_ne!(doc.pages[0].digest, doc.pages[1].digest);
}

#[wasm_bindgen_test]
fn unchanged_text_yields_identical_page_digests() {
    let text = "= Заголовок\n\nАбзац.\n";
    let first = compile_note(MAIN, text, 1, FontSet::builtin()).unwrap();
    let second = compile_note(MAIN, text, 2, FontSet::builtin()).unwrap();

    assert_eq!(first.pages[0].digest, second.pages[0].digest);
}

#[wasm_bindgen_test]
fn a_broken_note_fails_with_a_named_reason_and_a_place() {
    let doc = compile_note(MAIN, "#let = 1\n", 1, FontSet::builtin()).unwrap();

    assert_eq!(doc.outcome, CompileOutcome::Failed);
    assert!(doc.pages.is_empty());

    let error = doc
        .diagnostics
        .iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
        .expect("ошибка обязана быть названа");
    assert!(!error.message.is_empty());
    assert_eq!(
        error.app, None,
        "сообщение Typst показывается как есть (010 FR-004)"
    );

    let place = error.location.as_ref().expect("место обязано быть указано");
    assert!(place.is_open_note);
    assert_eq!(place.line, 1);
}

// ── Фаза резолва: связанные файлы (T014-T015, FR-021) ───────────────────
//
// Парные проверки к `preview_native.rs` — дословно те же: цикл резолва
// платформенно нейтрален, и расхождение между таргетами является дефектом
// (принцип II). Здесь они вдобавок гоняются в настоящем браузере.

mod common;

use common::MemoryVault;
// `CompileOutcome`, `DiagnosticSeverity` и `FontSet` уже импортированы выше в
// этом же файле: секции разделены комментариями, но модуль один.
use typsidian_core::messages::{AppMessage, FontProblem, ResolveReason};
use typsidian_core::preview::{compile_note_resolved, PackageCache};
use typsidian_core::vault::backend::ReadError;

const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"></svg>"#;

#[wasm_bindgen_test]
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

    assert_eq!(doc.outcome, CompileOutcome::Succeeded);
    assert!(!doc.pages.is_empty());
}

#[wasm_bindgen_test]
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
    let named = doc.diagnostics.iter().any(|d| {
        d.severity == DiagnosticSeverity::Error
            && d.app
                == Some(AppMessage::DependencyUnreadable {
                    path: "нет.svg".to_string(),
                    reason: ResolveReason::NotFound,
                })
    });
    assert!(named);
    assert_eq!(vault.read_count("нет.svg"), 1);
}

#[wasm_bindgen_test]
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
    assert!(doc.diagnostics.iter().any(|d| {
        d.app
            == Some(AppMessage::DependencyUnreadable {
                path: "битая.svg".to_string(),
                reason: ResolveReason::TooLarge,
            })
    }));
}

#[wasm_bindgen_test]
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
    assert!(vault.reads.borrow().is_empty());
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
#[wasm_bindgen_test]
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
    assert!(reads_after_first > 0);
    assert!(!packages.is_empty());

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
    assert_eq!(second.pages.len(), first.pages.len());
}

/// Кэш общий на сеанс, но не общий на весь процесс.
#[wasm_bindgen_test]
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
    assert!(vault.reads.borrow().values().sum::<u32>() > reads_before);
}

// ── Текстовый слой (US1, T031-T034, FR-031, FR-032) ─────────────────────
//
// Та же таблица, что в `preview_native.rs`, дословно.

use common::textlayer_cases as textlayer;

#[wasm_bindgen_test]
fn a_run_carries_its_text_place_size_and_language() {
    textlayer::a_run_carries_its_text_place_size_and_language();
}

#[wasm_bindgen_test]
fn a_run_inside_a_transformed_group_lands_in_page_coordinates() {
    textlayer::a_run_inside_a_transformed_group_lands_in_page_coordinates();
}

#[wasm_bindgen_test]
fn runs_follow_reading_order_across_columns_and_pages() {
    textlayer::runs_follow_reading_order_across_columns_and_pages();
}

#[wasm_bindgen_test]
fn generated_content_reaches_the_layer() {
    textlayer::generated_content_reaches_the_layer();
}

// ── Шрифты из папки vault (T100, T102, FR-027…FR-029) ───────────────────
//
// Встроенный набор в этих проверках пуст: иначе шрифт нашёлся бы и без vault,
// и проверка ничего бы не доказывала. В vault кладётся файл Libertinus из
// `typst-assets` — настоящий шрифт, которому неоткуда взяться, кроме папки.

/// Байты одного начертания Libertinus Serif.
fn libertinus_bytes() -> Vec<u8> {
    typst_assets::fonts()
        .find(|bytes| {
            typst::text::Font::new(Bytes::new(bytes.to_vec()), 0)
                .is_some_and(|font| font.info().family == "Libertinus Serif")
        })
        .expect("Libertinus в typst-assets")
        .to_vec()
}

const LIBERTINUS_NOTE: &str = "#set text(font: \"Libertinus Serif\")\nТекст\n";

fn unknown_font(doc: &typsidian_core::preview::CompiledDocument) -> bool {
    doc.diagnostics
        .iter()
        .any(|d| d.message.to_lowercase().contains("unknown font family"))
}

#[wasm_bindgen_test]
async fn font_from_vault_folder_is_available_to_the_note() {
    let vault = MemoryVault::new().with_file("шрифты/Libertinus.OTF", libertinus_bytes());

    let doc = compile_note_resolved(
        &vault,
        &PackageCache::new(),
        MAIN,
        LIBERTINUS_NOTE,
        1,
        FontSet::default(),
    )
    .await
    .unwrap();

    assert_eq!(
        doc.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        doc.diagnostics
    );
    assert!(
        !unknown_font(&doc),
        "шрифт из vault обязан найтись: {:?}",
        doc.diagnostics
    );
}

#[wasm_bindgen_test]
async fn font_outside_vault_is_reported_unknown() {
    let doc = compile_note_resolved(
        &MemoryVault::new(),
        &PackageCache::new(),
        MAIN,
        LIBERTINUS_NOTE,
        1,
        FontSet::default(),
    )
    .await
    .unwrap();

    assert!(
        unknown_font(&doc),
        "без файла в vault шрифта нет: {:?}",
        doc.diagnostics
    );
}

#[wasm_bindgen_test]
async fn vault_font_is_read_once_per_change_not_per_compile() {
    let vault = MemoryVault::new().with_file("Libertinus.otf", libertinus_bytes());
    let packages = PackageCache::new();

    for revision in 1..=3 {
        compile_note_resolved(
            &vault,
            &packages,
            MAIN,
            LIBERTINUS_NOTE,
            revision,
            FontSet::default(),
        )
        .await
        .unwrap();
    }

    assert_eq!(vault.read_count("Libertinus.otf"), 1);
}

#[wasm_bindgen_test]
async fn broken_vault_font_is_named_and_the_note_still_compiles() {
    let vault = MemoryVault::new().with_file("шрифты/битый.ttf", "не шрифт");

    let doc = compile_note_resolved(
        &vault,
        &PackageCache::new(),
        MAIN,
        "Текст\n",
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
    let named = doc.diagnostics.iter().any(|d| {
        d.severity == DiagnosticSeverity::Warning
            && d.app
                == Some(AppMessage::VaultFontUnusable {
                    path: "шрифты/битый.ttf".to_string(),
                    reason: FontProblem::Invalid,
                })
    });
    assert!(
        named,
        "битый шрифт обязан быть назван: {:?}",
        doc.diagnostics
    );
}

// ---------------------------------------------------------------------------
// Фича 010 — сообщения приложения (T042/T043): общие проверки из common/message_cases.rs
// ---------------------------------------------------------------------------

use common::message_cases as app_messages;

#[wasm_bindgen_test]
fn every_variant_has_a_stable_json_shape() {
    app_messages::every_variant_has_a_stable_json_shape();
}

#[wasm_bindgen_test]
fn a_diagnostic_carries_the_message_kind_only_when_the_app_made_it() {
    app_messages::a_diagnostic_carries_the_message_kind_only_when_the_app_made_it();
}

#[wasm_bindgen_test]
fn log_text_names_the_user_values_verbatim() {
    app_messages::log_text_names_the_user_values_verbatim();
}

// ── Общие случаи сборки (T054…T056, T073…T076, T091, T093, T095, T099) ────

#[wasm_bindgen_test]
async fn a_two_level_import_and_an_image_build_with_the_note() {
    common::preview_cases::a_two_level_import_and_an_image_build_with_the_note().await;
}

#[wasm_bindgen_test]
async fn a_missing_file_is_a_dependency_too() {
    common::preview_cases::a_missing_file_is_a_dependency_too().await;
}

#[wasm_bindgen_test]
async fn a_cyclic_import_fails_with_a_named_reason() {
    common::preview_cases::a_cyclic_import_fails_with_a_named_reason().await;
}

#[wasm_bindgen_test]
async fn the_build_uses_the_given_text_and_never_reads_the_note_file() {
    common::preview_cases::the_build_uses_the_given_text_and_never_reads_the_note_file().await;
}

#[wasm_bindgen_test]
fn the_result_carries_the_revision_of_its_text() {
    common::preview_cases::the_result_carries_the_revision_of_its_text();
}

#[wasm_bindgen_test]
fn a_diagnostic_names_its_reason_and_place() {
    common::preview_cases::a_diagnostic_names_its_reason_and_place();
}

#[wasm_bindgen_test]
fn diagnostics_are_ordered_by_place() {
    common::preview_cases::diagnostics_are_ordered_by_place();
}

#[wasm_bindgen_test]
fn a_warning_does_not_fail_the_build() {
    common::preview_cases::a_warning_does_not_fail_the_build();
}

#[wasm_bindgen_test]
async fn a_diagnostic_in_a_linked_file_points_at_that_file() {
    common::preview_cases::a_diagnostic_in_a_linked_file_points_at_that_file().await;
}

#[wasm_bindgen_test]
fn an_anchor_follows_its_text_to_another_page() {
    common::preview_cases::an_anchor_follows_its_text_to_another_page();
}

#[wasm_bindgen_test]
fn an_anchor_picks_the_nearest_of_equal_fragments() {
    common::preview_cases::an_anchor_picks_the_nearest_of_equal_fragments();
}

#[wasm_bindgen_test]
fn a_vanished_anchor_resolves_nearby_and_never_to_the_start() {
    common::preview_cases::a_vanished_anchor_resolves_nearby_and_never_to_the_start();
}

#[wasm_bindgen_test]
fn no_anchor_without_a_request_or_without_text() {
    common::preview_cases::no_anchor_without_a_request_or_without_text();
}

#[wasm_bindgen_test]
async fn a_build_over_the_time_limit_is_named_and_changes_nothing() {
    common::preview_cases::a_build_over_the_time_limit_is_named_and_changes_nothing().await;
}

#[wasm_bindgen_test]
async fn every_compiler_pass_goes_through_the_executor() {
    common::preview_cases::every_compiler_pass_goes_through_the_executor().await;
}

#[wasm_bindgen_test]
async fn a_font_added_to_and_removed_from_the_vault_updates_the_set() {
    // T102: набор шрифтов следует за папкой vault в обе стороны, и кэш
    // разобранных шрифтов между сборками этому не мешает.
    let vault = MemoryVault::new();
    let packages = PackageCache::new();
    let build = |revision| {
        compile_note_resolved(
            &vault,
            &packages,
            MAIN,
            LIBERTINUS_NOTE,
            revision,
            FontSet::default(),
        )
    };

    assert!(unknown_font(&build(1).await.unwrap()));

    vault
        .files_handle()
        .borrow_mut()
        .insert("Libertinus.otf".into(), libertinus_bytes());
    assert!(!unknown_font(&build(2).await.unwrap()), "шрифт появился");

    vault
        .files_handle()
        .borrow_mut()
        .remove(std::path::Path::new("Libertinus.otf"));
    assert!(unknown_font(&build(3).await.unwrap()), "шрифт удалён");
}

#[wasm_bindgen_test]
async fn a_cancelled_build_yields_nothing_to_show() {
    common::preview_cases::a_cancelled_build_yields_nothing_to_show().await;
}

#[wasm_bindgen_test]
async fn an_unavailable_vault_is_named_and_the_text_still_builds() {
    common::preview_cases::an_unavailable_vault_is_named_and_the_text_still_builds().await;
}

#[wasm_bindgen_test]
async fn the_dependency_mark_follows_the_files() {
    common::preview_cases::the_dependency_mark_follows_the_files().await;
}

#[wasm_bindgen_test]
async fn a_linked_file_is_read_once_until_it_changes() {
    common::preview_cases::a_linked_file_is_read_once_until_it_changes().await;
}
