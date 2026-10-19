//! Отметки цитат с ненайденным ключом (FR-006, SC-003, research.md §10).
//!
//! Правило одно и в ядре: `cite(<k>)` без источника — битая; `@k` — цитата,
//! если `k` — ключ источника, ссылка, если `k` — метка заметки или известная
//! метка (параметр для 018), и битая неоднозначная, если ни то ни другое при
//! подключённых источниках. Без файлов источников `@k` — дело 018.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::structure::{parse, SourcePlace};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CitationMark {
    pub place: SourcePlace,
    pub kind: CitationMarkKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum CitationMarkKind {
    /// `ambiguous` — запись `@k`, которая могла быть и ссылкой на метку: от
    /// этого зависит текст причины.
    MissingSource { key: String, ambiguous: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct NoteCheck {
    pub marks: Vec<CitationMark>,
    pub citations: u32,
    pub broken: u32,
    pub has_bibliography: bool,
}

/// Проверяет живой текст заметки. `is_key` — есть ли источник с ключом,
/// `has_files` — есть ли в vault файлы источников.
pub fn check_note(
    text: &str,
    is_key: &dyn Fn(&str) -> bool,
    has_files: bool,
    known_labels: &[String],
) -> NoteCheck {
    let structure = parse(text);
    let labels: BTreeSet<&str> = structure
        .labels
        .iter()
        .map(|label| label.name.as_str())
        .chain(known_labels.iter().map(String::as_str))
        .collect();
    let mut marks = Vec::new();
    let mut citations = 0;
    for site in &structure.cites {
        citations += 1;
        if !is_key(&site.key) {
            marks.push(CitationMark {
                place: site.place,
                kind: CitationMarkKind::MissingSource {
                    key: site.key.clone(),
                    ambiguous: false,
                },
            });
        }
    }
    for site in &structure.refs {
        if is_key(&site.target) {
            citations += 1;
        } else if !labels.contains(site.target.as_str()) && has_files {
            citations += 1;
            marks.push(CitationMark {
                place: site.place,
                kind: CitationMarkKind::MissingSource {
                    key: site.target.clone(),
                    ambiguous: true,
                },
            });
        }
    }
    marks.sort_by_key(|mark| mark.place.from);
    NoteCheck {
        broken: marks.len() as u32,
        marks,
        citations,
        has_bibliography: !structure.bibliographies.is_empty(),
    }
}
