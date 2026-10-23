//! Режим показа слайдов (фича 019).
//!
//! Слайд — страница собранного документа. Ядро делает три вещи и ничего не
//! хранит между вызовами (принцип I, одинаково в wasm и в Tauri):
//!
//! - [`collect`] при сборке (005) записывает про каждую страницу, какой текст
//!   заметки на ней стоит, какой у неё логический номер слайда и какие заметки
//!   докладчика к ней приложены;
//! - [`deck`] отвечает на вопросы показа: с какой страницы начать, куда
//!   перейти, как подписать счётчик, куда вернуть каретку;
//! - [`keys`] отдаёт таблицу клавиш показа окну, у которого ядра нет.
//!
//! Состояние показа — значение [`ShowState`]: его держит окно редактора и
//! передаёт в ядро вместе с действием (research.md §4).

pub(crate) mod collect;
pub mod deck;
pub mod export;
pub mod keys;
#[cfg(target_arch = "wasm32")]
pub mod web;

use serde::{Deserialize, Serialize};

use crate::messages::AppMessage;

pub use deck::{caret_at_page, carry, navigate, slide_label, start};
pub use keys::show_keymap;

/// Место в тексте заметки: смещение для сравнения с кареткой и строка с
/// колонкой для перехода в редакторе (data-model.md «SlidePlace»).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SlidePlace {
    /// Смещение в кодовых единицах UTF-16, как у каретки редактора.
    pub offset: u32,
    /// Строка, с нуля.
    pub line: u32,
    /// Колонка, с нуля.
    pub column: u32,
}

/// Сведения о странице собранного документа, нужные показу.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PageSlideInfo {
    /// Физический номер страницы, с нуля; совпадает с `RenderedPage.index`.
    pub page_index: u32,
    /// Начало текста заметки на странице; `None` — на странице нет текста
    /// главного файла (картинка, текст из `#include`).
    pub source_start: Option<SlidePlace>,
    /// Номер логического слайда из разметки `<pdfpc>`, с единицы.
    pub logical_slide: Option<u32>,
    /// Заметки докладчика к странице.
    pub notes: Option<String>,
}

/// Состояние показа. Держит окно редактора, ядро возвращает новое значение.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ShowState {
    /// Текущая страница, с нуля; всегда `< total`.
    pub page: u32,
    /// Число страниц сборки, на которой стоит показ.
    pub total: u32,
    /// Момент запуска по часам окна (`Date.now()`).
    pub started_at_ms: f64,
}

/// Действие листания.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ShowAction {
    Next,
    Previous,
    First,
    Last,
    /// Перейти к слайду с этим номером (с единицы).
    GoToSlide {
        number: u32,
    },
    /// Перейти к странице (с нуля) — щелчок по ленте вида докладчика.
    GoToPage {
        index: u32,
    },
}

/// Подпись для счётчиков показа.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SlideLabel {
    /// Номер слайда: логический, если известен, иначе страница + 1.
    pub current: u32,
    /// Число слайдов: логических или страниц.
    pub total: u32,
    /// Номер шага внутри слайда, с единицы; `None` — шаг один или слайды не
    /// размечены.
    pub step: Option<u32>,
    /// Число шагов слайда.
    pub steps: Option<u32>,
    /// Страница «Дальше» для вида докладчика; `None` на последней.
    pub next_page: Option<u32>,
}

/// Одна клавиша показа.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ShowKeyBinding {
    /// Значение `KeyboardEvent.key`.
    pub key: String,
    pub action: ShowAction,
}

/// Таблица клавиш показа (research.md §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ShowKeymap {
    pub bindings: Vec<ShowKeyBinding>,
}

/// Почему показ не запустился.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum SlidesError {
    /// У сборки нет страниц: показывать нечего.
    #[error("в документе нет страниц")]
    EmptyDeck,
}

impl SlidesError {
    /// Сообщение для пользователя (010): текст собирает интерфейс.
    pub fn message(&self) -> AppMessage {
        match self {
            SlidesError::EmptyDeck => AppMessage::SlidesEmptyDeck,
        }
    }
}
