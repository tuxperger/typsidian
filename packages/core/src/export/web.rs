//! wasm-биндинги экспорта (contracts/core-export-api.md → Биндинги).
//!
//! Переходы пакета — чистые функции без состояния: цикл пакета держит
//! значение в UI и передаёт его туда и обратно (research.md §5). Сам экспорт
//! заметки — метод `PreviewEngine.exportPdf`, потому что ему нужны шрифты и
//! кэш шаблонов движка предпросмотра.

use wasm_bindgen::prelude::*;

use super::batch::{self, BatchState, BatchStep, BatchView};
use super::{ExportCandidate, ExportReport};
use crate::vault::vault_settings::VaultOverrides;
use crate::vault::web::WebBackend;

/// Строки Центра экспорта (FR-010, FR-011).
#[wasm_bindgen(js_name = exportCandidates)]
pub async fn export_candidates_js(
    backend: &WebBackend,
    overrides: VaultOverrides,
) -> Vec<ExportCandidate> {
    batch::candidates_of(backend, &overrides).await
}

#[wasm_bindgen(js_name = exportBatchStart)]
pub fn export_batch_start_js(
    paths: Vec<String>,
    stop_on_error: bool,
    overrides: VaultOverrides,
) -> BatchState {
    batch::start(paths, stop_on_error, &overrides)
}

#[wasm_bindgen(js_name = exportBatchNext)]
pub fn export_batch_next_js(state: BatchState) -> BatchStep {
    batch::step(state)
}

#[wasm_bindgen(js_name = exportBatchRecord)]
pub fn export_batch_record_js(
    state: BatchState,
    report: ExportReport,
    elapsed_ms: u32,
) -> BatchState {
    batch::record(state, report, elapsed_ms)
}

#[wasm_bindgen(js_name = exportBatchSkip)]
pub fn export_batch_skip_js(state: BatchState, note_path: &str) -> BatchState {
    batch::skip(state, note_path)
}

#[wasm_bindgen(js_name = exportBatchCancel)]
pub fn export_batch_cancel_js(state: BatchState) -> BatchState {
    batch::cancel(state)
}

#[wasm_bindgen(js_name = exportBatchView)]
pub fn export_batch_view_js(state: BatchState) -> BatchView {
    batch::view(state)
}
