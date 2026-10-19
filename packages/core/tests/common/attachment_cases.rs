//! Вложения (фича 016): общие случаи для `attachments_native.rs` и
//! `attachments_wasm.rs`.
//!
//! Правила и пачка лежат над трейтом `VaultBackend`, поэтому проверяются на
//! бэкенде в памяти одинаково на обоих таргетах (принцип II, FR-025).

#![allow(dead_code)]

use std::path::Path;

use typsidian_core::attachments::{
    self, batch, image_markup, plan_markup, sniff, AttachmentBatchPhase, AttachmentBatchState,
    AttachmentBatchView, AttachmentRefusalReason, AttachmentsFolderProblem, AttachmentsPlacement,
    ImageKind, IncomingItem, InsertMoment, ItemStatus, NextAction, Origin, StoreProblem,
    LARGE_IMAGE_BYTES,
};
use typsidian_core::sync::ignore::IgnoreFile;
use typsidian_core::sync::ignore_match::excludes;
use typsidian_core::templates::{InsertionPoint, TextSpan};
use typsidian_core::vault::vault_settings::VaultOverrides;

use super::MemoryVault;

const NOTE: &str = "Диссертация/Глава.typ";
const FOLDER: &str = "Диссертация/attachments";
const MOMENT: InsertMoment = InsertMoment {
    year: 2026,
    month: 10,
    day: 10,
    hour: 14,
    minute: 23,
    second: 5,
};
const GENERATED: &str = "Глава-2026-10-10-142305.png";

/// PNG с отличающимся хвостом: содержимое разное, вид один.
fn png(tag: u8) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    bytes.extend([tag; 4]);
    bytes
}

fn jpeg(tag: u8) -> Vec<u8> {
    vec![0xFF, 0xD8, 0xFF, 0xE0, tag, tag]
}

fn clip(bytes: &[u8]) -> IncomingItem {
    IncomingItem {
        name: None,
        size: bytes.len() as u64,
        origin: Origin::Clipboard,
    }
}

fn dropped(name: &str, bytes: &[u8]) -> IncomingItem {
    IncomingItem {
        name: Some(name.to_owned()),
        size: bytes.len() as u64,
        origin: Origin::File,
    }
}

/// Прогоняет пачку целиком, как цикл UI: подтверждает большие файлы и отдаёт
/// байты каждого `Copy`.
async fn run(
    vault: &mut MemoryVault,
    overrides: &VaultOverrides,
    note: &str,
    items: Vec<(IncomingItem, Vec<u8>)>,
) -> AttachmentBatchView {
    let (incoming, bytes): (Vec<_>, Vec<_>) = items.into_iter().unzip();
    let mut state = batch::start(&*vault, overrides, note, incoming, MOMENT).await;
    loop {
        let step = batch::step(state);
        state = step.state;
        match step.next {
            Some(NextAction::Confirm { index }) => state = batch::confirm(state, index, true),
            Some(NextAction::Copy { index }) => {
                state = batch::store(vault, state, index, &bytes[index as usize]).await
            }
            None => break,
        }
    }
    batch::view(state)
}

fn status(view: &AttachmentBatchView, index: usize) -> &ItemStatus {
    &view.state.items[index].status
}

// ── Правила (T016) ─────────────────────────────────────────────────────────

pub fn the_five_kinds_are_recognised_by_content_and_the_rest_refused() {
    assert_eq!(sniff(&png(1)), Some(ImageKind::Png));
    assert_eq!(sniff(&jpeg(1)), Some(ImageKind::Jpeg));
    assert_eq!(sniff(b"GIF89a\x01\x00"), Some(ImageKind::Gif));
    assert_eq!(sniff(b"RIFF\x24\0\0\0WEBPVP8 "), Some(ImageKind::Webp));
    assert_eq!(
        sniff(b"<?xml version=\"1.0\"?>\n<svg width=\"1\"/>"),
        Some(ImageKind::Svg)
    );
    for refused in [
        &b"%PDF-1.7\n"[..],
        b"\0\0\0\x18ftypheic",
        b"",
        "обычный текст".as_bytes(),
    ] {
        assert_eq!(sniff(refused), None, "{refused:?}");
    }
}

pub fn names_follow_the_note_the_moment_and_the_kind() {
    use attachments::{free_name, generated_name, kept_name};
    assert_eq!(
        generated_name(Path::new(NOTE), &MOMENT, ImageKind::Png),
        GENERATED
    );
    assert_eq!(kept_name("схема.png", ImageKind::Png), "схема.png");
    assert_eq!(kept_name("photo.png", ImageKind::Jpeg), "photo.jpg");
    assert_eq!(kept_name("a:b*c?.png", ImageKind::Png), "a-b-c-.png");
    assert_eq!(kept_name("", ImageKind::Gif), "image.gif");
    assert_eq!(
        free_name("схема.png", |name| name == "схема.png"),
        Some("схема-2.png".into())
    );
    assert_eq!(free_name("x.png", |_| true), None);
}

pub fn folder_and_path_for_both_placements_and_three_depths() {
    use attachments::{markup_path, target_folder};
    let next = VaultOverrides::default();
    let shared = VaultOverrides {
        attachments_placement: Some(AttachmentsPlacement::VaultFolder),
        attachments_folder: Some("файлы".into()),
        ..VaultOverrides::default()
    };
    for (note, next_folder, shared_markup) in [
        ("Корень.typ", "attachments", "файлы/x.png"),
        ("a/Глава.typ", "a/attachments", "../файлы/x.png"),
        ("a/b/Глава.typ", "a/b/attachments", "../../файлы/x.png"),
    ] {
        let note = Path::new(note);
        assert_eq!(target_folder(&next, note), Path::new(next_folder));
        assert_eq!(
            markup_path(note, &target_folder(&next, note).join("x.png")),
            "attachments/x.png"
        );
        assert_eq!(target_folder(&shared, note), Path::new("файлы"));
        assert_eq!(
            markup_path(note, &target_folder(&shared, note).join("x.png")),
            shared_markup
        );
    }
}

/// Обязательство 1: правило названия папки — то же, что у папки экспорта.
pub fn the_folder_rule_is_the_export_folder_rule() {
    use typsidian_core::export::folder::validate_folder as export_rule;
    for raw in [
        "attachments",
        " файлы//картинки ",
        "a\\b",
        "",
        "  ",
        "/abs",
        "C:\\x",
        "../x",
        "a/./b",
        ".typsidian",
        ".typsidian/x",
    ] {
        let ours = attachments::validate_folder(raw);
        let theirs = export_rule(raw).map_err(AttachmentsFolderProblem::from);
        assert_eq!(ours, theirs, "{raw:?}");
    }
}

pub fn markup_is_escaped_and_planned_inline_or_by_lines() {
    assert_eq!(image_markup("a\"b\\c.png"), "#image(\"a\\\"b\\\\c.png\")");
    let at = |offset| InsertionPoint {
        offset,
        selection: None,
    };
    let one = vec!["attachments/x.png".to_string()];
    let edit = plan_markup(12, &at(3), "При", "мер", &one, false).unwrap();
    assert_eq!(edit.text, "#image(\"attachments/x.png\")");
    assert_eq!(edit.replace, TextSpan { start: 3, end: 3 });
    assert_eq!(edit.caret, 3 + edit.text.encode_utf16().count());

    let two = vec!["a.png".to_string(), "b.png".to_string()];
    let edit = plan_markup(12, &at(3), "При", "мер", &two, true).unwrap();
    assert_eq!(edit.text, "\n#image(\"a.png\")\n#image(\"b.png\")\n");
    let edit = plan_markup(0, &at(0), " ", "\t", &two, true).unwrap();
    assert_eq!(edit.text, "#image(\"a.png\")\n#image(\"b.png\")");

    let selected = InsertionPoint {
        offset: 1,
        selection: Some(TextSpan { start: 1, end: 4 }),
    };
    let edit = plan_markup(12, &selected, "", "", &one, false).unwrap();
    assert_eq!(edit.replace, TextSpan { start: 1, end: 4 });
    assert!(plan_markup(12, &at(0), "", "", &[], true).is_none());
}

// ── US1: одна картинка из буфера (T021) ─────────────────────────────────────

pub async fn a_clipboard_image_lands_in_the_attachments_folder() {
    let mut vault = MemoryVault::new().with_file(NOTE, "= Глава");
    let bytes = png(1);
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        NOTE,
        vec![(clip(&bytes), bytes.clone())],
    )
    .await;
    let path = format!("{FOLDER}/{GENERATED}");
    assert_eq!(
        status(&view, 0),
        &ItemStatus::Stored {
            path: path.clone(),
            markup_path: format!("attachments/{GENERATED}"),
            bytes: bytes.len() as u64,
            saved_as: None,
        }
    );
    assert_eq!(view.markup_paths, vec![format!("attachments/{GENERATED}")]);
    assert_eq!(view.state.phase, AttachmentBatchPhase::Finished);
    assert!(view.is_over);
    // Обычный файл vault вне служебного места, читается байт в байт (FR-015).
    assert_eq!(vault.bytes_of(&path), Some(bytes));
    assert!(!path.starts_with(".typsidian"));
}

pub async fn an_existing_folder_with_other_images_still_takes_the_new_one() {
    let mut vault = MemoryVault::new()
        .with_file(NOTE, "= Глава")
        .with_file(&format!("{FOLDER}/старое.png"), png(9));
    let bytes = png(1);
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        NOTE,
        vec![(clip(&bytes), bytes)],
    )
    .await;
    assert!(matches!(status(&view, 0), ItemStatus::Stored { .. }));
    assert_eq!(vault.paths_under(FOLDER).len(), 2);
}

/// SC-003: занятое имя даёт `-2`, прежний файл цел байт в байт.
pub async fn a_busy_name_gets_a_suffix_and_the_old_file_is_untouched() {
    let old = png(7);
    let mut vault = MemoryVault::new()
        .with_file(NOTE, "= Глава")
        .with_file(&format!("{FOLDER}/{GENERATED}"), old.clone());
    let bytes = png(1);
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        NOTE,
        vec![(clip(&bytes), bytes.clone())],
    )
    .await;
    let renamed = "Глава-2026-10-10-142305-2.png";
    match status(&view, 0) {
        ItemStatus::Stored { path, saved_as, .. } => {
            assert_eq!(path, &format!("{FOLDER}/{renamed}"));
            assert_eq!(saved_as.as_deref(), Some(renamed));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(vault.bytes_of(&format!("{FOLDER}/{GENERATED}")), Some(old));
    assert_eq!(vault.bytes_of(&format!("{FOLDER}/{renamed}")), Some(bytes));
}

/// FR-010, SC-004: то же содержимое в папке назначения — ссылка на него.
pub async fn the_same_content_in_the_target_folder_is_reused() {
    let bytes = png(3);
    let mut vault = MemoryVault::new()
        .with_file(NOTE, "= Глава")
        .with_file(&format!("{FOLDER}/уже.png"), bytes.clone());
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        NOTE,
        vec![(clip(&bytes), bytes)],
    )
    .await;
    assert_eq!(
        status(&view, 0),
        &ItemStatus::Reused {
            path: format!("{FOLDER}/уже.png"),
            markup_path: "attachments/уже.png".into(),
        }
    );
    assert_eq!(vault.paths_under(FOLDER).len(), 1, "второй файл не создан");
}

pub async fn an_unsupported_kind_is_skipped_without_writing() {
    let mut vault = MemoryVault::new().with_file(NOTE, "= Глава");
    let pdf = b"%PDF-1.7\n".to_vec();
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        NOTE,
        vec![(clip(&pdf), pdf)],
    )
    .await;
    assert_eq!(status(&view, 0), &ItemStatus::Skipped);
    assert!(view.markup_paths.is_empty());
    assert!(vault.paths_under(FOLDER).is_empty());
}

/// Порядок шагов: вид проверяется первым — неподдерживаемый файл с тем же
/// содержимым, что лежит в папке, пропускается, а не «переиспользуется».
pub async fn the_kind_is_checked_before_matching() {
    let text = b"not an image".to_vec();
    let mut vault = MemoryVault::new()
        .with_file(NOTE, "= Глава")
        .with_file(&format!("{FOLDER}/x.png"), text.clone());
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        NOTE,
        vec![(clip(&text), text)],
    )
    .await;
    assert_eq!(status(&view, 0), &ItemStatus::Skipped);
}

pub async fn a_read_only_vault_or_a_non_note_refuses_without_touching_the_disk() {
    let bytes = png(1);
    let vault = MemoryVault::new().with_file(NOTE, "= Глава").read_only();
    let state = batch::start(
        &vault,
        &VaultOverrides::default(),
        NOTE,
        vec![clip(&bytes)],
        MOMENT,
    )
    .await;
    assert_eq!(
        state.phase,
        AttachmentBatchPhase::Refused {
            reason: AttachmentRefusalReason::ReadOnly
        }
    );
    assert_eq!(batch::step(state.clone()).next, None);
    assert!(batch::view(state).is_over);

    let vault = MemoryVault::new();
    let state = batch::start(
        &vault,
        &VaultOverrides::default(),
        "Доска.excalidraw",
        vec![clip(&bytes)],
        MOMENT,
    )
    .await;
    assert_eq!(
        state.phase,
        AttachmentBatchPhase::Refused {
            reason: AttachmentRefusalReason::NotANote
        }
    );
    assert_eq!(vault.total_reads(), 0, "к диску не обращались");
}

/// FR-014: отказ записи — ни файла, ни пути для вставки.
pub async fn a_failed_write_leaves_no_file_and_no_markup() {
    let mut vault = MemoryVault::new()
        .with_file(NOTE, "= Глава")
        .with_write_failure(&format!("{FOLDER}/{GENERATED}"));
    let bytes = png(1);
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        NOTE,
        vec![(clip(&bytes), bytes)],
    )
    .await;
    assert_eq!(
        status(&view, 0),
        &ItemStatus::Failed {
            problem: StoreProblem::Io
        }
    );
    assert!(view.markup_paths.is_empty());
    assert!(vault.paths_under(FOLDER).is_empty());
}

// ── Переходы (T022) ────────────────────────────────────────────────────────

pub async fn steps_hand_out_one_file_at_a_time() {
    let vault = MemoryVault::new().with_file(NOTE, "= Глава");
    let (a, b) = (png(1), png(2));
    let state = batch::start(
        &vault,
        &VaultOverrides::default(),
        NOTE,
        vec![dropped("a.png", &a), dropped("b.png", &b)],
        MOMENT,
    )
    .await;
    let step = batch::step(state);
    assert_eq!(step.next, Some(NextAction::Copy { index: 0 }));
    // Пока первый в работе, второго не выдают.
    let again = batch::step(step.state.clone());
    assert_eq!(again.next, None);
    assert_eq!(again.state, step.state);

    // `store` и `source_failed` не для `Copying` состояние не меняют.
    let mut vault = vault;
    let untouched = batch::store(&mut vault, step.state.clone(), 1, &b).await;
    assert_eq!(untouched, step.state);
    assert_eq!(batch::source_failed(step.state.clone(), 1), step.state);

    let failed = batch::source_failed(step.state, 0);
    assert_eq!(
        failed.items[0].status,
        ItemStatus::Failed {
            problem: StoreProblem::SourceUnreadable
        }
    );
    let next = batch::step(failed);
    assert_eq!(next.next, Some(NextAction::Copy { index: 1 }));
    let view = batch::view(next.state);
    assert_eq!((view.processed, view.total, view.is_over), (1, 2, false));
}

pub async fn an_empty_batch_is_finished_at_once() {
    let vault = MemoryVault::new().with_file(NOTE, "= Глава");
    let state = batch::start(&vault, &VaultOverrides::default(), NOTE, vec![], MOMENT).await;
    assert_eq!(state.phase, AttachmentBatchPhase::Finished);
    let view = batch::view(state);
    assert!(view.is_over);
    assert!(view.markup_paths.is_empty());
}

// ── US2: несколько файлов (T037) ───────────────────────────────────────────

/// FR-011: файл с тем же именем и содержимым где-то в vault не копируется.
pub async fn a_dropped_file_already_in_the_vault_is_referenced() {
    let bytes = png(5);
    let mut vault = MemoryVault::new()
        .with_file(NOTE, "= Глава")
        .with_file("Общее/схема.png", bytes.clone());
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        NOTE,
        vec![(dropped("схема.png", &bytes), bytes)],
    )
    .await;
    assert_eq!(
        status(&view, 0),
        &ItemStatus::Reused {
            path: "Общее/схема.png".into(),
            markup_path: "../Общее/схема.png".into(),
        }
    );
    assert!(vault.paths_under(FOLDER).is_empty());
}

pub async fn two_identical_files_in_one_batch_make_one_file() {
    let bytes = png(4);
    let mut vault = MemoryVault::new().with_file(NOTE, "= Глава");
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        NOTE,
        vec![
            (dropped("раз.png", &bytes), bytes.clone()),
            (dropped("два.png", &bytes), bytes),
        ],
    )
    .await;
    assert!(matches!(status(&view, 0), ItemStatus::Stored { .. }));
    assert!(matches!(status(&view, 1), ItemStatus::Reused { .. }));
    assert_eq!(vault.paths_under(FOLDER), vec![format!("{FOLDER}/раз.png")]);
    assert_eq!(view.markup_paths.len(), 2, "обе записи вставляются");
}

pub async fn a_mixed_batch_writes_images_and_skips_the_rest_by_name() {
    let (a, b) = (png(1), jpeg(2));
    let pdf = b"%PDF-1.4".to_vec();
    let mut vault = MemoryVault::new()
        .with_file(NOTE, "= Глава")
        .with_write_failure(&format!("{FOLDER}/сбой.png"));
    let failing = png(3);
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        NOTE,
        vec![
            (dropped("первая.png", &a), a),
            (dropped("отчёт.pdf", &pdf), pdf),
            (dropped("сбой.png", &failing), failing),
            (dropped("Фото.JPG", &b), b),
        ],
    )
    .await;
    assert!(matches!(status(&view, 0), ItemStatus::Stored { .. }));
    assert_eq!(status(&view, 1), &ItemStatus::Skipped);
    assert!(matches!(status(&view, 2), ItemStatus::Failed { .. }));
    assert!(
        matches!(status(&view, 3), ItemStatus::Stored { saved_as: None, .. }),
        "имя перетащенного файла сохранено (FR-012)"
    );
    assert_eq!(
        vault.paths_under(FOLDER),
        vec![format!("{FOLDER}/Фото.JPG"), format!("{FOLDER}/первая.png")],
        "PDF и отказавший файл в vault не появились"
    );
    assert_eq!(
        (
            view.summary.stored,
            view.summary.skipped,
            view.summary.failed
        ),
        (2, 1, 1)
    );
    assert_eq!(
        view.markup_paths,
        vec!["attachments/первая.png", "attachments/Фото.JPG"]
    );
}

// ── Подтверждение и отмена (T038) ──────────────────────────────────────────

fn sized(size: u64) -> IncomingItem {
    IncomingItem {
        name: Some(format!("{size}.png")),
        size,
        origin: Origin::File,
    }
}

pub async fn large_files_are_confirmed_before_any_copy() {
    let vault = MemoryVault::new().with_file(NOTE, "= Глава");
    let state = batch::start(
        &vault,
        &VaultOverrides::default(),
        NOTE,
        vec![
            sized(LARGE_IMAGE_BYTES),
            sized(LARGE_IMAGE_BYTES + 1),
            sized(10),
            sized(LARGE_IMAGE_BYTES + 2),
        ],
        MOMENT,
    )
    .await;
    assert_eq!(state.items[0].status, ItemStatus::Queued, "ровно 20 МБ");
    assert_eq!(state.items[1].status, ItemStatus::AwaitingConfirmation);
    let step = batch::step(state);
    assert_eq!(step.next, Some(NextAction::Confirm { index: 1 }));
    // Ответ не для ожидающего ничего не меняет.
    let same = batch::confirm(step.state.clone(), 0, false);
    assert_eq!(same, step.state);
    let state = batch::confirm(step.state, 1, false);
    assert_eq!(state.items[1].status, ItemStatus::Declined);
    let step = batch::step(state);
    assert_eq!(step.next, Some(NextAction::Confirm { index: 3 }));
    let state = batch::confirm(step.state, 3, true);
    assert_eq!(state.items[3].status, ItemStatus::Queued);
    assert_eq!(batch::step(state).next, Some(NextAction::Copy { index: 0 }));
}

/// FR-023: отмена во время копирования оставляет текущий файл, остальные
/// отменены; записанное цело и вставляется.
pub async fn cancel_keeps_the_current_and_written_files() {
    let (a, b, c) = (png(1), png(2), png(3));
    let mut vault = MemoryVault::new().with_file(NOTE, "= Глава");
    let mut state = batch::start(
        &vault,
        &VaultOverrides::default(),
        NOTE,
        vec![
            dropped("a.png", &a),
            dropped("b.png", &b),
            dropped("c.png", &c),
        ],
        MOMENT,
    )
    .await;
    let step = batch::step(state);
    state = batch::store(&mut vault, step.state, 0, &a).await;
    let step = batch::step(state);
    assert_eq!(step.next, Some(NextAction::Copy { index: 1 }));
    // Отмена, пока второй в работе.
    state = batch::cancel(step.state);
    assert_eq!(state.phase, AttachmentBatchPhase::Cancelled);
    assert_eq!(state.items[1].status, ItemStatus::Copying);
    assert_eq!(state.items[2].status, ItemStatus::Cancelled);
    assert!(!batch::view(state.clone()).is_over, "текущий ещё пишется");
    state = batch::store(&mut vault, state, 1, &b).await;
    assert_eq!(batch::step(state.clone()).next, None);
    let view = batch::view(state);
    assert!(view.is_over);
    assert_eq!(view.summary.cancelled, 1);
    assert_eq!(
        view.markup_paths,
        vec!["attachments/a.png", "attachments/b.png"]
    );
    assert_eq!(vault.paths_under(FOLDER).len(), 2);
}

// ── US3: правила исключения и размещение (T052, T053) ───────────────────────

pub fn ignore_rules_exclude_a_folder_by_the_research_table() {
    let git = IgnoreFile::GitIgnore;
    let st = IgnoreFile::StIgnore;
    assert!(excludes(git, "attachments/\n", "Глава/attachments"));
    assert!(excludes(git, "/attachments\n", "attachments"));
    assert!(!excludes(git, "/attachments\n", "Глава/attachments"));
    assert!(excludes(git, "**/attachments\n", "a/b/attachments"));
    assert!(!excludes(git, "attachments\n!attachments\n", "attachments"));
    assert!(!excludes(st, "!attachments\nattachments\n", "attachments"));
    assert!(excludes(git, "Архив/\n", "Архив/Глава/attachments"));
    assert!(excludes(st, "(?i)ATTACHMENTS\n", "attachments"));
    assert!(excludes(st, "{files,attachments}\n", "attachments"));
    assert!(!excludes(git, "# attachments\n", "attachments"));
}

/// FR-020: исключённая папка — предупреждение, а не запрет.
pub async fn an_excluded_folder_is_reported_and_still_written() {
    let bytes = png(1);
    let mut vault = MemoryVault::new()
        .with_file(NOTE, "= Глава")
        .with_file(".gitignore", "attachments/\n")
        .with_file(".stignore", "// ничего\n");
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        NOTE,
        vec![(clip(&bytes), bytes)],
    )
    .await;
    assert_eq!(view.state.excluded_by, vec![IgnoreFile::GitIgnore]);
    assert!(matches!(status(&view, 0), ItemStatus::Stored { .. }));
}

/// FR-019: смена настройки между вставками не трогает первый файл, и его
/// запись по-прежнему ведёт к нему.
pub async fn changing_the_placement_moves_nothing() {
    let first = png(1);
    let second = png(2);
    let mut vault = MemoryVault::new().with_file("a/b/Глава.typ", "= Глава");
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        "a/b/Глава.typ",
        vec![(clip(&first), first.clone())],
    )
    .await;
    let first_path = format!("a/b/attachments/{GENERATED}");
    assert_eq!(view.markup_paths, vec![format!("attachments/{GENERATED}")]);

    let shared = VaultOverrides {
        attachments_placement: Some(AttachmentsPlacement::VaultFolder),
        attachments_folder: Some("файлы/картинки".into()),
        ..VaultOverrides::default()
    };
    let view = run(
        &mut vault,
        &shared,
        "a/b/Глава.typ",
        vec![(clip(&second), second)],
    )
    .await;
    assert_eq!(
        view.markup_paths,
        vec![format!("../../файлы/картинки/{GENERATED}")]
    );
    assert_eq!(vault.bytes_of(&first_path), Some(first), "первый на месте");
}

// ── US4: камера (T060) ─────────────────────────────────────────────────────

pub async fn a_camera_shot_is_named_like_a_clipboard_image() {
    let bytes = jpeg(1);
    let mut vault = MemoryVault::new().with_file(NOTE, "= Глава");
    let shot = IncomingItem {
        name: Some("JPEG_20261010_142305_123.jpg".into()),
        size: bytes.len() as u64,
        origin: Origin::Camera,
    };
    let view = run(
        &mut vault,
        &VaultOverrides::default(),
        NOTE,
        vec![(shot, bytes)],
    )
    .await;
    assert_eq!(
        view.markup_paths,
        vec!["attachments/Глава-2026-10-10-142305.jpg"]
    );
}

/// FR-022: отменённая съёмка — пустой выбор, пачка сразу закончена.
pub async fn a_cancelled_shot_changes_nothing() {
    let mut vault = MemoryVault::new().with_file(NOTE, "= Глава");
    let view = run(&mut vault, &VaultOverrides::default(), NOTE, vec![]).await;
    assert_eq!(view.state.phase, AttachmentBatchPhase::Finished);
    assert!(view.markup_paths.is_empty());
    assert!(vault.paths_under(FOLDER).is_empty());
}

/// Значение пачки проходит через границу без потерь (wasm и Tauri гоняют его
/// JSON туда и обратно).
pub async fn the_batch_state_survives_serialization() {
    let vault = MemoryVault::new().with_file(NOTE, "= Глава");
    let bytes = png(1);
    let state = batch::start(
        &vault,
        &VaultOverrides::default(),
        NOTE,
        vec![clip(&bytes), sized(LARGE_IMAGE_BYTES + 1)],
        MOMENT,
    )
    .await;
    let text = serde_json::to_string(&state).unwrap();
    let back: AttachmentBatchState = serde_json::from_str(&text).unwrap();
    assert_eq!(back, state);
}
