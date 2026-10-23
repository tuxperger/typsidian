//! wasm-биндинги встраивания (фича 031): команда «Встроить заметку».

use wasm_bindgen::prelude::*;

use super::EmbedInsertion;
use crate::vault::web::WebBackend;

/// Библиотека в vault, `#import` и `#embed("")` у каретки; отказ записи —
/// исключение.
#[wasm_bindgen(js_name = embedInsert)]
pub async fn embed_insert_js(
    backend: &mut WebBackend,
    text: &str,
    offset: u32,
) -> Result<EmbedInsertion, JsValue> {
    super::insert_embed(backend, text, offset)
        .await
        .map_err(|error| JsValue::from_str(&format!("{error:?}")))
}
