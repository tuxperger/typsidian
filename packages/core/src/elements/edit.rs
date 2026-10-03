//! Правка элемента на месте точечными заменами (research.md §4, FR-008, FR-009,
//! FR-012, FR-012a, FR-020).
//!
//! Элемент не пересобирается: меняется только значение изменённого параметра,
//! добавленный параметр вставляется, сброшенный к умолчанию — убирается вместе
//! со своей запятой. Незнакомые аргументы, выражения, комментарии, переносы
//! строк и отступы никогда не адресуются заменой — этим они и сохраняются.
//! Правка без изменений — пустой список замен (FR-009).

use std::ops::Range;

use serde::{Deserialize, Serialize};
use typst::syntax::{parse, LinkedNode, SyntaxKind};

use super::locate::{locate, Located, ParsedElement, UnavailableReason};
use super::schema::{ElementSyntax, ParamSpec};
use super::values::{field_source, label_is_valid, FieldError, FieldProblem, FieldValue};
use super::{byte_at, utf16_at};

/// Замена в тексте заметки (UTF-16). Все замены плана относятся к исходному
/// тексту и применяются вместе, одной транзакцией редактора (FR-010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TextEdit {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

/// Почему правку применить нельзя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum PlanEditError {
    /// Текст элемента изменился с открытия формы (FR-020).
    Stale,
    /// Элемента на прежнем месте больше нет.
    Gone,
    /// Элемент стал неразбираемым (FR-011).
    Unavailable { reason: UnavailableReason },
    /// Значения полей не годятся (FR-015).
    Invalid { errors: Vec<FieldError> },
}

/// Точечные замены для правки на месте. Поля, которых форма не прислала, не
/// трогаются; `label` — желаемая метка целиком (`None` — без метки).
pub fn plan_element_edit(
    text: &str,
    parsed: &ParsedElement,
    fields: &[FieldValue],
    label: Option<&str>,
) -> Result<Vec<TextEdit>, PlanEditError> {
    let root = parse(text);
    let located = match locate(text, &root, byte_at(text, parsed.from)) {
        None => return Err(PlanEditError::Gone),
        Some(Err((_, reason))) => return Err(PlanEditError::Unavailable { reason }),
        Some(Ok(located)) => located,
    };
    if located.spec.id != parsed.id || located.revision(text) != parsed.revision {
        return Err(PlanEditError::Stale);
    }

    let mut plan = Plan {
        text,
        fields,
        located: &located,
        edits: Vec::new(),
        errors: Vec::new(),
    };

    match located.spec.syntax {
        ElementSyntax::Call | ElementSyntax::Set => {
            let math = located.node.kind() == SyntaxKind::MathCall;
            if let Some(args) = args_of(&located.node) {
                plan.args(&args, &located.spec.params, "", math);
                if located.inner {
                    if let Some(inner) = &located.spec.inner {
                        if let Some(inner_args) = inner_args(&located, &args, &inner.param) {
                            let prefix = format!("{}.", inner.param);
                            plan.args(&inner_args, &inner.params, &prefix, false);
                        }
                    }
                }
            }
        }
        ElementSyntax::MarkupHeading => plan.heading(),
        ElementSyntax::MarkupRaw => plan.raw(),
        ElementSyntax::MarkupEquation => plan.equation(),
    }

    if located.spec.label.is_some() {
        plan.label(&root, label);
    }

    let Plan {
        mut edits, errors, ..
    } = plan;
    if !errors.is_empty() {
        return Err(PlanEditError::Invalid { errors });
    }
    edits.sort_by_key(|(range, _)| (range.start, range.end));
    Ok(edits
        .into_iter()
        .map(|(range, insert)| TextEdit {
            from: utf16_at(text, range.start),
            to: utf16_at(text, range.end),
            insert,
        })
        .collect())
}

struct Plan<'t, 'l, 'a> {
    text: &'t str,
    fields: &'t [FieldValue],
    located: &'l Located<'a>,
    edits: Vec<(Range<usize>, String)>,
    errors: Vec<FieldError>,
}

fn args_of<'a>(node: &LinkedNode<'a>) -> Option<LinkedNode<'a>> {
    node.children()
        .find(|child| matches!(child.kind(), SyntaxKind::Args | SyntaxKind::MathArgs))
}

/// Аргументы вложенного вызова (`image(…)` внутри рисунка).
fn inner_args<'a>(
    located: &Located<'a>,
    args: &LinkedNode<'a>,
    param: &str,
) -> Option<LinkedNode<'a>> {
    let prefix = format!("{param}.");
    // Любое значение вложенного элемента лежит внутри его вызова: по нему и
    // находится вызов, без повторного сопоставления позиций.
    let probe = located
        .values
        .iter()
        .find(|value| value.name.starts_with(&prefix))
        .map(|value| value.range.start);
    let body = args
        .children()
        .flat_map(|arg| match arg.kind() {
            SyntaxKind::Named => arg.children().last(),
            _ => Some(arg),
        })
        .find(|node| {
            node.kind() == SyntaxKind::FuncCall && probe.is_none_or(|at| node.range().contains(&at))
        })?;
    args_of(&body)
}

/// Аргумент как единица правки: диапазон и запятая ли это.
struct Item {
    range: Range<usize>,
    comma: bool,
}

impl Plan<'_, '_, '_> {
    fn field(&self, name: &str) -> Option<FieldValue> {
        self.fields.iter().find(|field| field.name == name).cloned()
    }

    fn push(&mut self, range: Range<usize>, insert: impl Into<String>) {
        self.edits.push((range, insert.into()));
    }

    fn error(&mut self, name: &str, problem: FieldProblem) {
        self.errors.push(FieldError {
            name: name.to_string(),
            problem,
        });
    }

    /// Аргументы до закрывающей скобки: значения и запятые. В формуле значение
    /// — несколько узлов подряд, они сливаются в один.
    fn items(
        &self,
        args: &LinkedNode,
        math: bool,
    ) -> (Vec<Item>, Option<Range<usize>>, Option<Range<usize>>) {
        let mut items: Vec<Item> = Vec::new();
        let mut left = None;
        let mut right = None;
        for child in args.children() {
            match child.kind() {
                SyntaxKind::LeftParen => left = Some(child.range()),
                SyntaxKind::RightParen => {
                    right = Some(child.range());
                }
                kind if kind.is_trivia() => {}
                _ if right.is_some() => {} // тело `[…]` после скобки
                SyntaxKind::Comma => items.push(Item {
                    range: child.range(),
                    comma: true,
                }),
                _ => {
                    if math {
                        if let Some(last) = items.last_mut().filter(|item| !item.comma) {
                            last.range.end = child.range().end;
                            continue;
                        }
                    }
                    items.push(Item {
                        range: child.range(),
                        comma: false,
                    });
                }
            }
        }
        (items, left, right)
    }

    fn args(&mut self, args: &LinkedNode, params: &[ParamSpec], prefix: &str, math: bool) {
        let inner_param = self
            .located
            .spec
            .inner
            .as_ref()
            .filter(|_| self.located.inner && prefix.is_empty())
            .map(|inner| inner.param.clone());

        for param in params {
            if Some(&param.name) == inner_param.as_ref() {
                continue; // правится своими полями `body.*`
            }
            let full = format!("{prefix}{}", param.name);
            let Some(field) = self.field(&full) else {
                continue; // поле не прислано — параметр не трогается
            };
            let old = self.located.values.iter().find(|value| value.name == full);

            // Неизменённое не порождает замены (FR-009): сравнение с тем, что
            // форма получила, а не с пересобранной записью — иначе строка с
            // другим, но равнозначным экранированием переписалась бы зря.
            if let Some(old) = old {
                if field.source.as_deref() == Some(old.source.as_str())
                    || (field.source.is_none()
                        && field.plain.as_deref() == Some(old.plain.as_str()))
                {
                    continue;
                }
            }

            let new = match field_source(param, Some(&field), math) {
                Ok(new) => new,
                Err(problem) => {
                    self.error(&full, problem);
                    continue;
                }
            };

            match (old, new) {
                (Some(old), Some(new)) => {
                    if new != old.source {
                        self.push(old.range.clone(), new);
                    }
                }
                (Some(old), None) => {
                    let arg = old.arg.clone().unwrap_or_else(|| old.range.clone());
                    self.remove_arg(args, math, arg);
                }
                (None, Some(new)) => {
                    if !param.positional && param.default.as_deref() == Some(new.as_str()) {
                        continue; // FR-013
                    }
                    let arg = if param.positional {
                        new
                    } else {
                        format!("{}: {new}", param.name)
                    };
                    if param.positional && !param.variadic {
                        self.insert_first(args, math, arg);
                    } else {
                        self.insert_last(args, math, arg);
                    }
                }
                (None, None) => {}
            }
        }
    }

    /// Новый аргумент в конец списка — в стиле вызова: в строку или новой
    /// строкой с тем же отступом, если аргументы уже разнесены по строкам.
    fn insert_last(&mut self, args: &LinkedNode, math: bool, arg: String) {
        let (items, left, right) = self.items(args, math);
        let Some(left) = left else {
            // `#quote[…]` без скобок — скобки появляются вместе с аргументом.
            self.push(args.offset()..args.offset(), format!("({arg})"));
            return;
        };
        let Some(_right) = right else { return };
        match items.last() {
            None => self.push(left.end..left.end, arg),
            Some(last) if last.comma => {
                let indent = items
                    .iter()
                    .rev()
                    .find(|item| !item.comma)
                    .map(|item| indent_of(self.text, item.range.start))
                    .unwrap_or_else(|| "  ".to_string());
                self.push(last.range.end..last.range.end, format!("\n{indent}{arg},"));
            }
            Some(last) => {
                let multiline = self.text[left.end..last.range.start].contains('\n');
                let insert = if multiline {
                    format!(",\n{}{arg}", indent_of(self.text, last.range.start))
                } else {
                    format!(", {arg}")
                };
                self.push(last.range.end..last.range.end, insert);
            }
        }
    }

    /// Новый позиционный — в начало списка.
    fn insert_first(&mut self, args: &LinkedNode, math: bool, arg: String) {
        let (items, left, _) = self.items(args, math);
        match left {
            None => self.push(args.offset()..args.offset(), format!("({arg})")),
            Some(left) => {
                let insert = if items.is_empty() {
                    arg
                } else {
                    format!("{arg}, ")
                };
                self.push(left.end..left.end, insert);
            }
        }
    }

    /// Убрать аргумент вместе с его запятой; соседей не трогать.
    fn remove_arg(&mut self, args: &LinkedNode, math: bool, arg: Range<usize>) {
        let (items, left, _) = self.items(args, math);
        let Some(index) = items.iter().position(|item| item.range == arg) else {
            return;
        };
        let next_is_comma = items.get(index + 1).is_some_and(|item| item.comma);
        let range = if next_is_comma && index + 2 < items.len() {
            // `a, b, c` → убрать `b, `: до начала следующего аргумента.
            arg.start..items[index + 2].range.start
        } else if index >= 1 && items[index - 1].comma {
            if next_is_comma {
                // Последний с висячей запятой — стиль «по строке на аргумент»:
                // убрать строку целиком, висячая запятая у предыдущего остаётся.
                items[index - 1].range.end..items[index + 1].range.end
            } else {
                // Последний: забрать запятую перед ним.
                items[index - 1].range.start..arg.end
            }
        } else if next_is_comma {
            // Единственный с висячей запятой.
            let start = left.map(|left| left.end).unwrap_or(arg.start);
            start..items[index + 1].range.end
        } else {
            arg
        };
        self.push(range, "");
    }

    fn heading(&mut self) {
        let Some(field) = self.field("level") else {
            return;
        };
        let Some(old) = self
            .located
            .values
            .iter()
            .find(|value| value.name == "level")
        else {
            return;
        };
        let text = field.source.or(field.plain).unwrap_or_default();
        match text.trim().parse::<usize>() {
            Ok(level) if (1..=6).contains(&level) => {
                if level.to_string() != old.source {
                    self.push(old.range.clone(), "=".repeat(level));
                }
            }
            _ => self.error("level", FieldProblem::WrongKind),
        }
    }

    fn raw(&mut self) {
        let Some(field) = self.field("lang") else {
            return;
        };
        let wanted = field
            .plain
            .or_else(|| {
                field.source.map(|source| {
                    source
                        .strip_prefix('"')
                        .and_then(|rest| rest.strip_suffix('"'))
                        .map(str::to_string)
                        .unwrap_or(source)
                })
            })
            .unwrap_or_default();
        let wanted = wanted.trim();
        if wanted.contains(|ch: char| ch.is_whitespace() || ch == '`') {
            self.error("lang", FieldProblem::WrongKind);
            return;
        }
        let old = self
            .located
            .values
            .iter()
            .find(|value| value.name == "lang");
        match old {
            Some(old) if old.plain == wanted => {}
            Some(old) => self.push(old.range.clone(), wanted),
            None if wanted.is_empty() => {}
            None => {
                let delim = self.located.node.children().next().map(|d| d.range().end);
                if let Some(at) = delim {
                    self.push(at..at, wanted);
                }
            }
        }
    }

    fn equation(&mut self) {
        let Some(field) = self.field("block") else {
            return;
        };
        let wanted = field.source.or(field.plain).unwrap_or_default() == "true";
        let children: Vec<LinkedNode> = self.located.node.children().collect();
        let (Some(first), Some(last)) = (children.first(), children.last()) else {
            return;
        };
        let lead = children.get(1).filter(|n| n.kind() == SyntaxKind::Space);
        let trail = children
            .len()
            .checked_sub(2)
            .and_then(|i| children.get(i))
            .filter(|n| n.kind() == SyntaxKind::Space);
        let block = lead.is_some();
        if block == wanted {
            return;
        }
        if wanted {
            self.push(first.range().end..first.range().end, " ");
            if trail.is_none() {
                self.push(last.offset()..last.offset(), " ");
            }
        } else {
            if let Some(lead) = lead {
                self.push(lead.range(), "");
            }
            if let Some(trail) = trail {
                self.push(trail.range(), "");
            }
        }
    }

    fn label(&mut self, root: &typst::syntax::SyntaxNode, wanted: Option<&str>) {
        let wanted = wanted.map(str::trim).filter(|label| !label.is_empty());
        let old = self.located.label_name();
        if wanted == old.as_deref() {
            return;
        }
        if let Some(wanted) = wanted {
            if !label_is_valid(wanted) {
                self.error("label", FieldProblem::LabelInvalid);
                return;
            }
            let own = self.located.label.as_ref().map(|label| label.range());
            if label_taken(root, wanted, own.as_ref()) {
                self.error("label", FieldProblem::LabelTaken);
                return;
            }
        }
        let end = self.located.node.range().end;
        match (&self.located.label, wanted) {
            (Some(label), Some(wanted)) => self.push(label.range(), format!("<{wanted}>")),
            (Some(label), None) => self.push(end..label.range().end, ""),
            (None, Some(wanted)) => self.push(end..end, format!(" <{wanted}>")),
            (None, None) => {}
        }
    }
}

fn label_taken(root: &typst::syntax::SyntaxNode, wanted: &str, own: Option<&Range<usize>>) -> bool {
    fn walk(node: &LinkedNode, wanted: &str, own: Option<&Range<usize>>) -> bool {
        if node.kind() == SyntaxKind::Label && Some(&node.range()) != own {
            let text = node.get().full_text();
            if text.trim_start_matches('<').trim_end_matches('>') == wanted {
                return true;
            }
        }
        node.children().any(|child| walk(&child, wanted, own))
    }
    walk(&LinkedNode::new(root), wanted, own)
}

/// Отступ строки, в которой стоит позиция.
fn indent_of(text: &str, at: usize) -> String {
    let line_start = text[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
    text[line_start..]
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect()
}
