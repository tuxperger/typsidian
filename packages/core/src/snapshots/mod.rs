//! Локальные снимки заметок (фича 029).
//!
//! Снимки лежат в служебной папке vault (`.typsidian/snapshots/`): опись
//! `index.json` и по файлу `<id>.txt` на снимок (research.md §1). Путь входит в
//! производные пути устройства — правила истории 011 и Syncthing его не видят.
//! Всё работает поверх `VaultBackend`, поэтому одинаково на всех оболочках.
//!
//! Здесь решается, пора ли делать снимок, одинаков ли текст с прошлым, что
//! истекло по сроку, как вернуть снимок и восстановить удалённую заметку. Когда
//! звать — дело оболочки (открытие, сохранение, закрытие, внешнее изменение);
//! снимок перед удалением и перенос истории при переименовании делает сам
//! backend, чтобы их не обошла ни одна оболочка.

#[cfg(target_arch = "wasm32")]
pub mod web;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::elements::edit::TextEdit;
use crate::text_offsets::utf16_at;
use crate::vault::backend::{ReadError, VaultBackend, WriteError};
use crate::vault::conflict::{line_diff, DiffLine};
use crate::vault::service_dir::snapshots_dir;
use crate::vault::{classify, AccessState, FileKind};

/// Срок хранения по умолчанию, дней (FR-005).
pub const DEFAULT_KEEP_DAYS: u32 = 7;
/// Интервал снимков при сохранении по умолчанию, минут (FR-001).
pub const DEFAULT_INTERVAL_MINUTES: u32 = 5;

const DAY_MS: i64 = 86_400_000;
const MINUTE_MS: i64 = 60_000;
const INDEX_FILE: &str = "index.json";
const INDEX_VERSION: u32 = 1;
/// Знаков хэша текста в идентификаторе снимка.
const HASH_CHARS: usize = 12;

/// Повод снимка.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SnapshotReason {
    Open,
    /// После сохранения правок — не чаще интервала.
    Save,
    Close,
    /// Файл изменили мимо приложения.
    External,
    /// Текущий текст перед возвратом снимка.
    BeforeRestore,
    Delete,
}

/// Один снимок заметки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SnapshotEntry {
    /// `<время мс>-<начало хэша текста>`; имя файла снимка.
    pub id: String,
    pub at_ms: i64,
    pub reason: SnapshotReason,
    /// Размер текста, байт.
    pub size: u64,
    /// Строк добавлено относительно предыдущего снимка заметки.
    pub added: u32,
    /// Строк удалено относительно предыдущего снимка заметки.
    pub removed: u32,
}

/// Запись описи: история одной заметки.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct NoteRecord {
    path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    deleted_at_ms: Option<i64>,
    /// Старые первыми.
    #[serde(default)]
    entries: Vec<SnapshotEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct SnapshotIndex {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    notes: Vec<NoteRecord>,
}

/// Почему снимок не сделан, хотя ничего не сломалось.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SnapshotSkip {
    /// Текст совпадает с последним снимком.
    Same,
    /// Прошлый снимок моложе интервала (только для `Save`).
    TooSoon,
    /// Не заметка Typst: доски и прочие файлы снимками не покрываются.
    NotNote,
}

/// Почему снимок не записан.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SnapshotFailure {
    ReadOnly,
    /// Запись отказала — в том числе от нехватки места (data-model.md).
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum TakeOutcome {
    Taken { entry: SnapshotEntry },
    Skipped { why: SnapshotSkip },
    Failed { why: SnapshotFailure },
}

/// История заметки для окна «История заметки».
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct NoteHistory {
    pub path: String,
    /// Новые первыми.
    pub entries: Vec<SnapshotEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub deleted_at_ms: Option<i64>,
}

/// Удалённая заметка, которую можно восстановить.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DeletedNote {
    pub path: String,
    pub title: String,
    /// Папка относительно корня; пустая строка — корень.
    pub folder: String,
    pub deleted_at_ms: i64,
    pub snapshots: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum RestoreOutcome {
    /// Заметка открыта: правку на весь текст применяет редактор — одной
    /// единицей отмены (FR-003).
    EditorEdit {
        edit: TextEdit,
    },
    /// Файл записан.
    Written,
    /// Снимка или заметки больше нет.
    Missing,
    ReadOnly,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum RecoverOutcome {
    Recovered {
        path: String,
    },
    /// Путь занят другой заметкой — предложено свободное имя рядом.
    Occupied {
        suggested: String,
    },
    Missing,
    ReadOnly,
    Failed,
}

/// Сколько места занимают снимки vault.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SnapshotUsage {
    pub snapshots: u32,
    pub bytes: u64,
}

// ── Опись и файлы ───────────────────────────────────────────────────────────

fn index_path() -> PathBuf {
    snapshots_dir().join(INDEX_FILE)
}

fn text_path(id: &str) -> PathBuf {
    snapshots_dir().join(format!("{id}.txt"))
}

/// Идентификатор снимка годен как имя файла: цифры, `-` и шестнадцатеричные
/// знаки. Иное пришло не из описи — читать по нему нельзя.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|ch| ch == '-' || ch.is_ascii_hexdigit())
}

fn hash_of(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex()[..HASH_CHARS].to_owned()
}

fn hash_in(id: &str) -> &str {
    id.rsplit_once('-').map_or("", |(_, hash)| hash)
}

/// Испорченная или чужая опись — пустая: снимки теряются, заметки нет.
async fn read_index<B: VaultBackend>(backend: &B) -> SnapshotIndex {
    match backend.read(&index_path()).await {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => SnapshotIndex::default(),
    }
}

async fn write_index<B: VaultBackend>(
    backend: &mut B,
    index: &SnapshotIndex,
) -> Result<(), WriteError> {
    let index = SnapshotIndex {
        version: INDEX_VERSION,
        notes: index.notes.clone(),
    };
    let text = serde_json::to_string(&index).map_err(|_| WriteError::Io)?;
    backend.ensure_folder(&snapshots_dir()).await?;
    backend.write_bytes(&index_path(), text.as_bytes()).await
}

async fn read_text<B: VaultBackend>(backend: &B, id: &str) -> Option<String> {
    if !valid_id(id) {
        return None;
    }
    backend.read(&text_path(id)).await.ok()
}

/// Срок и интервал из настроек vault, в мс.
async fn limits<B: VaultBackend>(backend: &B) -> (i64, i64) {
    let settings = crate::vault::vault_settings::read_vault_settings(backend)
        .await
        .unwrap_or_default();
    let keep = settings.snapshot_keep_days.unwrap_or(DEFAULT_KEEP_DAYS);
    let interval = settings
        .snapshot_interval_minutes
        .unwrap_or(DEFAULT_INTERVAL_MINUTES);
    (i64::from(keep) * DAY_MS, i64::from(interval) * MINUTE_MS)
}

/// Убирает истёкшее из описи; возвращает идентификаторы убранных снимков.
fn prune(index: &mut SnapshotIndex, now: i64, keep_ms: i64) -> Vec<String> {
    let horizon = now - keep_ms;
    let mut gone = Vec::new();
    for record in &mut index.notes {
        let expired_note = record.deleted_at_ms.is_some_and(|at| at < horizon);
        record.entries.retain(|entry| {
            let keep = !expired_note && entry.at_ms >= horizon;
            if !keep {
                gone.push(entry.id.clone());
            }
            keep
        });
    }
    index.notes.retain(|record| !record.entries.is_empty());
    gone
}

async fn remove_texts<B: VaultBackend>(backend: &mut B, ids: &[String]) {
    for id in ids {
        if valid_id(id) {
            let _ = backend.remove_service_file(&text_path(id)).await;
        }
    }
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Строк добавлено и удалено от `old` к `new`.
fn changes(old: &str, new: &str) -> (u32, u32) {
    let mut added = 0;
    let mut removed = 0;
    for line in line_diff(old, new) {
        match line {
            DiffLine::Same { .. } => {}
            DiffLine::Local { .. } => removed += 1,
            DiffLine::External { .. } => added += 1,
        }
    }
    (added, removed)
}

// ── Снимок ──────────────────────────────────────────────────────────────────

/// Делает снимок заметки `path` с текстом `text`, если пора (research.md §2).
pub async fn snapshot_take<B: VaultBackend>(
    backend: &mut B,
    path: &str,
    text: &str,
    reason: SnapshotReason,
    now: i64,
) -> TakeOutcome {
    if classify(Path::new(path)) != FileKind::Note {
        return TakeOutcome::Skipped {
            why: SnapshotSkip::NotNote,
        };
    }
    if backend.access_state() != AccessState::ReadWrite {
        return TakeOutcome::Failed {
            why: SnapshotFailure::ReadOnly,
        };
    }
    let (keep_ms, interval_ms) = limits(backend).await;
    let mut index = read_index(backend).await;
    let gone = prune(&mut index, now, keep_ms);

    let hash = hash_of(text);
    let last = index
        .notes
        .iter()
        .find(|record| record.path == path)
        .and_then(|record| record.entries.last().cloned());
    if let Some(last) = &last {
        if hash_in(&last.id) == hash {
            return TakeOutcome::Skipped {
                why: SnapshotSkip::Same,
            };
        }
        if reason == SnapshotReason::Save && now - last.at_ms < interval_ms {
            return TakeOutcome::Skipped {
                why: SnapshotSkip::TooSoon,
            };
        }
    }

    let previous = match &last {
        Some(last) => read_text(backend, &last.id).await.unwrap_or_default(),
        None => String::new(),
    };
    let (added, removed) = changes(&previous, text);
    let entry = SnapshotEntry {
        id: format!("{now}-{hash}"),
        at_ms: now,
        reason,
        size: text.len() as u64,
        added,
        removed,
    };

    if backend.ensure_folder(&snapshots_dir()).await.is_err()
        || backend
            .write_bytes(&text_path(&entry.id), text.as_bytes())
            .await
            .is_err()
    {
        return TakeOutcome::Failed {
            why: SnapshotFailure::Io,
        };
    }
    match index.notes.iter_mut().find(|record| record.path == path) {
        Some(record) => {
            // Снимок существующей заметки по пути, где раньше была удалённая:
            // история продолжается, отметки удаления больше нет.
            record.deleted_at_ms = None;
            record.entries.push(entry.clone());
        }
        None => index.notes.push(NoteRecord {
            path: path.to_owned(),
            deleted_at_ms: None,
            entries: vec![entry.clone()],
        }),
    }
    if write_index(backend, &index).await.is_err() {
        let _ = backend.remove_service_file(&text_path(&entry.id)).await;
        return TakeOutcome::Failed {
            why: SnapshotFailure::Io,
        };
    }
    remove_texts(backend, &gone).await;
    TakeOutcome::Taken { entry }
}

// ── История, различия, возврат ──────────────────────────────────────────────

/// История заметки: снимки в пределах срока, новые первыми (FR-002).
pub async fn snapshot_history<B: VaultBackend>(backend: &B, path: &str, now: i64) -> NoteHistory {
    let (keep_ms, _) = limits(backend).await;
    let mut index = read_index(backend).await;
    prune(&mut index, now, keep_ms);
    let Some(record) = index.notes.into_iter().find(|record| record.path == path) else {
        return NoteHistory {
            path: path.to_owned(),
            ..NoteHistory::default()
        };
    };
    let mut entries = record.entries;
    entries.reverse();
    NoteHistory {
        path: record.path,
        entries,
        deleted_at_ms: record.deleted_at_ms,
    }
}

/// Текст снимка; `None` — снимка нет.
pub async fn snapshot_text<B: VaultBackend>(backend: &B, id: &str) -> Option<String> {
    read_text(backend, id).await
}

/// Построчные различия снимка `id` с текстом `against`: `Local` — строка
/// только в снимке, `External` — только в `against`. `None` — снимка нет.
pub async fn snapshot_diff<B: VaultBackend>(
    backend: &B,
    id: &str,
    against: &str,
) -> Option<Vec<DiffLine>> {
    let text = read_text(backend, id).await?;
    Some(line_diff(&text, against))
}

/// Возвращает снимок `id` в заметку `path` (FR-003): текущий текст сначала сам
/// становится снимком, затем открытая заметка получает правку для редактора, а
/// закрытая переписывается со сверкой отпечатка.
pub async fn snapshot_restore<B: VaultBackend>(
    backend: &mut B,
    path: &str,
    id: &str,
    editor_text: Option<&str>,
    now: i64,
) -> RestoreOutcome {
    if backend.access_state() != AccessState::ReadWrite {
        return RestoreOutcome::ReadOnly;
    }
    let Some(text) = read_text(backend, id).await else {
        return RestoreOutcome::Missing;
    };
    let note = Path::new(path);

    if let Some(current) = editor_text {
        snapshot_take(backend, path, current, SnapshotReason::BeforeRestore, now).await;
        return RestoreOutcome::EditorEdit {
            edit: TextEdit {
                from: 0,
                to: utf16_at(current, current.len()),
                insert: text,
            },
        };
    }

    let current = match backend.read(note).await {
        Ok(current) => current,
        Err(ReadError::NotFound) => return RestoreOutcome::Missing,
        Err(_) => return RestoreOutcome::Failed,
    };
    snapshot_take(backend, path, &current, SnapshotReason::BeforeRestore, now).await;
    let folder = note.parent().unwrap_or_else(|| Path::new(""));
    let stamp = backend
        .list_folder(folder)
        .await
        .into_iter()
        .find(|entry| slashed(&entry.path) == path)
        .map(|entry| entry.stamp);
    match backend.write(note, &text, stamp).await {
        Ok(()) => RestoreOutcome::Written,
        Err(WriteError::AccessDenied) => RestoreOutcome::ReadOnly,
        Err(_) => RestoreOutcome::Failed,
    }
}

// ── Переименование и удаление ───────────────────────────────────────────────

/// Новый путь записи при переезде `from` → `to` (заметка или папка).
fn moved(path: &str, from: &str, to: &str) -> Option<String> {
    if path == from {
        return Some(to.to_owned());
    }
    path.strip_prefix(&format!("{from}/"))
        .map(|tail| format!("{to}/{tail}"))
}

/// История едет за заметкой или папкой (Edge Cases): как отметки 006 и 028.
pub async fn move_snapshots<B: VaultBackend>(
    backend: &mut B,
    from: &Path,
    to: &Path,
) -> Result<(), WriteError> {
    let from = slashed(from);
    let to = slashed(to);
    let mut index = read_index(backend).await;
    let mut changed = false;
    let mut arrived: Vec<NoteRecord> = Vec::new();
    index
        .notes
        .retain(|record| match moved(&record.path, &from, &to) {
            Some(path) => {
                arrived.push(NoteRecord {
                    path,
                    ..record.clone()
                });
                changed = true;
                false
            }
            None => true,
        });
    if !changed {
        return Ok(());
    }
    for record in arrived {
        // На новом месте могла остаться история удалённой заметки — сливаются.
        match index
            .notes
            .iter_mut()
            .find(|other| other.path == record.path)
        {
            Some(other) => {
                other.entries.extend(record.entries);
                other.entries.sort_by_key(|entry| entry.at_ms);
                other.deleted_at_ms = record.deleted_at_ms;
            }
            None => index.notes.push(record),
        }
    }
    write_index(backend, &index).await
}

/// Снимок перед удалением и отметка «удалена» (FR-001, FR-004). Зовётся
/// backend до удаления файла; для папки — по каждой заметке внутри.
pub async fn before_delete<B: VaultBackend>(backend: &mut B, path: &Path, now: i64) {
    let target = slashed(path);
    let notes: Vec<String> = if classify(path) == FileKind::Note {
        vec![target]
    } else {
        let prefix = format!("{target}/");
        backend
            .list()
            .await
            .into_iter()
            .map(|note| slashed(&note.path))
            .filter(|note| note.starts_with(&prefix) && classify(Path::new(note)) == FileKind::Note)
            .collect()
    };
    if notes.is_empty() {
        return;
    }
    for note in &notes {
        if let Ok(text) = backend.read(Path::new(note)).await {
            snapshot_take(backend, note, &text, SnapshotReason::Delete, now).await;
        }
    }
    let mut index = read_index(backend).await;
    let mut marked = false;
    for record in &mut index.notes {
        if notes.contains(&record.path) {
            record.deleted_at_ms = Some(now);
            marked = true;
        }
    }
    if marked {
        let _ = write_index(backend, &index).await;
    }
}

/// Удалённые заметки в пределах срока, недавние первыми (US2).
pub async fn deleted_notes<B: VaultBackend>(backend: &B, now: i64) -> Vec<DeletedNote> {
    let (keep_ms, _) = limits(backend).await;
    let mut index = read_index(backend).await;
    prune(&mut index, now, keep_ms);
    let mut out: Vec<DeletedNote> = index
        .notes
        .into_iter()
        .filter_map(|record| {
            let deleted_at_ms = record.deleted_at_ms?;
            Some(DeletedNote {
                title: crate::links::title_of(&record.path),
                folder: record
                    .path
                    .rsplit_once('/')
                    .map_or(String::new(), |(folder, _)| folder.to_owned()),
                deleted_at_ms,
                snapshots: record.entries.len() as u32,
                path: record.path,
            })
        })
        .collect();
    out.sort_by(|a, b| b.deleted_at_ms.cmp(&a.deleted_at_ms));
    out
}

/// Свободное имя рядом: `Имя (2).typ`, `Имя (3).typ`…
async fn free_name<B: VaultBackend>(backend: &B, path: &str) -> String {
    let (stem, extension) = path.rsplit_once('.').unwrap_or((path, ""));
    let mut number = 2;
    loop {
        let candidate = if extension.is_empty() {
            format!("{stem} ({number})")
        } else {
            format!("{stem} ({number}).{extension}")
        };
        if backend.file_mark(Path::new(&candidate)).await.is_none() {
            return candidate;
        }
        number += 1;
    }
}

/// Восстанавливает удалённую заметку из последнего снимка (US2): по прежнему
/// пути или по `as_path`; занятый путь — `Occupied` с предложением имени.
pub async fn recover_note<B: VaultBackend>(
    backend: &mut B,
    path: &str,
    as_path: Option<&str>,
) -> RecoverOutcome {
    if backend.access_state() != AccessState::ReadWrite {
        return RecoverOutcome::ReadOnly;
    }
    let index = read_index(backend).await;
    let Some(last) = index
        .notes
        .iter()
        .find(|record| record.path == path && record.deleted_at_ms.is_some())
        .and_then(|record| record.entries.last())
    else {
        return RecoverOutcome::Missing;
    };
    let Some(text) = read_text(backend, &last.id).await else {
        return RecoverOutcome::Missing;
    };
    let target = as_path.unwrap_or(path).to_owned();
    if classify(Path::new(&target)) != FileKind::Note {
        return RecoverOutcome::Failed;
    }
    if backend.file_mark(Path::new(&target)).await.is_some() {
        return RecoverOutcome::Occupied {
            suggested: free_name(backend, &target).await,
        };
    }
    let note = Path::new(&target);
    if let Some(folder) = note.parent().filter(|folder| *folder != Path::new("")) {
        if backend.ensure_folder(folder).await.is_err() {
            return RecoverOutcome::Failed;
        }
    }
    if backend.create(note).await.is_err() || backend.write(note, &text, None).await.is_err() {
        return RecoverOutcome::Failed;
    }
    if target != path {
        let _ = move_snapshots(backend, Path::new(path), note).await;
    }
    let mut index = read_index(backend).await;
    if let Some(record) = index.notes.iter_mut().find(|record| record.path == target) {
        record.deleted_at_ms = None;
    }
    let _ = write_index(backend, &index).await;
    RecoverOutcome::Recovered { path: target }
}

// ── Объём и очистка ─────────────────────────────────────────────────────────

/// Сколько снимков и байт хранится (US3).
pub async fn snapshot_usage<B: VaultBackend>(backend: &B) -> SnapshotUsage {
    let index = read_index(backend).await;
    index
        .notes
        .iter()
        .flat_map(|record| record.entries.iter())
        .fold(SnapshotUsage::default(), |usage, entry| SnapshotUsage {
            snapshots: usage.snapshots + 1,
            bytes: usage.bytes + entry.size,
        })
}

/// Удаляет все снимки vault (US3, «Очистить»); возвращает, сколько удалено.
pub async fn snapshots_clear<B: VaultBackend>(backend: &mut B) -> Result<u32, WriteError> {
    if backend.access_state() != AccessState::ReadWrite {
        return Err(WriteError::AccessDenied);
    }
    let index = read_index(backend).await;
    let ids: Vec<String> = index
        .notes
        .iter()
        .flat_map(|record| record.entries.iter().map(|entry| entry.id.clone()))
        .collect();
    write_index(backend, &SnapshotIndex::default()).await?;
    remove_texts(backend, &ids).await;
    Ok(ids.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_checked_before_reading() {
        assert!(valid_id("1700000000000-0123456789ab"));
        assert!(!valid_id("../vault.json"));
        assert!(!valid_id(""));
    }

    #[test]
    fn moved_follows_notes_and_folders() {
        assert_eq!(
            moved("a/b.typ", "a/b.typ", "c.typ").as_deref(),
            Some("c.typ")
        );
        assert_eq!(moved("a/b.typ", "a", "x/a").as_deref(), Some("x/a/b.typ"));
        assert_eq!(moved("ab/b.typ", "a", "x"), None);
    }

    #[test]
    fn changes_count_lines() {
        assert_eq!(changes("а\nб\nв", "а\nв\nг\nд"), (2, 1));
        assert_eq!(changes("", "а\nб"), (2, 0));
    }
}
