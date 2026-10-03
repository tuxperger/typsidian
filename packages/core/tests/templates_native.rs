//! cargo test — только native (принцип II). Парный файл — `templates_wasm.rs`:
//! оба вызывают одну и ту же таблицу `common::template_cases`, потому что
//! правила библиотеки заготовок платформенно нейтральны, а расхождение между
//! таргетами является дефектом, а не особенностью платформы.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::template_cases as cases;

// ── Phase 2: состав библиотеки и состояние папки ───────────────────────

#[tokio::test]
async fn a_missing_folder_is_not_an_error() {
    cases::a_missing_folder_is_not_an_error().await;
}

#[tokio::test]
async fn an_unreadable_folder_does_not_collapse_into_absent() {
    cases::an_unreadable_folder_does_not_collapse_into_absent().await;
}

#[tokio::test]
async fn a_read_only_vault_makes_the_folder_not_writable() {
    cases::a_read_only_vault_makes_the_folder_not_writable().await;
}

#[tokio::test]
async fn any_note_inside_the_folder_is_a_template() {
    cases::any_note_inside_the_folder_is_a_template().await;
}

#[tokio::test]
async fn a_board_in_the_folder_is_not_a_template() {
    cases::a_board_in_the_folder_is_not_a_template().await;
}

#[tokio::test]
async fn a_nested_templates_folder_is_someone_elses() {
    cases::a_nested_templates_folder_is_someone_elses().await;
}

// ── US1: превью и применение заготовки ─────────────────────────────────

#[tokio::test]
async fn an_unreadable_template_is_reported_and_left_alone() {
    cases::an_unreadable_template_is_reported_and_left_alone().await;
}

#[tokio::test]
async fn a_disk_failure_is_not_the_same_as_bad_content() {
    cases::a_disk_failure_is_not_the_same_as_bad_content().await;
}

#[tokio::test]
async fn the_excerpt_stops_at_a_line_boundary() {
    cases::the_excerpt_stops_at_a_line_boundary().await;
}

#[tokio::test]
async fn the_excerpt_source_never_claims_the_content_is_broken() {
    cases::the_excerpt_source_never_claims_the_content_is_broken().await;
}

#[tokio::test]
async fn the_cache_is_keyed_by_content_not_by_path() {
    cases::the_cache_is_keyed_by_content_not_by_path().await;
}

#[tokio::test]
async fn an_unreadable_template_previews_as_unavailable() {
    cases::an_unreadable_template_previews_as_unavailable().await;
}

#[tokio::test]
async fn applying_a_template_leaves_its_file_untouched() {
    cases::applying_a_template_leaves_its_file_untouched().await;
}

// ── US2: сохранение как заготовки ──────────────────────────────────────

#[tokio::test]
async fn an_invalid_name_never_touches_the_disk() {
    cases::an_invalid_name_never_touches_the_disk().await;
}

#[tokio::test]
async fn a_taken_name_without_confirmation_keeps_the_existing_file() {
    cases::a_taken_name_without_confirmation_keeps_the_existing_file().await;
}

#[tokio::test]
async fn the_first_save_creates_the_folder() {
    cases::the_first_save_creates_the_folder().await;
}

#[tokio::test]
async fn a_read_only_vault_refuses_the_save_with_a_reason() {
    cases::a_read_only_vault_refuses_the_save_with_a_reason().await;
}

#[tokio::test]
async fn the_saved_content_is_frozen_at_save_time() {
    cases::the_saved_content_is_frozen_at_save_time().await;
}

// ── US3: вставка в позицию курсора ─────────────────────────────────────

#[tokio::test]
async fn insertion_is_verbatim_and_replaces_the_selection() {
    cases::insertion_is_verbatim_and_replaces_the_selection().await;
}

#[tokio::test]
async fn inserting_does_not_touch_the_template_file() {
    cases::inserting_does_not_touch_the_template_file().await;
}

// ── Phase 7: наследство от vault-manager ───────────────────────────────

#[tokio::test]
async fn templates_appear_in_the_tree_as_ordinary_notes() {
    cases::templates_appear_in_the_tree_as_ordinary_notes().await;
}

#[tokio::test]
async fn a_template_is_edited_through_the_ordinary_note_path() {
    cases::a_template_is_edited_through_the_ordinary_note_path().await;
}

#[tokio::test]
async fn templates_take_part_in_the_link_graph() {
    cases::templates_take_part_in_the_link_graph().await;
}

// ── Настоящая файловая система: то, чего фейк проверить не может ────────────
//
// Общая таблица выше идёт над `MemoryVault` и проверяет правила библиотеки.
// Ниже — проверки самого `NativeBackend::folder_access`: различить «папки нет»,
// «по пути лежит файл» и «прав нет» можно только на настоящем диске, и на
// wasm32 такого теста не существует в принципе — файловой системы там нет.
//
// Парного файла у этой части нет намеренно: она проверяет не правило фичи, а
// его платформенную опору, и `templates_wasm.rs` проверяет ту же опору у
// `WebBackend` через `library()`.

use std::fs;
use tempfile::TempDir;
use typsidian_core::templates::{folder_state, library, save_as_template, TemplateFolderState};
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::native::NativeBackend;
use typsidian_core::vault::FolderProblem;

#[tokio::test]
async fn a_vault_without_the_folder_reports_absent_on_disk() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("заметка.typ"), "= Заметка\n").unwrap();

    let backend = NativeBackend::open(dir.path()).await.expect("vault открыт");
    assert_eq!(folder_state(&backend).await, TemplateFolderState::Absent);
    assert!(library(&backend).await.templates.is_empty());
}

#[tokio::test]
async fn a_file_named_templates_is_not_a_folder() {
    // Штатный, хоть и редкий случай: пользователь завёл файл с таким именем.
    // Схлопнуть это в `Absent` значило бы предложить сохранение, которое
    // провалится, — причина обязана быть названа заранее.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("templates"), "не папка").unwrap();

    let backend = NativeBackend::open(dir.path()).await.expect("vault открыт");
    assert_eq!(
        folder_state(&backend).await,
        TemplateFolderState::Unreadable {
            reason: FolderProblem::NotADirectory
        }
    );
}

#[tokio::test]
async fn the_first_save_creates_the_folder_on_disk() {
    // FR-002 сквозь настоящую файловую систему: папки не было, отдельного
    // действия пользователь не совершал, файл читается сторонними средствами
    // (Сценарий 4 шаги 1-2 quickstart.md).
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("Результаты.typ"), "= Результаты\n").unwrap();

    let mut backend = NativeBackend::open(dir.path()).await.expect("vault открыт");
    let saved = save_as_template(
        &mut backend,
        "Раздел диссертации",
        "= {{заголовок}}\n",
        false,
    )
    .await
    .expect("сохранение проходит");

    let on_disk = dir.path().join("templates").join("Раздел диссертации.typ");
    assert!(on_disk.is_file(), "обычный файл в обычной папке");
    assert_eq!(fs::read_to_string(&on_disk).unwrap(), "= {{заголовок}}\n");
    assert_eq!(
        saved.path,
        std::path::PathBuf::from("templates/Раздел диссертации.typ")
    );
    assert_eq!(folder_state(&backend).await, TemplateFolderState::Present);
}

#[cfg(unix)]
#[tokio::test]
async fn a_folder_without_write_permission_is_reported_not_writable() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates");
    fs::create_dir(&templates).unwrap();
    fs::write(templates.join("Раздел.typ"), "= Раздел\n").unwrap();
    fs::set_permissions(&templates, fs::Permissions::from_mode(0o555)).unwrap();

    let backend = NativeBackend::open(dir.path()).await.expect("vault открыт");
    let state = folder_state(&backend).await;

    // Права возвращаются до любых assert: иначе TempDir не сможет убраться.
    fs::set_permissions(&templates, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(
        state,
        TemplateFolderState::NotWritable {
            reason: FolderProblem::AccessDenied
        },
        "галерея работает, сохранение отклоняется с причиной (US2 AS6)"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_unreadable_folder_is_reported_on_disk() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates");
    fs::create_dir(&templates).unwrap();
    fs::set_permissions(&templates, fs::Permissions::from_mode(0o000)).unwrap();

    let backend = NativeBackend::open(dir.path()).await.expect("vault открыт");
    let state = folder_state(&backend).await;

    fs::set_permissions(&templates, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(
        state,
        TemplateFolderState::Unreadable {
            reason: FolderProblem::AccessDenied
        },
        "«папки нет» и «папка не читается» не схлопываются (FR-009, FR-016)"
    );
}
