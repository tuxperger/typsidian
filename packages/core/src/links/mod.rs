//! Связи заметки: обратные ссылки, упоминания названия и действие «Связать»
//! (фича 013).
//!
//! Модуль устроен по образцу `export/`: чистые функции и функции поверх
//! `VaultBackend`, без платформенного кода (принцип II), и тонкие биндинги в
//! `web.rs`. Сам разбор `[[…]]` остаётся в `graph` — здесь второго разбора нет
//! (принцип I): панель и индекс ссылок обязаны видеть одни и те же ссылки.
//!
//! Ничего из типов этого модуля на диске не хранится. Места вхождений
//! вычисляются по текущему тексту заметки и устаревают при каждой правке, поэтому
//! их место — ответ ядра, а не таблица индекса (research.md §2).

pub mod backlinks;
pub mod link;
pub mod mentions;
pub mod snippet;

#[cfg(target_arch = "wasm32")]
pub mod web;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::search::LineSegment;
use crate::vault::backend::{ReadError, VaultBackend};

pub use backlinks::{backlinks_page, BacklinksPage, BACKLINKS_PAGE_MAX};
pub use link::{link_mention, plan_link, LinkOutcome, LinkPlan, LinkRequest, Stale};
pub use mentions::{
    availability, in_notes, mentions_in_text, MentionScan, MentionStep, MentionsAvailability,
    MentionsStart, ScanPhase, MENTION_STEP_BYTES, MENTION_STEP_NOTES, MIN_MENTION_TITLE_CHARS,
};

/// Текст заметки, открытой в редакторе (research.md §10). Для такой заметки
/// места и фрагменты считаются по этому тексту, а не по диску: иначе место,
/// посчитанное по файлу, не совпало бы с текстом редактора с несохранёнными
/// правками.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct OpenText {
    pub path: String,
    pub text: String,
}

/// Место вхождения в тексте заметки — в тех же единицах, что у
/// `preview::DiagnosticLocation`: строка и колонка с единицы, байтовый
/// диапазон. Новых соглашений о позициях на границе не вводится.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SitePlace {
    pub line: u32,
    pub column: u32,
    pub byte_start: u32,
    pub byte_end: u32,
}

/// Строка панели: одно вхождение.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SiteRow {
    pub place: SitePlace,
    /// Текст вхождения дословно: `[[Проект Альфа]]` или `проект альфа`.
    pub text: String,
    /// Фрагмент строки; подсвечено ровно это вхождение.
    pub snippet: Vec<LineSegment>,
}

/// Почему у источника нет строк.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SourceProblem {
    /// Заметка больше предела размера.
    TooLarge,
    /// Заметка есть в индексе, но не читается.
    Unreadable,
}

/// Заметка-источник со своими вхождениями.
///
/// Группа без строк и без `problem` не существует (data-model.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SourceGroup {
    pub path: String,
    /// Имя файла без расширения.
    pub title: String,
    /// Папка относительно корня; пустая строка — корень.
    pub folder: String,
    /// Та же папка, что у цели: папку тогда не показывают.
    pub same_folder: bool,
    pub rows: Vec<SiteRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub problem: Option<SourceProblem>,
}

/// Путь строкой с `/` — как его хранят остальные типы границы.
pub(crate) fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Название заметки для показа и поиска упоминаний: имя файла без последнего
/// расширения (`глава.1.typ` → `глава.1`, `схема.excalidraw` → `схема`).
pub fn title_of(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Папка заметки — всё до последнего разделителя; пустая строка — корень.
pub(crate) fn folder_of(path: &str) -> String {
    match path.rfind('/') {
        Some(cut) => path[..cut].to_owned(),
        None => String::new(),
    }
}

/// Расстояние между двумя папками по дереву: сколько шагов вверх и вниз от
/// одной до другой. Одна и та же папка — 0, соседняя — 2, родитель — 1.
fn folder_distance(left: &str, right: &str) -> usize {
    let left: Vec<&str> = left.split('/').filter(|part| !part.is_empty()).collect();
    let right: Vec<&str> = right.split('/').filter(|part| !part.is_empty()).collect();
    let common = left
        .iter()
        .zip(right.iter())
        .take_while(|(a, b)| a == b)
        .count();
    (left.len() - common) + (right.len() - common)
}

/// Ключ порядка источников: близость папки к папке цели, затем путь
/// (research.md §4, §7). Ключ зависит только от самих путей, поэтому проход
/// упоминаний по нему переживает появление и исчезновение заметок.
pub(crate) fn proximity_key(target: &str, source: &str) -> (usize, String) {
    (
        folder_distance(&folder_of(target), &folder_of(source)),
        source.to_owned(),
    )
}

/// Упорядочивает пути источников «близость папки, затем путь».
pub(crate) fn sort_by_proximity(target: &str, paths: &mut [String]) {
    paths.sort_by_cached_key(|path| proximity_key(target, path));
}

/// Собирает группу источника. `None`, если строк нет и проблемы тоже нет.
pub(crate) fn group_of(
    target: &str,
    path: &str,
    rows: Vec<SiteRow>,
    problem: Option<SourceProblem>,
) -> Option<SourceGroup> {
    if rows.is_empty() && problem.is_none() {
        return None;
    }
    let folder = folder_of(path);
    Some(SourceGroup {
        title: title_of(path),
        same_folder: folder == folder_of(target),
        folder,
        path: path.to_owned(),
        rows,
        problem,
    })
}

/// Что вернуло чтение источника.
pub(crate) enum Fetched {
    Text(String),
    /// Файл есть, но прочитать его нельзя.
    Problem(SourceProblem),
    /// Файла уже нет: индекс отстал от диска.
    Gone,
}

/// Текст источника: из открытого редактора, если заметка там открыта,
/// иначе с диска (research.md §10).
pub(crate) async fn fetch<B: VaultBackend>(
    backend: &B,
    path: &str,
    open_texts: &[OpenText],
) -> Fetched {
    if let Some(open) = open_texts.iter().find(|open| open.path == path) {
        return Fetched::Text(open.text.clone());
    }
    match backend.read(Path::new(path)).await {
        Ok(text) => Fetched::Text(text),
        Err(ReadError::NotFound) => Fetched::Gone,
        Err(ReadError::TooLarge) => Fetched::Problem(SourceProblem::TooLarge),
        Err(ReadError::Io) => Fetched::Problem(SourceProblem::Unreadable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_drops_only_the_last_extension() {
        assert_eq!(title_of("a/Проект Альфа.typ"), "Проект Альфа");
        assert_eq!(title_of("глава.1.typ"), "глава.1");
        assert_eq!(title_of("схема.excalidraw"), "схема");
        assert_eq!(title_of(""), "");
    }

    #[test]
    fn sources_are_ordered_by_folder_proximity_then_path() {
        let target = "a/b/цель.typ";
        let mut paths: Vec<String> = [
            "z/y/w/глубоко.typ", // 5 шагов
            "корень.typ",        // 2 шага вверх
            "a/d/сосед.typ",     // 2: вверх и вниз
            "a/b/c/внутри.typ",  // 1 шаг вниз
            "a/b/б-рядом.typ",   // та же папка
            "a/b/а-рядом.typ",   // та же папка, раньше по пути
            "a/родитель.typ",    // 1 шаг вверх
        ]
        .iter()
        .map(|p| p.to_string())
        .collect();
        sort_by_proximity(target, &mut paths);
        assert_eq!(
            paths,
            vec![
                "a/b/а-рядом.typ",
                "a/b/б-рядом.typ",
                "a/b/c/внутри.typ",
                "a/родитель.typ",
                "a/d/сосед.typ",
                "корень.typ",
                "z/y/w/глубоко.typ",
            ]
        );
    }

    #[test]
    fn group_without_rows_and_problem_does_not_exist() {
        assert!(group_of("t.typ", "s.typ", Vec::new(), None).is_none());
        let group = group_of("t.typ", "s.typ", Vec::new(), Some(SourceProblem::TooLarge));
        assert!(group.is_some_and(|group| group.same_folder));
    }
}
