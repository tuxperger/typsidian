//! Клавиши показа (research.md §6).
//!
//! У окна показа ядра нет, поэтому правило приезжает к нему таблицей. Набор
//! номера слайда цифрами, `Enter` и `Escape` при наборе — поведение поля
//! номера в окне, а не таблицы; `Enter` и `Backspace` здесь — для случая, когда
//! номер не набирается.

use super::{ShowAction, ShowKeyBinding, ShowKeymap};

/// Таблица по `KeyboardEvent.key`. Кликеры шлют `PageUp`/`PageDown`, часть —
/// стрелки.
pub fn show_keymap() -> ShowKeymap {
    use ShowAction::*;
    let table = [
        ("ArrowRight", Next),
        ("ArrowDown", Next),
        (" ", Next),
        ("PageDown", Next),
        ("Enter", Next),
        ("ArrowLeft", Previous),
        ("ArrowUp", Previous),
        ("PageUp", Previous),
        ("Backspace", Previous),
        ("Home", First),
        ("End", Last),
    ];
    ShowKeymap {
        bindings: table
            .into_iter()
            .map(|(key, action)| ShowKeyBinding {
                key: key.to_owned(),
                action,
            })
            .collect(),
    }
}
