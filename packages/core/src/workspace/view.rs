//! Показ рабочей области: [`view`] (data-model.md «WorkspaceView»,
//! research.md §10).
//!
//! Чистая функция, состояние не меняет. Две группы при ширине окна меньше
//! [`SPLIT_MIN_WIDTH_PX`] даёт слитой полосой, а состояние остаётся: при
//! расширении окна разделение возвращается (FR-020).

use super::model::is_merged;
use super::{
    ActiveNote, Direction, Facts, Group, GroupSlot, GroupView, Presentation, TabView,
    WorkspaceState, WorkspaceView, COMPACT_MAX_WIDTH_PX, SPLIT_MIN_WIDTH_PX,
};
use crate::vault::FileKind;

/// Что показывать. См. описание модуля.
pub fn view(state: &WorkspaceState, facts: &Facts) -> WorkspaceView {
    let merged = is_merged(state, facts);
    let compact = facts.width_px <= COMPACT_MAX_WIDTH_PX;
    let presentation = if compact {
        Presentation::List
    } else if state.groups.len() == 2 && !merged {
        Presentation::Split
    } else {
        Presentation::Single
    };
    let active_index = state.active_group.index().min(state.groups.len() - 1);
    let active_slot = GroupSlot::of_index(active_index);

    let mut groups = if presentation == Presentation::Split {
        state
            .groups
            .iter()
            .enumerate()
            .map(|(index, group)| group_view(group, GroupSlot::of_index(index), facts))
            .collect::<Vec<_>>()
    } else {
        vec![merged_view(state, active_index, facts)]
    };
    set_hints(&mut groups);

    let active_group = &state.groups[active_index];
    let active = active_group
        .active
        .and_then(|id| active_group.tabs.iter().find(|tab| tab.id == id))
        .map(|tab| ActiveNote {
            tab: tab.id,
            path: tab.path.clone(),
            name: file_name(&tab.path).to_owned(),
            kind: tab.kind,
        });

    let tab_count = groups.iter().map(|group| group.tabs.len() as u32).sum();
    let wide_enough = facts.width_px >= SPLIT_MIN_WIDTH_PX;
    let can_split = wide_enough
        && state.groups.len() == 1
        && active
            .as_ref()
            .is_some_and(|note| note.kind != FileKind::Board || state.groups[0].tabs.len() > 1);
    WorkspaceView {
        presentation,
        groups,
        active_group: active_slot,
        ratio_permille: state.ratio_permille,
        active,
        can_split,
        can_unsplit: state.groups.len() == 2,
        can_move_to_other: state.groups.len() == 2 && !merged,
        tab_count,
    }
}

fn exists(facts: &Facts, path: &str) -> bool {
    facts
        .note(path)
        .is_some_and(|fact| matches!(fact.kind, FileKind::Note | FileKind::Board))
}

fn tab_view(group: &Group, slot: GroupSlot, index: usize, facts: &Facts) -> TabView {
    let tab = &group.tabs[index];
    let fact = facts.note(&tab.path);
    TabView {
        id: tab.id,
        group: slot,
        path: tab.path.clone(),
        title: title_of(&tab.path),
        hint: None,
        kind: tab.kind,
        unsaved: fact.is_some_and(|fact| fact.unsaved),
        conflict: fact.is_some_and(|fact| fact.conflict),
        active: group.active == Some(tab.id),
    }
}

fn can_walk(group: &Group, direction: Direction, facts: &Facts) -> bool {
    group
        .history
        .step(direction, group.active.is_none(), |path| {
            exists(facts, path)
        })
        .is_some()
}

fn group_view(group: &Group, slot: GroupSlot, facts: &Facts) -> GroupView {
    GroupView {
        slot,
        tabs: (0..group.tabs.len())
            .map(|index| tab_view(group, slot, index, facts))
            .collect(),
        active: group.active,
        can_back: can_walk(group, Direction::Previous, facts),
        can_forward: can_walk(group, Direction::Next, facts),
    }
}

/// Одна полоса: вкладки всех групп подряд, заметка из обеих групп — один раз,
/// записью активной группы. История и активная вкладка — активной группы.
fn merged_view(state: &WorkspaceState, active_index: usize, facts: &Facts) -> GroupView {
    let slot = GroupSlot::of_index(active_index);
    let mut tabs: Vec<TabView> = Vec::new();
    for (group_index, group) in state.groups.iter().enumerate() {
        for index in 0..group.tabs.len() {
            let view = tab_view(group, GroupSlot::of_index(group_index), index, facts);
            match tabs.iter().position(|shown| shown.path == view.path) {
                // Повтор: остаётся запись активной группы, на месте первой.
                Some(position) => {
                    if group_index == active_index {
                        tabs[position] = view;
                    }
                }
                None => tabs.push(view),
            }
        }
    }
    let group = &state.groups[active_index];
    GroupView {
        slot,
        tabs,
        active: group.active,
        can_back: can_walk(group, Direction::Previous, facts),
        can_forward: can_walk(group, Direction::Next, facts),
    }
}

/// Имя файла: последний сегмент пути.
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Имя без расширения.
fn title_of(path: &str) -> String {
    let name = file_name(path);
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_owned(),
        _ => name.to_owned(),
    }
}

/// Папка пути: всё до последнего сегмента.
fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

/// Подсказки для одноимённых вкладок: у каждой — кратчайший хвост папки,
/// отличающий её от остальных с тем же названием (FR-007). Одна и та же
/// заметка в двух группах двусмысленности не создаёт.
fn set_hints(groups: &mut [GroupView]) {
    let mut paths: Vec<String> = Vec::new();
    for tab in groups.iter().flat_map(|group| group.tabs.iter()) {
        if !paths.contains(&tab.path) {
            paths.push(tab.path.clone());
        }
    }
    for group in groups.iter_mut() {
        for tab in &mut group.tabs {
            let rivals: Vec<&str> = paths
                .iter()
                .filter(|path| **path != tab.path && title_of(path) == tab.title)
                .map(String::as_str)
                .collect();
            if !rivals.is_empty() {
                tab.hint = Some(shortest_tail(&tab.path, &rivals));
            }
        }
    }
}

/// Хвост папки `path` из наименьшего числа сегментов, не совпадающий с таким
/// же хвостом ни у одного из `rivals`. Заметка в корне vault — «/».
fn shortest_tail(path: &str, rivals: &[&str]) -> String {
    let folder = folder_of(path);
    if folder.is_empty() {
        return "/".to_owned();
    }
    let segments: Vec<&str> = folder.split('/').collect();
    let tail = |segments: &[&str], count: usize| -> String {
        segments[segments.len().saturating_sub(count)..].join("/")
    };
    for count in 1..=segments.len() {
        let mine = tail(&segments, count);
        let clash = rivals.iter().any(|rival| {
            let rival_segments: Vec<&str> = folder_of(rival)
                .split('/')
                .filter(|segment| !segment.is_empty())
                .collect();
            tail(&rival_segments, count) == mine
        });
        if !clash {
            return mine;
        }
    }
    folder.to_owned()
}
