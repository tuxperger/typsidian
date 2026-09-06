//! wasm-биндинги предпросмотра.
//!
//! Отдельный объект, а не метод `WebBackend`: предпросмотр не читает vault и не
//! владеет им, а держит только набор шрифтов. Отдельный объект заодно означает
//! отдельное заимствование на границе wasm-bindgen — сборка документа не
//! конфликтует с циклом опроса vault, который живёт в `WebBackend`.
//!
//! Здесь пока нет исполнителя в Web Worker (T022): сборка идёт в том же потоке,
//! то есть FR-007 выполняется лишь для коротких документов. Форма вызова от
//! появления воркера не изменится — изменится место исполнения.

use wasm_bindgen::prelude::*;

use super::compile::{compile_note, CompiledDocument};
use super::fonts::FontSet;
use typst::foundations::Bytes;

/// Движок предпросмотра: набор шрифтов плюс сборка документа.
#[wasm_bindgen]
pub struct PreviewEngine {
    fonts: FontSet,
}

#[wasm_bindgen]
impl PreviewEngine {
    /// Движок со встроенным набором шрифтов (FR-027).
    #[wasm_bindgen(constructor)]
    pub fn new() -> PreviewEngine {
        PreviewEngine {
            fonts: FontSet::builtin(),
        }
    }

    /// Добавляет шрифты из папки vault (FR-027, FR-029).
    #[wasm_bindgen(js_name = addVaultFonts)]
    pub fn add_vault_fonts(&mut self, files: Vec<js_sys::Uint8Array>) {
        let bytes = files.into_iter().map(|file| Bytes::new(file.to_vec()));
        self.fonts = std::mem::take(&mut self.fonts).with_vault_fonts(bytes);
    }

    /// Сколько начертаний доступно документу. Нужен UI, чтобы отличить
    /// «шрифтов нет» от «документ пуст».
    #[wasm_bindgen(js_name = fontCount)]
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }

    /// Собирает заметку из текста редактора (FR-002, FR-006).
    #[wasm_bindgen(js_name = compile)]
    pub fn compile_js(
        &self,
        note_path: &str,
        source_text: &str,
        revision: u64,
    ) -> Result<CompiledDocument, JsError> {
        compile_note(note_path, source_text, revision, self.fonts.clone())
            .map_err(|err| JsError::new(&format!("{err:?}")))
    }
}

impl Default for PreviewEngine {
    fn default() -> Self {
        Self::new()
    }
}
