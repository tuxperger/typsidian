//! Параметры элементов Typst (фича 008, contracts/element-params.md).
//!
//! Форма параметров, которая раньше была только у рисунка и таблицы, здесь
//! становится общим правилом каталога вставки. Три опоры:
//!
//! 1. [`schema`] — **схема параметров**: файл `schema.json`, где явно перечислены
//!    все параметры каждого элемента каталога. Машинная часть сверяется с самим
//!    Typst тестом (research.md §1).
//! 2. [`locate`] — элемент под кареткой ищется по синтаксическому дереву
//!    Typst, а не регулярками (research.md §2).
//! 3. [`edit`] — правка применяется **точечными заменами**: меняется только
//!    значение изменённого параметра, остальное не трогается (research.md §4).
//!
//! Все функции чистые: ни файловой системы, ни сети (FR-023), ни состояния
//! между вызовами. Смещения наружу — в кодовых единицах UTF-16, как у
//! `templates::InsertionPoint`: ядро считает так же, как редактор.

pub mod edit;
pub mod locate;
pub mod render;
pub mod schema;
pub mod values;




pub use edit::{plan_element_edit, PlanEditError, TextEdit};
pub use locate::{element_at, ElementAtResult, ParsedElement, UnavailableReason};
pub use render::{render_element, RenderedElement};
pub use schema::{element_schema, schema, ElementSpec, ElementSyntax, ParamKind, ParamSpec};
pub use values::{FieldError, FieldProblem, FieldValue, ParamValue};


/// Смещение в байтах → в кодовых единицах UTF-16.
pub(crate) fn utf16_at(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].encode_utf16().count()
}

/// Смещение в кодовых единицах UTF-16 → в байтах.
///
/// Смещение внутри суррогатной пары или за концом текста прижимается к
/// ближайшей границе символа слева: редактор такого не пришлёт, а паники из-за
/// чужой ошибки на границе быть не должно.
pub(crate) fn byte_at(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (index, ch) in text.char_indices() {
        if units >= utf16 {
            return index;
        }
        units += ch.len_utf16();
        if units > utf16 {
            return index;
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_round_trip_through_utf16() {
        let text = "Ж𝔸x";
        // Ж — 2 байта, 1 единица; 𝔸 — 4 байта, 2 единицы; x — 1 и 1.
        assert_eq!(utf16_at(text, 2), 1);
        assert_eq!(utf16_at(text, 6), 3);
        assert_eq!(byte_at(text, 1), 2);
        assert_eq!(byte_at(text, 3), 6);
        assert_eq!(byte_at(text, 2), 2, "середина пары прижимается влево");
        assert_eq!(byte_at(text, 99), text.len());
    }
}

// ── Граница клиентов (research.md §8) ──────────────────────────────────────
//
// Одни и те же формы запроса и ответа для wasm и для команд Tauri: списки и
// `Result` едут внутри размеченных структур, а не голыми аргументами — так
// транспорт у клиентов разный, а контракт один (FR-022).

use serde::{Deserialize, Serialize};

/// Запрос «Будет вставлено» (FR-004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RenderRequest {
    pub id: String,
    pub fields: Vec<FieldValue>,
    #[serde(default)]
    pub label: Option<String>,
    /// Метки, уже занятые в заметке.
    #[serde(default)]
    pub taken_labels: Vec<String>,
    /// Текст заметки: занятые метки ядро соберёт из него само — разбирать
    /// разметку в интерфейсе нельзя (принцип I).
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum RenderOutcome {
    Rendered { element: RenderedElement },
    /// Значения, которые элемент не примет (FR-015): «Применить» заперто.
    Invalid { errors: Vec<FieldError> },
}

pub fn render(request: &RenderRequest) -> RenderOutcome {
    let mut taken = request.taken_labels.clone();
    if let Some(note) = &request.note {
        for label in locate::labels_in(note) {
            if !taken.contains(&label) {
                taken.push(label);
            }
        }
    }
    match render_element(&request.id, &request.fields, request.label.as_deref(), &taken) {
        Ok(element) => RenderOutcome::Rendered { element },
        Err(errors) => RenderOutcome::Invalid { errors },
    }
}

/// Запрос правки на месте (FR-008).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct EditRequest {
    /// Текст заметки сейчас — не тот, что был при открытии формы: по нему и
    /// проверяется, не изменился ли элемент (FR-020).
    pub text: String,
    pub parsed: ParsedElement,
    pub fields: Vec<FieldValue>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum EditOutcome {
    /// Замены — пустой список, если ничего не изменилось (FR-009).
    Planned { edits: Vec<TextEdit> },
    Refused { error: PlanEditError },
}

pub fn plan(request: &EditRequest) -> EditOutcome {
    match plan_element_edit(
        &request.text,
        &request.parsed,
        &request.fields,
        request.label.as_deref(),
    ) {
        Ok(edits) => EditOutcome::Planned { edits },
        Err(error) => EditOutcome::Refused { error },
    }
}
