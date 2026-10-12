//! Видимые строки оглавления и текущий раздел (contracts/core-structure-api.md
//! § `outline_view`, `outline_active`).
//!
//! Порядок шагов проверяется тестами: виды → свёрнутость или фильтр →
//! вложенные и родители → текущий раздел → счётчики и состояние.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::outline::{Outline, OutlineKind, OutlineKinds};
use super::SourcePlace;
use crate::search::LineSegment;

/// Что и как показывать (data-model.md «OutlineRequest»).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(default)]
pub struct OutlineRequest {
    pub kinds: OutlineKinds,
    /// Ключи свёрнутых заголовков; неизвестные игнорируются.
    pub collapsed: Vec<String>,
    /// Пустая строка — фильтра нет; пробелы по краям не учитываются.
    pub filter: String,
    /// Каретка, UTF-16; `None` — подсветка не нужна.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub caret: Option<u32>,
}

/// Строка оглавления: ровно то, что рисуется.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct OutlineRow {
    pub key: String,
    pub kind: OutlineKind,
    /// Уровень отступа.
    pub level: u8,
    /// Текст строки кусками; `highlighted` — совпадение фильтра.
    pub segments: Vec<LineSegment>,
    pub text_is_source: bool,
    /// Метка при заголовке или рисунке; только когда включён показ меток.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub label: Option<String>,
    /// Есть вложенные пункты включённых видов.
    pub has_children: bool,
    /// Ветка свёрнута (при фильтре всегда `false`).
    pub collapsed: bool,
    /// Строка не подходит под фильтр и показана как родитель.
    pub dimmed: bool,
    /// Ключ видимого родителя — для клавиш ← и →.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub parent: Option<String>,
    pub place: SourcePlace,
}

/// Сколько пунктов каждого вида во всей заметке, без учёта фильтра и
/// свёрнутости.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct OutlineCounts {
    pub headings: u32,
    pub figures: u32,
    pub tables: u32,
    pub labels: u32,
}

/// Чем заполнен список (FR-012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum OutlineState {
    Ready,
    /// В заметке нет ни одного заголовка. Пункты других видов при этом
    /// показываются, если включены: пояснение стоит над ними.
    NoHeadings,
    /// Фильтр задан и ничего не подошло.
    NoMatches,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct OutlineView {
    pub rows: Vec<OutlineRow>,
    /// Ключ подсвечиваемой строки (research.md §8); всегда среди `rows`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub active: Option<String>,
    pub counts: OutlineCounts,
    pub state: OutlineState,
}

/// Кто виден и кто подошёл под фильтр — общее для строк и подсветки.
struct Shown {
    included: Vec<bool>,
    visible: Vec<bool>,
    matched: Vec<bool>,
    needle: Vec<char>,
}

impl Shown {
    fn filtering(&self) -> bool {
        !self.needle.is_empty()
    }
}

fn shown(outline: &Outline, request: &OutlineRequest) -> Shown {
    let items = &outline.items;
    // Шаг 1: виды; заголовки — всегда.
    let included: Vec<bool> = items
        .iter()
        .map(|item| match item.kind {
            OutlineKind::Heading => true,
            OutlineKind::Figure | OutlineKind::Table => request.kinds.figures,
            OutlineKind::Label => request.kinds.labels,
        })
        .collect();
    let needle = lowered_chars(request.filter.trim());
    let mut matched = vec![false; items.len()];
    let mut visible = vec![false; items.len()];

    if needle.is_empty() {
        // Шаг 2: потомки свёрнутых заголовков скрыты.
        let collapsed: HashSet<&str> = request.collapsed.iter().map(String::as_str).collect();
        let mut hidden = vec![false; items.len()];
        for (index, item) in items.iter().enumerate() {
            if let Some(parent) = item.parent {
                let parent = parent as usize;
                hidden[index] = hidden[parent] || collapsed.contains(items[parent].key.as_str());
            }
            visible[index] = included[index] && !hidden[index];
        }
    } else {
        // Шаг 3: подходящие пункты и все их предки; свёрнутость не действует.
        for (index, item) in items.iter().enumerate() {
            let label = item.label.as_deref().filter(|_| request.kinds.labels);
            matched[index] = included[index]
                && (!occurrences(&lowered(&item.text), &needle).is_empty()
                    || label
                        .is_some_and(|label| !occurrences(&lowered(label), &needle).is_empty()));
            visible[index] = matched[index];
        }
        // Предок всегда раньше потомка, поэтому один проход с конца
        // поднимает видимость по всей цепочке.
        for index in (0..items.len()).rev() {
            if visible[index] {
                if let Some(parent) = items[index].parent {
                    visible[parent as usize] = true;
                }
            }
        }
    }
    Shown {
        included,
        visible,
        matched,
        needle,
    }
}

/// Строки, которые видны сейчас.
pub fn outline_view(outline: &Outline, request: &OutlineRequest) -> OutlineView {
    let items = &outline.items;
    let shown = shown(outline, request);
    let collapsed: HashSet<&str> = request.collapsed.iter().map(String::as_str).collect();

    // Шаг 4: вложенные включённых видов.
    let mut has_children = vec![false; items.len()];
    for (index, item) in items.iter().enumerate() {
        if let (Some(parent), true) = (item.parent, shown.included[index]) {
            has_children[parent as usize] = true;
        }
    }

    let mut rows = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if !shown.visible[index] {
            continue;
        }
        let highlight = if shown.matched[index] {
            highlighted_chars(&item.text, &shown.needle)
        } else {
            Vec::new()
        };
        rows.push(OutlineRow {
            key: item.key.clone(),
            kind: item.kind,
            level: item.level,
            segments: segments_of(&item.text, &highlight),
            text_is_source: item.text_is_source,
            label: item.label.clone().filter(|_| request.kinds.labels),
            has_children: has_children[index],
            collapsed: !shown.filtering()
                && has_children[index]
                && collapsed.contains(item.key.as_str()),
            dimmed: shown.filtering() && !shown.matched[index],
            // Родитель виден всегда: потомок виден, только если виден он.
            parent: item.parent.map(|parent| items[parent as usize].key.clone()),
            place: item.place,
        });
    }

    // Шаг 5.
    let active = active_key(outline, request, &shown);

    // Шаг 6.
    let mut counts = OutlineCounts::default();
    for item in items {
        match item.kind {
            OutlineKind::Heading => counts.headings += 1,
            OutlineKind::Figure => counts.figures += 1,
            OutlineKind::Table => counts.tables += 1,
            OutlineKind::Label => counts.labels += 1,
        }
    }
    let state = if counts.headings == 0 {
        OutlineState::NoHeadings
    } else if shown.filtering() && rows.is_empty() {
        OutlineState::NoMatches
    } else {
        OutlineState::Ready
    };
    OutlineView {
        rows,
        active,
        counts,
        state,
    }
}

/// Текущий раздел: тот же ответ, что `outline_view(..).active`, без построения
/// строк — для движения каретки (research.md §8).
pub fn outline_active(outline: &Outline, request: &OutlineRequest) -> Option<String> {
    request.caret?;
    active_key(outline, request, &shown(outline, request))
}

/// Последний заголовок, начало которого не правее каретки; скрыт — ближайший
/// видимый предок; нет — ничего.
fn active_key(outline: &Outline, request: &OutlineRequest, shown: &Shown) -> Option<String> {
    let caret = request.caret?;
    let items = &outline.items;
    let mut current = items
        .iter()
        .rposition(|item| item.kind == OutlineKind::Heading && item.place.from <= caret)?;
    loop {
        if shown.visible[current] {
            return Some(items[current].key.clone());
        }
        current = items[current].parent? as usize;
    }
}

/// Нижний регистр по Unicode, по знакам.
fn lowered_chars(text: &str) -> Vec<char> {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// Нижний регистр с привязкой к знаку оригинала: у некоторых знаков нижний
/// регистр длиннее (`İ`), и позиции в приведённом тексте не равны позициям в
/// исходном.
fn lowered(text: &str) -> Vec<(char, usize)> {
    text.chars()
        .enumerate()
        .flat_map(|(origin, ch)| ch.to_lowercase().map(move |low| (low, origin)))
        .collect()
}

/// Непересекающиеся вхождения `needle` в приведённый текст, слева направо.
fn occurrences(hay: &[(char, usize)], needle: &[char]) -> Vec<std::ops::Range<usize>> {
    let mut found = Vec::new();
    let mut at = 0;
    while !needle.is_empty() && at + needle.len() <= hay.len() {
        if hay[at..at + needle.len()]
            .iter()
            .map(|(ch, _)| ch)
            .eq(needle.iter())
        {
            found.push(at..at + needle.len());
            at += needle.len();
        } else {
            at += 1;
        }
    }
    found
}

/// Какие знаки исходного текста входят в совпадения.
fn highlighted_chars(text: &str, needle: &[char]) -> Vec<bool> {
    let hay = lowered(text);
    let mut marks = vec![false; text.chars().count()];
    for found in occurrences(&hay, needle) {
        for (_, origin) in &hay[found] {
            marks[*origin] = true;
        }
    }
    marks
}

/// Текст кусками: подряд идущие знаки с одной отметкой — один кусок. Без
/// отметок — один кусок целиком.
fn segments_of(text: &str, highlight: &[bool]) -> Vec<LineSegment> {
    let mut segments: Vec<LineSegment> = Vec::new();
    for (index, ch) in text.chars().enumerate() {
        let marked = highlight.get(index).copied().unwrap_or(false);
        match segments.last_mut() {
            Some(last) if last.highlighted == marked => last.text.push(ch),
            _ => segments.push(LineSegment {
                text: ch.to_string(),
                highlighted: marked,
            }),
        }
    }
    if segments.is_empty() {
        segments.push(LineSegment {
            text: String::new(),
            highlighted: false,
        });
    }
    segments
}
