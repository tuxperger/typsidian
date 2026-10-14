//! Сессия подсветки одного редактора (research.md §2, запасной вариант).
//!
//! Полный разбор заметки на 5 000 строк стоит ~2 мс нативно и втрое больше в
//! wasm — на каждое нажатие это съедает кадр (SC-002, замер — quickstart §4).
//! Сессия держит разобранный текст и применяет к нему правки редактора:
//! инкрементальный разбор Typst (`Source::edit`) стоит доли миллисекунды и
//! даёт то же дерево, что и разбор с нуля (FR-007).
//!
//! Текст сессии — копия текста редактора. Каждая правка сверяется по итоговой
//! длине: расхождение — не ошибка ядра, а сигнал клиенту открыть сессию заново.

use serde::{Deserialize, Serialize};
use typst::syntax::Source;

use super::{walk, HighlightSpans, HighlightWindow};
use crate::text_offsets::byte_at;

/// Правка текста редактора в кодовых единицах UTF-16: `[from, to)` заменяется
/// на `insert`. Координаты — в тексте после применения предыдущих правок списка.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct HighlightEdit {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

pub struct HighlightSession {
    source: Source,
}

impl HighlightSession {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            source: Source::detached(text),
        }
    }

    pub fn text(&self) -> &str {
        self.source.text()
    }

    /// Применить правки по очереди. `false` — правка вне текста или итоговая
    /// длина (`len_utf16`) не сходится с редактором: сессия больше не копия, её
    /// надо открыть заново с полным текстом.
    pub fn edit(&mut self, edits: &[HighlightEdit], len_utf16: usize) -> bool {
        for edit in edits {
            let lines = self.source.lines();
            if edit.from > edit.to || edit.to > lines.len_utf16() {
                return false;
            }
            let text = self.source.text();
            let to_byte = |utf16| {
                lines
                    .utf16_to_byte(utf16)
                    .unwrap_or_else(|| byte_at(text, utf16))
            };
            let range = to_byte(edit.from)..to_byte(edit.to);
            self.source.edit(range, &edit.insert);
        }
        self.source.lines().len_utf16() == len_utf16
    }

    pub fn highlight(&self, windows: &[HighlightWindow]) -> HighlightSpans {
        walk::highlight_source(&self.source, windows)
    }

    /// Правки и раскраска одним вызовом — так их зовут клиенты: на Tauri это
    /// один обход IPC вместо двух. `None` — сессия разошлась с редактором.
    pub fn update(
        &mut self,
        edits: &[HighlightEdit],
        len_utf16: usize,
        windows: &[HighlightWindow],
    ) -> Option<HighlightSpans> {
        self.edit(edits, len_utf16).then(|| self.highlight(windows))
    }
}
