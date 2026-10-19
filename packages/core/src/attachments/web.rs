//! wasm-биндинги вложений (contracts/core-attachments-api.md → Биндинги).
//!
//! Переходы пачки — чистые функции без состояния: UI держит значение и
//! передаёт его туда и обратно. Вызовы с `backend` идут через очередь вызовов
//! wasm на стороне UI, как прочие обращения к `WebBackend`.

use wasm_bindgen::prelude::*;

use super::batch::{
    self, AttachmentBatchState, AttachmentBatchStep, AttachmentBatchView, IncomingItem,
};
use super::folder::{validate_folder, AttachmentsFolderProblem};
use super::name::InsertMoment;
use crate::templates::{InsertionEdit, InsertionPoint};
use crate::vault::vault_settings::VaultOverrides;
use crate::vault::web::WebBackend;

#[wasm_bindgen(js_name = attachmentsBatchStart)]
pub async fn attachments_batch_start_js(
    backend: &WebBackend,
    overrides: VaultOverrides,
    note_path: String,
    items: Vec<IncomingItem>,
    moment: InsertMoment,
) -> AttachmentBatchState {
    batch::start(backend, &overrides, &note_path, items, moment).await
}

#[wasm_bindgen(js_name = attachmentsBatchNext)]
pub fn attachments_batch_next_js(state: AttachmentBatchState) -> AttachmentBatchStep {
    batch::step(state)
}

#[wasm_bindgen(js_name = attachmentsBatchConfirm)]
pub fn attachments_batch_confirm_js(
    state: AttachmentBatchState,
    index: u32,
    accept: bool,
) -> AttachmentBatchState {
    batch::confirm(state, index, accept)
}

#[wasm_bindgen(js_name = attachmentsBatchStore)]
pub async fn attachments_batch_store_js(
    backend: &mut WebBackend,
    state: AttachmentBatchState,
    index: u32,
    bytes: &[u8],
) -> AttachmentBatchState {
    batch::store(backend, state, index, bytes).await
}

#[wasm_bindgen(js_name = attachmentsBatchSourceFailed)]
pub fn attachments_batch_source_failed_js(
    state: AttachmentBatchState,
    index: u32,
) -> AttachmentBatchState {
    batch::source_failed(state, index)
}

#[wasm_bindgen(js_name = attachmentsBatchCancel)]
pub fn attachments_batch_cancel_js(state: AttachmentBatchState) -> AttachmentBatchState {
    batch::cancel(state)
}

#[wasm_bindgen(js_name = attachmentsBatchView)]
pub fn attachments_batch_view_js(state: AttachmentBatchState) -> AttachmentBatchView {
    batch::view(state)
}

/// Правка заметки для вставленных путей; `undefined` — путей нет.
#[wasm_bindgen(js_name = attachmentsPlanMarkup)]
pub fn attachments_plan_markup_js(
    text_length: usize,
    point: InsertionPoint,
    before: &str,
    after: &str,
    paths: Vec<String>,
    block: bool,
) -> Option<InsertionEdit> {
    super::markup::plan_markup(text_length, &point, before, after, &paths, block)
}

/// Проверка названия папки вложений в настройках (FR-018): нормальная форма
/// либо причина отказа.
#[wasm_bindgen(js_name = attachmentsValidateFolder)]
pub fn attachments_validate_folder_js(raw: &str) -> Result<String, AttachmentsFolderProblem> {
    validate_folder(raw)
}
