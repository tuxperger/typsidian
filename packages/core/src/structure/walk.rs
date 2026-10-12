//! Один обход дерева `typst::syntax::parse` (contracts/core-structure-api.md,
//! правила 1–9).
//!
//! Обход единственный: заголовки, рисунки, метки и ссылки собираются за один
//! проход, а новый вопрос к тексту (цитаты 017, ссылки 018) добавляет поле в
//! [`NoteStructure`] и ветку здесь, но не второй `parse`.

use std::ops::Range;

use typst::syntax::{ast, Lines, LinkedNode, SyntaxKind};

use super::plain::{plain_text, source_head, string_literal};
use super::{
    FigureKind, FigureSite, HeadingSite, LabelOwner, LabelSite, NoteStructure, RefSite, SourcePlace,
};

/// Разбирает текст заметки и собирает всё, что в нём записано.
///
/// Текст с ошибками даёт всё, что удалось распознать; узлы-ошибки
/// пропускаются. Не паникует на любом входе.
pub fn parse(text: &str) -> NoteStructure {
    let root = typst::syntax::parse(text);
    // Один раз на вызов: пересчёт байт в UTF-16 и в строку через
    // `elements::utf16_at` проходил бы текст с начала на каждый пункт и на
    // 2 000 заголовках стал бы квадратичным (research.md §3).
    let lines = Lines::new(text);
    let mut walker = Walker {
        lines: &lines,
        found: NoteStructure::default(),
    };
    walker.children(&LinkedNode::new(&root));
    walker.found
}

/// Что только что записано в списки: к нему может привязаться метка.
#[derive(Clone, Copy)]
enum Owned {
    Heading(usize),
    Figure(usize),
}

struct Walker<'a> {
    lines: &'a Lines<&'a str>,
    found: NoteStructure,
}

impl Walker<'_> {
    fn place(&self, range: Range<usize>) -> SourcePlace {
        let (line, column) = self
            .lines
            .byte_to_line_column(range.start)
            .unwrap_or((0, 0));
        let utf16 = |byte| self.lines.byte_to_utf16(byte).unwrap_or(0) as u32;
        SourcePlace {
            line: line as u32 + 1,
            column: column as u32 + 1,
            from: utf16(range.start),
            to: utf16(range.end),
        }
    }

    /// Обходит детей узла по порядку. В разметке следит за соседями, чтобы
    /// привязать метку к предыдущему элементу (правило 6).
    fn children(&mut self, node: &LinkedNode) {
        let in_markup = node.kind() == SyntaxKind::Markup;
        let mut owner: Option<Owned> = None;
        let mut hash: Option<usize> = None;
        for child in node.children() {
            match child.kind() {
                // Пробел не разрывает привязку: метка может стоять на
                // следующей строке; разрыв абзаца — разрывает.
                SyntaxKind::Space => continue,
                SyntaxKind::Parbreak => {
                    owner = None;
                    hash = None;
                    continue;
                }
                SyntaxKind::Hash => {
                    hash = Some(child.offset());
                    continue;
                }
                // Определение метки — только метка в разметке; в коде
                // (`#cite(<ключ>)`) это употребление (правило 5).
                SyntaxKind::Label if in_markup => {
                    self.label(&child, owner.take());
                    hash = None;
                    continue;
                }
                _ => {}
            }
            owner = self.visit(&child, hash.take());
        }
    }

    fn visit(&mut self, node: &LinkedNode, hash: Option<usize>) -> Option<Owned> {
        match node.kind() {
            // Правило 2: здесь заголовки создаются вычислением либо лежат в
            // другом файле.
            SyntaxKind::Closure
            | SyntaxKind::ForLoop
            | SyntaxKind::WhileLoop
            | SyntaxKind::ShowRule
            | SyntaxKind::SetRule
            | SyntaxKind::ModuleImport
            | SyntaxKind::ModuleInclude
            | SyntaxKind::Error => None,
            SyntaxKind::Heading => Some(self.heading(node)),
            SyntaxKind::FuncCall => self.call(node, hash),
            SyntaxKind::Ref => {
                self.reference(node);
                None
            }
            _ => {
                self.children(node);
                None
            }
        }
    }

    fn heading(&mut self, node: &LinkedNode) -> Owned {
        let level = node
            .cast::<ast::Heading>()
            .map_or(1, |heading| heading.depth().get());
        let place = self.place(node.range());
        Owned::Heading(self.push_heading(level, node, plain_text(node), place))
    }

    fn push_heading(
        &mut self,
        level: usize,
        node: &LinkedNode,
        readable: String,
        place: SourcePlace,
    ) -> usize {
        let (text, text_is_source) = if readable.is_empty() {
            (source_head(node), true)
        } else {
            (readable, false)
        };
        self.found.headings.push(HeadingSite {
            level: level.clamp(1, u8::MAX as usize) as u8,
            text,
            text_is_source,
            label: None,
            place,
        });
        self.found.headings.len() - 1
    }

    fn call(&mut self, node: &LinkedNode, hash: Option<usize>) -> Option<Owned> {
        let callee = node
            .children()
            .next()
            .filter(|first| first.kind() == SyntaxKind::Ident)
            .map(|first| first.leaf_text().to_string());
        // Запись вызова в разметке начинается с `#`, а не с имени функции.
        let place = self.place(hash.unwrap_or(node.offset())..node.range().end);
        match callee.as_deref() {
            Some("heading") => {
                let args = args_of(node);
                let level = args.as_ref().map_or(1, heading_level);
                let readable = args.as_ref().map(body_text).unwrap_or_default();
                Some(Owned::Heading(
                    self.push_heading(level, node, readable, place),
                ))
            }
            Some("figure") => {
                let args = args_of(node);
                let (kind, caption) = args
                    .as_ref()
                    .map_or((FigureKind::Figure, None), figure_facts);
                self.found.figures.push(FigureSite {
                    kind,
                    caption,
                    label: None,
                    place,
                });
                let index = self.found.figures.len() - 1;
                // Внутри рисунка тоже бывают ссылки и метки.
                self.children(node);
                Some(Owned::Figure(index))
            }
            _ => {
                self.children(node);
                None
            }
        }
    }

    fn reference(&mut self, node: &LinkedNode) {
        if let Some(reference) = node.cast::<ast::Ref>() {
            self.found.refs.push(RefSite {
                target: reference.target().to_string(),
                place: self.place(node.range()),
            });
        }
    }

    fn label(&mut self, node: &LinkedNode, owner: Option<Owned>) {
        let Some(label) = node.cast::<ast::Label>() else {
            return;
        };
        let name = label.get().to_string();
        // Вторая метка у того же элемента к нему уже не привязана.
        let attached = match owner {
            Some(Owned::Heading(index)) if self.found.headings[index].label.is_none() => {
                self.found.headings[index].label = Some(name.clone());
                LabelOwner::Heading
            }
            Some(Owned::Figure(index)) if self.found.figures[index].label.is_none() => {
                self.found.figures[index].label = Some(name.clone());
                LabelOwner::Figure
            }
            _ => LabelOwner::Other,
        };
        self.found.labels.push(LabelSite {
            name,
            attached,
            place: self.place(node.range()),
        });
    }
}

fn args_of<'a>(call: &LinkedNode<'a>) -> Option<LinkedNode<'a>> {
    call.children()
        .find(|child| child.kind() == SyntaxKind::Args)
}

/// Именованные аргументы: имя и значение.
fn named<'a>(args: &LinkedNode<'a>) -> Vec<(String, LinkedNode<'a>)> {
    args.children()
        .filter(|child| child.kind() == SyntaxKind::Named)
        .filter_map(|child| {
            let name = child.children().next()?.leaf_text().to_string();
            let value = child.children().next_back()?;
            Some((name, value))
        })
        .collect()
}

fn int_literal(node: &LinkedNode) -> Option<i64> {
    (node.kind() == SyntaxKind::Int)
        .then(|| node.leaf_text().parse::<i64>().ok())
        .flatten()
}

/// Уровень вызова `heading(…)` (research.md §4): целый литерал `level:`;
/// без него — сумма литералов `offset:` и `depth:` (по умолчанию 0 и 1);
/// уровень, заданный выражением, — 1.
fn heading_level(args: &LinkedNode) -> usize {
    let named = named(args);
    let find = |name: &str| named.iter().find(|(key, _)| key == name).map(|(_, v)| v);
    if let Some(value) = find("level") {
        if value.kind() != SyntaxKind::Auto {
            return int_literal(value).filter(|n| *n >= 1).unwrap_or(1) as usize;
        }
    }
    let number = |name: &str, default: i64| match find(name) {
        Some(value) => int_literal(value),
        None => Some(default),
    };
    match (number("offset", 0), number("depth", 1)) {
        (Some(offset), Some(depth)) => (offset + depth).max(1) as usize,
        _ => 1,
    }
}

/// Текст тела вызова: первый блок содержимого или строка среди позиционных
/// аргументов.
fn body_text(args: &LinkedNode) -> String {
    for child in args.children() {
        match child.kind() {
            SyntaxKind::ContentBlock => return plain_text(&child),
            SyntaxKind::Str => return string_literal(&child).unwrap_or_default().trim().to_owned(),
            _ => {}
        }
    }
    String::new()
}

/// Вид и подпись рисунка (research.md §5).
fn figure_facts(args: &LinkedNode) -> (FigureKind, Option<String>) {
    let named = named(args);
    let is_table_name = |value: &LinkedNode| {
        (value.kind() == SyntaxKind::Ident && value.leaf_text() == "table")
            || string_literal(value).is_some_and(|text| text == "table")
    };
    let positional_table = args
        .children()
        .find(|child| {
            !matches!(
                child.kind(),
                SyntaxKind::LeftParen
                    | SyntaxKind::RightParen
                    | SyntaxKind::Comma
                    | SyntaxKind::Space
                    | SyntaxKind::Named
                    | SyntaxKind::ContentBlock
            )
        })
        .is_some_and(|first| {
            first.kind() == SyntaxKind::FuncCall
                && first.children().next().is_some_and(|callee| {
                    callee.kind() == SyntaxKind::Ident && callee.leaf_text() == "table"
                })
        });
    let kind_named = named
        .iter()
        .find(|(key, _)| key == "kind")
        .is_some_and(|(_, value)| is_table_name(value));
    let kind = if positional_table || kind_named {
        FigureKind::Table
    } else {
        FigureKind::Figure
    };
    let caption = named
        .iter()
        .find(|(key, _)| key == "caption")
        .and_then(|(_, value)| match value.kind() {
            SyntaxKind::ContentBlock => Some(plain_text(value)),
            SyntaxKind::Str => string_literal(value).map(|text| text.trim().to_owned()),
            _ => None,
        })
        .filter(|text| !text.is_empty());
    (kind, caption)
}
