//! Задачи из заметок (фича 033).
//!
//! Задача — пункт списка с отметкой: `- [ ] текст`, `- [x] текст` (и `+`).
//! Срок — дата `ГГГГ-ММ-ДД` после `📅`, `срок:` или `due:` (research.md §1).
//! Пункт в коде, сыром тексте или комментарии задачей не считается: строку
//! засчитывает тот же разбор Typst, которым собирается заметка.
//!
//! Индекс задач — производный файл устройства `.typsidian/tasks.json` с
//! отпечатком каждой заметки: показ перечитывает только изменённые заметки
//! (research.md §2, SC-001). Отметка меняет один знак одной строки, найденной
//! по тексту (research.md §4, SC-002).

#[cfg(target_arch = "wasm32")]
pub mod web;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use typst::syntax::{LinkedNode, Side, SyntaxKind};

use crate::elements::edit::TextEdit;
use crate::properties::query::matches_all;
use crate::properties::Condition;
use crate::stamp::FileStamp;
use crate::text_offsets::{byte_at, utf16_at};
use crate::vault::backend::{VaultBackend, WriteError};
use crate::vault::service_dir::{service_dir, tasks_index_path};
use crate::vault::{classify, AccessState, FileKind};

const INDEX_VERSION: u32 = 1;

// ── Задача ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct VaultTask {
    pub path: String,
    /// Номер строки, с 1.
    pub line: u32,
    /// Отступ в знаках — для вложенности.
    pub indent: u32,
    pub done: bool,
    /// Текст без маркера, отметки и срока.
    pub text: String,
    /// Строка целиком — по ней отметка находит задачу.
    pub raw: String,
    /// Срок `ГГГГ-ММ-ДД`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub due: Option<String>,
    /// Строка родительской задачи.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub parent: Option<u32>,
}

/// Разбор строки задачи: отступ, позиция знака отметки, выполнена, остаток.
fn task_line(line: &str) -> Option<(usize, usize, bool, &str)> {
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    let rest = &line[indent..];
    let after_marker = rest
        .strip_prefix("- ")
        .or_else(|| rest.strip_prefix("+ "))?;
    let bytes = after_marker.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'[' || bytes[2] != b']' {
        return None;
    }
    let done = match bytes[1] {
        b' ' => false,
        b'x' | b'X' => true,
        _ => return None,
    };
    let text = after_marker[3..]
        .strip_prefix(' ')
        .unwrap_or(&after_marker[3..]);
    // Знак отметки: отступ, маркер, пробел, `[` — следом сам знак.
    Some((indent, indent + 3, done, text))
}

fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(at, byte)| at == 4 || at == 7 || byte.is_ascii_digit())
}

/// Срок и текст без него.
fn split_due(text: &str) -> (String, Option<String>) {
    for marker in ["📅", "срок:", "due:"] {
        let mut from = 0;
        while let Some(found) = text[from..].find(marker) {
            let at = from + found;
            let after = &text[at + marker.len()..];
            let spaces = after.len() - after.trim_start().len();
            let date = after[spaces..].get(..10).filter(|date| is_date(date));
            if let Some(date) = date {
                let end = at + marker.len() + spaces + 10;
                let rest = format!("{}{}", &text[..at], &text[end..]);
                let rest = rest.split_whitespace().collect::<Vec<_>>().join(" ");
                return (rest, Some(date.to_owned()));
            }
            from = at + marker.len();
        }
    }
    (text.trim().to_owned(), None)
}

/// Задачи заметки `path` с текстом `text` (FR-001).
pub fn tasks_in(path: &str, text: &str) -> Vec<VaultTask> {
    if !text.contains("[ ]") && !text.contains("[x]") && !text.contains("[X]") {
        return Vec::new();
    }
    let root = typst::syntax::parse(text);
    let linked = LinkedNode::new(&root);
    let mut out: Vec<VaultTask> = Vec::new();
    let mut offset = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let start = offset;
        offset += line.len();
        let line = line.trim_end_matches(['\n', '\r']);
        let Some((indent, _, done, rest)) = task_line(line) else {
            continue;
        };
        // Маркер списка в разметке — не код, не сырой текст, не комментарий.
        let marker = start + indent;
        let in_list = linked
            .leaf_at(marker + 1, Side::Before)
            .is_some_and(|leaf| {
                matches!(leaf.kind(), SyntaxKind::ListMarker | SyntaxKind::EnumMarker)
            });
        if !in_list {
            continue;
        }
        let (body, due) = split_due(rest);
        let number = index as u32 + 1;
        let parent = out
            .iter()
            .rev()
            .find(|task| (task.indent as usize) < indent)
            .filter(|_| indent > 0)
            .map(|task| task.line);
        out.push(VaultTask {
            path: path.to_owned(),
            line: number,
            indent: indent as u32,
            done,
            text: body,
            raw: line.to_owned(),
            due,
            parent,
        });
    }
    out
}

// ── Индекс ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct IndexedTasks {
    stamp: Option<FileStamp>,
    tasks: Vec<VaultTask>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TaskIndex {
    version: u32,
    notes: BTreeMap<String, IndexedTasks>,
}

async fn read_index<B: VaultBackend>(backend: &B) -> TaskIndex {
    match backend.read(&tasks_index_path()).await {
        Ok(text) => serde_json::from_str::<TaskIndex>(&text)
            .ok()
            .filter(|index| index.version == INDEX_VERSION)
            .unwrap_or_default(),
        Err(_) => TaskIndex::default(),
    }
}

/// Все задачи vault из индекса; изменённые заметки перечитываются, индекс
/// пишется, если что-то поменялось (на только чтение — не пишется).
pub async fn all_tasks<B: VaultBackend>(backend: &mut B) -> Vec<VaultTask> {
    let mut index = read_index(backend).await;
    let files = backend.list().await;
    let mut changed = index.version != INDEX_VERSION;
    let mut seen = std::collections::HashSet::new();
    for note in files.iter().filter(|note| note.kind == FileKind::Note) {
        let path = note.path.to_string_lossy().replace('\\', "/");
        if classify(Path::new(&path)) != FileKind::Note {
            continue;
        }
        seen.insert(path.clone());
        let fresh = index
            .notes
            .get(&path)
            .is_some_and(|entry| entry.stamp.as_ref() == Some(&note.stamp));
        if fresh {
            continue;
        }
        let tasks = match backend.read(&note.path).await {
            Ok(text) => tasks_in(&path, &text),
            Err(_) => Vec::new(),
        };
        index.notes.insert(
            path,
            IndexedTasks {
                stamp: Some(note.stamp.clone()),
                tasks,
            },
        );
        changed = true;
    }
    let before = index.notes.len();
    index.notes.retain(|path, _| seen.contains(path));
    changed |= index.notes.len() != before;

    if changed && backend.access_state() == AccessState::ReadWrite {
        index.version = INDEX_VERSION;
        if let Ok(text) = serde_json::to_string(&index) {
            if backend.ensure_folder(&service_dir()).await.is_ok() {
                let _ = backend
                    .write_bytes(&tasks_index_path(), text.as_bytes())
                    .await;
            }
        }
    }
    index
        .notes
        .into_values()
        .flat_map(|entry| entry.tasks)
        .collect()
}

// ── Фильтры и вид ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum TaskState {
    #[default]
    Open,
    Done,
    All,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum DueFilter {
    #[default]
    Any,
    Overdue,
    Today,
    Week,
    NoDue,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum TaskGrouping {
    #[default]
    ByNote,
    ByDue,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TaskFilter {
    pub state: TaskState,
    pub due: DueFilter,
    /// Папка: задачи заметок под ней.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub folder: Option<String>,
    /// Условия по свойствам заметки (020), все сразу.
    #[serde(default)]
    pub conditions: Vec<Condition>,
    pub group: TaskGrouping,
}

/// Срок относительно сегодня.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum DueState {
    NoDue,
    Overdue,
    Today,
    Week,
    Later,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TaskRow {
    pub task: VaultTask,
    pub note_title: String,
    pub due_state: DueState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TaskGroup {
    /// Путь заметки или вид срока (`Overdue`, `Today`, `Week`, `Later`, `NoDue`).
    pub key: String,
    /// Имя заметки; у группы по сроку пусто — подпись даёт UI по ключу.
    pub title: String,
    pub folder: String,
    pub rows: Vec<TaskRow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TasksView {
    pub groups: Vec<TaskGroup>,
    /// Невыполненных во всём vault.
    pub open: u32,
    /// Просроченных невыполненных.
    pub overdue: u32,
    /// Показано после фильтров.
    pub shown: u32,
}

/// Номер дня от 1970-01-01 для `ГГГГ-ММ-ДД` (алгоритм Хиннанта).
fn day_number(date: &str) -> Option<i64> {
    if !is_date(date) {
        return None;
    }
    let year: i64 = date[..4].parse().ok()?;
    let month: i64 = date[5..7].parse().ok()?;
    let day: i64 = date[8..].parse().ok()?;
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn due_state(due: Option<&str>, today: i64) -> DueState {
    let Some(day) = due.and_then(day_number) else {
        return DueState::NoDue;
    };
    match day - today {
        delta if delta < 0 => DueState::Overdue,
        0 => DueState::Today,
        1..=6 => DueState::Week,
        _ => DueState::Later,
    }
}

fn folder_of(path: &str) -> String {
    path.rsplit_once('/')
        .map_or(String::new(), |(folder, _)| folder.to_owned())
}

/// Окно «Задачи» (FR-001, FR-003): `today` — `ГГГГ-ММ-ДД` по часам устройства.
pub async fn tasks_view<B: VaultBackend>(
    backend: &mut B,
    filter: &TaskFilter,
    today: &str,
) -> TasksView {
    let tasks = all_tasks(backend).await;
    let today = day_number(today).unwrap_or(0);
    let notes = if filter.conditions.is_empty() {
        Vec::new()
    } else {
        backend.indexed_notes().await
    };
    let now = crate::properties::now_ms();

    let open = tasks.iter().filter(|task| !task.done).count() as u32;
    let overdue = tasks
        .iter()
        .filter(|task| !task.done && due_state(task.due.as_deref(), today) == DueState::Overdue)
        .count() as u32;

    let allowed_note = |path: &str| {
        if let Some(folder) = &filter.folder {
            let folder = folder.trim_matches('/');
            if !folder.is_empty() && !path.starts_with(&format!("{folder}/")) {
                return false;
            }
        }
        if filter.conditions.is_empty() {
            return true;
        }
        notes
            .iter()
            .find(|note| note.path == path)
            .is_some_and(|note| matches_all(note, &filter.conditions, now))
    };

    let mut rows: Vec<TaskRow> = tasks
        .into_iter()
        .filter(|task| match filter.state {
            TaskState::Open => !task.done,
            TaskState::Done => task.done,
            TaskState::All => true,
        })
        .map(|task| {
            let due_state = due_state(task.due.as_deref(), today);
            TaskRow {
                note_title: crate::links::title_of(&task.path),
                due_state,
                task,
            }
        })
        .filter(|row| match filter.due {
            DueFilter::Any => true,
            DueFilter::Overdue => row.due_state == DueState::Overdue,
            DueFilter::Today => row.due_state == DueState::Today,
            DueFilter::Week => matches!(row.due_state, DueState::Today | DueState::Week),
            DueFilter::NoDue => row.due_state == DueState::NoDue,
        })
        .filter(|row| allowed_note(&row.task.path))
        .collect();
    rows.sort_by(|a, b| {
        a.task
            .path
            .cmp(&b.task.path)
            .then(a.task.line.cmp(&b.task.line))
    });
    let shown = rows.len() as u32;

    let mut groups: Vec<TaskGroup> = Vec::new();
    match filter.group {
        TaskGrouping::ByNote => {
            for row in rows {
                match groups.last_mut() {
                    Some(group) if group.key == row.task.path => group.rows.push(row),
                    _ => groups.push(TaskGroup {
                        key: row.task.path.clone(),
                        title: row.note_title.clone(),
                        folder: folder_of(&row.task.path),
                        rows: vec![row],
                    }),
                }
            }
        }
        TaskGrouping::ByDue => {
            let order = [
                DueState::Overdue,
                DueState::Today,
                DueState::Week,
                DueState::Later,
                DueState::NoDue,
            ];
            for state in order {
                let mut inside: Vec<TaskRow> = rows
                    .iter()
                    .filter(|row| row.due_state == state)
                    .cloned()
                    .collect();
                if inside.is_empty() {
                    continue;
                }
                inside.sort_by(|a, b| a.task.due.cmp(&b.task.due));
                groups.push(TaskGroup {
                    key: format!("{state:?}"),
                    title: String::new(),
                    folder: String::new(),
                    rows: inside,
                });
            }
        }
    }
    TasksView {
        groups,
        open,
        overdue,
        shown,
    }
}

// ── Отметка ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TaskToggleRequest {
    pub path: String,
    pub line: u32,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum TaskToggleOutcome {
    /// Заметка открыта — одна правка редактора.
    EditorEdit {
        edit: TextEdit,
    },
    Written,
    /// Строки с этим текстом в заметке нет — ничего не изменено.
    Stale,
    ReadOnly,
    Failed,
}

/// Байтовое начало строки `number` (с 1) и сама строка.
fn line_at(text: &str, number: usize) -> Option<(usize, &str)> {
    let mut offset = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        if index + 1 == number {
            return Some((offset, line.trim_end_matches(['\n', '\r'])));
        }
        offset += line.len();
    }
    None
}

/// Строка задачи по тексту: на прежнем номере, иначе ближайшая с тем же
/// текстом. Байтовое начало строки.
fn find_line(text: &str, request: &TaskToggleRequest) -> Option<usize> {
    if let Some((start, line)) = line_at(text, request.line as usize) {
        if line == request.raw {
            return Some(start);
        }
    }
    let mut best: Option<(usize, usize)> = None;
    let mut offset = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        if line.trim_end_matches(['\n', '\r']) == request.raw {
            let distance = (index + 1).abs_diff(request.line as usize);
            if best.is_none_or(|(_, known)| distance < known) {
                best = Some((offset, distance));
            }
        }
        offset += line.len();
    }
    best.map(|(start, _)| start)
}

/// Правка знака отметки строки, начинающейся с байта `start`.
fn toggle_edit(text: &str, start: usize) -> Option<TextEdit> {
    let end = text[start..].find('\n').map_or(text.len(), |at| start + at);
    let line = text[start..end].trim_end_matches('\r');
    let (_, mark, done, _) = task_line(line)?;
    let at = start + mark;
    Some(TextEdit {
        from: utf16_at(text, at),
        to: utf16_at(text, at + 1),
        insert: if done { " " } else { "x" }.to_owned(),
    })
}

fn apply(text: &str, edit: &TextEdit) -> String {
    let from = byte_at(text, edit.from);
    let to = byte_at(text, edit.to);
    format!("{}{}{}", &text[..from], edit.insert, &text[to..])
}

/// Отметить задачу в списке (FR-002, SC-002). `editor_text` — текст открытой
/// заметки: тогда ответ — правка редактора.
pub async fn task_toggle<B: VaultBackend>(
    backend: &mut B,
    request: &TaskToggleRequest,
    editor_text: Option<&str>,
) -> TaskToggleOutcome {
    if backend.access_state() != AccessState::ReadWrite {
        return TaskToggleOutcome::ReadOnly;
    }
    if let Some(text) = editor_text {
        return match find_line(text, request).and_then(|start| toggle_edit(text, start)) {
            Some(edit) => TaskToggleOutcome::EditorEdit { edit },
            None => TaskToggleOutcome::Stale,
        };
    }
    let path = Path::new(&request.path);
    let folder = path.parent().unwrap_or_else(|| Path::new(""));
    let Some(stamp) = backend
        .list_folder(folder)
        .await
        .into_iter()
        .find(|note| note.path.to_string_lossy().replace('\\', "/") == request.path)
        .map(|note| note.stamp)
    else {
        return TaskToggleOutcome::Stale;
    };
    let Ok(text) = backend.read(path).await else {
        return TaskToggleOutcome::Stale;
    };
    let Some(edit) = find_line(&text, request).and_then(|start| toggle_edit(&text, start)) else {
        return TaskToggleOutcome::Stale;
    };
    match backend.write(path, &apply(&text, &edit), Some(stamp)).await {
        Ok(()) => TaskToggleOutcome::Written,
        Err(WriteError::Conflict) => TaskToggleOutcome::Stale,
        Err(WriteError::AccessDenied) => TaskToggleOutcome::ReadOnly,
        Err(_) => TaskToggleOutcome::Failed,
    }
}

/// «Переключить задачу» для строки под кареткой `offset` (UTF-16, US3):
/// задача — переключить отметку; пункт списка — сделать задачей.
pub fn task_toggle_at(text: &str, offset: u32) -> Option<TextEdit> {
    let caret = byte_at(text, offset as usize);
    let start = text[..caret].rfind('\n').map_or(0, |at| at + 1);
    if let Some(edit) = toggle_edit(text, start) {
        return Some(edit);
    }
    let end = text[start..].find('\n').map_or(text.len(), |at| start + at);
    let line = &text[start..end];
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    let rest = &line[indent..];
    let marker = if rest.starts_with("- ") || rest.starts_with("+ ") {
        2
    } else if rest == "-" || rest == "+" {
        1
    } else {
        return None;
    };
    let at = start + indent + marker;
    let insert = if marker == 1 { " [ ]" } else { "[ ] " };
    Some(TextEdit {
        from: utf16_at(text, at),
        to: utf16_at(text, at),
        insert: insert.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_count_days() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(
            day_number("2026-10-24").unwrap() - day_number("2026-10-23").unwrap(),
            1
        );
        assert_eq!(
            day_number("2024-03-01").unwrap() - day_number("2024-02-28").unwrap(),
            2
        );
        assert_eq!(day_number("не дата"), None);
    }

    #[test]
    fn due_is_split_from_the_text() {
        assert_eq!(
            split_due("Сдать главу 📅 2026-10-30 руководителю"),
            ("Сдать главу руководителю".into(), Some("2026-10-30".into()))
        );
        assert_eq!(
            split_due("Отчёт срок: 2026-11-01"),
            ("Отчёт".into(), Some("2026-11-01".into()))
        );
        assert_eq!(split_due("Без срока"), ("Без срока".into(), None));
    }
}
