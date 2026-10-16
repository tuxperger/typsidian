//! wasm-биндинги импорта (contracts/core-import-api.md → Биндинги).
//!
//! Тонкие обёртки без правил. Папка-источник — отдельный объект
//! [`ImportSource`]: хендл папки, выбранной `showDirectoryPicker({ mode:
//! "read" })`, оборачивается в [`ReadOnly`], так что записать в источник нельзя
//! по типу. Vault назначения — обычный `WebBackend`, уже открытый окном.

use wasm_bindgen::prelude::*;

use super::markdown::{convert, Converted};
use super::names::ConvertContext;
use super::{ImportError, ImportJob, ImportState, ImportSurvey};
use crate::vault::backend::ReadOnly;
use crate::vault::web::WebBackend;

/// Папка-источник импорта, открытая только для чтения.
#[wasm_bindgen]
pub struct ImportSource {
    inner: ReadOnly<WebBackend>,
}

#[wasm_bindgen]
impl ImportSource {
    /// Из хендла папки (`FileSystemDirectoryHandle`). Ничего не читает и не
    /// пишет: служебного места в источнике не заводится.
    #[wasm_bindgen(js_name = fromHandle)]
    pub fn from_handle(name: String, handle: JsValue) -> ImportSource {
        ImportSource {
            inner: ReadOnly::new(WebBackend::for_reading(name, handle)),
        }
    }
}

/// C1, C4: что в источнике и можно ли продолжить.
#[wasm_bindgen(js_name = importSurvey)]
pub async fn import_survey_js(
    source: &ImportSource,
    vault: &WebBackend,
    job: ImportJob,
) -> ImportSurvey {
    super::survey(&source.inner, vault, &job).await
}

/// Строит план. Ничего не пишет.
#[wasm_bindgen(js_name = importBegin)]
pub async fn import_begin_js(
    source: &ImportSource,
    vault: &WebBackend,
    job: ImportJob,
) -> ImportState {
    super::import_begin(&source.inner, vault, &job).await
}

/// Один шаг — один файл.
#[wasm_bindgen(js_name = importStep)]
pub async fn import_step_js(
    source: &ImportSource,
    vault: &mut WebBackend,
    state: ImportState,
) -> Result<ImportState, ImportError> {
    super::import_step(&source.inner, vault, state).await
}

/// Пишет журнал и отчёт; возвращает путь заметки отчёта.
#[wasm_bindgen(js_name = importFinish)]
pub async fn import_finish_js(
    vault: &mut WebBackend,
    state: ImportState,
) -> Result<String, ImportError> {
    super::import_finish(vault, &state).await
}

/// Перевод одной заметки без записи — для проверок и предпросмотра.
#[wasm_bindgen(js_name = importConvert)]
pub fn import_convert_js(markdown: &str, ctx: ConvertContext) -> Converted {
    convert(markdown, &ctx)
}
