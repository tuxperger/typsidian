//! Индекс «кто цитирует источник» (FR-005, SC-004, research.md §8, §9).
//!
//! Машина состояний в ядре: проход по заметкам порциями, отмена сохраняет
//! собранное, повторный проход пропускает заметки с прежним отпечатком. После
//! прохода индекс правится по событиям vault одной заметкой. Индекс отражает
//! диск, как граф ссылок; живёт только в памяти (принцип IX).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::search::{HitLine, LineSegment};
use crate::stamp::FileStamp;
use crate::structure::{parse, SourcePlace};
use crate::vault::backend::VaultBackend;
use crate::vault::index::Note;
use crate::vault::{classify, FileKind};

/// Фаза прохода. Имя с приставкой: у упоминаний 013 есть свой `ScanPhase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum CitationScanPhase {
    #[default]
    Idle,
    Running,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CitationScanView {
    pub phase: CitationScanPhase,
    pub processed: u32,
    pub total: u32,
}

/// Место цитаты для перехода (`revealPlace`, 013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CitationPlace {
    pub line: u32,
    pub column: u32,
}

/// Заметка, цитирующая источник.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CitingNote {
    pub path: String,
    pub name: String,
    pub lines: Vec<HitLine>,
    pub places: Vec<CitationPlace>,
}

/// Процитированный ключ, которого нет ни в одном файле источников.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MissingKey {
    pub key: String,
    pub notes: u32,
    pub first: CitationPlace,
    pub first_path: String,
}

/// Цитата в заметке: ключ, место и строка текста вокруг — для фрагмента.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cited {
    key: String,
    /// Записано вызовом `cite(…)` — цитата всегда; иначе `@ключ`.
    call: bool,
    place: SourcePlace,
    line_text: String,
    /// Цитата внутри строки, в знаках строки.
    span: (usize, usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NoteCitations {
    stamp: FileStamp,
    name: String,
    citations: Vec<Cited>,
    bibliography_paths: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CitationIndex {
    notes: BTreeMap<String, NoteCitations>,
    queue: VecDeque<(String, FileStamp)>,
    phase: CitationScanPhase,
    processed: u32,
    total: u32,
}

/// Строки фрагмента не длиннее этого — строка-абзац обрезается вокруг цитаты.
const LINE_WINDOW: usize = 160;

impl CitationIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn phase(&self) -> CitationScanPhase {
        self.phase
    }

    /// Начинает проход: в очередь идут заметки Typst с новым отпечатком.
    pub fn scan_start(&mut self, notes: &[Note]) -> CitationScanView {
        let present: BTreeSet<String> = notes.iter().map(|note| slash(&note.path)).collect();
        self.notes.retain(|path, _| present.contains(path));
        self.queue = notes
            .iter()
            .filter(|note| classify(&note.path) == FileKind::Note)
            .filter(|note| {
                self.notes
                    .get(&slash(&note.path))
                    .is_none_or(|known| known.stamp.content_hash != note.stamp.content_hash)
            })
            .map(|note| (slash(&note.path), note.stamp.clone()))
            .collect();
        self.processed = 0;
        self.total = self.queue.len() as u32;
        self.phase = if self.queue.is_empty() {
            CitationScanPhase::Done
        } else {
            CitationScanPhase::Running
        };
        self.view()
    }

    /// Читает не больше `max_notes` заметок; вне `Running` — текущий вид.
    pub async fn scan_step<B: VaultBackend>(
        &mut self,
        backend: &B,
        max_notes: u32,
    ) -> CitationScanView {
        if self.phase != CitationScanPhase::Running {
            return self.view();
        }
        for _ in 0..max_notes.max(1) {
            let Some((path, stamp)) = self.queue.pop_front() else {
                break;
            };
            // Нечитаемая заметка пропускается и считается обработанной.
            if let Ok(text) = backend.read(Path::new(&path)).await {
                self.notes
                    .insert(path.clone(), read_note(&path, stamp, &text));
            }
            self.processed += 1;
        }
        if self.queue.is_empty() {
            self.phase = CitationScanPhase::Done;
        }
        self.view()
    }

    /// Отмена: собранное остаётся, индекс помечен частичным.
    pub fn scan_cancel(&mut self) -> CitationScanView {
        if self.phase == CitationScanPhase::Running {
            self.queue.clear();
            self.phase = CitationScanPhase::Cancelled;
        }
        self.view()
    }

    pub fn view(&self) -> CitationScanView {
        CitationScanView {
            phase: self.phase,
            processed: self.processed,
            total: self.total,
        }
    }

    /// Перечитать одну заметку — после правки или создания. До первого
    /// прохода индекс не ведётся: читать vault ради закрытой панели незачем.
    pub async fn note_changed<B: VaultBackend>(&mut self, backend: &B, path: &str) {
        if self.phase == CitationScanPhase::Idle || classify(Path::new(path)) != FileKind::Note {
            return;
        }
        match backend.read(Path::new(path)).await {
            Ok(text) => {
                // Время изменения здесь неизвестно; сравнение идёт по хэшу.
                let stamp = FileStamp::from_content(0, text.as_bytes());
                self.notes
                    .insert(path.to_owned(), read_note(path, stamp, &text));
            }
            Err(_) => {
                self.notes.remove(path);
            }
        }
    }

    pub fn note_removed(&mut self, path: &str) {
        self.notes.remove(path);
    }

    pub fn note_renamed(&mut self, from: &str, to: &str) {
        if let Some(mut moved) = self.notes.remove(from) {
            moved.name = note_name(to);
            self.notes.insert(to.to_owned(), moved);
        }
    }

    /// Сколько раз ключ процитирован во всех заметках.
    pub fn count(&self, key: &str) -> u32 {
        self.notes
            .values()
            .flat_map(|note| note.citations.iter())
            .filter(|cited| cited.key == key)
            .count() as u32
    }

    /// Число цитат каждого ключа за один проход — для выдачи поиска: `count`
    /// на каждый из 10 000 источников обходил бы все заметки заново (T054).
    pub fn counts(&self) -> std::collections::HashMap<&str, u32> {
        let mut counts = std::collections::HashMap::new();
        for cited in self.notes.values().flat_map(|note| note.citations.iter()) {
            *counts.entry(cited.key.as_str()).or_insert(0) += 1;
        }
        counts
    }

    /// Заметки, цитирующие ключ, в порядке пути, с фрагментами.
    pub fn citing(&self, key: &str) -> Vec<CitingNote> {
        self.notes
            .iter()
            .filter_map(|(path, note)| {
                let found: Vec<&Cited> = note
                    .citations
                    .iter()
                    .filter(|cited| cited.key == key)
                    .collect();
                if found.is_empty() {
                    return None;
                }
                Some(CitingNote {
                    path: path.clone(),
                    name: note.name.clone(),
                    lines: found.iter().map(|cited| hit_line(cited)).collect(),
                    places: found
                        .iter()
                        .map(|cited| CitationPlace {
                            line: cited.place.line,
                            column: cited.place.column,
                        })
                        .collect(),
                })
            })
            .collect()
    }

    /// Процитированные ключи, которых нет в библиотеке. `@имя` попадает сюда,
    /// только если в vault есть файлы источников (research.md §10).
    pub fn missing_keys(&self, is_key: &dyn Fn(&str) -> bool, has_files: bool) -> Vec<MissingKey> {
        let mut missing: BTreeMap<String, MissingKey> = BTreeMap::new();
        for (path, note) in &self.notes {
            let mut counted = BTreeSet::new();
            for cited in &note.citations {
                if is_key(&cited.key) || (!cited.call && !has_files) {
                    continue;
                }
                let entry = missing
                    .entry(cited.key.clone())
                    .or_insert_with(|| MissingKey {
                        key: cited.key.clone(),
                        notes: 0,
                        first: CitationPlace {
                            line: cited.place.line,
                            column: cited.place.column,
                        },
                        first_path: path.clone(),
                    });
                if counted.insert(cited.key.clone()) {
                    entry.notes += 1;
                }
            }
        }
        missing.into_values().collect()
    }

    /// Пути `bibliography(…)` всех заметок — для опознания YAML.
    pub fn referenced(&self) -> BTreeSet<String> {
        self.notes
            .values()
            .flat_map(|note| note.bibliography_paths.iter().cloned())
            .collect()
    }
}

fn read_note(path: &str, stamp: FileStamp, text: &str) -> NoteCitations {
    let structure = parse(text);
    // `@имя`, совпавшее с меткой самой заметки, — ссылка, а не цитата.
    let labels: BTreeSet<&str> = structure
        .labels
        .iter()
        .map(|label| label.name.as_str())
        .collect();
    let lines: Vec<&str> = text.lines().collect();
    let mut citations: Vec<Cited> = structure
        .cites
        .iter()
        .map(|site| cited(&lines, text, &site.key, true, site.place))
        .chain(
            structure
                .refs
                .iter()
                .filter(|site| !labels.contains(site.target.as_str()))
                .map(|site| cited(&lines, text, &site.target, false, site.place)),
        )
        .collect();
    citations.sort_by_key(|cited| cited.place.from);
    let folder = Path::new(path).parent().unwrap_or(Path::new(""));
    NoteCitations {
        stamp,
        name: note_name(path),
        citations,
        bibliography_paths: structure
            .bibliographies
            .iter()
            .flat_map(|site| site.paths.iter())
            .map(|raw| super::cite::resolve(folder, raw))
            .collect(),
    }
}

fn cited(lines: &[&str], text: &str, key: &str, call: bool, place: SourcePlace) -> Cited {
    let line_text = lines
        .get(place.line.saturating_sub(1) as usize)
        .copied()
        .unwrap_or_default();
    let start = place.column.saturating_sub(1) as usize;
    // Длина цитаты в знаках: место дано в UTF-16, текст строки — в знаках.
    let width = text
        .encode_utf16()
        .skip(place.from as usize)
        .take((place.to - place.from) as usize)
        .collect::<Vec<u16>>();
    let width = String::from_utf16_lossy(&width).chars().count();
    Cited {
        key: key.to_owned(),
        call,
        place,
        line_text: line_text.to_owned(),
        span: (start, start + width),
    }
}

/// Строка-фрагмент с подсвеченной цитатой; длинная строка обрезается вокруг.
fn hit_line(cited: &Cited) -> HitLine {
    let chars: Vec<char> = cited.line_text.chars().collect();
    let (from, to) = (cited.span.0.min(chars.len()), cited.span.1.min(chars.len()));
    let left = from.saturating_sub(LINE_WINDOW / 2);
    let right = (to + LINE_WINDOW / 2).min(chars.len());
    let piece = |a: usize, b: usize| chars[a..b].iter().collect::<String>();
    let mut segments = Vec::new();
    let before = piece(left, from);
    if !before.is_empty() {
        segments.push(LineSegment {
            text: before,
            highlighted: false,
        });
    }
    segments.push(LineSegment {
        text: piece(from, to),
        highlighted: true,
    });
    let after = piece(to, right);
    if !after.is_empty() {
        segments.push(LineSegment {
            text: after,
            highlighted: false,
        });
    }
    HitLine {
        number: cited.place.line as usize,
        segments,
        is_match: true,
    }
}

fn note_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn slash(path: &PathBuf) -> String {
    path.to_string_lossy().replace('\\', "/")
}
