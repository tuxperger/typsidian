//! Где каретка: ссылка на метку, ссылка на заметку или ничего (research.md §2,
//! FR-004).
//!
//! Открывающий знак ищется назад от каретки в той же строке; режим места
//! решает тот же разбор Typst, что сборка, — ближайший предок листа под
//! открывающим знаком. Своего разбора текста здесь нет.

use typst::syntax::{LinkedNode, Side, SyntaxKind, SyntaxNode};

use super::NoCompletionReason;
use crate::text_offsets::{byte_at, utf16_at};

/// Что набирается у каретки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// `@имя`.
    Label,
    /// `[[цель`; `closed` — сразу за кареткой уже стоит `]]`.
    Note { closed: bool },
    /// Строка цели `#embed("…")` (031): до `#` — заметка, после — раздел или
    /// метка заметки [`CaretContext::embed_note`].
    Embed { part: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaretContext {
    pub target: Trigger,
    /// Набранная часть после открывающего знака (UTF-16).
    pub from: u32,
    pub to: u32,
    pub query: String,
    /// Заметка цели встраивания, когда набирается её раздел или метка.
    pub embed_note: String,
}

/// Знак, который может стоять в имени метки после `@`.
fn name_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.')
}

/// Контекст каретки `offset` (UTF-16). `manual` — вызов командой: тот же
/// поиск, но отказ значим для пользователя (FR-009).
pub fn at(text: &str, offset: u32, manual: bool) -> Result<CaretContext, NoCompletionReason> {
    at_in(&typst::syntax::parse(text), text, offset, manual)
}

/// То же над уже разобранным деревом текста: [`super::completions`] разбирает
/// заметку один раз на контекст и на метки.
pub fn at_in(
    root: &SyntaxNode,
    text: &str,
    offset: u32,
    manual: bool,
) -> Result<CaretContext, NoCompletionReason> {
    let _ = manual;
    let caret = byte_at(text, offset as usize);
    let line_start = text[..caret].rfind('\n').map_or(0, |at| at + 1);
    let before = &text[line_start..caret];

    // Строка цели встраивания (031): `embed("` и до каретки нет кавычки.
    if let Some(embed) = embed_context(root, text, line_start, before, caret, offset) {
        return Ok(embed);
    }

    // Ссылка на метку: `@` и после него только знаки имени.
    let name_start = before
        .char_indices()
        .rev()
        .take_while(|(_, ch)| name_char(*ch))
        .last()
        .map_or(before.len(), |(at, _)| at);
    let label = before[..name_start]
        .strip_suffix('@')
        .map(|head| (line_start + head.len(), head.chars().next_back()));
    // Ссылка на заметку: `[[` и до каретки ни `]`, ни перевода строки.
    let note = before
        .rfind("[[")
        .filter(|at| !before[at + 2..].contains(']'));

    let (trigger, open_byte, query_byte) = match (label, note) {
        // Ближайший к каретке открывающий знак решает.
        (Some((at, previous)), note) if note.is_none_or(|open| line_start + open < at) => {
            // `user@example.com`: перед `@` буква или цифра — адрес, не ссылка.
            if previous.is_some_and(char::is_alphanumeric) {
                return Err(NoCompletionReason::InAddress);
            }
            (Trigger::Label, at, at + 1)
        }
        (_, Some(open)) => {
            let closed = text[caret..].starts_with("]]");
            (
                Trigger::Note { closed },
                line_start + open,
                line_start + open + 2,
            )
        }
        _ => return Err(NoCompletionReason::NoTrigger),
    };

    mode_allows(root, open_byte)?;
    Ok(CaretContext {
        target: trigger,
        from: utf16_at(text, query_byte) as u32,
        to: offset.min(utf16_at(text, text.len()) as u32),
        query: text[query_byte..caret].to_owned(),
        embed_note: String::new(),
    })
}

/// Каретка в строке первого аргумента `embed("…")`: лист у кавычки — строка
/// (или незакрытая строка), а не текст разметки.
fn embed_context(
    root: &SyntaxNode,
    text: &str,
    line_start: usize,
    before: &str,
    caret: usize,
    offset: u32,
) -> Option<CaretContext> {
    const OPEN: &str = "embed(\"";
    let open = before.rfind(OPEN)?;
    let inner = &before[open + OPEN.len()..];
    if inner.contains('"') {
        return None;
    }
    let quote = line_start + open + OPEN.len() - 1;
    let linked = LinkedNode::new(root);
    let leaf = linked.leaf_at(quote + 1, Side::After)?;
    if !matches!(leaf.kind(), SyntaxKind::Str | SyntaxKind::Error) {
        return None;
    }
    let (part, embed_note, query_byte) = match inner.find('#') {
        Some(hash) => (true, inner[..hash].trim().to_owned(), quote + 1 + hash + 1),
        None => (false, String::new(), quote + 1),
    };
    Some(CaretContext {
        target: Trigger::Embed { part },
        from: utf16_at(text, query_byte) as u32,
        to: offset.min(utf16_at(text, text.len()) as u32),
        query: text[query_byte..caret].to_owned(),
        embed_note,
    })
}

/// Режим места открывающего знака: разметка — предлагать, прочее — нет.
fn mode_allows(root: &SyntaxNode, open_byte: usize) -> Result<(), NoCompletionReason> {
    let linked = LinkedNode::new(root);
    // Лист, которому принадлежит сам открывающий знак.
    let Some(leaf) = linked.leaf_at(open_byte + 1, Side::Before) else {
        return Ok(());
    };
    match leaf.kind() {
        SyntaxKind::Str => return Err(NoCompletionReason::InString),
        SyntaxKind::LineComment | SyntaxKind::BlockComment => {
            return Err(NoCompletionReason::InComment)
        }
        SyntaxKind::Link => return Err(NoCompletionReason::InAddress),
        SyntaxKind::Escape => return Err(NoCompletionReason::NoTrigger),
        _ => {}
    }
    let mut node = leaf.parent().cloned();
    while let Some(current) = node {
        match current.kind() {
            SyntaxKind::Raw => return Err(NoCompletionReason::InRaw),
            SyntaxKind::Math | SyntaxKind::Equation => return Err(NoCompletionReason::InMath),
            SyntaxKind::Markup | SyntaxKind::ContentBlock => return Ok(()),
            SyntaxKind::Code
            | SyntaxKind::CodeBlock
            | SyntaxKind::Args
            | SyntaxKind::Array
            | SyntaxKind::Dict
            | SyntaxKind::Parenthesized
            | SyntaxKind::LetBinding
            | SyntaxKind::Closure
            | SyntaxKind::Conditional
            | SyntaxKind::ForLoop
            | SyntaxKind::WhileLoop
            | SyntaxKind::FuncCall => return Err(NoCompletionReason::InCode),
            _ => node = current.parent().cloned(),
        }
    }
    Ok(())
}
