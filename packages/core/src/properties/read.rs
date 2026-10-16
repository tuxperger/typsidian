//! Разбор записи `<properties>` (research.md §1–§2, contracts/core-properties-api.md).
//!
//! Один `typst::syntax::parse` и обход верхнего уровня разметки. Запись
//! **однозначна**, если её аргумент — словарь из литералов без повторов ключей
//! и в заметке одна такая запись; иначе читаются те значения, что литералы, а
//! правка формой выключается (FR-004).

use std::ops::Range;

use typst::syntax::ast::AstNode;
use typst::syntax::{ast, Lines, LinkedNode, SyntaxKind, SyntaxNode};

use super::{valid_date, NoteProperties, PropertiesProblem, Property, PropertyValue, RECORD_LABEL};
use crate::elements::values::has_error;
use crate::structure::SourcePlace;

/// Запись, найденная в тексте: диапазон и узлы словаря.
pub(crate) struct Record<'a> {
    /// От `#` до конца метки.
    pub range: Range<usize>,
    /// Вызов `metadata(…)`.
    pub call: LinkedNode<'a>,
}

/// Прочитанный элемент словаря: где ключ, где значение и что в них.
pub(crate) struct DictEntry {
    pub key: Option<String>,
    pub key_range: Range<usize>,
    pub value_range: Range<usize>,
    pub value: PropertyValue,
    /// Диапазон пары целиком, от ключа до конца значения.
    pub item_range: Range<usize>,
}

/// Результат поиска записей в тексте.
pub(crate) struct Found<'a> {
    pub records: Vec<Record<'a>>,
}

/// Находит записи `#metadata(…) <properties>` на верхнем уровне разметки.
pub(crate) fn find_records<'a>(root: &'a LinkedNode<'a>) -> Found<'a> {
    let children: Vec<LinkedNode<'a>> = root.children().collect();
    let mut records = Vec::new();
    let mut index = 0;
    while index < children.len() {
        let child = &children[index];
        if child.kind() == SyntaxKind::Hash {
            if let Some(call) = children.get(index + 1).filter(|n| is_metadata_call(n)) {
                if let Some(label) = label_after(&children, index + 2) {
                    records.push(Record {
                        range: child.offset()..label.range().end,
                        call: call.clone(),
                    });
                    index += 2;
                }
            }
        }
        index += 1;
    }
    Found { records }
}

fn is_metadata_call(node: &LinkedNode) -> bool {
    node.kind() == SyntaxKind::FuncCall
        && node.children().next().is_some_and(|callee| {
            callee.kind() == SyntaxKind::Ident && callee.get().full_text() == "metadata"
        })
}

/// Метка `<properties>` сразу после вызова: допускается пробел, но не пустая
/// строка (она разрывает абзац и отрывает метку от вызова).
fn label_after<'a>(children: &[LinkedNode<'a>], from: usize) -> Option<LinkedNode<'a>> {
    let mut at = from;
    if children
        .get(at)
        .is_some_and(|n| n.kind() == SyntaxKind::Space)
    {
        at += 1;
    }
    let node = children.get(at)?;
    (node.kind() == SyntaxKind::Label
        && node.get().full_text().trim_matches(['<', '>']) == RECORD_LABEL)
        .then(|| node.clone())
}

/// Словарь — единственный позиционный аргумент вызова `metadata`.
pub(crate) fn dict_of<'a>(call: &LinkedNode<'a>) -> Option<LinkedNode<'a>> {
    let args = call.children().find(|n| n.kind() == SyntaxKind::Args)?;
    let mut values = args.children().filter(|n| {
        !n.kind().is_trivia()
            && !matches!(
                n.kind(),
                SyntaxKind::LeftParen | SyntaxKind::RightParen | SyntaxKind::Comma
            )
    });
    let first = values.next()?;
    if values.next().is_some() {
        return None;
    }
    (first.kind() == SyntaxKind::Dict).then_some(first)
}

/// Разбор записи в свойства и найденные проблемы.
pub(crate) fn dict_entries(
    text: &str,
    dict: &LinkedNode,
) -> (Vec<DictEntry>, Vec<PropertiesProblem>) {
    let mut entries = Vec::new();
    let mut problems = Vec::new();
    for item in dict.children() {
        match item.kind() {
            SyntaxKind::Named | SyntaxKind::Keyed => {
                let mut parts = item.children().filter(|n| !n.kind().is_trivia());
                let Some(key_node) = parts.next() else {
                    continue;
                };
                let Some(value_node) = item.children().rfind(|n| !n.kind().is_trivia()) else {
                    continue;
                };
                let key = match key_node.kind() {
                    SyntaxKind::Ident => Some(key_node.get().full_text().to_string()),
                    SyntaxKind::Str => key_node.cast::<ast::Str>().map(|s| s.get().to_string()),
                    // Повтор ключа парсер Typst помечает ошибкой на втором ключе.
                    SyntaxKind::Error
                        if key_node
                            .get()
                            .errors_and_warnings()
                            .0
                            .iter()
                            .any(|error| error.message.starts_with("duplicate key")) =>
                    {
                        let key = key_node.get().full_text().to_string();
                        problems.push(PropertiesProblem::DuplicateKey { key: key.clone() });
                        Some(key)
                    }
                    _ => None,
                };
                let value = literal_value(value_node.get(), text, value_node.range());
                if matches!(value, PropertyValue::Unknown { .. }) {
                    problems.push(PropertiesProblem::NotLiteral {
                        key: key.clone().unwrap_or_default(),
                    });
                }
                if key.is_none() {
                    problems.push(PropertiesProblem::NotLiteral { key: String::new() });
                }
                entries.push(DictEntry {
                    key,
                    key_range: key_node.range(),
                    value_range: value_node.range(),
                    value,
                    item_range: item.range(),
                });
            }
            SyntaxKind::Spread => problems.push(PropertiesProblem::Spread),
            _ => {}
        }
    }
    // Повтор ключа.
    for (position, entry) in entries.iter().enumerate() {
        if let Some(key) = &entry.key {
            if entries[..position]
                .iter()
                .any(|before| before.key.as_ref() == Some(key))
            {
                problems.push(PropertiesProblem::DuplicateKey { key: key.clone() });
            }
        }
    }
    (entries, problems)
}

/// Литерал → значение; всё остальное — `Unknown` с исходным текстом.
fn literal_value(node: &SyntaxNode, text: &str, range: Range<usize>) -> PropertyValue {
    literal(node).unwrap_or_else(|| PropertyValue::Unknown {
        source: text[range].to_string(),
    })
}

fn literal(node: &SyntaxNode) -> Option<PropertyValue> {
    match node.kind() {
        SyntaxKind::Str => Some(PropertyValue::Text {
            value: node.cast::<ast::Str>()?.get().to_string(),
        }),
        SyntaxKind::Int => Some(PropertyValue::Number {
            value: node.cast::<ast::Int>()?.get() as f64,
        }),
        SyntaxKind::Float => Some(PropertyValue::Number {
            value: node.cast::<ast::Float>()?.get(),
        }),
        SyntaxKind::Bool => Some(PropertyValue::Flag {
            value: node.cast::<ast::Bool>()?.get(),
        }),
        SyntaxKind::Unary => negative_number(node),
        SyntaxKind::Array => string_array(node),
        SyntaxKind::FuncCall => datetime_call(node),
        _ => None,
    }
}

/// `-5`, `-1.5`: унарный минус перед числом.
fn negative_number(node: &SyntaxNode) -> Option<PropertyValue> {
    let unary = node.cast::<ast::Unary>()?;
    if !matches!(unary.op(), ast::UnOp::Neg) {
        return None;
    }
    match literal(unary.expr().to_untyped())? {
        PropertyValue::Number { value } => Some(PropertyValue::Number { value: -value }),
        _ => None,
    }
}

/// `("a", "b")`, `()`: массив из строк.
fn string_array(node: &SyntaxNode) -> Option<PropertyValue> {
    let mut values = Vec::new();
    for child in node.children() {
        match child.kind() {
            SyntaxKind::Str => values.push(child.cast::<ast::Str>()?.get().to_string()),
            kind if kind.is_trivia()
                || matches!(
                    kind,
                    SyntaxKind::LeftParen | SyntaxKind::RightParen | SyntaxKind::Comma
                ) => {}
            _ => return None,
        }
    }
    Some(PropertyValue::List { values })
}

/// `datetime(year: 2026, month: 10, day: 15)`: все три именованных числа.
fn datetime_call(node: &SyntaxNode) -> Option<PropertyValue> {
    let callee = node.children().next()?;
    if callee.kind() != SyntaxKind::Ident || callee.full_text() != "datetime" {
        return None;
    }
    let args = node.children().find(|n| n.kind() == SyntaxKind::Args)?;
    let (mut year, mut month, mut day) = (None, None, None);
    for arg in args.children() {
        match arg.kind() {
            SyntaxKind::Named => {
                let name = arg.children().next()?.full_text().to_string();
                let value = arg.children().rfind(|n| !n.kind().is_trivia())?;
                if value.kind() != SyntaxKind::Int {
                    return None;
                }
                let number = value.cast::<ast::Int>()?.get();
                match name.as_str() {
                    "year" => year = Some(number),
                    "month" => month = Some(number),
                    "day" => day = Some(number),
                    _ => return None,
                }
            }
            kind if kind.is_trivia()
                || matches!(
                    kind,
                    SyntaxKind::LeftParen | SyntaxKind::RightParen | SyntaxKind::Comma
                ) => {}
            _ => return None,
        }
    }
    let (year, month, day) = (year?, month?, day?);
    let ok = i32::try_from(year)
        .ok()
        .zip(u8::try_from(month).ok())
        .zip(u8::try_from(day).ok());
    let ((year, month), day) = ok?;
    valid_date(year, month, day).then_some(PropertyValue::Date { year, month, day })
}

/// Свойства заметки из её текста (contracts/core-properties-api.md).
pub fn read(text: &str) -> NoteProperties {
    let root = typst::syntax::parse(text);
    let lines = Lines::new(text);
    let place = |range: Range<usize>| place_of(&lines, range);

    let linked = LinkedNode::new(&root);
    let found = find_records(&linked);
    let Some(record) = found.records.first() else {
        return NoteProperties {
            properties: Vec::new(),
            editable: true,
            problem: None,
            record: None,
        };
    };

    let mut problems = Vec::new();
    if found.records.len() > 1 {
        problems.push(PropertiesProblem::SecondRecord);
    }
    if has_error(record.call.get()) {
        problems.push(PropertiesProblem::Malformed);
    }
    let mut properties = Vec::new();
    match dict_of(&record.call) {
        None => problems.push(PropertiesProblem::NotADictionary),
        Some(dict) => {
            let (entries, mut found_problems) = dict_entries(text, &dict);
            problems.append(&mut found_problems);
            for entry in entries {
                let Some(key) = entry.key else { continue };
                properties.push(Property {
                    key,
                    value: entry.value,
                    place: place(entry.value_range),
                });
            }
        }
    }

    problems.sort_by_key(PropertiesProblem::rank);
    let problem = problems.into_iter().next();
    NoteProperties {
        properties,
        editable: problem.is_none(),
        problem,
        record: Some(place(record.range.clone())),
    }
}

/// Место в тексте по диапазону байт: строка и колонка с единицы, смещения в
/// кодовых единицах UTF-16.
pub(crate) fn place_of(lines: &Lines<&str>, range: Range<usize>) -> SourcePlace {
    let (line, column) = lines.byte_to_line_column(range.start).unwrap_or((0, 0));
    let utf16 = |byte| lines.byte_to_utf16(byte).unwrap_or(0) as u32;
    SourcePlace {
        line: line as u32 + 1,
        column: column as u32 + 1,
        from: utf16(range.start),
        to: utf16(range.end),
    }
}
