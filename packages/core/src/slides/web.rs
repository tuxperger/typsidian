//! wasm-биндинги показа слайдов (contracts/core-slides-api.md).
//!
//! Тонкие обёртки: правил здесь нет. Функции свободные и объектов ядра не
//! заимствуют (как в `structure/web.rs`).

use wasm_bindgen::prelude::*;

use super::{
    deck, keys, PageSlideInfo, ShowAction, ShowKeymap, ShowState, SlideLabel, SlidePlace,
    SlidesError,
};

#[wasm_bindgen(js_name = slidesStart)]
pub fn slides_start_js(
    slides: Vec<PageSlideInfo>,
    caret: Option<u32>,
    now_ms: f64,
) -> Result<ShowState, SlidesError> {
    deck::start(&slides, caret, now_ms)
}

#[wasm_bindgen(js_name = slidesNavigate)]
pub fn slides_navigate_js(
    state: ShowState,
    slides: Vec<PageSlideInfo>,
    action: ShowAction,
) -> ShowState {
    deck::navigate(state, &slides, action)
}

#[wasm_bindgen(js_name = slidesCarry)]
pub fn slides_carry_js(
    state: ShowState,
    slides: Vec<PageSlideInfo>,
) -> Result<ShowState, SlidesError> {
    deck::carry(state, &slides)
}

#[wasm_bindgen(js_name = slidesCaret)]
pub fn slides_caret_js(slides: Vec<PageSlideInfo>, page: u32) -> Option<SlidePlace> {
    deck::caret_at_page(&slides, page)
}

#[wasm_bindgen(js_name = slidesLabel)]
pub fn slides_label_js(slides: Vec<PageSlideInfo>, page: u32) -> SlideLabel {
    deck::slide_label(&slides, page)
}

#[wasm_bindgen(js_name = slidesKeymap)]
pub fn slides_keymap_js() -> ShowKeymap {
    keys::show_keymap()
}
