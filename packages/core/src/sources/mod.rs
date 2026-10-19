//! Библиография vault (фича 017).
//!
//! Источники — записи файлов BibTeX `.bib` и Hayagriva `.yml`, лежащих в папке
//! заметок. Чтение — теми же крейтами, что у сборки Typst (research.md §1), но
//! терпимо к ошибкам: одна негодная запись не убирает остальные (§2). Поверх —
//! библиотека с поиском (§14), индекс «кто цитирует» (§8), запись цитаты и
//! предложение списка литературы (§12, §13), отметки битых цитат (§10).
//!
//! Сети модуль не касается вовсе (FR-009); всё состояние — в памяти.

pub mod bibtex;
pub mod check;
pub mod citations;
pub mod cite;
pub mod discover;
pub mod engine;
pub mod library;
pub mod yaml;

#[cfg(target_arch = "wasm32")]
pub mod web;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::vault::backend::{VaultBackend, WriteError};

pub use check::{check_note, CitationMark, CitationMarkKind, NoteCheck};
pub use citations::{
    CitationIndex, CitationPlace, CitationScanPhase, CitationScanView, CitingNote, MissingKey,
};
pub use cite::{
    bibliography_offer, citation_text, plan_citation, BibliographyOffer, CitationPlan,
    CitationPlanError,
};
pub use discover::{source_format, validate_source_file, SourceFileProblem};
pub use engine::SourceEngine;
pub use library::{LibrarySummary, RefreshStep, SourceHit, SourceHits, SourceLibrary};

/// Имя создаваемого файла источников — латиницей, одно для всех языков
/// интерфейса: это содержимое vault (research.md §5).
pub const NEW_SOURCE_FILE: &str = "references.bib";

/// Больше стольких названных ошибок на файл не показывается (research.md §2).
pub const MAX_PROBLEMS_PER_FILE: usize = 200;

/// Формат файла источников.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SourceFormat {
    BibTex,
    Hayagriva,
}

/// Кандидат в файлы источников — только метаданные, без чтения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SourceCandidate {
    /// Путь от корня vault, через `/`.
    pub path: String,
    pub size: u64,
    pub modified_ms: i64,
}

/// Почему файл считается файлом источников (research.md §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum RecognizedBy {
    /// `.bib` — источник всегда.
    Extension,
    /// В YAML нашлась запись Hayagriva.
    Content,
    /// Файл назван в `#bibliography(…)` заметки.
    Referenced,
    /// Файл указан пользователем в настройках vault.
    Registered,
}

/// Сводка по одному файлу источников.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SourceFileInfo {
    pub path: String,
    pub format: SourceFormat,
    pub source_count: u32,
    pub problem_count: u32,
    pub recognized_by: RecognizedBy,
}

/// Одна запись файла источников (data-model.md «Источник»). Тождество —
/// пара `(file, key)`: один ключ в двух файлах — два источника.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Source {
    pub key: String,
    /// «Фамилия И.» в порядке записи.
    pub authors: Vec<String>,
    /// Пусто — названия нет.
    pub title: String,
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub year: Option<i32>,
    /// Идентификатор вида записи Hayagriva: `article`, `book`, …
    pub kind: String,
    /// Журнал, сборник — название родительской записи.
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub container: Option<String>,
    /// Путь файла источников в vault.
    pub file: String,
    /// Строка записи в файле, с единицы.
    pub line: u32,
}

/// Ошибка чтения файла источников (FR-008). Имя с приставкой: у ссылок 013
/// уже есть свой `SourceProblem`, и в `.d.ts` два типа столкнулись бы.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SourceReadProblem {
    pub file: String,
    /// С единицы; `0` — место неизвестно.
    pub line: u32,
    pub column: u32,
    /// `None` — ошибка уровня файла.
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub key: Option<String>,
    pub kind: SourceReadProblemKind,
}

/// Вид ошибки чтения. `detail` — текст крейта (английский, технический):
/// показывается подробностью под переведённой причиной.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum SourceReadProblemKind {
    Syntax { detail: String },
    Field { field: String, detail: String },
    DuplicateKey,
    NoEntryType,
    FileUnreadable { reason: String },
    FileSyntax { detail: String },
    TooMany { hidden: u32 },
}

/// Итог чтения одного файла: чистая функция текста, без файловой системы.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReading {
    pub sources: Vec<Source>,
    pub problems: Vec<SourceReadProblem>,
    /// Хоть одна запись прочиталась как запись — так опознаётся YAML.
    pub has_entries: bool,
}

impl FileReading {
    /// Сворачивает ошибки сверх предела в одну `TooMany`.
    pub(crate) fn capped(mut self, file: &str) -> Self {
        if self.problems.len() > MAX_PROBLEMS_PER_FILE {
            let hidden = (self.problems.len() - MAX_PROBLEMS_PER_FILE) as u32;
            self.problems.truncate(MAX_PROBLEMS_PER_FILE);
            self.problems.push(SourceReadProblem {
                file: file.to_owned(),
                line: 0,
                column: 0,
                key: None,
                kind: SourceReadProblemKind::TooMany { hidden },
            });
        }
        self
    }
}

/// Почему файл источников не создан.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum CreateSourceFileError {
    AlreadyExists,
    AccessDenied,
    Io,
}

/// Создаёт пустой `references.bib` в корне vault (research.md §5).
pub async fn create_source_file<B: VaultBackend>(
    backend: &mut B,
) -> Result<String, CreateSourceFileError> {
    let path = Path::new(NEW_SOURCE_FILE);
    if backend.access_state() != crate::vault::AccessState::ReadWrite {
        return Err(CreateSourceFileError::AccessDenied);
    }
    if backend.file_mark(path).await.is_some() || backend.read_bytes(path).await.is_ok() {
        return Err(CreateSourceFileError::AlreadyExists);
    }
    backend
        .write_bytes(path, b"")
        .await
        .map_err(|error| match error {
            WriteError::AccessDenied => CreateSourceFileError::AccessDenied,
            _ => CreateSourceFileError::Io,
        })?;
    Ok(NEW_SOURCE_FILE.to_owned())
}

/// Начала строк текста: строка и столбец по байту за логарифм, а не проходом
/// от начала файла — на `.bib` в 10 000 записей разница в секунды (T054).
pub(crate) struct LineIndex<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        let starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(at, _)| at + 1))
            .collect();
        Self { text, starts }
    }

    /// Строка и столбец (в знаках) байтового смещения, с единицы.
    pub(crate) fn at(&self, byte: usize) -> (u32, u32) {
        let byte = floor_char_boundary(self.text, byte.min(self.text.len()));
        let line = self.starts.partition_point(|&start| start <= byte) - 1;
        let column = self.text[self.starts[line]..byte].chars().count() as u32 + 1;
        (line as u32 + 1, column)
    }
}

fn floor_char_boundary(text: &str, mut byte: usize) -> usize {
    while byte > 0 && !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

/// Поля источника из записи Hayagriva — одно правило для обоих форматов.
pub(crate) fn source_of(entry: &hayagriva::Entry, file: &str, line: u32) -> Source {
    let authors = entry
        .authors()
        .map(|people| people.iter().map(person_short).collect())
        .unwrap_or_default();
    let title = entry
        .title()
        .map(|title| title.to_string())
        .unwrap_or_default();
    let year = entry.date().map(|date| date.year).or_else(|| {
        entry
            .parents()
            .iter()
            .find_map(|parent| parent.date().map(|date| date.year))
    });
    let kind = serde_json::to_value(entry.entry_type())
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    let container = entry
        .parents()
        .iter()
        .find_map(|parent| parent.title().map(|title| title.to_string()));
    Source {
        key: entry.key().to_owned(),
        authors,
        title,
        year,
        kind,
        container,
        file: file.to_owned(),
        line,
    }
}

/// «Фамилия И. О.» — фамилия с приставкой и инициалы имени.
fn person_short(person: &hayagriva::types::Person) -> String {
    let family = match &person.prefix {
        Some(prefix) => format!("{prefix} {}", person.name),
        None => person.name.clone(),
    };
    let initials: Vec<String> = person
        .given_name
        .as_deref()
        .unwrap_or_default()
        .split([' ', '-'])
        .filter_map(|part| part.chars().next())
        .map(|first| format!("{first}."))
        .collect();
    if initials.is_empty() {
        family
    } else {
        format!("{family} {}", initials.join(" "))
    }
}
