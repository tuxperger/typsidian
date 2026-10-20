//! Свидетельство удачной сборки для отметок 018 (research.md §4).
//!
//! Текст не видит меток, созданных кодом (цикл, функция шаблона,
//! `#label(…)`), и не знает о правиле `show ref`. Удачная сборка знает: всё,
//! на что ссылается её текст, разрешилось. Поэтому сборка отдаёт имена меток
//! документа и ссылок своего текста, а отметки используют их только как
//! подтверждение — снять или не поставить отметку.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use typst::introspection::Introspector;
use typst_layout::PagedDocument;

/// Метка собранного документа: имя и число вхождений (`count ≥ 1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BuiltLabel {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BuildEvidence {
    /// Метки документа по имени.
    pub labels: Vec<BuiltLabel>,
    /// Имена ссылок `@имя` в тексте, из которого собран документ, без повторов.
    pub references: Vec<String>,
}

impl BuildEvidence {
    /// Сколько раз метка встретилась в документе; `0` — её там нет.
    pub fn count(&self, name: &str) -> u32 {
        self.labels
            .binary_search_by(|label| label.name.as_str().cmp(name))
            .map_or(0, |at| self.labels[at].count)
    }

    pub fn confirms(&self, name: &str) -> bool {
        self.count(name) > 0 || self.references.iter().any(|reference| reference == name)
    }
}

/// Собирает свидетельство из документа и текста той же сборки.
pub fn collect(document: &PagedDocument, source_text: &str) -> BuildEvidence {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for content in document.introspector().query_labelled().iter() {
        if let Some(label) = content.label() {
            *counts
                .entry(label.resolve().as_str().to_owned())
                .or_insert(0) += 1;
        }
    }
    let mut references: Vec<String> = crate::structure::parse(source_text)
        .refs
        .into_iter()
        .map(|reference| reference.target)
        .collect();
    references.sort();
    references.dedup();
    BuildEvidence {
        labels: counts
            .into_iter()
            .map(|(name, count)| BuiltLabel { name, count })
            .collect(),
        references,
    }
}
