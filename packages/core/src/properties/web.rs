//! wasm-биндинги свойств и выборок (contracts/core-properties-api.md).
//!
//! Тонкие обёртки: правил здесь нет. Свободные функции работают с текстом и
//! объектов ядра не заимствуют (как `structure/web.rs`); то, что читает индекс
//! или vault, — методы `WebBackend` с теми же именами, что у команд Tauri.

use wasm_bindgen::prelude::*;

use super::select::{ensure_select_library, inputs_digest};
use super::{
    edit, index, read, select_blocks, select_import_edits, select_library, LibraryStatus,
    NoteProperties, PropertyEdit, PropertyKeyInfo, ReindexOutcome, ReindexStep, SelectBlock,
    SelectLibrary, Selection, SelectionResult,
};
use crate::elements::{EditOutcome, TextEdit};
use crate::search::{parse_conditions, ParsedConditions};
use crate::vault::backend::{VaultBackend, WriteError};
use crate::vault::web::{JsMoveControl, WebBackend};

#[wasm_bindgen(js_name = propertiesRead)]
pub fn properties_read_js(text: &str) -> NoteProperties {
    read(text)
}

#[wasm_bindgen(js_name = propertiesEdit)]
pub fn properties_edit_js(text: &str, change: PropertyEdit) -> EditOutcome {
    edit(text, &change)
}

#[wasm_bindgen(js_name = selectBlocks)]
pub fn select_blocks_js(text: &str) -> Vec<SelectBlock> {
    select_blocks(text)
}

#[wasm_bindgen(js_name = selectImportEdits)]
pub fn select_import_edits_js(text: &str) -> Vec<TextEdit> {
    select_import_edits(text)
}

#[wasm_bindgen(js_name = selectLibrary)]
pub fn select_library_js() -> SelectLibrary {
    select_library()
}

#[wasm_bindgen(js_name = searchParseConditions)]
pub fn search_parse_conditions_js(text: &str, known_keys: Vec<String>) -> ParsedConditions {
    parse_conditions(text, &known_keys)
}

#[wasm_bindgen]
impl WebBackend {
    /// Отбор заметок по выборке (B3).
    #[wasm_bindgen(js_name = vaultSelect)]
    pub async fn vault_select_js(&self, selection: Selection) -> SelectionResult {
        index::select_notes(self, &selection).await
    }

    /// Ключи свойств vault и виды их значений — для подсказок.
    #[wasm_bindgen(js_name = vaultPropertyKeys)]
    pub async fn vault_property_keys_js(&self) -> Vec<PropertyKeyInfo> {
        index::property_keys(self).await
    }

    /// Построены ли свойства по текущему формату; `false` — запустить
    /// `vaultReindexProperties`.
    #[wasm_bindgen(js_name = vaultPropertiesCurrent)]
    pub async fn vault_properties_current_js(&self) -> bool {
        VaultBackend::properties_current(self).await
    }

    /// Пакетный пересчёт свойств с ходом и отменой. `on_progress(done, total)`
    /// вызывается после каждого шага; `cancel` — объект-флаг
    /// `{ cancelled: boolean }`, который интерфейс держит у себя.
    #[wasm_bindgen(js_name = vaultReindexProperties)]
    pub async fn vault_reindex_properties_js(
        &mut self,
        on_progress: Option<js_sys::Function>,
        cancel: JsValue,
    ) -> ReindexOutcome {
        let control = JsMoveControl {
            on_progress,
            cancel,
        };
        index::reindex_properties(self, &control).await
    }

    /// Один шаг пересчёта — для вызывающих, которые ведут цикл сами.
    #[wasm_bindgen(js_name = vaultReindexPropertiesStep)]
    pub async fn vault_reindex_properties_step_js(&mut self, from: u32, count: u32) -> ReindexStep {
        index::reindex_step(self, from, count).await
    }

    /// Кладёт `.typsidian/lib/select.typ` в vault, если его нет (§10).
    #[wasm_bindgen(js_name = vaultEnsureSelectLibrary)]
    pub async fn vault_ensure_select_library_js(&mut self) -> Result<LibraryStatus, WriteError> {
        ensure_select_library(self).await
    }

    /// Отпечаток входов блоков заметки: изменился — пересобрать (SC-002);
    /// `undefined` — блоков нет.
    #[wasm_bindgen(js_name = vaultSelectDigest)]
    pub async fn vault_select_digest_js(&self, text: &str) -> Option<String> {
        inputs_digest(self, text).await
    }
}
