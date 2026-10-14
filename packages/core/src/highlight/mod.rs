//! Подсветка синтаксиса Typst (фича 024, contracts/core-highlight-api.md).
//!
//! Роли фрагментов берутся у того же парсера, которым ядро собирает заметку, —
//! `typst::syntax::highlight`, с уточнениями под палитру редактора
//! (research.md §4). Раскраска, разошедшаяся со сборкой, — ошибка подсветки.
//!
//! Ни файловой системы, ни сети. Разбирается весь текст (режим в любом месте
//! зависит от всего, что выше), а отдаются только фрагменты, пересекающие окна
//! редактора (research.md §2). Редактор держит [`HighlightSession`]: разбор
//! обновляется правками, а не делается заново на каждое нажатие.
//! Смещения — в кодовых единицах UTF-16, как считает редактор (research.md §3).

mod roles;
mod session;
mod walk;

#[cfg(target_arch = "wasm32")]
mod web;

use serde::{Deserialize, Serialize};

pub use session::{HighlightEdit, HighlightSession};

/// Роль фрагмента. Номер — часть контракта с интерфейсом
/// (`ui/src/editor/highlight/roles.ts`): новые роли — только в конец.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SyntaxRole {
    Comment = 0,
    Punctuation = 1,
    Escape = 2,
    MarkupMarker = 3,
    HeadingMarker = 4,
    ListMarker = 5,
    ListTerm = 6,
    Link = 7,
    Raw = 8,
    Label = 9,
    Ref = 10,
    MathDelimiter = 11,
    MathOperator = 12,
    MathIdent = 13,
    Keyword = 14,
    Constant = 15,
    Operator = 16,
    Number = 17,
    String = 18,
    Function = 19,
    ArgName = 20,
    Variable = 21,
    /// Текст без своей роли, но с начертанием от предков (`*…*`, заголовок).
    Plain = 22,
}

impl SyntaxRole {
    /// Все роли в порядке номеров — для сверки с интерфейсом и разбора ответа.
    pub const ALL: [SyntaxRole; 23] = [
        SyntaxRole::Comment,
        SyntaxRole::Punctuation,
        SyntaxRole::Escape,
        SyntaxRole::MarkupMarker,
        SyntaxRole::HeadingMarker,
        SyntaxRole::ListMarker,
        SyntaxRole::ListTerm,
        SyntaxRole::Link,
        SyntaxRole::Raw,
        SyntaxRole::Label,
        SyntaxRole::Ref,
        SyntaxRole::MathDelimiter,
        SyntaxRole::MathOperator,
        SyntaxRole::MathIdent,
        SyntaxRole::Keyword,
        SyntaxRole::Constant,
        SyntaxRole::Operator,
        SyntaxRole::Number,
        SyntaxRole::String,
        SyntaxRole::Function,
        SyntaxRole::ArgName,
        SyntaxRole::Variable,
        SyntaxRole::Plain,
    ];

    pub fn from_u8(value: u8) -> Option<SyntaxRole> {
        SyntaxRole::ALL.get(usize::from(value)).copied()
    }
}

/// Начертание, унаследованное от предков узла.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StyleFlags(pub u8);

impl StyleFlags {
    pub const NONE: StyleFlags = StyleFlags(0);
    pub const STRONG: StyleFlags = StyleFlags(1);
    pub const EMPH: StyleFlags = StyleFlags(1 << 1);
    pub const HEADING: StyleFlags = StyleFlags(1 << 2);

    pub fn contains(self, other: StyleFlags) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for StyleFlags {
    type Output = StyleFlags;
    fn bitor(self, other: StyleFlags) -> StyleFlags {
        StyleFlags(self.0 | other.0)
    }
}

/// Окно редактора `[from, to)` в кодовых единицах UTF-16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct HighlightWindow {
    pub from: usize,
    pub to: usize,
}

/// Один фрагмент ответа в развёрнутом виде.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightSpan {
    pub from: usize,
    pub to: usize,
    pub role: SyntaxRole,
    pub flags: StyleFlags,
}

/// Ответ: тройки `[from, to, role | flags << 8]` (data-model.md).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HighlightSpans {
    pub packed: Vec<u32>,
}

impl HighlightSpans {
    /// Развернуть тройки — для тестов и отладки; интерфейс читает `packed`.
    pub fn spans(&self) -> Vec<HighlightSpan> {
        self.packed
            .chunks_exact(3)
            .map(|triple| HighlightSpan {
                from: triple[0] as usize,
                to: triple[1] as usize,
                role: SyntaxRole::from_u8((triple[2] & 0xff) as u8)
                    .expect("ядро пишет только известные роли"),
                flags: StyleFlags((triple[2] >> 8) as u8),
            })
            .collect()
    }
}

/// Раскраска текста без сессии — разбор с нуля. Пустой список окон — весь текст.
pub fn highlight(text: &str, windows: &[HighlightWindow]) -> HighlightSpans {
    walk::highlight(text, windows)
}
