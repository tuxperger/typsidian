//! Удержание места при пересборке (FR-004, FR-034, SC-005;
//! contracts/frontend-preview.md § Удержание места).
//!
//! Якорь — место в **содержимом**, а не смещение от начала документа: после
//! правки выше по тексту то же место оказывается на другой высоте, а то и на
//! другой странице. Интерфейс снимает якорь с того, что видно на экране, и
//! прокручивает туда, куда укажет это правило; сам он ничего не сопоставляет —
//! иначе правило жило бы в двух местах (принцип I).

use serde::{Deserialize, Serialize};

use super::pages::RenderedPage;
use super::textlayer::TextRun;

/// Место документа, которое нужно удержать на экране.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ScrollAnchor {
    /// Страница, с которой снят якорь, с нуля.
    pub page_index: u32,
    /// Текст прогона, по которому место опознаётся после переверстки.
    pub run_text: String,
    /// Номер прогона на странице — различает повторяющийся текст.
    pub run_index: u32,
}

/// Куда прокрутить после пересборки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ResolvedAnchor {
    pub page_index: u32,
    pub run_index: u32,
    /// Найден ли тот же фрагмент, или это ближайшее сохранившееся место
    /// (FR-034).
    pub exact: bool,
}

/// Находит в новом документе место, с которого снят якорь.
///
/// Тот же текст есть — возвращается его прогон, даже если он переехал на
/// другую страницу; из нескольких одинаковых выбирается ближайший к прежнему
/// положению в порядке чтения. Текста больше нет — возвращается прогон,
/// стоящий теперь на прежнем месте в порядке чтения, а **не** начало
/// документа. `None` — только когда в документе нет ни одного прогона: тогда
/// удерживать нечего.
pub fn resolve(anchor: &ScrollAnchor, pages: &[RenderedPage]) -> Option<ResolvedAnchor> {
    // Положение каждого прогона в сквозном порядке чтения.
    let mut offsets = Vec::with_capacity(pages.len());
    let mut total = 0usize;
    for page in pages {
        offsets.push(total);
        total += page.text_runs.len();
    }
    if total == 0 {
        return None;
    }

    // Прежнее место в новой нумерации. Страницы, которой больше нет, и номера
    // за концом страницы прижимаются к последнему прогону документа.
    let page = (anchor.page_index as usize).min(pages.len() - 1);
    let wanted = (offsets[page] + anchor.run_index as usize).min(total - 1);

    let at = |ordinal: usize, exact: bool| {
        let page = offsets.partition_point(|start| *start <= ordinal) - 1;
        ResolvedAnchor {
            page_index: page as u32,
            run_index: (ordinal - offsets[page]) as u32,
            exact,
        }
    };

    // Пустой текст места не опознаёт: пустых прогонов в документе много.
    let same = (!anchor.run_text.is_empty())
        .then(|| {
            pages
                .iter()
                .zip(&offsets)
                .flat_map(|(page, start)| {
                    page.text_runs
                        .iter()
                        .enumerate()
                        .filter(|(_, run)| run.text == anchor.run_text)
                        .map(move |(index, _)| start + index)
                })
                .min_by_key(|ordinal| ordinal.abs_diff(wanted))
        })
        .flatten();

    Some(match same {
        Some(ordinal) => at(ordinal, true),
        None => at(wanted, false),
    })
}

/// Что искать в собранном документе, чтобы показать заголовок (фича 015,
/// research.md §10). Ядро выводит пробу из оглавления
/// ([`crate::structure::heading_probe`]), интерфейс её не видит.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct HeadingProbe {
    /// Читаемый текст заголовка.
    pub text: String,
    /// Номер среди заголовков заметки с таким же текстом, с нуля.
    pub occurrence: u32,
}

/// Находит заголовок среди прогонов текстового слоя.
///
/// `runs[i]` — прогоны страницы `i` в порядке чтения, как в
/// `RenderedPage::text_runs`. Совпадение — прогон либо цепочка соседних
/// прогонов одной страницы, чья склейка без пробелов равна тексту пробы
/// (заголовок с оформлением и перенесённый на две строки дают несколько
/// прогонов). Из совпадений берётся наибольший кегль: строка оглавления
/// документа и колонтитул повторяют текст заголовка обычным кеглем. Из них —
/// `probe.occurrence`-е по порядку чтения; номер за концом — последнее.
///
/// Результат указывает на первый прогон совпадения. Совпадений нет — `None`:
/// предпросмотр остаётся на месте. [`resolve`] не затрагивается.
pub fn locate_heading(runs: &[Vec<TextRun>], probe: &HeadingProbe) -> Option<ResolvedAnchor> {
    let target: String = probe.text.split_whitespace().collect();
    if target.is_empty() {
        return None;
    }
    // (страница, первый прогон, кегль) в порядке чтения.
    let mut found: Vec<(u32, u32, f64)> = Vec::new();
    for (page, page_runs) in runs.iter().enumerate() {
        let mut start = 0;
        while start < page_runs.len() {
            let mut joined = String::new();
            let mut end = start;
            let mut matched = false;
            while end < page_runs.len() {
                joined.extend(page_runs[end].text.split_whitespace().flat_map(str::chars));
                end += 1;
                if joined == target {
                    matched = true;
                    break;
                }
                // Склейка уже не начало текста — дальше цепочку не тянуть.
                if !target.starts_with(&joined) {
                    break;
                }
            }
            if matched {
                found.push((page as u32, start as u32, page_runs[start].size_pt));
                start = end;
            } else {
                start += 1;
            }
        }
    }
    let largest = found
        .iter()
        .map(|(_, _, size)| *size)
        .fold(f64::MIN, f64::max);
    let best: Vec<&(u32, u32, f64)> = found
        .iter()
        .filter(|(_, _, size)| *size >= largest - f64::EPSILON)
        .collect();
    let pick = best.get(probe.occurrence as usize).or(best.last())?;
    Some(ResolvedAnchor {
        page_index: pick.0,
        run_index: pick.1,
        exact: true,
    })
}
