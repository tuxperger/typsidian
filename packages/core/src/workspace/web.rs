//! wasm-биндинги вкладок (contracts/core-workspace-api.md → Биндинги).
//!
//! Тонкие обёртки: правил здесь нет. Состояние между вызовами держит
//! интерфейс, как пакет экспорта.

use wasm_bindgen::prelude::*;

use super::persist::{self, SavedWorkspace};
use super::{facts, Transition, WorkspaceAction, WorkspaceState, WorkspaceView};
use crate::vault::web::WebBackend;

#[wasm_bindgen(js_name = workspaceEmpty)]
pub fn workspace_empty_js() -> WorkspaceState {
    WorkspaceState::empty()
}

#[wasm_bindgen(js_name = workspaceApply)]
pub async fn workspace_apply_js(
    backend: &WebBackend,
    state: WorkspaceState,
    action: WorkspaceAction,
    width_px: u32,
) -> Transition {
    facts::apply_with(backend, state, action, width_px).await
}

#[wasm_bindgen(js_name = workspaceView)]
pub async fn workspace_view_js(
    backend: &WebBackend,
    state: WorkspaceState,
    width_px: u32,
) -> WorkspaceView {
    facts::view_with(backend, &state, width_px).await
}

#[wasm_bindgen(js_name = workspaceSnapshot)]
pub fn workspace_snapshot_js(state: WorkspaceState) -> SavedWorkspace {
    persist::snapshot(&state)
}

#[wasm_bindgen(js_name = workspaceRestore)]
pub async fn workspace_restore_js(backend: &WebBackend, saved: SavedWorkspace) -> WorkspaceState {
    facts::restore_with(backend, &saved).await
}
