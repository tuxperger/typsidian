use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// `FileStamp` живёт в `crate::stamp` — он нужен не только vault, но и
/// отслеживанию конфига режима редактирования (research.md §8 фичи 004).
/// Ре-экспорт оставлен, чтобы существующий код и тесты, ссылающиеся на
/// `vault::index::FileStamp`, продолжали работать без правок.
pub use crate::stamp::FileStamp;

/// Заметка внутри vault (data-model.md «Заметка»).
///
/// «Заметка» здесь — запись дерева: с фичей 002 это и текстовая заметка, и доска
/// (FR-004 фичи 002), а различие несёт [`kind`](Note::kind).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Note {
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub path: PathBuf,
    pub name: String,
    pub stamp: FileStamp,
    pub has_unsaved_edits: bool,
    /// Чем является файл (фича 002, `vault::classify`).
    ///
    /// Приходит вместе с записью, а не спрашивается отдельным вызовом: вид нужен
    /// дереву на каждую отрисовку, и на desktop, где ядро живёт за границей
    /// процесса, синхронного вопроса «это доска?» не существует вовсе — UI начал
    /// бы сверять расширение сам, то есть завёл второй источник истины
    /// (принцип I, 001 T113).
    pub kind: super::FileKind,
}

impl Note {
    /// Запись дерева по пути и отпечатку.
    ///
    /// Единственное место, где выводятся имя и вид: раньше каждый вызывающий
    /// собирал `Note` полями, и добавление `kind` пришлось бы повторить в семи
    /// местах — ровно так расходятся правила между таргетами (принцип II).
    pub fn new(path: PathBuf, stamp: FileStamp) -> Self {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            kind: super::classify(&path),
            name,
            path,
            stamp,
            has_unsaved_edits: false,
        }
    }

    /// Та же запись с явно заданным именем — для кэша, который хранит имя
    /// отдельной колонкой и не обязан пересчитывать его из пути.
    pub fn with_name(path: PathBuf, name: String, stamp: FileStamp) -> Self {
        Self {
            kind: super::classify(&path),
            name,
            path,
            stamp,
            has_unsaved_edits: false,
        }
    }
}

/// In-memory индекс путей/метаданных (T011). Папки не хранятся отдельно — иерархия
/// выводится из общих префиксов путей (data-model.md «Папка»).
#[derive(Debug, Default)]
pub struct Index {
    notes: HashMap<PathBuf, Note>,
}

impl Index {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, note: Note) {
        self.notes.insert(note.path.clone(), note);
    }

    pub fn remove(&mut self, path: &Path) -> Option<Note> {
        self.notes.remove(path)
    }

    pub fn get(&self, path: &Path) -> Option<&Note> {
        self.notes.get(path)
    }

    pub fn get_mut(&mut self, path: &Path) -> Option<&mut Note> {
        self.notes.get_mut(path)
    }

    pub fn rename(&mut self, from: &Path, to: &Path) -> Option<Note> {
        let mut note = self.notes.remove(from)?;
        note.path = to.to_path_buf();
        note.name = to
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.notes.insert(to.to_path_buf(), note.clone());
        Some(note)
    }

    pub fn len(&self) -> usize {
        self.notes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    /// Все заметки (FR-002/FR-003 — плоский список, дерево строит UI из путей).
    pub fn all(&self) -> Vec<Note> {
        let mut items: Vec<Note> = self.notes.values().cloned().collect();
        items.sort_by(|a, b| a.path.cmp(&b.path));
        items
    }

    /// Заметки непосредственно внутри `folder` (не рекурсивно) — для ленивой
    /// подгрузки дерева по мере раскрытия (Clarifications, «пригодность для
    /// навигации»).
    pub fn in_folder(&self, folder: &Path) -> Vec<Note> {
        let mut items: Vec<Note> = self
            .notes
            .values()
            .filter(|n| n.path.parent() == Some(folder))
            .cloned()
            .collect();
        items.sort_by(|a, b| a.path.cmp(&b.path));
        items
    }
}
