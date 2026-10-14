//! Параметры элементов Typst (фича 008, contracts/element-params.md).
//!
//! Форма параметров, которая раньше была только у рисунка и таблицы, здесь
//! становится общим правилом каталога вставки. Три опоры:
//!
//! 1. [`schema`] — **схема параметров**: читается из самого Typst в момент
//!    обращения — полный набор параметров любой встроенной функции, без файла
//!    схемы и без его сверки (research.md §1).
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
pub use schema::{
    element_schema, element_schema_for, key_for, spec, ElementSpec, ElementSyntax, ParamKind,
    ParamSpec,
};
pub use values::{FieldError, FieldProblem, FieldValue, ParamValue};

// Смещения UTF-16 общие с подсветкой (фича 024) и живут в `text_offsets`.
pub(crate) use crate::text_offsets::{byte_at, utf16_at};

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
    Rendered {
        element: RenderedElement,
    },
    /// Значения, которые элемент не примет (FR-015): «Применить» заперто.
    Invalid {
        errors: Vec<FieldError>,
    },
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
    match render_element(
        &request.id,
        &request.fields,
        request.label.as_deref(),
        &taken,
    ) {
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
    Planned {
        edits: Vec<TextEdit>,
    },
    Refused {
        error: PlanEditError,
    },
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
