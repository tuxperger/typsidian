//! Отбор и сортировка заметок по свойствам (research.md §6, FR-006).
//!
//! Чистая функция над сведениями индекса: ни диска, ни часов — «сейчас» ей
//! передают. Поэтому она одинакова на native и web и питает сразу три места:
//! фишки условий поиска, команду `vaultSelect` и входы блоков `#select`.
//!
//! Правила, общие для всех условий:
//!
//! - все условия соединяются через «и» (US2, сценарий 2);
//! - сравнение идёт только внутри одного вида значения; несравнимое в условии
//!   не проходит (Edge Cases);
//! - заметка без свойств остаётся в игре для встроенных сведений: название,
//!   папка, даты;
//! - «создана» в индексе нет ни на одном таргете (`created_ms` всегда `None`),
//!   поэтому условие по ней не проходит, а ячейка пуста (research.md §6).

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use super::index::IndexedNote;
use super::{date_of_ms, days_from_civil, PropertyValue, ValueKind, TAGS_KEY};

/// Календарная дата в условиях.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Ymd {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum DateRange {
    /// За последние N суток.
    LastDays { days: u32 },
    /// Включительно, по календарным дням UTC.
    Between { from: Ymd, to: Ymd },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

/// Условие отбора (data-model.md «Selection»).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum Condition {
    /// Тег есть у заметки; регистр не важен, ведущее `#` не считается.
    TagHas {
        tag: String,
    },
    Compare {
        key: String,
        op: CompareOp,
        value: PropertyValue,
    },
    /// Свойство задано (любым значением).
    Exists {
        key: String,
    },
    /// Заметка лежит в папке или глубже; пустая — любая.
    Folder {
        folder: String,
    },
    Modified {
        range: DateRange,
    },
    Created {
        range: DateRange,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SortKey {
    pub key: String,
    pub descending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SelectView {
    #[default]
    Table,
    List,
}

/// Выборка: условия, порядок, столбцы, предел (data-model.md «Selection»).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Selection {
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub sort: Option<SortKey>,
    /// `название`, `папка`, `изменена`, `создана` или ключ свойства. Пустой
    /// список — одно `название`.
    #[serde(default)]
    pub columns: Vec<String>,
    /// Не больше [`MAX_ROWS`]; `0` — [`DEFAULT_LIMIT`].
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub view: SelectView,
}

/// Предел строк выборки (Scale/Scope плана).
pub const MAX_ROWS: u32 = 500;
/// Предел по умолчанию.
pub const DEFAULT_LIMIT: u32 = 50;
/// Столбец по умолчанию.
pub const DEFAULT_COLUMN: &str = "название";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SelectionRow {
    pub path: String,
    /// По ячейке на столбец; `None` — значения нет.
    pub cells: Vec<Option<PropertyValue>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SelectionResult {
    pub rows: Vec<SelectionRow>,
    pub columns: Vec<String>,
    /// Подходящих заметок больше предела; показаны первые.
    pub truncated: bool,
    /// Сколько заметок подошло всего.
    pub total: u32,
}

impl Selection {
    /// Предел строк с учётом умолчания и потолка.
    pub fn effective_limit(&self) -> usize {
        let limit = match self.limit {
            0 => DEFAULT_LIMIT,
            n => n.min(MAX_ROWS),
        };
        limit as usize
    }

    /// Столбцы с учётом умолчания.
    pub fn effective_columns(&self) -> Vec<String> {
        if self.columns.is_empty() {
            vec![DEFAULT_COLUMN.to_string()]
        } else {
            self.columns.clone()
        }
    }
}

// ── Встроенные сведения ────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Builtin {
    Name,
    Folder,
    Modified,
    Created,
}

fn builtin(key: &str) -> Option<Builtin> {
    match key {
        "название" | "name" | "title" => Some(Builtin::Name),
        "папка" | "folder" => Some(Builtin::Folder),
        "изменена" | "modified" => Some(Builtin::Modified),
        "создана" | "created" => Some(Builtin::Created),
        _ => None,
    }
}

/// Название заметки — имя файла без расширения.
fn title_of(note: &IndexedNote) -> String {
    match note.name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => note.name.clone(),
    }
}

/// Папка заметки относительно корня, через `/`; корень — пустая строка.
fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

fn date_value(ms: Option<i64>) -> Option<PropertyValue> {
    ms.map(|ms| {
        let (year, month, day) = date_of_ms(ms);
        PropertyValue::Date { year, month, day }
    })
}

/// Значение столбца или ключа: встроенное сведение либо свойство.
pub(crate) fn value_of(note: &IndexedNote, key: &str) -> Option<PropertyValue> {
    match builtin(key) {
        Some(Builtin::Name) => Some(PropertyValue::Text {
            value: title_of(note),
        }),
        Some(Builtin::Folder) => Some(PropertyValue::Text {
            value: folder_of(&note.path).to_string(),
        }),
        Some(Builtin::Modified) => date_value(note.modified_ms),
        Some(Builtin::Created) => date_value(note.created_ms),
        None => note
            .properties
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.value.clone()),
    }
}

// ── Условия ────────────────────────────────────────────────────────────────

fn has_tag(note: &IndexedNote, tag: &str) -> bool {
    let wanted = tag.trim().trim_start_matches('#').to_lowercase();
    note.properties
        .iter()
        .filter(|entry| entry.key == TAGS_KEY)
        .any(|entry| match &entry.value {
            PropertyValue::List { values } => values
                .iter()
                .any(|value| value.trim().trim_start_matches('#').to_lowercase() == wanted),
            PropertyValue::Text { value } => {
                value.trim().trim_start_matches('#').to_lowercase() == wanted
            }
            _ => false,
        })
}

fn in_folder(note: &IndexedNote, folder: &str) -> bool {
    let wanted = folder.trim().trim_matches('/');
    if wanted.is_empty() {
        return true;
    }
    let actual = folder_of(&note.path);
    actual == wanted
        || actual
            .strip_prefix(wanted)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn ymd_days(date: &Ymd) -> i64 {
    days_from_civil(date.year, date.month, date.day)
}

fn in_range(ms: Option<i64>, range: &DateRange, now: i64) -> bool {
    let Some(ms) = ms else { return false };
    match range {
        DateRange::LastDays { days } => {
            ms <= now && ms >= now.saturating_sub(i64::from(*days) * 86_400_000)
        }
        DateRange::Between { from, to } => {
            let day = ms.div_euclid(86_400_000);
            day >= ymd_days(from) && day <= ymd_days(to)
        }
    }
}

/// Порядок двух значений одного вида; `None` — виды разные или порядка нет.
pub(crate) fn compare_same_kind(a: &PropertyValue, b: &PropertyValue) -> Option<Ordering> {
    use PropertyValue::*;
    match (a, b) {
        (Text { value: x }, Text { value: y }) => Some(x.cmp(y)),
        (Number { value: x }, Number { value: y }) => x.partial_cmp(y),
        (
            Date {
                year: ya,
                month: ma,
                day: da,
            },
            Date {
                year: yb,
                month: mb,
                day: db,
            },
        ) => Some((ya, ma, da).cmp(&(yb, mb, db))),
        (Flag { value: x }, Flag { value: y }) => Some(x.cmp(y)),
        (List { values: x }, List { values: y }) => Some(x.cmp(y)),
        _ => None,
    }
}

fn compare(actual: &PropertyValue, op: CompareOp, wanted: &PropertyValue) -> bool {
    // Список и текст: «Eq» — «содержит», «Ne» — «не содержит».
    if let (PropertyValue::List { values }, PropertyValue::Text { value }) = (actual, wanted) {
        return match op {
            CompareOp::Eq => values.contains(value),
            CompareOp::Ne => !values.contains(value),
            _ => false,
        };
    }
    let Some(order) = compare_same_kind(actual, wanted) else {
        return false;
    };
    match op {
        CompareOp::Eq => order == Ordering::Equal,
        CompareOp::Ne => order != Ordering::Equal,
        CompareOp::Lt => order == Ordering::Less,
        CompareOp::Gt => order == Ordering::Greater,
        CompareOp::Le => order != Ordering::Greater,
        CompareOp::Ge => order != Ordering::Less,
    }
}

/// Проходит ли заметка условие.
pub fn matches(note: &IndexedNote, condition: &Condition, now: i64) -> bool {
    match condition {
        Condition::TagHas { tag } => has_tag(note, tag),
        Condition::Compare { key, op, value } => {
            // Несколько записей одного ключа в индексе не бывает (повтор делает
            // запись неоднозначной), но проверяем все: порядок не важен.
            if builtin(key).is_some() {
                value_of(note, key).is_some_and(|actual| compare(&actual, *op, value))
            } else {
                note.properties
                    .iter()
                    .filter(|entry| &entry.key == key)
                    .any(|entry| compare(&entry.value, *op, value))
            }
        }
        Condition::Exists { key } => value_of(note, key).is_some(),
        Condition::Folder { folder } => in_folder(note, folder),
        Condition::Modified { range } => in_range(note.modified_ms, range, now),
        Condition::Created { range } => in_range(note.created_ms, range, now),
    }
}

/// Все ли условия выполнены.
pub fn matches_all(note: &IndexedNote, conditions: &[Condition], now: i64) -> bool {
    conditions
        .iter()
        .all(|condition| matches(note, condition, now))
}

// ── Порядок ────────────────────────────────────────────────────────────────

fn kind_rank(kind: ValueKind) -> u8 {
    match kind {
        ValueKind::Text => 0,
        ValueKind::Number => 1,
        ValueKind::Date => 2,
        ValueKind::Flag => 3,
        ValueKind::List => 4,
        ValueKind::Unknown => 5,
    }
}

/// Сортирует заметки: значения основного вида по порядку, затем значения
/// других видов, затем заметки без значения. Порядок полный, поэтому сортировка
/// не может сломаться на смеси видов.
fn sort_notes(notes: &mut [&IndexedNote], sort: &SortKey) {
    let values: Vec<Option<PropertyValue>> =
        notes.iter().map(|note| value_of(note, &sort.key)).collect();
    let mut counts = [0usize; 6];
    for value in values.iter().flatten() {
        counts[kind_rank(value.kind()) as usize] += 1;
    }
    let primary = counts
        .iter()
        .enumerate()
        .rev()
        .max_by_key(|(_, count)| **count)
        .map_or(0, |(rank, _)| rank as u8);

    let mut keyed: Vec<(Option<PropertyValue>, &IndexedNote)> =
        values.into_iter().zip(notes.iter().copied()).collect();
    keyed.sort_by(|(a, na), (b, nb)| {
        let class = |value: &Option<PropertyValue>| match value {
            None => 2u8,
            Some(v) if kind_rank(v.kind()) == primary => 0,
            Some(_) => 1,
        };
        let by_value = match (a, b) {
            (Some(x), Some(y)) if class(a) == class(b) => {
                let within = if kind_rank(x.kind()) == kind_rank(y.kind()) {
                    compare_same_kind(x, y).unwrap_or(Ordering::Equal)
                } else {
                    kind_rank(x.kind()).cmp(&kind_rank(y.kind()))
                };
                if sort.descending {
                    within.reverse()
                } else {
                    within
                }
            }
            _ => class(a).cmp(&class(b)),
        };
        by_value.then_with(|| na.path.cmp(&nb.path))
    });
    for (slot, (_, note)) in notes.iter_mut().zip(keyed) {
        *slot = note;
    }
}

/// Выполняет выборку над сведениями индекса (contracts/core-properties-api.md).
///
/// `now_ms` — «сейчас» для условий «за последние N суток»; часов здесь нет
/// намеренно (принцип II: одинаковый результат на любом таргете).
pub fn run_selection(notes: &[IndexedNote], selection: &Selection, now_ms: i64) -> SelectionResult {
    let mut hits: Vec<&IndexedNote> = notes
        .iter()
        .filter(|note| matches_all(note, &selection.conditions, now_ms))
        .collect();
    hits.sort_by(|a, b| a.path.cmp(&b.path));
    if let Some(sort) = &selection.sort {
        sort_notes(&mut hits, sort);
    }

    let total = hits.len();
    let limit = selection.effective_limit();
    let columns = selection.effective_columns();
    let rows = hits
        .into_iter()
        .take(limit)
        .map(|note| SelectionRow {
            path: note.path.clone(),
            cells: columns
                .iter()
                .map(|column| value_of(note, column))
                .collect(),
        })
        .collect();
    SelectionResult {
        rows,
        columns,
        truncated: total > limit,
        total: total as u32,
    }
}

/// Столбец (или ключ) — название заметки: оно показывается ссылкой.
pub(crate) fn is_name_column(key: &str) -> bool {
    builtin(key) == Some(Builtin::Name)
}
