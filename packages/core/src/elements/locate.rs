//! Элемент под кареткой (research.md §2, FR-006, FR-011, FR-016).
//!
//! Разбор — парсером самого Typst: он знает скобки внутри строк, вложенные
//! `[…]` в подписи и экранированные кавычки, на которых ломался прежний
//! регулярный разбор в интерфейсе. От листа под кареткой — вверх по предкам до
//! ближайшего узла, который знает схема: это и есть элемент, непосредственно
//! содержащий каретку.

use serde::{Deserialize, Serialize};
use typst::syntax::{parse, LinkedNode, Side, SyntaxKind, SyntaxNode};

use super::schema::{inner_param_of, key_for, raw_spec, ElementSpec, ElementSyntax, ParamSpec};
use super::values::{has_error, literal_matches, plain_of, quote, ParamValue};
use super::{byte_at, utf16_at};

/// Найденный в тексте элемент (data-model.md «ParsedElement»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ParsedElement {
    /// Элемент схемы.
    pub id: String,
    /// Границы элемента в тексте (UTF-16), метка включена.
    pub from: usize,
    pub to: usize,
    /// Значения параметров, заданных в тексте. Незнакомые аргументы сюда не
    /// попадают и никогда не адресуются правкой — этим они и сохраняются
    /// (FR-012).
    pub values: Vec<ParamValue>,
    /// Вложенный элемент разобран: его параметры лежат в `values` с приставкой
    /// `<param>.`, и форма показывает их вместо поля самого параметра.
    pub inner: bool,
    /// Метка элемента целиком, без угловых скобок: `fig:schema`.
    pub label: Option<String>,
    /// Метки заметки, кроме собственной, — занятые (Edge Cases «параметр-метка»).
    pub taken_labels: Vec<String>,
    /// Отпечаток текста элемента: правка применяется, только пока он тот же
    /// (FR-020).
    pub revision: String,
}

/// Почему правка не предлагается (FR-011). Каждая причина называется словами.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum UnavailableReason {
    /// Незакрытая скобка или строка — границу элемента не установить.
    Unclosed,
    /// Параметр указан дважды: какое значение показывать — неизвестно.
    DuplicateParam { name: String },
    /// `..args`: какие параметры задал разворот, известно только при
    /// вычислении.
    SpreadArgs,
    /// Позиционных значений больше, чем знает схема.
    UnknownPositional,
    /// Строки матрицы через `;` — форма их не разбирает.
    MathRows,
}

/// Результат поиска элемента под кареткой.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ElementAtResult {
    /// Каретка не в элементе схемы (US2 AS5).
    None,
    Found { element: ParsedElement },
    Unavailable { id: String, reason: UnavailableReason },
}

/// Элемент, непосредственно содержащий каретку (FR-006, FR-016).
///
/// Каретка на границе элемента — перед `#` или сразу после `)`, `]` или
/// метки — считается внутри. Если по обе стороны каретки разные элементы,
/// берётся более глубоко вложенный.
pub fn element_at(text: &str, offset: usize) -> ElementAtResult {
    let root = parse(text);
    match locate(text, &root, byte_at(text, offset)) {
        None => ElementAtResult::None,
        Some(Err((spec, reason))) => ElementAtResult::Unavailable {
            id: spec.id.clone(),
            reason,
        },
        Some(Ok(located)) => ElementAtResult::Found {
            element: located.parsed(text, &root),
        },
    }
}

/// Найденный элемент вместе с узлами дерева — нужен и разбору, и правке.
pub(crate) struct Located<'a> {
    pub spec: &'static ElementSpec,
    /// Узел элемента: `FuncCall`, `MathCall`, `SetRule`, `Heading`, `Raw`,
    /// `Equation`.
    pub node: LinkedNode<'a>,
    /// Начало с `#`, если он есть.
    pub start: usize,
    /// Метка после элемента.
    pub label: Option<LinkedNode<'a>>,
    /// Разобранные значения — в байтах.
    pub values: Vec<ByteValue>,
    pub inner: bool,
}

/// Значение параметра с диапазоном в байтах.
pub(crate) struct ByteValue {
    pub name: String,
    pub source: String,
    pub plain: String,
    pub expression: bool,
    pub range: std::ops::Range<usize>,
    /// Узел аргумента целиком (для именованного — `Named`), чтобы его можно
    /// было убрать вместе с запятой. Для разметки — `None`.
    pub arg: Option<std::ops::Range<usize>>,
}

impl<'a> Located<'a> {
    /// Конец элемента с меткой.
    pub fn end(&self) -> usize {
        self.label
            .as_ref()
            .map(|label| label.range().end)
            .unwrap_or_else(|| self.node.range().end)
    }

    pub fn revision(&self, text: &str) -> String {
        blake3::hash(text[self.start..self.end()].as_bytes())
            .to_hex()
            .to_string()
    }

    pub fn label_name(&self) -> Option<String> {
        self.label.as_ref().map(|label| {
            let text = label.get().full_text();
            text.trim_start_matches('<').trim_end_matches('>').to_string()
        })
    }

    fn parsed(&self, text: &str, root: &SyntaxNode) -> ParsedElement {
        let own = self.label.as_ref().map(|label| label.range());
        let mut taken = Vec::new();
        collect_labels(&LinkedNode::new(root), own.as_ref(), &mut taken);
        ParsedElement {
            id: self.spec.id.clone(),
            from: utf16_at(text, self.start),
            to: utf16_at(text, self.end()),
            values: self
                .values
                .iter()
                .map(|value| ParamValue {
                    name: value.name.clone(),
                    source: value.source.clone(),
                    plain: value.plain.clone(),
                    expression: value.expression,
                    from: utf16_at(text, value.range.start),
                    to: utf16_at(text, value.range.end),
                })
                .collect(),
            inner: self.inner,
            label: self.label_name(),
            taken_labels: taken,
            revision: self.revision(text),
        }
    }
}

fn collect_labels(
    node: &LinkedNode,
    own: Option<&std::ops::Range<usize>>,
    out: &mut Vec<String>,
) {
    if node.kind() == SyntaxKind::Label && Some(&node.range()) != own {
        let text = node.get().full_text();
        let name = text.trim_start_matches('<').trim_end_matches('>').to_string();
        if !out.contains(&name) {
            out.push(name);
        }
    }
    for child in node.children() {
        collect_labels(&child, own, out);
    }
}

type LocateResult<'a> = Result<Located<'a>, (&'static ElementSpec, UnavailableReason)>;

/// Элемент под позицией `byte`. `None` — каретка не в элементе схемы.
pub(crate) fn locate<'a>(
    text: &str,
    root: &'a SyntaxNode,
    byte: usize,
) -> Option<LocateResult<'a>> {
    let linked = LinkedNode::new(root);
    let after = linked.leaf_at(byte, Side::After).and_then(|leaf| climb(leaf));
    let before = linked.leaf_at(byte, Side::Before).and_then(|leaf| climb(leaf));

    // По обе стороны каретки — разные элементы (сразу после `)` вложенного):
    // берётся более глубоко вложенный, то есть более короткий.
    let chosen = match (after, before) {
        (Some(a), Some(b)) => {
            if b.1.range().len() < a.1.range().len() {
                Some(b)
            } else {
                Some(a)
            }
        }
        (a, b) => a.or(b),
    }?;
    let (spec, node) = chosen;
    Some(build(text, spec, node))
}

/// От листа — вверх до ближайшего элемента схемы.
fn climb(leaf: LinkedNode) -> Option<(&'static ElementSpec, LinkedNode)> {
    // `#` стоит перед выражением соседним узлом: каретка перед ним — в
    // элементе справа. Метка — после элемента: каретка на ней — в элементе
    // слева.
    let start = match leaf.kind() {
        SyntaxKind::Hash => leaf.next_sibling()?,
        SyntaxKind::Label => leaf.prev_sibling()?,
        _ => leaf,
    };
    let mut node = Some(start);
    while let Some(current) = node {
        if let Some(spec) = spec_of(&current) {
            return Some((spec, current));
        }
        node = current.parent().cloned();
    }
    None
}

/// Элемент, которому соответствует узел: вызов встроенной функции Typst,
/// правило `set`, заголовок, блок кода или формула. Функция без параметров и
/// функция, которой в Typst нет (своя из шаблона пользователя), элементом не
/// считаются — подъём продолжается к внешнему.
fn spec_of(node: &LinkedNode) -> Option<&'static ElementSpec> {
    let found = |syntax: ElementSyntax, path: &str, inner: Option<(&str, &str)>| {
        raw_spec(&key_for(syntax, path, inner))
    };

    match node.kind() {
        SyntaxKind::FuncCall => {
            let callee = node.children().next()?;
            if !matches!(callee.kind(), SyntaxKind::Ident | SyntaxKind::FieldAccess) {
                return None;
            }
            let path = callee.get().full_text().to_string();
            // `#math.frac(…)` в коде — не тот вызов, что `frac(…)` в формуле:
            // аргументы там — код, а форма формулы пишет их разметкой.
            if path.starts_with("math.") {
                return None;
            }
            if let Some(param) = inner_param_of(&path) {
                if let Some(inner) = inner_callee(node, &path, param) {
                    if let Some(spec) = found(ElementSyntax::Call, &path, Some((param, &inner))) {
                        return Some(spec);
                    }
                }
            }
            found(ElementSyntax::Call, &path, None)
        }
        SyntaxKind::MathCall => {
            let callee = node.children().next()?;
            if callee.kind() != SyntaxKind::MathIdent {
                return None;
            }
            found(ElementSyntax::Call, &format!("math.{}", callee.get().full_text()), None)
        }
        SyntaxKind::SetRule => {
            let target = node
                .children()
                .find(|child| matches!(child.kind(), SyntaxKind::Ident | SyntaxKind::FieldAccess))?;
            found(ElementSyntax::Set, &target.get().full_text(), None)
        }
        SyntaxKind::Heading => found(ElementSyntax::MarkupHeading, "heading", None),
        SyntaxKind::Raw => {
            let fenced = node
                .children()
                .next()
                .is_some_and(|delim| delim.get().full_text().starts_with("```"));
            if !fenced {
                return None;
            }
            found(ElementSyntax::MarkupRaw, "raw", None)
        }
        SyntaxKind::Equation => found(ElementSyntax::MarkupEquation, "math.equation", None),
        _ => None,
    }
}

/// Функция, вызванная в параметре `param` вызова `node`: `image` в
/// `#figure(image("a.png"))`. `None` — там не вызов встроенной функции.
fn inner_callee(node: &LinkedNode, path: &str, param: &str) -> Option<String> {
    let outer = raw_spec(&key_for(ElementSyntax::Call, path, None))?;
    let args = node.children().find(|child| child.kind() == SyntaxKind::Args)?;
    let value = inner_arg(&args, &outer.params, param)?;
    if value.kind() != SyntaxKind::FuncCall {
        return None;
    }
    let callee = value.children().next()?;
    if !matches!(callee.kind(), SyntaxKind::Ident | SyntaxKind::FieldAccess) {
        return None;
    }
    let name = callee.get().full_text().to_string();
    raw_spec(&key_for(ElementSyntax::Call, &name, None)).map(|_| name)
}

/// Разбор найденного элемента.
fn build<'a>(text: &str, spec: &'static ElementSpec, node: LinkedNode<'a>) -> LocateResult<'a> {
    // `#` — соседний лист прямо перед узлом; элемент начинается с него.
    let start = node
        .prev_leaf()
        .filter(|leaf| leaf.kind() == SyntaxKind::Hash && leaf.range().end == node.offset())
        .map(|leaf| leaf.offset())
        .unwrap_or_else(|| node.offset());

    // Повтор параметра Typst сам помечает ошибкой разбора, поэтому он
    // проверяется раньше общей ошибки: у пользователя другая причина и другое
    // действие — убрать лишний, а не искать незакрытую скобку (FR-011).
    if let Some(args) = node
        .children()
        .find(|child| matches!(child.kind(), SyntaxKind::Args | SyntaxKind::MathArgs))
    {
        let mut names: Vec<String> = Vec::new();
        for arg in args.children().filter(|arg| arg.kind() == SyntaxKind::Named) {
            if let Some(name) = arg.children().next().map(|n| n.get().full_text().to_string()) {
                if names.contains(&name) {
                    return Err((spec, UnavailableReason::DuplicateParam { name }));
                }
                names.push(name);
            }
        }
    }

    if has_error(node.get()) {
        return Err((spec, UnavailableReason::Unclosed));
    }

    let label = match &spec.label {
        Some(_) => node
            .next_sibling()
            .filter(|sibling| sibling.kind() == SyntaxKind::Label)
            .filter(|label| only_spaces(&text[node.range().end..label.offset()])),
        None => None,
    };

    let mut values = Vec::new();
    let mut inner = false;
    match spec.syntax {
        ElementSyntax::Call | ElementSyntax::Set => {
            let math = node.kind() == SyntaxKind::MathCall;
            let args = node
                .children()
                .find(|child| matches!(child.kind(), SyntaxKind::Args | SyntaxKind::MathArgs));
            if let Some(args) = args {
                read_args(text, spec, &spec.params, &args, math, "", &mut values)
                    .map_err(|reason| (spec, reason))?;

                // Вложенный элемент: его параметры — в той же форме.
                if let Some(inner_spec) = &spec.inner {
                    let target = super::schema::spec(&inner_spec.element);
                    let body = inner_arg(&args, &spec.params, &inner_spec.param);
                    if let (Some(target), Some(body)) = (target, body) {
                        let callee = body.children().next().map(|c| c.get().full_text());
                        if body.kind() == SyntaxKind::FuncCall
                            && callee.as_deref() == Some(target.typst.as_str())
                        {
                            if let Some(inner_args) =
                                body.children().find(|c| c.kind() == SyntaxKind::Args)
                            {
                                let prefix = format!("{}.", inner_spec.param);
                                read_args(
                                    text,
                                    spec,
                                    &inner_spec.params,
                                    &inner_args,
                                    false,
                                    &prefix,
                                    &mut values,
                                )
                                .map_err(|reason| (spec, reason))?;
                                inner = true;
                            }
                        }
                    }
                }
            }
        }
        ElementSyntax::MarkupHeading => {
            if let Some(marker) = node
                .children()
                .find(|child| child.kind() == SyntaxKind::HeadingMarker)
            {
                let level = marker.get().full_text().chars().count().to_string();
                values.push(ByteValue {
                    name: "level".into(),
                    source: level.clone(),
                    plain: level,
                    expression: false,
                    range: marker.range(),
                    arg: None,
                });
            }
        }
        ElementSyntax::MarkupRaw => {
            if let Some(lang) = node.children().find(|child| child.kind() == SyntaxKind::RawLang) {
                let name = lang.get().full_text().to_string();
                values.push(ByteValue {
                    name: "lang".into(),
                    source: quote(&name),
                    plain: name,
                    expression: false,
                    range: lang.range(),
                    arg: None,
                });
            }
        }
        ElementSyntax::MarkupEquation => {
            let block = node
                .children()
                .nth(1)
                .is_some_and(|second| second.kind() == SyntaxKind::Space);
            let value = if block { "true" } else { "false" };
            let dollar = node.children().next().map(|d| d.range()).unwrap_or(0..0);
            values.push(ByteValue {
                name: "block".into(),
                source: value.into(),
                plain: value.into(),
                expression: false,
                range: dollar.end..dollar.end,
                arg: None,
            });
        }
    }

    Ok(Located {
        spec,
        node,
        start,
        label,
        values,
        inner,
    })
}

fn only_spaces(text: &str) -> bool {
    text.chars().all(char::is_whitespace)
}

/// Узел значения вложенного элемента — позиционный или именованный.
fn inner_arg<'a>(
    args: &LinkedNode<'a>,
    params: &[ParamSpec],
    name: &str,
) -> Option<LinkedNode<'a>> {
    for arg in args.children() {
        if arg.kind() == SyntaxKind::Named
            && arg.children().next().map(|n| n.get().full_text()).as_deref() == Some(name)
        {
            return arg.children().last();
        }
    }
    let index = params
        .iter()
        .filter(|param| param.positional && !param.variadic)
        .position(|param| param.name == name)?;
    positionals(args, false)
        .ok()?
        .into_iter()
        .nth(index)
        .and_then(|group| group.into_iter().next())
}

/// Позиционные аргументы. У вызова в коде — по узлу на значение; в формуле
/// значение — последовательность узлов между запятыми.
fn positionals<'a>(
    args: &LinkedNode<'a>,
    math: bool,
) -> Result<Vec<Vec<LinkedNode<'a>>>, UnavailableReason> {
    let mut groups: Vec<Vec<LinkedNode<'a>>> = Vec::new();
    let mut current: Vec<LinkedNode<'a>> = Vec::new();
    for child in args.children() {
        match child.kind() {
            SyntaxKind::LeftParen | SyntaxKind::RightParen | SyntaxKind::Named => {}
            SyntaxKind::Spread => return Err(UnavailableReason::SpreadArgs),
            SyntaxKind::Semicolon => return Err(UnavailableReason::MathRows),
            SyntaxKind::Comma => {
                if !current.is_empty() {
                    groups.push(std::mem::take(&mut current));
                }
            }
            kind if kind.is_trivia() => {}
            _ => {
                if math {
                    current.push(child);
                } else {
                    groups.push(vec![child]);
                }
            }
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    Ok(groups)
}

/// Значения аргументов вызова.
fn read_args(
    text: &str,
    spec: &ElementSpec,
    params: &[ParamSpec],
    args: &LinkedNode,
    math: bool,
    prefix: &str,
    out: &mut Vec<ByteValue>,
) -> Result<(), UnavailableReason> {
    let settable_only = spec.syntax == ElementSyntax::Set;
    let mut seen: Vec<String> = Vec::new();

    // Именованные.
    for arg in args.children().filter(|arg| arg.kind() == SyntaxKind::Named) {
        let Some(name) = arg.children().next().map(|n| n.get().full_text().to_string()) else {
            continue;
        };
        if seen.contains(&name) {
            return Err(UnavailableReason::DuplicateParam { name });
        }
        seen.push(name.clone());
        let Some(param) = params.iter().find(|param| param.name == name) else {
            // Незнакомый аргумент: не читается и не адресуется (FR-012).
            continue;
        };
        let parts: Vec<LinkedNode> = arg
            .children()
            .skip_while(|child| child.kind() != SyntaxKind::Colon)
            .skip(1)
            .filter(|child| !child.kind().is_trivia())
            .collect();
        let (Some(first), Some(last)) = (parts.first(), parts.last()) else {
            continue;
        };
        let range = first.offset()..last.range().end;
        out.push(value_of(text, param, &parts, range, math, prefix, Some(arg.range())));
    }

    // Позиционные — по порядку схемы; сколько угодно — последнему,
    // вариадическому.
    let groups = positionals(args, math)?;
    if groups.is_empty() {
        return Ok(());
    }
    if settable_only {
        return Err(UnavailableReason::UnknownPositional);
    }
    let slots: Vec<&ParamSpec> = params.iter().filter(|param| param.positional).collect();
    let mut index = 0;
    for slot in &slots {
        if index >= groups.len() {
            break;
        }
        if slot.variadic {
            let first = &groups[index][0];
            let last = groups.last().and_then(|g| g.last()).unwrap_or(first);
            let range = first.offset()..last.range().end;
            let nodes: Vec<LinkedNode> = groups[index..].iter().flatten().cloned().collect();
            out.push(ByteValue {
                name: format!("{prefix}{}", slot.name),
                source: text[range.clone()].to_string(),
                plain: text[range.clone()].to_string(),
                expression: true,
                arg: Some(range.clone()),
                range,
            });
            let _ = nodes;
            index = groups.len();
            break;
        }
        let group = &groups[index];
        let range = group[0].offset()..group[group.len() - 1].range().end;
        out.push(value_of(text, slot, group, range.clone(), math, prefix, Some(range)));
        index += 1;
    }
    if index < groups.len() {
        return Err(UnavailableReason::UnknownPositional);
    }
    Ok(())
}

fn value_of(
    text: &str,
    param: &ParamSpec,
    parts: &[LinkedNode],
    range: std::ops::Range<usize>,
    math: bool,
    prefix: &str,
    arg: Option<std::ops::Range<usize>>,
) -> ByteValue {
    let source = text[range.clone()].to_string();
    let single = (parts.len() == 1).then(|| parts[0].get());
    let expression = math || single.is_none_or(|node| !literal_matches(&param.kind, node));
    let plain = match single {
        Some(node) if !math => plain_of(node, &source),
        _ => source.clone(),
    };
    ByteValue {
        name: format!("{prefix}{}", param.name),
        source,
        plain,
        expression,
        range,
        arg,
    }
}

/// Все метки заметки — для проверки занятости при вставке нового элемента.
pub(crate) fn labels_in(text: &str) -> Vec<String> {
    let root = parse(text);
    let mut labels = Vec::new();
    collect_labels(&LinkedNode::new(&root), None, &mut labels);
    labels
}
