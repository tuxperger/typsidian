//! Сбор фактов и обёртки с бэкендом (contracts/core-workspace-api.md →
//! «Факты», «Обёртки с бэкендом»). Их и зовут биндинги wasm и команды Tauri.

use std::path::Path;

use super::{
    apply, restore, view, Facts, NoteFact, SavedWorkspace, Transition, WorkspaceAction,
    WorkspaceState, WorkspaceView,
};
use crate::vault::backend::VaultBackend;
use crate::vault::classify;

/// Факты о путях, названных состоянием (вкладки, история) и действием
/// (`Open.path`, `Renamed.to`, `Deleted.path`).
///
/// Берёт из индекса бэкенда вид и признак несохранённых правок; `conflict` —
/// `pending_conflict(path)` либо путь среди `unresolved_edit_paths()`. Путь с
/// неразрешённым конфликтом, файла которого нет (удаление с правками),
/// попадает в факты как конфликтный: вкладка при этом не закрывается
/// (research.md §7). Прочие пути без записи в индексе в факты не попадают —
/// для модели это «заметки нет».
pub async fn facts_of<B: VaultBackend>(
    backend: &B,
    state: &WorkspaceState,
    action: Option<&WorkspaceAction>,
    width_px: u32,
) -> Facts {
    let mut named: Vec<String> = Vec::new();
    let mut name = |path: &str| {
        if !named.iter().any(|known| known == path) {
            named.push(path.to_owned());
        }
    };
    for group in &state.groups {
        for tab in &group.tabs {
            name(&tab.path);
        }
        for entry in &group.history.entries {
            name(&entry.path);
        }
    }
    match action {
        Some(WorkspaceAction::Open { path, .. }) | Some(WorkspaceAction::Deleted { path }) => {
            name(path)
        }
        Some(WorkspaceAction::Renamed { from, to }) => {
            name(from);
            name(to);
        }
        _ => {}
    }

    let indexed = backend.list().await;
    let unresolved = backend.unresolved_edit_paths();
    let mut notes = Vec::with_capacity(named.len());
    for path in named {
        let as_path = Path::new(&path);
        let conflict = backend.pending_conflict(as_path).is_some()
            || unresolved.iter().any(|known| known == as_path);
        match indexed.iter().find(|note| note.path == as_path) {
            Some(note) => notes.push(NoteFact {
                unsaved: note.has_unsaved_edits || backend.has_unsaved_edits(as_path),
                kind: note.kind,
                conflict,
                path,
            }),
            None if conflict => notes.push(NoteFact {
                kind: classify(as_path),
                unsaved: true,
                conflict,
                path,
            }),
            None => {}
        }
    }
    Facts { notes, width_px }
}

/// Переход с фактами, собранными из бэкенда.
pub async fn apply_with<B: VaultBackend>(
    backend: &B,
    state: WorkspaceState,
    action: WorkspaceAction,
    width_px: u32,
) -> Transition {
    let facts = facts_of(backend, &state, Some(&action), width_px).await;
    apply(state, &action, &facts)
}

/// Показ с фактами, собранными из бэкенда.
pub async fn view_with<B: VaultBackend>(
    backend: &B,
    state: &WorkspaceState,
    width_px: u32,
) -> WorkspaceView {
    let facts = facts_of(backend, state, None, width_px).await;
    view(state, &facts)
}

/// Восстановление набора по индексу бэкенда.
pub async fn restore_with<B: VaultBackend>(backend: &B, saved: &SavedWorkspace) -> WorkspaceState {
    restore(saved, &backend.list().await)
}
