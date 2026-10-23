//! Закладки vault (фича 032).
//!
//! Закладки — список в настройках vault (`bookmarks.entries` в
//! `.typsidian/vault.json`, research.md §1): заметка, раздел, поиск или папка,
//! группы в один уровень. Дубли, порядок, группы, переименование и
//! доступность целей решаются здесь; каждая правка читает настройки, меняет
//! список, пишет и отдаёт новый вид. Пути переписывает [`move_bookmarks`],
//! который зовут переименование и перенос заметок (как отметки 028).

#[cfg(target_arch = "wasm32")]
pub mod web;

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::search::SearchQuery;
use crate::vault::backend::VaultBackend;
use crate::vault::vault_settings::{read_vault_settings, write_vault_settings, VaultSettingsError};
use crate::vault::AccessState;

// ── Модель ──────────────────────────────────────────────────────────────────

/// Что закреплено (research.md §2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum BookmarkTarget {
    Note {
        path: String,
    },
    /// Раздел заметки по тексту заголовка.
    Section {
        path: String,
        heading: String,
    },
    Search {
        query: SearchQuery,
    },
    Folder {
        path: String,
    },
}

impl BookmarkTarget {
    /// Имя по умолчанию: имя заметки, заголовок, строка поиска, папка.
    fn default_name(&self) -> String {
        match self {
            BookmarkTarget::Note { path } => crate::links::title_of(path),
            BookmarkTarget::Section { heading, .. } => heading.clone(),
            BookmarkTarget::Search { query } => query.text.trim().to_owned(),
            BookmarkTarget::Folder { path } => path.clone(),
        }
    }

    /// Путь цели в vault; у поиска пути нет.
    fn path_mut(&mut self) -> Option<&mut String> {
        match self {
            BookmarkTarget::Note { path }
            | BookmarkTarget::Section { path, .. }
            | BookmarkTarget::Folder { path } => Some(path),
            BookmarkTarget::Search { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Bookmark {
    pub id: String,
    pub name: String,
    pub target: BookmarkTarget,
}

/// Запись списка: закладка верхнего уровня или группа.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "type")]
pub enum BookmarkEntry {
    Item {
        bookmark: Bookmark,
    },
    Group {
        id: String,
        name: String,
        #[serde(default)]
        collapsed: bool,
        #[serde(default)]
        items: Vec<Bookmark>,
    },
}

// ── Вид ─────────────────────────────────────────────────────────────────────

/// Доступна ли цель (research.md §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum BookmarkState {
    Ready,
    /// Заметки или папки нет.
    Missing,
    /// Заметка есть, заголовка нет — закладка откроет заметку.
    SectionMissing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BookmarkRow {
    pub bookmark: Bookmark,
    pub state: BookmarkState,
    /// Раздел: строка и колонка заголовка (с 1) — UI открывает заметку на нём.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub place: Option<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "type")]
pub enum ViewEntry {
    Item {
        row: BookmarkRow,
    },
    Group {
        id: String,
        name: String,
        collapsed: bool,
        rows: Vec<BookmarkRow>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BookmarksView {
    pub entries: Vec<ViewEntry>,
    /// Всего закладок, в группах тоже.
    pub count: u32,
}

/// Ответ «В закладки».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum AddOutcome {
    Added {
        id: String,
        view: BookmarksView,
    },
    /// Такая цель уже в закладках — второй не добавлено.
    Exists {
        id: String,
        view: BookmarksView,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum BookmarkError {
    ReadOnly,
    NotFound,
    Io,
}

/// Куда переместить: группа (`None` — верхний уровень) и позиция в ней.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BookmarkPlace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub group: Option<String>,
    pub index: u32,
}

// ── Чтение и запись ─────────────────────────────────────────────────────────

fn settings_error(error: VaultSettingsError) -> BookmarkError {
    match error {
        VaultSettingsError::NotWritable => BookmarkError::ReadOnly,
        _ => BookmarkError::Io,
    }
}

async fn load<B: VaultBackend>(backend: &B) -> Vec<BookmarkEntry> {
    read_vault_settings(backend)
        .await
        .map(|settings| settings.bookmarks)
        .unwrap_or_default()
}

async fn store<B: VaultBackend>(
    backend: &mut B,
    entries: Vec<BookmarkEntry>,
) -> Result<BookmarksView, BookmarkError> {
    let mut settings = read_vault_settings(backend).await.map_err(settings_error)?;
    settings.bookmarks = entries;
    write_vault_settings(backend, &settings)
        .await
        .map_err(settings_error)?;
    Ok(bookmarks_view(backend).await)
}

fn writable<B: VaultBackend>(backend: &B) -> Result<(), BookmarkError> {
    if backend.access_state() == AccessState::ReadWrite {
        Ok(())
    } else {
        Err(BookmarkError::ReadOnly)
    }
}

/// Идентификатор: 12 знаков blake3 от затравки и времени.
fn new_id(seed: &str) -> String {
    let now = crate::properties::now_ms();
    blake3::hash(format!("{seed}\u{0}{now}").as_bytes()).to_hex()[..12].to_owned()
}

fn bookmarks_of(entries: &[BookmarkEntry]) -> impl Iterator<Item = &Bookmark> {
    entries.iter().flat_map(|entry| match entry {
        BookmarkEntry::Item { bookmark } => std::slice::from_ref(bookmark).iter(),
        BookmarkEntry::Group { items, .. } => items.iter(),
    })
}

fn bookmarks_mut(entries: &mut [BookmarkEntry]) -> impl Iterator<Item = &mut Bookmark> {
    entries.iter_mut().flat_map(|entry| match entry {
        BookmarkEntry::Item { bookmark } => std::slice::from_mut(bookmark).iter_mut(),
        BookmarkEntry::Group { items, .. } => items.iter_mut(),
    })
}

/// Вынимает закладку из списка.
fn take(entries: &mut Vec<BookmarkEntry>, id: &str) -> Option<Bookmark> {
    if let Some(at) = entries
        .iter()
        .position(|entry| matches!(entry, BookmarkEntry::Item { bookmark } if bookmark.id == id))
    {
        let BookmarkEntry::Item { bookmark } = entries.remove(at) else {
            unreachable!()
        };
        return Some(bookmark);
    }
    for entry in entries.iter_mut() {
        if let BookmarkEntry::Group { items, .. } = entry {
            if let Some(at) = items.iter().position(|bookmark| bookmark.id == id) {
                return Some(items.remove(at));
            }
        }
    }
    None
}

fn group_mut<'a>(entries: &'a mut [BookmarkEntry], id: &str) -> Option<&'a mut BookmarkEntry> {
    entries
        .iter_mut()
        .find(|entry| matches!(entry, BookmarkEntry::Group { id: group, .. } if group == id))
}

// ── Вид ─────────────────────────────────────────────────────────────────────

/// Закладки с доступностью целей (research.md §5).
pub async fn bookmarks_view<B: VaultBackend>(backend: &B) -> BookmarksView {
    let entries = load(backend).await;
    if entries.is_empty() {
        return BookmarksView::default();
    }
    let files = backend.list().await;
    let paths: HashSet<String> = files
        .iter()
        .map(|file| file.path.to_string_lossy().replace('\\', "/"))
        .collect();
    let folder_exists = |folder: &str| {
        folder.is_empty()
            || paths
                .iter()
                .any(|path| path.starts_with(&format!("{folder}/")))
    };

    let mut rows_of = Vec::new();
    for bookmark in bookmarks_of(&entries) {
        let mut place = None;
        let state = match &bookmark.target {
            BookmarkTarget::Note { path } if paths.contains(path) => BookmarkState::Ready,
            BookmarkTarget::Section { path, heading } if paths.contains(path) => {
                let text = backend.read(Path::new(path)).await.unwrap_or_default();
                let found = crate::structure::parse(&text)
                    .headings
                    .into_iter()
                    .find(|site| site.text.trim() == heading.trim());
                if let Some(site) = found {
                    place = Some((site.place.line, site.place.column));
                    BookmarkState::Ready
                } else {
                    BookmarkState::SectionMissing
                }
            }
            BookmarkTarget::Folder { path } if folder_exists(path) => BookmarkState::Ready,
            BookmarkTarget::Search { .. } => BookmarkState::Ready,
            _ => BookmarkState::Missing,
        };
        rows_of.push((bookmark.id.clone(), state, place));
    }
    let row = |bookmark: &Bookmark| {
        let found = rows_of.iter().find(|(id, _, _)| *id == bookmark.id);
        BookmarkRow {
            bookmark: bookmark.clone(),
            state: found.map_or(BookmarkState::Ready, |(_, state, _)| *state),
            place: found.and_then(|(_, _, place)| *place),
        }
    };
    let view_entries = entries
        .iter()
        .map(|entry| match entry {
            BookmarkEntry::Item { bookmark } => ViewEntry::Item { row: row(bookmark) },
            BookmarkEntry::Group {
                id,
                name,
                collapsed,
                items,
            } => ViewEntry::Group {
                id: id.clone(),
                name: name.clone(),
                collapsed: *collapsed,
                rows: items.iter().map(row).collect(),
            },
        })
        .collect();
    BookmarksView {
        entries: view_entries,
        count: rows_of.len() as u32,
    }
}

// ── Правки ──────────────────────────────────────────────────────────────────

/// «В закладки» (FR-001): та же цель второй раз не добавляется.
pub async fn bookmark_add<B: VaultBackend>(
    backend: &mut B,
    target: BookmarkTarget,
    name: Option<String>,
) -> Result<AddOutcome, BookmarkError> {
    writable(backend)?;
    let mut entries = load(backend).await;
    if let Some(existing) = bookmarks_of(&entries).find(|bookmark| bookmark.target == target) {
        let id = existing.id.clone();
        return Ok(AddOutcome::Exists {
            id,
            view: bookmarks_view(backend).await,
        });
    }
    let name = name
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| target.default_name());
    let seed = serde_json::to_string(&target).unwrap_or_default();
    let id = new_id(&seed);
    entries.push(BookmarkEntry::Item {
        bookmark: Bookmark {
            id: id.clone(),
            name,
            target,
        },
    });
    let view = store(backend, entries).await?;
    Ok(AddOutcome::Added { id, view })
}

pub async fn bookmark_remove<B: VaultBackend>(
    backend: &mut B,
    id: &str,
) -> Result<BookmarksView, BookmarkError> {
    writable(backend)?;
    let mut entries = load(backend).await;
    take(&mut entries, id).ok_or(BookmarkError::NotFound)?;
    store(backend, entries).await
}

pub async fn bookmark_rename<B: VaultBackend>(
    backend: &mut B,
    id: &str,
    name: &str,
) -> Result<BookmarksView, BookmarkError> {
    writable(backend)?;
    let name = name.trim();
    let mut entries = load(backend).await;
    let bookmark = bookmarks_mut(&mut entries)
        .find(|bookmark| bookmark.id == id)
        .ok_or(BookmarkError::NotFound)?;
    if !name.is_empty() {
        bookmark.name = name.to_owned();
    }
    store(backend, entries).await
}

/// Переставляет закладку или группу (FR-003). Группа ставится только на
/// верхний уровень; позиция за концом — в конец.
pub async fn bookmark_move<B: VaultBackend>(
    backend: &mut B,
    id: &str,
    place: &BookmarkPlace,
) -> Result<BookmarksView, BookmarkError> {
    writable(backend)?;
    let mut entries = load(backend).await;
    let index = place.index as usize;
    if let Some(at) = entries
        .iter()
        .position(|entry| matches!(entry, BookmarkEntry::Group { id: group, .. } if group == id))
    {
        let group = entries.remove(at);
        let index = index.min(entries.len());
        entries.insert(index, group);
        return store(backend, entries).await;
    }
    let bookmark = take(&mut entries, id).ok_or(BookmarkError::NotFound)?;
    match &place.group {
        None => {
            let index = index.min(entries.len());
            entries.insert(index, BookmarkEntry::Item { bookmark });
        }
        Some(group) => match group_mut(&mut entries, group) {
            Some(BookmarkEntry::Group { items, .. }) => {
                let index = index.min(items.len());
                items.insert(index, bookmark);
            }
            _ => return Err(BookmarkError::NotFound),
        },
    }
    store(backend, entries).await
}

/// Новая группа — в конец списка; ответ — её `id` в виде.
pub async fn bookmark_group_create<B: VaultBackend>(
    backend: &mut B,
    name: &str,
) -> Result<BookmarksView, BookmarkError> {
    writable(backend)?;
    let mut entries = load(backend).await;
    entries.push(BookmarkEntry::Group {
        id: new_id(name),
        name: name.trim().to_owned(),
        collapsed: false,
        items: Vec::new(),
    });
    store(backend, entries).await
}

pub async fn bookmark_group_rename<B: VaultBackend>(
    backend: &mut B,
    id: &str,
    name: &str,
) -> Result<BookmarksView, BookmarkError> {
    writable(backend)?;
    let mut entries = load(backend).await;
    match group_mut(&mut entries, id) {
        Some(BookmarkEntry::Group { name: current, .. }) => {
            if !name.trim().is_empty() {
                *current = name.trim().to_owned();
            }
        }
        _ => return Err(BookmarkError::NotFound),
    }
    store(backend, entries).await
}

pub async fn bookmark_group_toggle<B: VaultBackend>(
    backend: &mut B,
    id: &str,
) -> Result<BookmarksView, BookmarkError> {
    writable(backend)?;
    let mut entries = load(backend).await;
    match group_mut(&mut entries, id) {
        Some(BookmarkEntry::Group { collapsed, .. }) => *collapsed = !*collapsed,
        _ => return Err(BookmarkError::NotFound),
    }
    store(backend, entries).await
}

/// Удаляет группу; её закладки остаются на верхнем уровне на её месте.
pub async fn bookmark_group_delete<B: VaultBackend>(
    backend: &mut B,
    id: &str,
) -> Result<BookmarksView, BookmarkError> {
    writable(backend)?;
    let mut entries = load(backend).await;
    let at = entries
        .iter()
        .position(|entry| matches!(entry, BookmarkEntry::Group { id: group, .. } if group == id))
        .ok_or(BookmarkError::NotFound)?;
    let BookmarkEntry::Group { items, .. } = entries.remove(at) else {
        unreachable!()
    };
    for (offset, bookmark) in items.into_iter().enumerate() {
        entries.insert(at + offset, BookmarkEntry::Item { bookmark });
    }
    store(backend, entries).await
}

// ── Переименование и перенос ────────────────────────────────────────────────

/// Пути закладок едут за заметкой или папкой (FR-004, SC-002).
pub async fn move_bookmarks<B: VaultBackend>(
    backend: &mut B,
    from: &Path,
    to: &Path,
) -> Result<(), BookmarkError> {
    let from = from.to_string_lossy().replace('\\', "/");
    let to = to.to_string_lossy().replace('\\', "/");
    let mut entries = load(backend).await;
    let mut changed = false;
    for bookmark in bookmarks_mut(&mut entries) {
        let Some(path) = bookmark.target.path_mut().map(|path| path.clone()) else {
            continue;
        };
        let moved = if path == from {
            Some(to.clone())
        } else {
            path.strip_prefix(&format!("{from}/"))
                .map(|tail| format!("{to}/{tail}"))
        };
        if let Some(moved) = moved {
            // Имя по умолчанию следует за новым именем заметки.
            if matches!(bookmark.target, BookmarkTarget::Note { .. })
                && bookmark.name == crate::links::title_of(&path)
            {
                bookmark.name = crate::links::title_of(&moved);
            }
            if let Some(path) = bookmark.target.path_mut() {
                *path = moved;
            }
            changed = true;
        }
    }
    if !changed {
        return Ok(());
    }
    let mut settings = read_vault_settings(backend).await.map_err(settings_error)?;
    settings.bookmarks = entries;
    write_vault_settings(backend, &settings)
        .await
        .map_err(settings_error)
}
