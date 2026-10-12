//! wasm-биндинги оглавления заметки (contracts/core-structure-api.md →
//! Биндинги).
//!
//! Тонкие обёртки: правил здесь нет. Функции свободные и объектов ядра не
//! заимствуют, поэтому очередь вызовов wasm их не ждёт (как `elementAt`).
//! `parse` и `NoteStructure` наружу не выводятся: интерфейсу нужно только
//! оглавление.

use wasm_bindgen::prelude::*;

use super::{outline, view, Outline, OutlineRequest, OutlineView};

#[wasm_bindgen(js_name = noteOutline)]
pub fn note_outline_js(text: &str) -> Outline {
    outline::outline(text)
}

#[wasm_bindgen(js_name = noteOutlineView)]
pub fn note_outline_view_js(outline: Outline, request: OutlineRequest) -> OutlineView {
    view::outline_view(&outline, &request)
}

#[wasm_bindgen(js_name = noteOutlineActive)]
pub fn note_outline_active_js(outline: Outline, request: OutlineRequest) -> Option<String> {
    view::outline_active(&outline, &request)
}

#[wasm_bindgen(js_name = noteOutlineCarry)]
pub fn note_outline_carry_js(prev: Outline, next: Outline, collapsed: Vec<String>) -> Vec<String> {
    outline::carry_collapsed(&prev, &next, &collapsed)
}
