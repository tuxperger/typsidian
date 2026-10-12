//! Сохранённый набор вкладок: [`snapshot`] и [`restore`] (FR-021…FR-023,
//! research.md §11).
//!
//! Сохраняются пути вкладок по группам, активная вкладка каждой, активная
//! группа и доля ширины. Каретки, прокрутка и история не сохраняются. Куда
//! значение кладётся — решает `settings` (`Settings.workspaces`), здесь только
//! превращение туда и обратно.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::model::repair_recent;
use super::{
    Group, GroupSlot, Tab, TabId, WorkspaceState, RATIO_DEFAULT_PERMILLE, RATIO_MAX_PERMILLE,
    RATIO_MIN_PERMILLE,
};
use crate::vault::index::Note;
use crate::vault::FileKind;

/// Одна сохранённая группа.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SavedGroup {
    /// Пути вкладок в порядке полосы.
    pub tabs: Vec<String>,
    /// Индекс активной вкладки в `tabs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub active: Option<u32>,
}

/// Сохранённый набор вкладок vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SavedWorkspace {
    pub groups: Vec<SavedGroup>,
    pub active_group: GroupSlot,
    /// Доля первой группы, промилле.
    pub ratio_permille: u16,
}

impl SavedWorkspace {
    /// Набор без единой вкладки хранить незачем.
    pub fn is_empty(&self) -> bool {
        self.groups.iter().all(|group| group.tabs.is_empty())
    }
}

/// Снимок состояния для хранения. Пустые группы не пишутся.
pub fn snapshot(state: &WorkspaceState) -> SavedWorkspace {
    let mut groups = Vec::new();
    let mut active_group = GroupSlot::First;
    for (index, group) in state.groups.iter().enumerate() {
        if group.tabs.is_empty() {
            continue;
        }
        if index == state.active_group.index() {
            active_group = GroupSlot::of_index(groups.len());
        }
        groups.push(SavedGroup {
            tabs: group.tabs.iter().map(|tab| tab.path.clone()).collect(),
            active: group
                .active
                .and_then(|id| group.tabs.iter().position(|tab| tab.id == id))
                .map(|position| position as u32),
        });
    }
    SavedWorkspace {
        groups,
        active_group,
        ratio_permille: state.ratio_permille,
    }
}

/// Состояние из сохранённого набора и заметок vault.
///
/// Исправляет негодное, а не отказывает (FR-023): путь без заметки или доски в
/// индексе пропускается молча, повтор пути в группе и вторая доска одного пути
/// снимаются, пустая группа убирается, групп больше двух — лишние
/// отбрасываются, `active` вне диапазона (или указывающий на пропавшую
/// вкладку) заменяется первой вкладкой, доля подрезается. История пуста,
/// `recent` — активная, затем порядок полосы.
pub fn restore(saved: &SavedWorkspace, notes: &[Note]) -> WorkspaceState {
    let kinds = openable_kinds(saved, notes);
    let mut state = WorkspaceState::empty();
    let mut boards: Vec<&str> = Vec::new();
    let mut kept: Vec<(usize, Group)> = Vec::new();

    for (original, saved_group) in saved.groups.iter().enumerate() {
        let mut group = Group::default();
        let mut wanted_active: Option<TabId> = None;
        for (position, path) in saved_group.tabs.iter().enumerate() {
            let Some(&kind) = kinds.get(path.as_str()) else {
                continue;
            };
            if group.tabs.iter().any(|tab| &tab.path == path) {
                continue;
            }
            if kind == FileKind::Board {
                if boards.contains(&path.as_str()) {
                    continue;
                }
                boards.push(path);
            }
            let id = TabId(state.next_tab);
            state.next_tab += 1;
            group.tabs.push(Tab {
                id,
                path: path.clone(),
                kind,
            });
            if saved_group.active == Some(position as u32) {
                wanted_active = Some(id);
            }
        }
        if group.tabs.is_empty() {
            continue;
        }
        group.active = wanted_active.or_else(|| group.tabs.first().map(|tab| tab.id));
        repair_recent(&mut group);
        kept.push((original, group));
    }

    kept.truncate(2);
    if kept.is_empty() {
        return state;
    }
    state.active_group = kept
        .iter()
        .position(|(original, _)| *original == saved.active_group.index())
        .map_or(GroupSlot::First, GroupSlot::of_index);
    state.groups = kept.into_iter().map(|(_, group)| group).collect();
    state.ratio_permille = if saved.ratio_permille == 0 {
        RATIO_DEFAULT_PERMILLE
    } else {
        saved
            .ratio_permille
            .clamp(RATIO_MIN_PERMILLE, RATIO_MAX_PERMILLE)
    };
    state
}

/// Виды тех путей набора, что есть в индексе и открываются вкладкой.
fn openable_kinds<'a>(saved: &'a SavedWorkspace, notes: &[Note]) -> HashMap<&'a str, FileKind> {
    let mut wanted: HashMap<&str, Option<FileKind>> = saved
        .groups
        .iter()
        .flat_map(|group| group.tabs.iter())
        .map(|path| (path.as_str(), None))
        .collect();
    for note in notes {
        if !matches!(note.kind, FileKind::Note | FileKind::Board) {
            continue;
        }
        if let Some(slot) = wanted.get_mut(note.path.to_string_lossy().as_ref()) {
            *slot = Some(note.kind);
        }
    }
    wanted
        .into_iter()
        .filter_map(|(path, kind)| kind.map(|kind| (path, kind)))
        .collect()
}
