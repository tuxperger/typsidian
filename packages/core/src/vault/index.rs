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

    /// Переносит запись заметки — или все записи под папкой — на новое место
    /// (фича 023, research.md §1). Имя записи не меняется: перенос не
    /// переименовывает; `has_unsaved_edits` остаётся за записью.
    ///
    /// В отличие от [`Index::rename`], который переставляет одну запись, для
    /// папки переезжают все заметки под ней, иначе индекс остался бы под
    /// старыми путями. Возвращает пары по переехавшим записям, по прежнему
    /// пути.
    pub fn rename_prefix(&mut self, from: &Path, to: &Path) -> Vec<super::relocate::PathChange> {
        let mut keys: Vec<PathBuf> = self
            .notes
            .keys()
            .filter(|path| path.starts_with(from))
            .cloned()
            .collect();
        keys.sort();
        let mut changes = Vec::with_capacity(keys.len());
        for old in keys {
            let Some(mut note) = self.notes.remove(&old) else {
                continue;
            };
            let new = super::relocate::rebase(from, to, &old);
            note.path = new.clone();
            note.name = new
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Вид зависит от места (папка `*.assets` делает файлы снимками),
            // поэтому пересчитывается, а не копируется.
            note.kind = super::classify(&new);
            self.notes.insert(new.clone(), note);
            changes.push(super::relocate::PathChange { from: old, to: new });
        }
        changes
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

#[cfg(test)]
mod tests {
    use super::*;

    fn note(path: &str) -> Note {
        Note::new(PathBuf::from(path), FileStamp::from_content(0, b""))
    }

    #[test]
    fn rename_prefix_moves_one_entry_for_a_file() {
        let mut index = Index::new();
        index.insert(note("A/x.typ"));
        index.insert(note("A/y.typ"));
        index
            .get_mut(Path::new("A/x.typ"))
            .unwrap()
            .has_unsaved_edits = true;

        let changes = index.rename_prefix(Path::new("A/x.typ"), Path::new("B/x.typ"));

        assert_eq!(changes.len(), 1);
        assert!(index.get(Path::new("A/x.typ")).is_none());
        let moved = index
            .get(Path::new("B/x.typ"))
            .expect("запись на новом месте");
        assert_eq!(moved.name, "x.typ");
        assert!(
            moved.has_unsaved_edits,
            "несохранённые правки едут с записью"
        );
        assert!(index.get(Path::new("A/y.typ")).is_some());
    }

    #[test]
    fn rename_prefix_moves_every_entry_under_a_folder_and_only_them() {
        let mut index = Index::new();
        index.insert(note("A/x.typ"));
        index.insert(note("A/C/y.excalidraw"));
        index.insert(note("A-b/w.typ"));

        let changes = index.rename_prefix(Path::new("A"), Path::new("B/A"));

        let moved: Vec<_> = changes.iter().map(|c| c.to.clone()).collect();
        assert_eq!(
            moved,
            vec![
                PathBuf::from("B/A/C/y.excalidraw"),
                PathBuf::from("B/A/x.typ")
            ]
        );
        assert!(index.get(Path::new("B/A/C/y.excalidraw")).is_some());
        assert!(index.get(Path::new("A/x.typ")).is_none());
        assert!(
            index.get(Path::new("A-b/w.typ")).is_some(),
            "соседняя папка с общим началом имени не затронута"
        );
        assert_eq!(index.len(), 3);
    }
}
