//! wasm-биндинги снимков заметок (фича 029). Тонкие обёртки: время берётся
//! из часов браузера, правила — в `snapshots`.

use wasm_bindgen::prelude::*;

use super::{
    DeletedNote, NoteHistory, RecoverOutcome, RestoreOutcome, SnapshotReason, SnapshotUsage,
    TakeOutcome,
};
use crate::properties::now_ms;
use crate::vault::conflict::DiffLine;
use crate::vault::web::WebBackend;

#[wasm_bindgen(js_name = snapshotTake)]
pub async fn snapshot_take_js(
    backend: &mut WebBackend,
    path: &str,
    text: &str,
    reason: SnapshotReason,
) -> TakeOutcome {
    super::snapshot_take(backend, path, text, reason, now_ms()).await
}

#[wasm_bindgen(js_name = snapshotHistory)]
pub async fn snapshot_history_js(backend: &WebBackend, path: &str) -> NoteHistory {
    super::snapshot_history(backend, path, now_ms()).await
}

/// Различия снимка с текстом; пусто — снимка нет.
#[wasm_bindgen(js_name = snapshotDiff)]
pub async fn snapshot_diff_js(backend: &WebBackend, id: &str, against: &str) -> Vec<DiffLine> {
    super::snapshot_diff(backend, id, against)
        .await
        .unwrap_or_default()
}

#[wasm_bindgen(js_name = snapshotText)]
pub async fn snapshot_text_js(backend: &WebBackend, id: &str) -> Option<String> {
    super::snapshot_text(backend, id).await
}

#[wasm_bindgen(js_name = snapshotRestore)]
pub async fn snapshot_restore_js(
    backend: &mut WebBackend,
    path: &str,
    id: &str,
    editor_text: Option<String>,
) -> RestoreOutcome {
    super::snapshot_restore(backend, path, id, editor_text.as_deref(), now_ms()).await
}

#[wasm_bindgen(js_name = snapshotDeletedNotes)]
pub async fn deleted_notes_js(backend: &WebBackend) -> Vec<DeletedNote> {
    super::deleted_notes(backend, now_ms()).await
}

#[wasm_bindgen(js_name = snapshotRecoverNote)]
pub async fn recover_note_js(
    backend: &mut WebBackend,
    path: &str,
    as_path: Option<String>,
) -> RecoverOutcome {
    super::recover_note(backend, path, as_path.as_deref()).await
}

#[wasm_bindgen(js_name = snapshotUsage)]
pub async fn snapshot_usage_js(backend: &WebBackend) -> SnapshotUsage {
    super::snapshot_usage(backend).await
}

/// Удаляет все снимки vault; отказ записи — исключение.
#[wasm_bindgen(js_name = snapshotsClear)]
pub async fn snapshots_clear_js(backend: &mut WebBackend) -> Result<u32, JsValue> {
    super::snapshots_clear(backend)
        .await
        .map_err(|error| JsValue::from_str(&format!("{error:?}")))
}
