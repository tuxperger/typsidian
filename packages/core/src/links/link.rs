//! «Связать»: превратить упоминание в ссылку (research.md §9).
//!
//! Одна точка решения в ядре: функция сама выбирает между записью файла со
//! снимком `expected` и правкой для редактора открытой заметки — правило
//! FR-016 не может жить в интерфейсе (принцип I). Единственная запись в vault —
//! `write(source, new_text, Some(expected))` в исходе [`LinkOutcome::Written`];
//! во всех прочих исходах vault не тронут.

use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::mentions::is_free_standing;
use super::{path_string, title_of, SitePlace};
use crate::elements::edit::TextEdit;
use crate::elements::utf16_at;
use crate::graph::{link_sites, link_text};
use crate::vault::backend::{VaultBackend, WriteError};
use crate::vault::AccessState;

/// Какое упоминание и куда ссылать.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LinkRequest {
    /// Заметка, на которую ставится ссылка.
    pub target: String,
    /// Заметка с упоминанием.
    pub source: String,
    /// Место из `SiteRow`.
    pub place: SitePlace,
    /// Текст упоминания из `SiteRow`.
    pub text: String,
}

/// Результат планирования: новый текст и та же замена в единицах UTF-16.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkPlan {
    pub new_text: String,
    pub edit: TextEdit,
}

/// На прежнем месте упоминания больше нет (FR-015).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stale;

/// Чем кончилось «Связать». Отказ — вариант значения, а не ошибка вызова:
/// интерфейс различает исходы и показывает каждый по-своему.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum LinkOutcome {
    /// Файл записан, изменено только упоминание.
    Written,
    /// Файл не тронут: заменить текст в редакторе открытой заметки.
    EditorEdit { edit: TextEdit },
    /// Упоминания на прежнем месте нет; файл не тронут.
    Stale,
    /// Vault только для чтения.
    ReadOnly,
    /// У заметки несохранённые правки вне редактора: применить замену некуда.
    UnsavedElsewhere,
    /// Ссылку на эту заметку записать нельзя.
    Unexpressible,
    /// Запись не удалась.
    Failed,
}

/// Чистое планирование: текст в диапазоне `request.place` обязан побайтно
/// совпасть с `request.text` и оставаться упоминанием по правилам поиска
/// (границы слов, вне ссылки), иначе [`Stale`]. Результат отличается от `text`
/// только заменённым диапазоном (FR-014, SC-005).
pub fn plan_link(text: &str, request: &LinkRequest, link: &str) -> Result<LinkPlan, Stale> {
    let start = request.place.byte_start as usize;
    let end = request.place.byte_end as usize;
    if end < start || text.get(start..end) != Some(request.text.as_str()) {
        return Err(Stale);
    }

    let title = title_of(&request.target);
    let still_the_title = Regex::new(&format!("(?i)^{}$", regex::escape(&title)))
        .is_ok_and(|exact| exact.is_match(&request.text));
    if !still_the_title || !is_free_standing(text, start, end, &link_sites(text)) {
        return Err(Stale);
    }

    let mut new_text = String::with_capacity(text.len() + link.len());
    new_text.push_str(&text[..start]);
    new_text.push_str(link);
    new_text.push_str(&text[end..]);
    Ok(LinkPlan {
        new_text,
        edit: TextEdit {
            from: utf16_at(text, start),
            to: utf16_at(text, end),
            insert: link.to_owned(),
        },
    })
}

/// Превращает упоминание в ссылку. Порядок шагов — research.md §9, тесты
/// проверяют его в том же порядке.
///
/// `editor_text` — `Some`, если заметка открыта в редакторе: тогда на диск не
/// пишется ничего, а правка уходит в редактор и дальше идёт обычным
/// автосохранением (FR-016).
pub async fn link_mention<B: VaultBackend>(
    backend: &mut B,
    request: &LinkRequest,
    editor_text: Option<&str>,
) -> LinkOutcome {
    if backend.access_state() != AccessState::ReadWrite {
        return LinkOutcome::ReadOnly;
    }

    let source = Path::new(&request.source);
    let Some(link) = link_text(source, Path::new(&request.target)) else {
        return LinkOutcome::Unexpressible;
    };

    if let Some(text) = editor_text {
        return match plan_link(text, request, &link) {
            Ok(plan) => LinkOutcome::EditorEdit { edit: plan.edit },
            Err(Stale) => LinkOutcome::Stale,
        };
    }

    if backend.has_unsaved_edits(source) {
        return LinkOutcome::UnsavedElsewhere;
    }

    // Отпечаток берётся из индекса самого backend, а не строится из прочитанного
    // текста: native сравнивает полный `FileStamp` с диска, web — хэш содержимого
    // с нулевым временем, и сравним на обоих только отпечаток backend.
    let folder = source.parent().unwrap_or_else(|| Path::new(""));
    let stamp = backend
        .list_folder(folder)
        .await
        .into_iter()
        .find(|note| path_string(&note.path) == request.source)
        .map(|note| note.stamp);
    let Some(stamp) = stamp else {
        return LinkOutcome::Stale;
    };
    let Ok(text) = backend.read(source).await else {
        return LinkOutcome::Stale;
    };
    let Ok(plan) = plan_link(&text, request, &link) else {
        return LinkOutcome::Stale;
    };

    match backend.write(source, &plan.new_text, Some(stamp)).await {
        Ok(()) => LinkOutcome::Written,
        Err(WriteError::Conflict) => LinkOutcome::Stale,
        Err(WriteError::AccessDenied) => LinkOutcome::ReadOnly,
        Err(WriteError::Io) => LinkOutcome::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::snippet::place_of;

    fn request(text: &str, needle: &str) -> LinkRequest {
        let start = text.find(needle).unwrap();
        LinkRequest {
            target: "Проект.typ".to_owned(),
            source: "б.typ".to_owned(),
            place: place_of(text, start, start + needle.len()),
            text: needle.to_owned(),
        }
    }

    #[test]
    fn plan_replaces_only_the_range_and_reports_utf16() {
        let text = "😀 см. проект тут";
        let request = request(text, "проект");
        let plan = plan_link(text, &request, "[[Проект]]").unwrap();
        assert_eq!(plan.new_text, "😀 см. [[Проект]] тут");
        // 😀 — две единицы UTF-16.
        assert_eq!((plan.edit.from, plan.edit.to), (2 + 5, 2 + 5 + 6));
        assert_eq!(plan.edit.insert, "[[Проект]]");
    }

    #[test]
    fn plan_refuses_shifted_or_changed_text() {
        let text = "см. проект тут";
        let request = request(text, "проект");
        assert_eq!(
            plan_link(&format!("вставка {text}"), &request, "[[Проект]]"),
            Err(Stale)
        );
        assert_eq!(
            plan_link("см. Проект тут", &request, "[[Проект]]"),
            Err(Stale)
        );
        assert_eq!(plan_link("", &request, "[[Проект]]"), Err(Stale));
    }

    #[test]
    fn plan_refuses_a_place_that_became_part_of_a_link_or_a_word() {
        let mut request = request("см. проект тут", "проект");
        request.place.byte_start = 7;
        request.place.byte_end = 7 + "проект".len() as u32;
        assert_eq!(
            plan_link("см. [[проект]] тут", &request, "[[Проект]]"),
            Err(Stale)
        );
        // Мультибайтная граница посреди символа не паникует.
        request.place.byte_start = 5;
        assert_eq!(
            plan_link("см. проект тут", &request, "[[Проект]]"),
            Err(Stale)
        );
    }
}
