//! Замена по vault (фича 027): предпросмотр, замена в заметке, отмена.
//!
//! Совпадения ищет тот же [`Matcher`], что и расширенный поиск: два
//! сопоставителя разошлись бы в регистре и границах (research.md §1). Через
//! границу ходят куски строк и номера совпадений, а не смещения: смещение
//! зависит от кодировки (тот же довод, что у поиска).
//!
//! Совпадение опознаётся парой «отпечаток текста заметки + номер по порядку»
//! (research.md §2): применение перечитывает текст, сверяет отпечаток и только
//! тогда меняет отмеченные места. Порядок решений в [`replace_in_note`] — как у
//! «Связать» 013 (`links::link_mention`): редактор открытой заметки, затем
//! несохранённое вне редактора, затем запись со сверкой отпечатка индекса.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{folder_of, Filters, Matcher, SearchError, SearchQuery};
use crate::elements::edit::TextEdit;
use crate::links::OpenText;
use crate::text_offsets::utf16_at;
use crate::vault::backend::{ReadError, VaultBackend, WriteError};
use crate::vault::{AccessState, FileKind};

/// Больше совпадений предпросмотр не показывает: список такой длины никто не
/// просмотрит, а применять непросмотренное нельзя (FR-008).
pub const MAX_REPLACE_MATCHES: usize = 10_000;

/// Сколько знаков строки показывается по обе стороны совпадения.
const CONTEXT_CHARS: usize = 60;

/// Что и на что меняем.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ReplaceQuery {
    pub search: SearchQuery,
    /// В режиме регулярного выражения понимает `$1` и `${имя}`; в текстовом
    /// знак `$` буквальный.
    pub replacement: String,
}

/// Кусок строки предпросмотра.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum PieceKind {
    Plain,
    /// Было — уходит.
    Removed,
    /// Станет — приходит.
    Inserted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ReplacePiece {
    pub text: String,
    pub kind: PieceKind,
}

/// Одно совпадение с его строкой «было → станет».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ReplaceMatch {
    /// Номер совпадения в заметке по порядку, с нуля.
    pub index: u32,
    /// Строка начала совпадения, с единицы.
    pub line: u32,
    pub pieces: Vec<ReplacePiece>,
}

/// Заметка с совпадениями.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ReplaceNote {
    pub path: String,
    pub name: String,
    /// Папка относительно корня vault; пустая строка — корень.
    pub folder: String,
    /// Отпечаток текста, по которому построен предпросмотр.
    pub digest: String,
    /// Текст взят из редактора открытой заметки: замена уйдёт в редактор.
    pub open_in_editor: bool,
    pub matches: Vec<ReplaceMatch>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ReplacePreview {
    pub notes: Vec<ReplaceNote>,
    pub total_matches: u32,
    pub total_notes: u32,
}

/// Почему предпросмотра нет.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ReplaceError {
    BadPattern {
        message: String,
    },
    /// Совпадений больше [`MAX_REPLACE_MATCHES`] — названо сколько (FR-008).
    TooMany {
        count: u32,
    },
}

impl From<SearchError> for ReplaceError {
    fn from(error: SearchError) -> Self {
        match error {
            SearchError::BadPattern { message } => ReplaceError::BadPattern { message },
        }
    }
}

/// Что заменить в одной заметке.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ReplaceNoteRequest {
    pub query: ReplaceQuery,
    pub path: String,
    /// Отпечаток из предпросмотра.
    pub digest: String,
    /// Номера отмеченных совпадений.
    pub selected: Vec<u32>,
}

/// Чем вернуть заметку к тексту до замены (research.md §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct UndoRecord {
    pub path: String,
    pub before: String,
    /// Отпечаток текста сразу после замены: отмена пишет, только если текст с
    /// тех пор не менялся.
    pub after_digest: String,
}

/// Исход замены в заметке. Отказ — вариант значения: интерфейс называет
/// каждый по-своему.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ReplaceNoteOutcome {
    /// Файл записан.
    Written {
        undo: UndoRecord,
    },
    /// Заметка открыта: файл не тронут, правку применяет редактор — одной
    /// единицей отмены.
    EditorEdit {
        edit: TextEdit,
        undo: UndoRecord,
    },
    /// Текст изменился после предпросмотра — ничего не записано (FR-007).
    Stale,
    /// У заметки несохранённые правки вне редактора.
    UnsavedElsewhere,
    ReadOnly,
    Failed,
    /// Ни одно совпадение не отмечено.
    NothingSelected,
}

/// Исход отмены в заметке.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum UndoOutcome {
    Restored,
    EditorEdit {
        edit: TextEdit,
    },
    /// Заметку изменили после замены — не тронута (FR-005).
    ChangedSince,
    /// Заметки больше нет.
    Missing,
    UnsavedElsewhere,
    ReadOnly,
    Failed,
}

/// Отпечаток текста — тот, что сверяется между вызовами.
pub fn digest(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

/// Совпадение в тексте: байтовый диапазон и текст замены.
struct Hit {
    start: usize,
    end: usize,
    with: String,
}

/// Все совпадения текста по порядку. Пустые совпадения пропускаются, как в
/// поиске: заменять «ничто» пользователь не просил.
fn hits(text: &str, matcher: &Matcher, query: &ReplaceQuery) -> Vec<Hit> {
    matcher
        .0
        .captures_iter(text)
        .filter_map(|caps| {
            let whole = caps.get(0)?;
            if whole.start() == whole.end() {
                return None;
            }
            let mut with = String::new();
            if query.search.regex {
                caps.expand(&query.replacement, &mut with);
            } else {
                with.push_str(&query.replacement);
            }
            Some(Hit {
                start: whole.start(),
                end: whole.end(),
                with,
            })
        })
        .collect()
}

/// Не больше `limit` знаков с конца строки, с многоточием, если обрезано.
fn tail_chars(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_owned();
    }
    let skip = count - limit;
    format!("…{}", text.chars().skip(skip).collect::<String>())
}

fn head_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    format!("{}…", text.chars().take(limit).collect::<String>())
}

/// Строка совпадения кусками «до — было — станет — после».
fn pieces(text: &str, hit: &Hit) -> Vec<ReplacePiece> {
    let line_start = text[..hit.start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = text[hit.end..]
        .find('\n')
        .map_or(text.len(), |at| hit.end + at);
    let mut out = Vec::with_capacity(4);
    let before = tail_chars(&text[line_start..hit.start], CONTEXT_CHARS);
    if !before.is_empty() {
        out.push(ReplacePiece {
            text: before,
            kind: PieceKind::Plain,
        });
    }
    out.push(ReplacePiece {
        text: text[hit.start..hit.end].to_owned(),
        kind: PieceKind::Removed,
    });
    if !hit.with.is_empty() {
        out.push(ReplacePiece {
            text: hit.with.clone(),
            kind: PieceKind::Inserted,
        });
    }
    let after = head_chars(&text[hit.end..line_end], CONTEXT_CHARS);
    if !after.is_empty() {
        out.push(ReplacePiece {
            text: after,
            kind: PieceKind::Plain,
        });
    }
    out
}

/// Предпросмотр: каждое совпадение каждой заметки, прошедшей фильтры поиска.
///
/// Открытые заметки читаются из `open_texts`: отпечаток и места тогда считаны
/// по тексту редактора, а не по диску, — замена уйдёт туда же.
pub async fn replace_preview<B: VaultBackend>(
    backend: &B,
    query: &ReplaceQuery,
    open_texts: &[OpenText],
) -> Result<ReplacePreview, ReplaceError> {
    let Some(matcher) = Matcher::build(&query.search)? else {
        return Ok(ReplacePreview::default());
    };
    let filters = Filters::load(backend, &query.search).await;

    let mut preview = ReplacePreview::default();
    let mut total: usize = 0;
    for note in backend.list().await {
        if note.kind != FileKind::Note {
            continue;
        }
        let path = note.path.to_string_lossy().replace('\\', "/");
        let open = open_texts.iter().find(|open| open.path == path);
        let text = match open {
            Some(open) => open.text.clone(),
            None => match backend.read(&note.path).await {
                Ok(text) => text,
                // Нечитаемая заметка не роняет предпросмотр — как в поиске.
                Err(_) => continue,
            },
        };
        let found = hits(&text, &matcher, query);
        if found.is_empty() || !filters.passes(&path, Some(true), query.search.mode) {
            continue;
        }
        total += found.len();
        if total > MAX_REPLACE_MATCHES {
            // Считаем дальше, чтобы назвать число, но кусков уже не режем.
            continue;
        }
        let mut line = 1u32;
        let mut cursor = 0usize;
        let matches = found
            .iter()
            .enumerate()
            .map(|(index, hit)| {
                line += text[cursor..hit.start].matches('\n').count() as u32;
                cursor = hit.start;
                ReplaceMatch {
                    index: index as u32,
                    line,
                    pieces: pieces(&text, hit),
                }
            })
            .collect();
        preview.notes.push(ReplaceNote {
            folder: folder_of(&path),
            name: crate::links::title_of(&path),
            digest: digest(&text),
            open_in_editor: open.is_some(),
            path,
            matches,
        });
    }
    if total > MAX_REPLACE_MATCHES {
        return Err(ReplaceError::TooMany {
            count: total.min(u32::MAX as usize) as u32,
        });
    }
    preview.total_matches = total as u32;
    preview.total_notes = preview.notes.len() as u32;
    Ok(preview)
}

/// Новый текст: отмеченные совпадения заменены, прочее байт в байт прежнее.
/// `None` — номер вне списка совпадений (текст уже не тот).
fn rewrite(text: &str, found: &[Hit], selected: &[u32]) -> Option<(String, usize, usize)> {
    let mut chosen: Vec<&Hit> = Vec::with_capacity(selected.len());
    for index in selected {
        chosen.push(found.get(*index as usize)?);
    }
    chosen.sort_by_key(|hit| hit.start);
    chosen.dedup_by_key(|hit| hit.start);
    let first = chosen.first()?.start;
    let last = chosen.last()?.end;
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for hit in &chosen {
        out.push_str(&text[cursor..hit.start]);
        out.push_str(&hit.with);
        cursor = hit.end;
    }
    out.push_str(&text[cursor..]);
    Some((out, first, last))
}

/// Отпечаток файла из индекса бэкенда — им сверяется запись (как в 013).
async fn stamp_of<B: VaultBackend>(backend: &B, path: &str) -> Option<crate::stamp::FileStamp> {
    let source = Path::new(path);
    let folder = source.parent().unwrap_or_else(|| Path::new(""));
    backend
        .list_folder(folder)
        .await
        .into_iter()
        .find(|note| note.path.to_string_lossy().replace('\\', "/") == path)
        .map(|note| note.stamp)
}

/// Заменяет отмеченные совпадения одной заметки.
///
/// `editor_text` — `Some`, если заметка открыта: тогда на диск не пишется
/// ничего, а одна правка от первого до последнего отмеченного места уходит в
/// редактор (FR-004).
pub async fn replace_in_note<B: VaultBackend>(
    backend: &mut B,
    request: &ReplaceNoteRequest,
    editor_text: Option<&str>,
) -> ReplaceNoteOutcome {
    if backend.access_state() != AccessState::ReadWrite {
        return ReplaceNoteOutcome::ReadOnly;
    }
    if request.selected.is_empty() {
        return ReplaceNoteOutcome::NothingSelected;
    }
    let Ok(Some(matcher)) = Matcher::build(&request.query.search) else {
        return ReplaceNoteOutcome::Failed;
    };

    if let Some(text) = editor_text {
        if digest(text) != request.digest {
            return ReplaceNoteOutcome::Stale;
        }
        let found = hits(text, &matcher, &request.query);
        let Some((new_text, first, last)) = rewrite(text, &found, &request.selected) else {
            return ReplaceNoteOutcome::Stale;
        };
        // Конец заменённого куска в новом тексте: длина хвоста после `last`
        // в обоих текстах одна и та же.
        let new_last = new_text.len() - (text.len() - last);
        return ReplaceNoteOutcome::EditorEdit {
            edit: TextEdit {
                from: utf16_at(text, first),
                to: utf16_at(text, last),
                insert: new_text[first..new_last].to_owned(),
            },
            undo: UndoRecord {
                path: request.path.clone(),
                before: text.to_owned(),
                after_digest: digest(&new_text),
            },
        };
    }

    let source = Path::new(&request.path);
    if backend.has_unsaved_edits(source) {
        return ReplaceNoteOutcome::UnsavedElsewhere;
    }
    let Some(stamp) = stamp_of(backend, &request.path).await else {
        return ReplaceNoteOutcome::Stale;
    };
    let Ok(text) = backend.read(source).await else {
        return ReplaceNoteOutcome::Stale;
    };
    if digest(&text) != request.digest {
        return ReplaceNoteOutcome::Stale;
    }
    let found = hits(&text, &matcher, &request.query);
    let Some((new_text, _, _)) = rewrite(&text, &found, &request.selected) else {
        return ReplaceNoteOutcome::Stale;
    };
    match backend.write(source, &new_text, Some(stamp)).await {
        Ok(()) => ReplaceNoteOutcome::Written {
            undo: UndoRecord {
                path: request.path.clone(),
                before: text,
                after_digest: digest(&new_text),
            },
        },
        Err(WriteError::Conflict) => ReplaceNoteOutcome::Stale,
        Err(WriteError::AccessDenied) => ReplaceNoteOutcome::ReadOnly,
        Err(WriteError::Io) => ReplaceNoteOutcome::Failed,
    }
}

/// Наименьшая правка, превращающая `from` в `to`: общий начальный и конечный
/// куски не трогаются — каретка и история редактора вне замены остаются.
fn minimal_edit(from: &str, to: &str) -> TextEdit {
    let prefix = from
        .char_indices()
        .zip(to.chars())
        .find(|((_, a), b)| a != b)
        .map_or(from.len().min(to.len()), |((at, _), _)| at);
    // Не заходим за общий начальный кусок ни в одном из текстов.
    let max_suffix = from.len().min(to.len()) - prefix;
    let mut suffix = 0;
    for (a, b) in from[prefix..].chars().rev().zip(to[prefix..].chars().rev()) {
        if a != b || suffix + a.len_utf8() > max_suffix {
            break;
        }
        suffix += a.len_utf8();
    }
    TextEdit {
        from: utf16_at(from, prefix),
        to: utf16_at(from, from.len() - suffix),
        insert: to[prefix..to.len() - suffix].to_owned(),
    }
}

/// Отменяет замену в одной заметке, если её с тех пор не меняли (FR-005).
pub async fn replace_undo<B: VaultBackend>(
    backend: &mut B,
    record: &UndoRecord,
    editor_text: Option<&str>,
) -> UndoOutcome {
    if backend.access_state() != AccessState::ReadWrite {
        return UndoOutcome::ReadOnly;
    }
    if let Some(text) = editor_text {
        if digest(text) != record.after_digest {
            return UndoOutcome::ChangedSince;
        }
        return UndoOutcome::EditorEdit {
            edit: minimal_edit(text, &record.before),
        };
    }
    let source = Path::new(&record.path);
    if backend.has_unsaved_edits(source) {
        return UndoOutcome::UnsavedElsewhere;
    }
    let text = match backend.read(source).await {
        Ok(text) => text,
        Err(ReadError::NotFound) => return UndoOutcome::Missing,
        Err(_) => return UndoOutcome::Failed,
    };
    if digest(&text) != record.after_digest {
        return UndoOutcome::ChangedSince;
    }
    let Some(stamp) = stamp_of(backend, &record.path).await else {
        return UndoOutcome::Missing;
    };
    match backend.write(source, &record.before, Some(stamp)).await {
        Ok(()) => UndoOutcome::Restored,
        Err(WriteError::Conflict) => UndoOutcome::ChangedSince,
        Err(WriteError::AccessDenied) => UndoOutcome::ReadOnly,
        Err(WriteError::Io) => UndoOutcome::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(text: &str, regex: bool, replacement: &str) -> ReplaceQuery {
        ReplaceQuery {
            search: SearchQuery {
                text: text.to_owned(),
                regex,
                ..SearchQuery::default()
            },
            replacement: replacement.to_owned(),
        }
    }

    fn found(text: &str, query: &ReplaceQuery) -> Vec<Hit> {
        let matcher = Matcher::build(&query.search).unwrap().unwrap();
        hits(text, &matcher, query)
    }

    #[test]
    fn groups_expand_only_in_regex_mode() {
        let regex = query(r"(\w+)ов\b", true, "${1}ова");
        assert_eq!(found("Иванов и Петров", &regex)[1].with, "Петрова");
        let plain = query("ов", false, "$1");
        assert_eq!(found("Иванов", &plain)[0].with, "$1");
    }

    #[test]
    fn rewrite_touches_only_the_selected_hits() {
        let text = "а б а в а";
        let q = query("а", false, "Я");
        let (out, first, last) = rewrite(text, &found(text, &q), &[0, 2]).unwrap();
        assert_eq!(out, "Я б а в Я");
        assert_eq!((first, last), (0, text.len()));
        assert!(rewrite(text, &found(text, &q), &[7]).is_none());
    }

    #[test]
    fn pieces_show_before_and_after_on_the_line() {
        let text = "первая\nруководитель — Иванов А. П.\nтретья";
        let q = query("Иванов", false, "Иванова");
        let hit = &found(text, &q)[0];
        let kinds: Vec<(String, PieceKind)> = pieces(text, hit)
            .into_iter()
            .map(|piece| (piece.text, piece.kind))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("руководитель — ".to_owned(), PieceKind::Plain),
                ("Иванов".to_owned(), PieceKind::Removed),
                ("Иванова".to_owned(), PieceKind::Inserted),
                (" А. П.".to_owned(), PieceKind::Plain),
            ]
        );
    }

    #[test]
    fn minimal_edit_keeps_common_ends_and_counts_utf16() {
        let edit = minimal_edit("😀 Иванова тут", "😀 Иванов тут");
        assert_eq!(edit.insert, "");
        // 😀 — две единицы UTF-16, затем пробел и «Иванов».
        assert_eq!((edit.from, edit.to), (2 + 1 + 6, 2 + 1 + 7));
        let same = minimal_edit("abc", "abc");
        assert_eq!((same.from, same.to, same.insert.as_str()), (3, 3, ""));
        let grow = minimal_edit("aa", "aaa");
        assert_eq!((grow.from, grow.to, grow.insert.as_str()), (2, 2, "a"));
    }
}
