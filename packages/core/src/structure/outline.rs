//! Оглавление: пункты всех видов в порядке текста (data-model.md «Outline»).
//!
//! Виды отбирает [`super::view::outline_view`], а не разбор: включение вида
//! не требует повторного разбора текста.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{parse, FigureKind, LabelOwner, NoteStructure, SourcePlace};
use crate::preview::anchor::HeadingProbe;

/// Вид пункта.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum OutlineKind {
    Heading,
    Figure,
    Table,
    Label,
}

/// Какие виды показывать кроме заголовков (FR-010). Сохраняется между
/// запусками в состоянии колонки заметки.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(default)]
pub struct OutlineKinds {
    /// Рисунки и таблицы.
    pub figures: bool,
    /// Метки.
    pub labels: bool,
}

/// Пункт оглавления.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct OutlineItem {
    /// Устойчивый ключ; для интерфейса непрозрачен (research.md §7).
    pub key: String,
    pub kind: OutlineKind,
    /// Уровень отступа: у заголовка — свой, у остальных — уровень раздела + 1.
    pub level: u8,
    /// Заголовок — читаемый текст; рисунок, таблица — подпись; метка — имя.
    pub text: String,
    pub text_is_source: bool,
    /// Метка при заголовке или рисунке.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub label: Option<String>,
    /// Индекс родителя в `items`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub parent: Option<u32>,
    /// Куда ставится каретка: `place.from`.
    pub place: SourcePlace,
}

/// Результат [`outline`]. Значение держит окно и передаёт обратно в ядро
/// (research.md §9): состояния между вызовами у ядра нет.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Outline {
    pub items: Vec<OutlineItem>,
}

/// Оглавление текста заметки.
pub fn outline(text: &str) -> Outline {
    Outline::from(&parse(text))
}

impl From<&NoteStructure> for Outline {
    fn from(structure: &NoteStructure) -> Self {
        // Метки рисунков: у рисунка без подписи пункта нет, и его метка
        // становится пунктом сама — показать её больше негде.
        let figure_labels: HashMap<&str, SourcePlace> = structure
            .labels
            .iter()
            .filter(|label| label.attached == LabelOwner::Figure)
            .map(|label| (label.name.as_str(), label.place))
            .collect();

        struct Raw {
            kind: OutlineKind,
            level: u8,
            text: String,
            text_is_source: bool,
            label: Option<String>,
            place: SourcePlace,
        }
        let mut raw: Vec<Raw> = Vec::new();
        for heading in &structure.headings {
            raw.push(Raw {
                kind: OutlineKind::Heading,
                level: heading.level,
                text: heading.text.clone(),
                text_is_source: heading.text_is_source,
                label: heading.label.clone(),
                place: heading.place,
            });
        }
        for figure in &structure.figures {
            match (&figure.caption, &figure.label) {
                (Some(caption), _) => raw.push(Raw {
                    kind: match figure.kind {
                        FigureKind::Figure => OutlineKind::Figure,
                        FigureKind::Table => OutlineKind::Table,
                    },
                    level: 0,
                    text: caption.clone(),
                    text_is_source: false,
                    label: figure.label.clone(),
                    place: figure.place,
                }),
                (None, Some(name)) => {
                    if let Some(place) = figure_labels.get(name.as_str()) {
                        raw.push(Raw {
                            kind: OutlineKind::Label,
                            level: 0,
                            text: name.clone(),
                            text_is_source: false,
                            label: None,
                            place: *place,
                        });
                    }
                }
                (None, None) => {}
            }
        }
        for label in &structure.labels {
            if label.attached == LabelOwner::Other {
                raw.push(Raw {
                    kind: OutlineKind::Label,
                    level: 0,
                    text: label.name.clone(),
                    text_is_source: false,
                    label: None,
                    place: label.place,
                });
            }
        }
        // Порядок текста; при равных началах (не бывает у разных записей)
        // сохраняется порядок добавления.
        raw.sort_by_key(|item| item.place.from);

        let mut items: Vec<OutlineItem> = Vec::with_capacity(raw.len());
        // Заголовки-предки текущего места: уровни строго возрастают.
        let mut stack: Vec<(u8, u32)> = Vec::new();
        let mut section: Option<(u32, u8)> = None;
        let mut seen: HashMap<String, u32> = HashMap::new();
        for item in raw {
            let index = items.len() as u32;
            let (level, parent) = if item.kind == OutlineKind::Heading {
                while stack.last().is_some_and(|(level, _)| *level >= item.level) {
                    stack.pop();
                }
                let parent = stack.last().map(|(_, index)| *index);
                stack.push((item.level, index));
                section = Some((index, item.level));
                (item.level, parent)
            } else {
                match section {
                    Some((heading, level)) => (level.saturating_add(1), Some(heading)),
                    None => (1, None),
                }
            };
            let prefix = match item.kind {
                OutlineKind::Heading => format!("h{level}"),
                OutlineKind::Figure => "f".to_owned(),
                OutlineKind::Table => "t".to_owned(),
                OutlineKind::Label => "l".to_owned(),
            };
            // Ключ — вид, уровень, текст и порядковый номер среди одинаковых:
            // правка текста внутри раздела ключей не меняет (research.md §7).
            let base = format!("{prefix}:{}", item.text);
            let ordinal = seen.entry(base.clone()).or_insert(0);
            let key = format!("{base}#{ordinal}");
            *ordinal += 1;
            items.push(OutlineItem {
                key,
                kind: item.kind,
                level,
                text: item.text,
                text_is_source: item.text_is_source,
                label: item.label,
                parent,
                place: item.place,
            });
        }
        Outline { items }
    }
}

/// Ключи заголовков по порядку.
fn heading_keys(outline: &Outline) -> Vec<&str> {
    outline
        .items
        .iter()
        .filter(|item| item.kind == OutlineKind::Heading)
        .map(|item| item.key.as_str())
        .collect()
}

/// Переносит свёрнутость через правку самого заголовка (research.md §7).
///
/// Ключ есть в `next` — остаётся. Нет, но число заголовков в `prev` и `next`
/// равно (переименование, смена уровня) — заменяется ключом заголовка с тем
/// же порядковым номером. Иначе снимается. Результат не содержит ключей,
/// которых нет в `next`, и повторов.
pub fn carry_collapsed(prev: &Outline, next: &Outline, collapsed: &[String]) -> Vec<String> {
    let before = heading_keys(prev);
    let after = heading_keys(next);
    let same_count = before.len() == after.len();
    let mut carried: Vec<String> = Vec::new();
    for key in collapsed {
        let target = if after.contains(&key.as_str()) {
            Some(key.as_str())
        } else if same_count {
            before
                .iter()
                .position(|known| known == key)
                .map(|ordinal| after[ordinal])
        } else {
            None
        };
        if let Some(target) = target {
            if !carried.iter().any(|known| known == target) {
                carried.push(target.to_owned());
            }
        }
    }
    carried
}

/// Проба для поиска заголовка в собранном документе. `None` — у пункта,
/// который не заголовок, у неизвестного ключа и у заголовка с
/// `text_is_source`: исходная запись в документе не встретится.
pub fn heading_probe(outline: &Outline, key: &str) -> Option<HeadingProbe> {
    let position = outline.items.iter().position(|item| item.key == key)?;
    let item = &outline.items[position];
    if item.kind != OutlineKind::Heading || item.text_is_source {
        return None;
    }
    let occurrence = outline.items[..position]
        .iter()
        .filter(|earlier| earlier.kind == OutlineKind::Heading && earlier.text == item.text)
        .count();
    Some(HeadingProbe {
        text: item.text.clone(),
        occurrence: occurrence as u32,
    })
}
