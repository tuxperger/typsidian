//! wasm-биндинг подсветки (contracts/core-highlight-api.md → wasm).
//!
//! Сессия — объект на редактор: живёт столько же, сколько вид CodeMirror, и
//! освобождается им (`free`). Ответ — `Uint32Array` троек
//! `[from, to, role | flags << 8]` или `undefined`, если сессия разошлась с
//! текстом редактора.

use wasm_bindgen::prelude::*;

use super::{HighlightEdit, HighlightSession, HighlightWindow};

#[wasm_bindgen(js_name = SyntaxSession)]
pub struct SyntaxSessionJs(HighlightSession);

#[wasm_bindgen(js_class = SyntaxSession)]
impl SyntaxSessionJs {
    #[wasm_bindgen(constructor)]
    pub fn new(text: String) -> SyntaxSessionJs {
        SyntaxSessionJs(HighlightSession::new(text))
    }

    pub fn update(
        &mut self,
        edits: Vec<HighlightEdit>,
        length: usize,
        windows: Vec<HighlightWindow>,
    ) -> Option<Vec<u32>> {
        self.0
            .update(&edits, length, &windows)
            .map(|spans| spans.packed)
    }

    /// Слова с ошибками в окнах — пары `[from, to)` (фича 030). Правки сессия
    /// уже получила в `update`: проверка идёт по её тексту.
    pub fn spelling(
        &self,
        windows: Vec<HighlightWindow>,
        checker: &crate::spellcheck::web::SpellCheckerJs,
    ) -> Vec<u32> {
        self.0.spelling(&windows, checker.inner())
    }
}
