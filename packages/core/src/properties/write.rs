//! Правка записи свойств точечными заменами (research.md §3, FR-002).
//!
//! Тот же принцип, что у параметров элементов 008: меняется только то, что
//! изменила форма. Значение существующего свойства заменяется на месте, новое
//! дописывается в конец словаря, удаляемое уходит вместе со своей запятой;
//! комментарии, переносы и отступы остальных свойств не затрагиваются.
//! Последнее удалённое свойство уносит запись целиком, а отсутствующая запись
//! создаётся в начале заметки с переводом строки после — так «создать» и
//! «удалить последнее» точно обратны друг другу.
//!
//! Если запись не разбирается однозначно, правка отклоняется (FR-004).

use typst::syntax::{LinkedNode, SyntaxKind};

use super::read::{dict_entries, dict_of, find_records, read, DictEntry};
use super::{PropertyEdit, PropertyValue, RECORD_LABEL};
use crate::elements::edit::{PlanEditError, TextEdit};
use crate::elements::values::{quote, FieldError, FieldProblem};
use crate::elements::{utf16_at, EditOutcome};

/// Слова, которые нельзя писать ключом без кавычек.
const KEYWORDS: &[&str] = &[
    "none", "auto", "true", "false", "let", "set", "show", "if", "else", "for", "in", "while",
    "break", "continue", "return", "import", "include", "as", "not", "and", "or", "context",
];

/// Ключ записью Typst: идентификатор как есть, иначе строка в кавычках.
pub fn render_key(key: &str) -> String {
    if typst::syntax::is_ident(key) && !KEYWORDS.contains(&key) {
        key.to_string()
    } else {
        quote(key)
    }
}

/// Значение записью Typst. `None` — значение записать нельзя.
pub fn render_value(value: &PropertyValue) -> Option<String> {
    if !value.is_writable() {
        return None;
    }
    Some(match value {
        PropertyValue::Text { value } => quote(value),
        PropertyValue::Number { value } => {
            if value.fract() == 0.0 && value.abs() < 1e15 {
                format!("{}", *value as i64)
            } else {
                format!("{value}")
            }
        }
        PropertyValue::Date { year, month, day } => {
            format!("datetime(year: {year}, month: {month}, day: {day})")
        }
        PropertyValue::Flag { value } => value.to_string(),
        PropertyValue::List { values } => match values.as_slice() {
            [] => "()".to_string(),
            [one] => format!("({},)", quote(one)),
            many => format!(
                "({})",
                many.iter().map(|v| quote(v)).collect::<Vec<_>>().join(", ")
            ),
        },
        PropertyValue::Unknown { .. } => return None,
    })
}

/// Запись из свойств целиком (для создания).
fn render_record(key: &str, value: &str) -> String {
    format!(
        "#metadata(({}: {value})) <{RECORD_LABEL}>\n",
        render_key(key)
    )
}

fn invalid(name: &str, problem: FieldProblem) -> EditOutcome {
    EditOutcome::Refused {
        error: PlanEditError::Invalid {
            errors: vec![FieldError {
                name: name.to_string(),
                problem,
            }],
        },
    }
}

/// Замена в байтах исходного текста.
struct ByteEdit {
    from: usize,
    to: usize,
    insert: String,
}

fn planned(text: &str, edits: Vec<ByteEdit>) -> EditOutcome {
    EditOutcome::Planned {
        edits: edits
            .into_iter()
            .map(|edit| TextEdit {
                from: utf16_at(text, edit.from),
                to: utf16_at(text, edit.to),
                insert: edit.insert,
            })
            .collect(),
    }
}

/// Правка свойств формой → план замен (contracts/core-properties-api.md).
///
/// Замены относятся к исходному тексту и применяются вместе. Пустой список —
/// ничего не изменилось.
pub fn edit(text: &str, change: &PropertyEdit) -> EditOutcome {
    let current = read(text);
    if let Some(problem) = current.problem {
        return EditOutcome::Refused {
            error: PlanEditError::Properties { problem },
        };
    }

    // Проверки значений идут до разбора записи: отказ не зависит от текста.
    match change {
        PropertyEdit::Set { key, value } => {
            if key.trim().is_empty() {
                return invalid(key, FieldProblem::Required);
            }
            if !value.is_writable() {
                return invalid(key, FieldProblem::WrongKind);
            }
        }
        PropertyEdit::Rename { from, to } => {
            if to.trim().is_empty() {
                return invalid(from, FieldProblem::Required);
            }
        }
        PropertyEdit::Remove { .. } => {}
    }

    let root = typst::syntax::parse(text);
    let linked = LinkedNode::new(&root);
    let found = find_records(&linked);
    let Some(record) = found.records.first() else {
        return match change {
            PropertyEdit::Set { key, value } => {
                let source = render_value(value).unwrap_or_default();
                planned(
                    text,
                    vec![ByteEdit {
                        from: 0,
                        to: 0,
                        insert: render_record(key, &source),
                    }],
                )
            }
            PropertyEdit::Remove { .. } => EditOutcome::Planned { edits: Vec::new() },
            PropertyEdit::Rename { .. } => EditOutcome::Refused {
                error: PlanEditError::Gone,
            },
        };
    };
    let Some(dict) = dict_of(&record.call) else {
        // Недостижимо после `read`, но паниковать на любом тексте нельзя.
        return EditOutcome::Refused {
            error: PlanEditError::Properties {
                problem: super::PropertiesProblem::NotADictionary,
            },
        };
    };
    let (entries, _) = dict_entries(text, &dict);
    let position = |key: &str| {
        entries
            .iter()
            .position(|entry| entry.key.as_deref() == Some(key))
    };

    match change {
        PropertyEdit::Set { key, value } => {
            let source = render_value(value).unwrap_or_default();
            match position(key) {
                Some(at) => {
                    if &entries[at].value == value {
                        return EditOutcome::Planned { edits: Vec::new() };
                    }
                    planned(
                        text,
                        vec![ByteEdit {
                            from: entries[at].value_range.start,
                            to: entries[at].value_range.end,
                            insert: source,
                        }],
                    )
                }
                None => planned(text, vec![append(text, &dict, &entries, key, &source)]),
            }
        }
        PropertyEdit::Remove { key } => {
            let Some(at) = position(key) else {
                return EditOutcome::Planned { edits: Vec::new() };
            };
            if entries.len() == 1 {
                return planned(text, vec![remove_record(text, &record.range)]);
            }
            let edit = if at + 1 < entries.len() {
                ByteEdit {
                    from: entries[at].item_range.start,
                    to: entries[at + 1].item_range.start,
                    insert: String::new(),
                }
            } else {
                ByteEdit {
                    from: entries[at - 1].item_range.end,
                    to: entries[at].item_range.end,
                    insert: String::new(),
                }
            };
            planned(text, vec![edit])
        }
        PropertyEdit::Rename { from, to } => {
            let Some(at) = position(from) else {
                return EditOutcome::Refused {
                    error: PlanEditError::Gone,
                };
            };
            if from == to {
                return EditOutcome::Planned { edits: Vec::new() };
            }
            if position(to).is_some() {
                return invalid(to, FieldProblem::LabelTaken);
            }
            planned(
                text,
                vec![ByteEdit {
                    from: entries[at].key_range.start,
                    to: entries[at].key_range.end,
                    insert: render_key(to),
                }],
            )
        }
    }
}

/// Дописывает свойство в конец словаря.
fn append(
    text: &str,
    dict: &LinkedNode,
    entries: &[DictEntry],
    key: &str,
    source: &str,
) -> ByteEdit {
    let pair = format!("{}: {source}", render_key(key));
    let Some(last) = entries.last() else {
        // Пустой словарь `(:)` заменяется целиком.
        let range = dict.range();
        return ByteEdit {
            from: range.start,
            to: range.end,
            insert: format!("({pair})"),
        };
    };
    let multiline = dict
        .children()
        .any(|child| child.kind() == SyntaxKind::Space && child.get().full_text().contains('\n'));
    let separator = if multiline {
        let line_start = text[..last.item_range.start]
            .rfind('\n')
            .map_or(0, |at| at + 1);
        let indent: String = text[line_start..last.item_range.start]
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .collect();
        format!(",\n{indent}")
    } else {
        ", ".to_string()
    };
    ByteEdit {
        from: last.item_range.end,
        to: last.item_range.end,
        insert: format!("{separator}{pair}"),
    }
}

/// Убирает запись целиком вместе с переводом строки после неё.
fn remove_record(text: &str, range: &std::ops::Range<usize>) -> ByteEdit {
    let rest = &text[range.end..];
    let tail = if rest.starts_with("\r\n") {
        2
    } else if rest.starts_with('\n') {
        1
    } else {
        0
    };
    ByteEdit {
        from: range.start,
        to: range.end + tail,
        insert: String::new(),
    }
}
