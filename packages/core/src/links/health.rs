//! Состояние ссылок vault (фича 028): битые ссылки по целям, заметки без
//! входящих и починка ссылок.
//!
//! Сведения берутся из источника графа 025 (`VaultBackend::graph_source`):
//! разрешённые ссылки и вставки из индекса, без чтения файлов (research.md §1).
//! Файлы читаются только у заметок с битыми ссылками — ради строк-примеров.
//! Ссылка здесь — то же, что у индекса, связей 013 и графа: запись `[[…]]` и
//! вставка `#image("…")`.
//!
//! Починка устроена как замена 027: место опознаётся отпечатком текста
//! заметки и номером записи, правка уходит в редактор открытой заметки или
//! пишется со сверкой отпечатка индекса, отмена — `search::replace::replace_undo`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{folder_of, title_of, OpenText};
use crate::elements::edit::TextEdit;
use crate::graph::{embed_sites, link_sites, link_text, resolve_link_target};
use crate::search::replace::{digest, PieceKind, ReplaceNoteOutcome, ReplacePiece, UndoRecord};
use crate::text_offsets::utf16_at;
use crate::vault::backend::{VaultBackend, WriteError};
use crate::vault::{AccessState, FileKind};

/// Сколько строк-примеров показывается у цели. Число ссылок — полное.
pub const MAX_SITES_PER_TARGET: usize = 20;

/// Сколько знаков строки по обе стороны записи ссылки.
const CONTEXT_CHARS: usize = 50;

/// Чем была цель битой ссылки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum TargetKind {
    Note,
    Board,
    /// Вставленный файл `#image`.
    Attachment,
}

/// Одно место битой ссылки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BrokenSite {
    pub source: String,
    pub source_title: String,
    /// Отпечаток текста источника, по которому найдено место.
    pub digest: String,
    /// Порядковый номер записи с этой целью в заметке-источнике, с нуля.
    pub index: u32,
    /// Строка записи, с единицы.
    pub line: u32,
    /// Строка кусками: запись ссылки помечена `Removed`.
    pub pieces: Vec<ReplacePiece>,
}

/// Заметка, ссылающаяся на битую цель, с отпечатком её текста — им сверяется
/// починка (research.md §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BrokenSource {
    pub path: String,
    pub digest: String,
}

/// Цель, на которую ведут битые ссылки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BrokenTarget {
    pub target: String,
    pub title: String,
    pub kind: TargetKind,
    /// Сколько ссылок ведёт на цель.
    pub links: u32,
    /// Из скольких заметок.
    pub sources: u32,
    /// Все заметки-источники — починка группы идёт по ним, а не по примерам.
    pub notes: Vec<BrokenSource>,
    /// Не больше [`MAX_SITES_PER_TARGET`].
    pub sites: Vec<BrokenSite>,
}

/// Вид заметки без входящих.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum OrphanKind {
    Note,
    /// Вход: заметка входящих или заметка дневника (022) — без ссылок по природе.
    Entry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Orphan {
    pub path: String,
    pub title: String,
    /// Папка относительно корня; пустая строка — корень.
    pub folder: String,
    /// Время изменения, мс от эпохи; `0` — неизвестно.
    pub modified_ms: i64,
    /// Размер файла в байтах.
    pub size: u64,
    pub kind: OrphanKind,
}

/// Всё, что показывает экран «Состояние ссылок».
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LinkHealth {
    pub broken: Vec<BrokenTarget>,
    /// Всего битых ссылок — число счётчика в подвале дерева.
    pub broken_links: u32,
    pub orphans: Vec<Orphan>,
    /// Сколько заметок скрыто отметкой «намеренно без ссылок».
    pub intentional: u32,
}

/// Что сделать с битой ссылкой.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum LinkFix {
    /// Перенаправить на существующую заметку или доску; подпись сохраняется.
    Redirect { to: String },
    /// Убрать ссылку, оставив её текст; вставку файла — целиком.
    Unlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LinkFixRequest {
    pub source: String,
    pub target: String,
    pub digest: String,
    pub fix: LinkFix,
    /// Одна запись по номеру; `None` — все записи цели в заметке.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub only: Option<u32>,
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Запись ссылки в тексте: диапазон байтов и куда она ведёт.
struct Site {
    start: usize,
    end: usize,
    target: PathBuf,
    /// `[[цель|подпись]]` — подпись; для вставки `None`.
    label: Option<String>,
    /// Как написана цель.
    raw: String,
    embed: bool,
    /// Встраивание заметки `#embed("…")` (031): весь вызов. Диапазон сайта —
    /// путь в кавычках.
    call: Option<(usize, usize)>,
}

/// Все записи ссылок и вставок заметки по порядку текста.
fn sites_of(source: &Path, text: &str) -> Vec<Site> {
    let mut out: Vec<Site> = link_sites(text)
        .into_iter()
        .filter_map(|site| {
            let target = resolve_link_target(source, &site.raw)?;
            (target != source).then_some(Site {
                start: site.byte_start,
                end: site.byte_end,
                target,
                label: site.label,
                raw: site.raw,
                embed: false,
                call: site.call,
            })
        })
        .collect();
    out.extend(
        embed_sites(source, text)
            .into_iter()
            .map(|(target, start, end)| Site {
                start,
                end,
                target,
                label: None,
                raw: String::new(),
                embed: true,
                call: None,
            }),
    );
    out.sort_by_key(|site| site.start);
    out
}

fn tail_chars(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_owned();
    }
    format!("…{}", text.chars().skip(count - limit).collect::<String>())
}

fn head_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    format!("{}…", text.chars().take(limit).collect::<String>())
}

fn pieces(text: &str, start: usize, end: usize) -> Vec<ReplacePiece> {
    let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = text[end..].find('\n').map_or(text.len(), |at| end + at);
    let mut out = Vec::with_capacity(3);
    let before = tail_chars(&text[line_start..start], CONTEXT_CHARS);
    if !before.is_empty() {
        out.push(ReplacePiece {
            text: before,
            kind: PieceKind::Plain,
        });
    }
    out.push(ReplacePiece {
        text: text[start..end].to_owned(),
        kind: PieceKind::Removed,
    });
    let after = head_chars(&text[end..line_end], CONTEXT_CHARS);
    if !after.is_empty() {
        out.push(ReplacePiece {
            text: after,
            kind: PieceKind::Plain,
        });
    }
    out
}

fn kind_of(target: &Path, embed: bool) -> TargetKind {
    if embed {
        TargetKind::Attachment
    } else if target.extension().and_then(|ext| ext.to_str()) == Some(crate::vault::BOARD_EXTENSION)
    {
        TargetKind::Board
    } else {
        TargetKind::Note
    }
}

/// Состояние ссылок vault одним вызовом.
///
/// `open_texts` — тексты открытых заметок: строки-примеры и отпечатки считаются
/// по ним, починка уйдёт туда же.
pub async fn link_health<B: VaultBackend>(backend: &B, open_texts: &[OpenText]) -> LinkHealth {
    let source = backend.graph_source().await;
    let files = backend.list().await;
    let mut existing: HashSet<String> = files.iter().map(|file| slashed(&file.path)).collect();

    // `list()` знает только заметки и доски; прочие цели (картинки вставок,
    // ссылки на файлы) проверяются по одной — их немного.
    // Список собирается до `await`: итератор с замыканием, живущий через
    // `await`, делает future команды Tauri не `Send`.
    let unknown: Vec<(String, PathBuf)> = source
        .links
        .values()
        .chain(source.embeds.values())
        .flatten()
        .map(|target| (slashed(target), target.clone()))
        .filter(|(key, _)| !existing.contains(key))
        .collect();
    let mut missing: HashSet<String> = HashSet::new();
    for (key, target) in unknown {
        if existing.contains(&key) || missing.contains(&key) {
            continue;
        }
        if backend.file_mark(&target).await.is_some() {
            existing.insert(key);
        } else {
            missing.insert(key);
        }
    }

    // Источники с битыми ссылками или вставками — только их читаем.
    let mut broken_sources: Vec<PathBuf> = Vec::new();
    let mut incoming: HashSet<String> = HashSet::new();
    for (from, targets) in source.links.iter().chain(source.embeds.iter()) {
        for target in targets {
            let target = slashed(target);
            if target != slashed(from) {
                incoming.insert(target.clone());
            }
            if missing.contains(&target) && !broken_sources.contains(from) {
                broken_sources.push(from.clone());
            }
        }
    }
    broken_sources.sort();

    let mut by_target: HashMap<String, BrokenTarget> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for from in &broken_sources {
        let path = slashed(from);
        let open = open_texts
            .iter()
            .find(|open| open.path == path)
            .map(|open| open.text.clone());
        let text = match open {
            Some(text) => text,
            None => match backend.read(from).await {
                Ok(text) => text,
                Err(_) => continue,
            },
        };
        let print = digest(&text);
        let mut counted: HashSet<String> = HashSet::new();
        let mut per_target: HashMap<String, u32> = HashMap::new();
        for site in sites_of(from, &text) {
            let target = slashed(&site.target);
            if existing.contains(&target) {
                continue;
            }
            let entry = by_target.entry(target.clone()).or_insert_with(|| {
                order.push(target.clone());
                BrokenTarget {
                    title: title_of(&target),
                    kind: kind_of(&site.target, site.embed),
                    target: target.clone(),
                    links: 0,
                    sources: 0,
                    notes: Vec::new(),
                    sites: Vec::new(),
                }
            });
            let index = per_target.entry(target.clone()).or_insert(0);
            entry.links += 1;
            if counted.insert(target.clone()) {
                entry.sources += 1;
                entry.notes.push(BrokenSource {
                    path: path.clone(),
                    digest: print.clone(),
                });
            }
            if entry.sites.len() < MAX_SITES_PER_TARGET {
                entry.sites.push(BrokenSite {
                    source: path.clone(),
                    source_title: title_of(&path),
                    digest: print.clone(),
                    index: *index,
                    line: text[..site.start].matches('\n').count() as u32 + 1,
                    pieces: pieces(&text, site.start, site.end),
                });
            }
            *index += 1;
        }
    }

    let settings = crate::vault::vault_settings::read_vault_settings(backend)
        .await
        .unwrap_or_default();
    let capture = crate::capture::capture_settings(settings.capture.as_ref());
    let daily = format!("{}/", capture.daily_folder.trim_end_matches('/'));
    let intentional: HashSet<&str> = settings
        .intentional_orphans
        .iter()
        .map(String::as_str)
        .collect();
    let modified: HashMap<&str, Option<i64>> = source
        .notes
        .iter()
        .map(|note| (note.path.as_str(), note.modified_ms))
        .collect();

    let mut hidden = 0u32;
    let mut orphans: Vec<Orphan> = Vec::new();
    for file in files.iter().filter(|file| file.kind == FileKind::Note) {
        let path = slashed(&file.path);
        if incoming.contains(&path) {
            continue;
        }
        if intentional.contains(path.as_str()) {
            hidden += 1;
            continue;
        }
        let entry = path == capture.inbox || path.starts_with(&daily);
        orphans.push(Orphan {
            title: title_of(&path),
            folder: folder_of(&path),
            modified_ms: modified
                .get(path.as_str())
                .copied()
                .flatten()
                .unwrap_or(file.stamp.mtime_millis),
            size: file.stamp.size,
            kind: if entry {
                OrphanKind::Entry
            } else {
                OrphanKind::Note
            },
            path,
        });
    }
    orphans.sort_by(|a, b| a.path.cmp(&b.path));

    let broken: Vec<BrokenTarget> = order
        .into_iter()
        .filter_map(|target| by_target.remove(&target))
        .collect();
    LinkHealth {
        broken_links: broken.iter().map(|target| target.links).sum(),
        broken,
        orphans,
        intentional: hidden,
    }
}

/// Новый текст после починки и диапазон изменённого куска.
fn fix_text(source: &Path, text: &str, request: &LinkFixRequest) -> Option<(String, usize, usize)> {
    let target = PathBuf::from(&request.target);
    let chosen: Vec<Site> = sites_of(source, text)
        .into_iter()
        .filter(|site| site.target == target)
        .enumerate()
        .filter(|(index, _)| request.only.is_none_or(|only| only as usize == *index))
        .map(|(_, site)| site)
        .collect();
    if chosen.is_empty() {
        return None;
    }
    let redirect = match &request.fix {
        LinkFix::Redirect { to } => Some(link_text(source, Path::new(to))?),
        LinkFix::Unlink => None,
    };
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    let first = chosen.first()?.start;
    let mut last = first;
    for site in &chosen {
        // Вставка `#image(...)` уходит вместе со знаком `#` перед вызовом.
        // Встраивание заметки (031): перенаправление меняет только путь в
        // кавычках — `#раздел` остаётся; «убрать» удаляет весь вызов.
        if let Some((call_start, call_end)) = site.call {
            match &redirect {
                Some(link) => {
                    out.push_str(&text[cursor..site.start]);
                    out.push_str(link.trim_start_matches("[[").trim_end_matches("]]"));
                    cursor = site.end;
                    last = site.end;
                }
                None => {
                    out.push_str(&text[cursor..call_start]);
                    cursor = call_end;
                    last = call_end;
                }
            }
            continue;
        }
        let start = if site.embed && site.start > 0 && text.as_bytes()[site.start - 1] == b'#' {
            site.start - 1
        } else {
            site.start
        };
        out.push_str(&text[cursor..start]);
        let replacement = match (&redirect, site.embed) {
            (Some(link), false) => match &site.label {
                // `[[новая цель|подпись]]`: подпись остаётся.
                Some(label) => format!("{}|{label}]]", link.trim_end_matches("]]")),
                None => link.clone(),
            },
            (None, false) => site.label.clone().unwrap_or_else(|| site.raw.clone()),
            // Вставку перенаправлять некуда: только убрать.
            (_, true) => String::new(),
        };
        out.push_str(&replacement);
        cursor = site.end;
        last = site.end;
    }
    out.push_str(&text[cursor..]);
    let first = match (chosen[0].call, &redirect) {
        (Some((call_start, _)), None) => call_start,
        _ if chosen[0].embed && first > 0 && text.as_bytes()[first - 1] == b'#' => first - 1,
        _ => first,
    };
    Some((out, first, last))
}

/// Чинит ссылки одной заметки. Исходы и отмена — как у замены 027.
pub async fn fix_links<B: VaultBackend>(
    backend: &mut B,
    request: &LinkFixRequest,
    editor_text: Option<&str>,
) -> ReplaceNoteOutcome {
    if backend.access_state() != AccessState::ReadWrite {
        return ReplaceNoteOutcome::ReadOnly;
    }
    let source = Path::new(&request.source);

    if let Some(text) = editor_text {
        if digest(text) != request.digest {
            return ReplaceNoteOutcome::Stale;
        }
        let Some((new_text, first, last)) = fix_text(source, text, request) else {
            return ReplaceNoteOutcome::Stale;
        };
        let new_last = new_text.len() - (text.len() - last);
        return ReplaceNoteOutcome::EditorEdit {
            edit: TextEdit {
                from: utf16_at(text, first),
                to: utf16_at(text, last),
                insert: new_text[first..new_last].to_owned(),
            },
            undo: UndoRecord {
                path: request.source.clone(),
                before: text.to_owned(),
                after_digest: digest(&new_text),
            },
        };
    }

    if backend.has_unsaved_edits(source) {
        return ReplaceNoteOutcome::UnsavedElsewhere;
    }
    let folder = source.parent().unwrap_or_else(|| Path::new(""));
    let Some(stamp) = backend
        .list_folder(folder)
        .await
        .into_iter()
        .find(|note| slashed(&note.path) == request.source)
        .map(|note| note.stamp)
    else {
        return ReplaceNoteOutcome::Stale;
    };
    let Ok(text) = backend.read(source).await else {
        return ReplaceNoteOutcome::Stale;
    };
    if digest(&text) != request.digest {
        return ReplaceNoteOutcome::Stale;
    }
    let Some((new_text, _, _)) = fix_text(source, &text, request) else {
        return ReplaceNoteOutcome::Stale;
    };
    match backend.write(source, &new_text, Some(stamp)).await {
        Ok(()) => ReplaceNoteOutcome::Written {
            undo: UndoRecord {
                path: request.source.clone(),
                before: text,
                after_digest: digest(&new_text),
            },
        },
        Err(WriteError::Conflict) => ReplaceNoteOutcome::Stale,
        Err(WriteError::AccessDenied) => ReplaceNoteOutcome::ReadOnly,
        Err(WriteError::Io) => ReplaceNoteOutcome::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(target: &str, fix: LinkFix, only: Option<u32>) -> LinkFixRequest {
        LinkFixRequest {
            source: "Диссертация/Введение.typ".into(),
            target: target.into(),
            digest: String::new(),
            fix,
            only,
        }
    }

    const SOURCE: &str = "Диссертация/Введение.typ";

    #[test]
    fn redirect_keeps_the_label() {
        let text = "по [[Методика|методике]] и [[Методика]].";
        let (out, _, _) = fix_text(
            Path::new(SOURCE),
            text,
            &request(
                "Диссертация/Методика.typ",
                LinkFix::Redirect {
                    to: "Диссертация/Методология.typ".into(),
                },
                None,
            ),
        )
        .unwrap();
        assert_eq!(out, "по [[Методология|методике]] и [[Методология]].");
    }

    #[test]
    fn unlink_leaves_the_text_and_only_touches_one() {
        let text = "по [[Методика|методике]] и [[Методика]].";
        let (out, _, _) = fix_text(
            Path::new(SOURCE),
            text,
            &request("Диссертация/Методика.typ", LinkFix::Unlink, Some(1)),
        )
        .unwrap();
        assert_eq!(out, "по [[Методика|методике]] и Методика.");
    }

    #[test]
    fn a_note_embed_is_redirected_keeping_its_section_or_removed() {
        // Фича 031: путь в кавычках меняется, `#раздел` остаётся; «убрать» —
        // весь вызов.
        let text = "до #embed(\"Методика#Выборка\") после";
        let redirect = request(
            "Диссертация/Методика.typ",
            LinkFix::Redirect {
                to: "Диссертация/Методология.typ".into(),
            },
            None,
        );
        let (out, _, _) = fix_text(Path::new(SOURCE), text, &redirect).unwrap();
        assert_eq!(out, "до #embed(\"Методология#Выборка\") после");
        let unlink = request("Диссертация/Методика.typ", LinkFix::Unlink, None);
        let (out, _, _) = fix_text(Path::new(SOURCE), text, &unlink).unwrap();
        assert_eq!(out, "до  после");
    }

    #[test]
    fn an_embed_is_removed_with_its_hash() {
        let text = "до\n#image(\"нет.png\")\nпосле";
        let (out, _, _) = fix_text(
            Path::new(SOURCE),
            text,
            &request("Диссертация/нет.png", LinkFix::Unlink, None),
        )
        .unwrap();
        assert_eq!(out, "до\n\nпосле");
    }
}
