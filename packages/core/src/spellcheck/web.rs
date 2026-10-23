//! wasm-биндинги проверки орфографии (фича 030, contracts/core-spellcheck-api.md).
//! Тонкие обёртки: правила — в `spellcheck`.

use wasm_bindgen::prelude::*;

use super::{SpellChecker, SpellConfig, SpellError, SpellLanguage};
use crate::vault::web::WebBackend;

/// Проверяющий открытого vault: настройки и словарь vault.
#[wasm_bindgen(js_name = SpellChecker)]
pub struct SpellCheckerJs(SpellChecker);

impl SpellCheckerJs {
    pub(crate) fn inner(&self) -> &SpellChecker {
        &self.0
    }
}

fn rejected(error: SpellError) -> JsValue {
    serde_wasm_bindgen::to_value(&error).unwrap_or_else(|_| JsValue::from_str("Io"))
}

#[wasm_bindgen(js_class = SpellChecker)]
impl SpellCheckerJs {
    #[wasm_bindgen(constructor)]
    pub fn new() -> SpellCheckerJs {
        SpellCheckerJs(SpellChecker::default())
    }

    /// Загружен ли словарь языка (словари web едут файлами рядом с модулем).
    #[wasm_bindgen(js_name = dictionaryReady)]
    pub fn dictionary_ready(language: SpellLanguage) -> bool {
        super::dictionary_ready(language)
    }

    /// Словарь языка из текстов `.aff` и `.dic`; ошибка разбора — исключение.
    #[wasm_bindgen(js_name = loadDictionary)]
    pub fn load_dictionary(language: SpellLanguage, aff: &str, dic: &str) -> Result<(), JsValue> {
        super::load_dictionary(language, aff, dic).map_err(|error| JsValue::from_str(&error))
    }

    pub async fn configure(&mut self, backend: &WebBackend) -> SpellConfig {
        super::spell_configure(backend, &mut self.0).await
    }

    /// Добавить слово в словарь vault; отказ — исключение `SpellError`.
    #[wasm_bindgen(js_name = addWord)]
    pub async fn add_word(
        &mut self,
        backend: &mut WebBackend,
        word: &str,
    ) -> Result<SpellConfig, JsValue> {
        super::spell_add_word(backend, &mut self.0, word)
            .await
            .map_err(rejected)
    }

    #[wasm_bindgen(js_name = removeWord)]
    pub async fn remove_word(
        &mut self,
        backend: &mut WebBackend,
        word: &str,
    ) -> Result<SpellConfig, JsValue> {
        super::spell_remove_word(backend, &mut self.0, word)
            .await
            .map_err(rejected)
    }

    pub fn suggest(&self, word: &str) -> Vec<String> {
        super::spell_suggest(&self.0, word)
    }
}

impl Default for SpellCheckerJs {
    fn default() -> Self {
        Self::new()
    }
}
