//! Строение текста заметки и оглавление (фича 015).
//!
//! Один разбор — `typst::syntax::parse` — и один обход дерева ([`walk`]) на
//! все фичи, которым нужно знать, что записано в тексте: заголовки, рисунки,
//! определения меток и ссылки `@имя`. Оглавление (015) строится поверх
//! [`NoteStructure`], метки и ссылки (018) и цитаты (017) берут из него же:
//! **новый вопрос к тексту — новое поле и новая ветка в `walk.rs`**, а не
//! второй обход (contracts/core-structure-api.md § Расширение).
//!
//! Все функции чистые: ни файловой системы, ни состояния между вызовами, и ни
//! одна не паникует на любом тексте. Платформенного кода здесь нет
//! (принцип II), биндинги wasm — в `web.rs`.

pub mod outline;
pub mod plain;
pub mod view;
pub mod walk;

#[cfg(target_arch = "wasm32")]
pub mod web;

use serde::{Deserialize, Serialize};

pub use outline::{
    carry_collapsed, heading_probe, outline, Outline, OutlineItem, OutlineKind, OutlineKinds,
};
pub use view::{
    outline_active, outline_view, OutlineCounts, OutlineRequest, OutlineRow, OutlineState,
    OutlineView,
};
pub use walk::{mode_at, parse};

/// Место в тексте заметки (data-model.md «TextPlace»).
///
/// Назван `SourcePlace`, а не `TextPlace`: `workspace::TextPlace` (строка и
/// колонка, фича 014) уже экспортируется в TypeScript под этим именем, и два
/// типа с одним именем столкнулись бы в `.d.ts`.
///
/// `line` и `column` — с единицы, как у `preview::DiagnosticLocation`; `from`
/// и `to` — в кодовых единицах UTF-16, как у `elements::ParsedElement`.
/// Инвариант: `from <= to`; `from` указывает на первый знак записи (`=` у
/// заголовка, `#` у вызова, `<` у метки, `@` у ссылки).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SourcePlace {
    pub line: u32,
    pub column: u32,
    pub from: u32,
    pub to: u32,
}

/// Результат [`parse`]. Все списки — в порядке текста.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct NoteStructure {
    pub headings: Vec<HeadingSite>,
    pub figures: Vec<FigureSite>,
    pub labels: Vec<LabelSite>,
    pub refs: Vec<RefSite>,
    /// Вызовы `cite(…)` (фича 017, research.md §9).
    #[serde(default)]
    pub cites: Vec<CiteSite>,
    /// Вызовы `bibliography(…)` (фича 017).
    #[serde(default)]
    pub bibliographies: Vec<BibliographySite>,
}

/// Заголовок: запись знаками `=` или вызов `heading(…)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct HeadingSite {
    /// Уровень от единицы (research.md §4).
    pub level: u8,
    /// Читаемый текст (research.md §6).
    pub text: String,
    /// Читаемого текста не осталось: в `text` — начало исходной записи.
    pub text_is_source: bool,
    /// Привязанная метка, без угловых скобок.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub label: Option<String>,
    /// Запись заголовка целиком, без метки.
    pub place: SourcePlace,
}

/// Что показывает вызов `figure(…)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum FigureKind {
    Figure,
    Table,
}

/// Рисунок или таблица: вызов `figure(…)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct FigureSite {
    pub kind: FigureKind,
    /// Читаемый текст подписи; `None` — подписи нет.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub caption: Option<String>,
    /// Привязанная метка, без угловых скобок.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub label: Option<String>,
    /// Вызов `figure(…)` целиком.
    pub place: SourcePlace,
}

/// К чему привязана метка.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum LabelOwner {
    Heading,
    Figure,
    Other,
}

/// Определение метки — узел `Label` в разметке. Употребления в коде
/// (`#cite(<ключ>)`, `#link(<раздел>)`) сюда не попадают.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LabelSite {
    /// Имя без угловых скобок.
    pub name: String,
    pub attached: LabelOwner,
    /// Сама метка `<имя>`.
    pub place: SourcePlace,
}

/// Ссылка `@имя`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RefSite {
    /// Имя после `@`.
    pub target: String,
    /// Ссылка целиком, с дополнением `[…]`.
    pub place: SourcePlace,
}

/// Вызов `cite(<ключ>)` или `cite(label("ключ"))` (фича 017).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CiteSite {
    pub key: String,
    /// Вызов целиком, с `#` в разметке.
    pub place: SourcePlace,
}

/// Вызов `bibliography(…)` (фича 017, research.md §13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BibliographySite {
    /// Пути-литералы в порядке записи; пусто, если аргумент не литерал.
    pub paths: Vec<String>,
    /// Аргумент — строка или массив строк.
    pub literal: bool,
    /// Вызов целиком.
    pub place: SourcePlace,
    /// Первый позиционный аргумент — его заменяет предложение «дополнить».
    pub arg: SourcePlace,
}

/// Что записано в месте каретки (фича 017): от этого зависит запись цитаты.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum TextMode {
    Markup,
    Math,
    Code,
    String,
    Comment,
}
