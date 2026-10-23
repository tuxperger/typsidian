//! Снимки заметок (фича 029) — общая таблица для обоих таргетов.
//!
//! Файлы `snapshots_native.rs` / `snapshots_wasm.rs` только вызывают эти
//! функции своим harness'ом: правила снимков платформенно нейтральны
//! (принцип II). Время передаётся явно — часы здесь не участвуют.

#![allow(dead_code)]

use std::path::Path;

use typsidian_core::elements::edit::TextEdit;
use typsidian_core::snapshots::{
    before_delete, deleted_notes, move_snapshots, recover_note, snapshot_diff, snapshot_history,
    snapshot_restore, snapshot_take, snapshot_text, snapshot_usage, snapshots_clear,
    RecoverOutcome, RestoreOutcome, SnapshotFailure, SnapshotReason, SnapshotSkip, TakeOutcome,
};
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::conflict::DiffLine;
use typsidian_core::vault::vault_settings::{read_vault_settings, write_vault_settings};

use super::MemoryVault;

const NOTE: &str = "Диссертация/Введение.typ";
const MINUTE: i64 = 60_000;
const DAY: i64 = 86_400_000;
/// 2026-10-22 12:00 UTC.
const T0: i64 = 1_792_670_400_000;

const FIRST: &str = "= Введение\n\nПостановка задачи.\nТребуется оценить образцы.\n";
const SECOND: &str = "= Введение\n\nЗадача.\n";

fn vault() -> MemoryVault {
    MemoryVault::new()
        .with_file(NOTE, FIRST)
        .with_file("Заметки/Встреча.typ", "- перечитать главу 2\n")
        .with_file("Заметки/схема.excalidraw", "{}")
        .checking_expected()
}

fn taken(outcome: TakeOutcome) -> typsidian_core::snapshots::SnapshotEntry {
    match outcome {
        TakeOutcome::Taken { entry } => entry,
        other => panic!("ждали Taken, получили {other:?}"),
    }
}

fn text_of(vault: &MemoryVault, path: &str) -> Option<String> {
    vault
        .bytes_of(path)
        .map(|bytes| String::from_utf8(bytes).unwrap())
}

fn snapshot_file(id: &str) -> String {
    format!(".typsidian/snapshots/{id}.txt")
}

/// Применяет правку в единицах UTF-16 — так её применит редактор.
fn apply_utf16(text: &str, edit: &TextEdit) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut out: Vec<u16> = units[..edit.from].to_vec();
    out.extend(edit.insert.encode_utf16());
    out.extend_from_slice(&units[edit.to..]);
    String::from_utf16(&out).unwrap()
}

async fn set_limits(vault: &mut MemoryVault, keep_days: u32, interval_minutes: u32) {
    let mut settings = read_vault_settings(vault).await.unwrap();
    settings.snapshot_keep_days = Some(keep_days);
    settings.snapshot_interval_minutes = Some(interval_minutes);
    write_vault_settings(vault, &settings).await.unwrap();
}

// ── US1 ─────────────────────────────────────────────────────────────────────

/// Снимок делается, одинаковый текст не дублируется, `Save` ждёт интервал;
/// изменение считается в строках (FR-001, FR-002, Edge Cases).
pub async fn snapshots_follow_interval_and_skip_duplicates() {
    let mut vault = vault();
    let first = taken(snapshot_take(&mut vault, NOTE, FIRST, SnapshotReason::Open, T0).await);
    assert_eq!((first.added, first.removed), (4, 0));
    assert_eq!(first.size, FIRST.len() as u64);
    assert_eq!(
        text_of(&vault, &snapshot_file(&first.id)).as_deref(),
        Some(FIRST)
    );

    assert_eq!(
        snapshot_take(&mut vault, NOTE, FIRST, SnapshotReason::Close, T0 + MINUTE).await,
        TakeOutcome::Skipped {
            why: SnapshotSkip::Same
        }
    );
    assert_eq!(
        snapshot_take(&mut vault, NOTE, SECOND, SnapshotReason::Save, T0 + MINUTE).await,
        TakeOutcome::Skipped {
            why: SnapshotSkip::TooSoon
        }
    );
    let second = taken(
        snapshot_take(
            &mut vault,
            NOTE,
            SECOND,
            SnapshotReason::Save,
            T0 + 6 * MINUTE,
        )
        .await,
    );
    assert_eq!((second.added, second.removed), (1, 2));

    // Не-`Save` интервала не ждёт.
    let third = taken(
        snapshot_take(
            &mut vault,
            NOTE,
            FIRST,
            SnapshotReason::External,
            T0 + 7 * MINUTE,
        )
        .await,
    );
    assert_eq!(third.reason, SnapshotReason::External);

    assert_eq!(
        snapshot_take(
            &mut vault,
            "Заметки/схема.excalidraw",
            "{}",
            SnapshotReason::Open,
            T0
        )
        .await,
        TakeOutcome::Skipped {
            why: SnapshotSkip::NotNote
        }
    );
}

/// История — новые первыми; различия со снимком построчно (FR-002).
pub async fn history_is_newest_first_with_line_diff() {
    let mut vault = vault();
    taken(snapshot_take(&mut vault, NOTE, FIRST, SnapshotReason::Open, T0).await);
    let second = taken(
        snapshot_take(
            &mut vault,
            NOTE,
            SECOND,
            SnapshotReason::Save,
            T0 + 6 * MINUTE,
        )
        .await,
    );

    let history = snapshot_history(&vault, NOTE, T0 + 7 * MINUTE).await;
    let reasons: Vec<SnapshotReason> = history.entries.iter().map(|entry| entry.reason).collect();
    assert_eq!(reasons, [SnapshotReason::Save, SnapshotReason::Open]);
    assert_eq!(history.entries[0].id, second.id);
    assert_eq!(history.deleted_at_ms, None);

    let first_id = &history.entries[1].id;
    let diff = snapshot_diff(&vault, first_id, SECOND).await.unwrap();
    assert!(diff.contains(&DiffLine::Local {
        text: "Постановка задачи.".into()
    }));
    assert!(diff.contains(&DiffLine::External {
        text: "Задача.".into()
    }));
    assert!(snapshot_diff(&vault, "../vault", SECOND).await.is_none());
    assert!(snapshot_history(&vault, "Заметки/Встреча.typ", T0)
        .await
        .entries
        .is_empty());
}

/// Возврат: открытая заметка — правка на весь текст, закрытая — запись байт в
/// байт; текущий текст перед этим сам становится снимком (FR-003, SC-002).
pub async fn restore_goes_to_the_editor_or_the_file_byte_for_byte() {
    let tricky = "😀 строка\r\nвторая\tстрока\r\n\r\n";
    let mut vault = MemoryVault::new()
        .with_file(NOTE, tricky)
        .checking_expected();
    let old = taken(snapshot_take(&mut vault, NOTE, tricky, SnapshotReason::Open, T0).await);

    let editor = "😀 всё стёрто";
    let outcome = snapshot_restore(&mut vault, NOTE, &old.id, Some(editor), T0 + MINUTE).await;
    let RestoreOutcome::EditorEdit { edit } = outcome else {
        panic!("ждали EditorEdit, получили {outcome:?}");
    };
    assert_eq!(apply_utf16(editor, &edit), tricky);
    assert_eq!(
        text_of(&vault, NOTE).as_deref(),
        Some(tricky),
        "диск не тронут"
    );
    let history = snapshot_history(&vault, NOTE, T0 + MINUTE).await;
    assert_eq!(history.entries[0].reason, SnapshotReason::BeforeRestore);
    assert_eq!(
        snapshot_text(&vault, &history.entries[0].id)
            .await
            .as_deref(),
        Some(editor)
    );

    // Закрытая заметка: файл переписан ровно текстом снимка.
    vault.replace_externally(NOTE, "стёрто на диске");
    assert_eq!(
        snapshot_restore(&mut vault, NOTE, &old.id, None, T0 + 2 * MINUTE).await,
        RestoreOutcome::Written
    );
    assert_eq!(vault.bytes_of(NOTE).unwrap(), tricky.as_bytes());

    assert_eq!(
        snapshot_restore(&mut vault, NOTE, "1-abc", None, T0).await,
        RestoreOutcome::Missing
    );
    let mut read_only = vault.read_only();
    assert_eq!(
        snapshot_restore(&mut read_only, NOTE, &old.id, None, T0).await,
        RestoreOutcome::ReadOnly
    );
    assert_eq!(
        snapshot_take(&mut read_only, NOTE, "x", SnapshotReason::Open, T0).await,
        TakeOutcome::Failed {
            why: SnapshotFailure::ReadOnly
        }
    );
}

/// Срок хранения: истёкшие снимки уходят из описи вместе с файлами (FR-005).
pub async fn expired_snapshots_are_removed_with_their_files() {
    let mut vault = vault();
    set_limits(&mut vault, 1, 5).await;
    let old = taken(snapshot_take(&mut vault, NOTE, FIRST, SnapshotReason::Open, T0).await);
    let fresh =
        taken(snapshot_take(&mut vault, NOTE, SECOND, SnapshotReason::Save, T0 + 2 * DAY).await);

    let history = snapshot_history(&vault, NOTE, T0 + 2 * DAY).await;
    let ids: Vec<&str> = history
        .entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(ids, [fresh.id.as_str()]);
    assert!(
        vault.bytes_of(&snapshot_file(&old.id)).is_none(),
        "файл снят"
    );
    assert!(vault.bytes_of(&snapshot_file(&fresh.id)).is_some());
}

/// История едет за заметкой и за папкой (Edge Cases).
pub async fn history_follows_renames() {
    let mut vault = vault();
    taken(snapshot_take(&mut vault, NOTE, FIRST, SnapshotReason::Open, T0).await);
    move_snapshots(
        &mut vault,
        Path::new(NOTE),
        Path::new("Диссертация/Глава 1.typ"),
    )
    .await
    .unwrap();
    move_snapshots(
        &mut vault,
        Path::new("Диссертация"),
        Path::new("Архив/Диссертация"),
    )
    .await
    .unwrap();
    assert!(snapshot_history(&vault, NOTE, T0).await.entries.is_empty());
    let moved = snapshot_history(&vault, "Архив/Диссертация/Глава 1.typ", T0).await;
    assert_eq!(moved.entries.len(), 1);
}

// ── US2 ─────────────────────────────────────────────────────────────────────

/// Удаление делает снимок и отметку; заметка восстанавливается на прежнее
/// место, а при занятом пути — под предложенным именем (FR-004, US2 AS1).
pub async fn deleted_notes_are_recovered_or_renamed() {
    let mut vault = vault();
    let meeting = "Заметки/Встреча.typ";
    before_delete(&mut vault, Path::new("Заметки"), T0).await;
    vault.delete(Path::new(meeting)).await.unwrap();

    let deleted = deleted_notes(&vault, T0 + MINUTE).await;
    assert_eq!(deleted.len(), 1, "доска снимков не получает");
    assert_eq!(
        (
            deleted[0].path.as_str(),
            deleted[0].title.as_str(),
            deleted[0].folder.as_str()
        ),
        (meeting, "Встреча", "Заметки")
    );
    assert_eq!((deleted[0].deleted_at_ms, deleted[0].snapshots), (T0, 1));

    assert_eq!(
        recover_note(&mut vault, meeting, None).await,
        RecoverOutcome::Recovered {
            path: meeting.into()
        }
    );
    assert_eq!(
        text_of(&vault, meeting).as_deref(),
        Some("- перечитать главу 2\n")
    );
    assert!(deleted_notes(&vault, T0 + MINUTE).await.is_empty());

    // Снова удалена, а на её месте — новая заметка с тем же именем.
    before_delete(&mut vault, Path::new(meeting), T0 + 2 * MINUTE).await;
    vault.delete(Path::new(meeting)).await.unwrap();
    vault.replace_externally(meeting, "новая");
    let occupied = recover_note(&mut vault, meeting, None).await;
    assert_eq!(
        occupied,
        RecoverOutcome::Occupied {
            suggested: "Заметки/Встреча (2).typ".into()
        }
    );
    assert_eq!(
        recover_note(&mut vault, meeting, Some("Заметки/Встреча (2).typ")).await,
        RecoverOutcome::Recovered {
            path: "Заметки/Встреча (2).typ".into()
        }
    );
    assert_eq!(
        text_of(&vault, "Заметки/Встреча (2).typ").as_deref(),
        Some("- перечитать главу 2\n")
    );
    assert_eq!(text_of(&vault, meeting).as_deref(), Some("новая"));
    assert_eq!(
        recover_note(&mut vault, "Заметки/Нет.typ", None).await,
        RecoverOutcome::Missing
    );
}

// ── US3 ─────────────────────────────────────────────────────────────────────

/// Занятое место и «Очистить»: все снимки и их файлы убраны (US3).
pub async fn usage_and_clear() {
    let mut vault = vault();
    let first = taken(snapshot_take(&mut vault, NOTE, FIRST, SnapshotReason::Open, T0).await);
    taken(snapshot_take(&mut vault, NOTE, SECOND, SnapshotReason::Close, T0 + MINUTE).await);
    let usage = snapshot_usage(&vault).await;
    assert_eq!(usage.snapshots, 2);
    assert_eq!(usage.bytes, (FIRST.len() + SECOND.len()) as u64);

    assert_eq!(snapshots_clear(&mut vault).await, Ok(2));
    assert_eq!(snapshot_usage(&vault).await.snapshots, 0);
    assert!(vault.bytes_of(&snapshot_file(&first.id)).is_none());
    assert_eq!(
        text_of(&vault, NOTE).as_deref(),
        Some(FIRST),
        "заметка не тронута"
    );
}
