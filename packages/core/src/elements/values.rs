//! Значения параметров: литерал или выражение, и приведение поля формы к
//! записи Typst (research.md §5, FR-012a, FR-015).
//!
//! Правила языка — кавычки строк, экранирование, скобки содержимого — живут
//! здесь, а не в интерфейсе (принцип I): форма передаёт то, что набрал
//! пользователь, а в текст Typst его переводит ядро.

use serde::{Deserialize, Serialize};
use typst::syntax::ast;
use typst::syntax::{SyntaxKind, SyntaxNode};

use super::schema::{ParamKind, ParamSpec};

/// Значение параметра, прочитанное из текста заметки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ParamValue {
    /// Имя параметра; у вложенного элемента — с приставкой: `body.width`.
    pub name: String,
    /// Исходный текст значения — ровно как в заметке.
    pub source: String,
    /// Значение для поля формы: строка без кавычек, содержимое без скобок,
    /// остальное — как `source`.
    pub plain: String,
    /// Записано выражением, а не литералом: поле «как есть» (FR-012a).
    pub expression: bool,
    /// Диапазон значения в тексте, в кодовых единицах UTF-16 — адрес точечной
    /// замены (research.md §4).
    pub from: usize,
    pub to: usize,
}

/// Поле формы. `source` — готовая запись Typst (поле «как есть», выбор из
/// списка, число с единицей); `plain` — то, что набрано в строке или тексте,
/// кавычки и скобки к нему добавит ядро. Оба пусты — параметр на умолчании и в
/// текст не пишется (FR-013).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct FieldValue {
    pub name: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub plain: Option<String>,
}

/// Почему значение поля не годится (FR-015).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum FieldProblem {
    /// Обязательный параметр пуст.
    Required,
    /// Запись не разбирается как значение Typst.
    Unparsable,
    /// Литерал не того вида: строка вместо длины и т. п.
    WrongKind,
    /// Значения нет среди допустимых (FR-014).
    NotAnOption,
    /// Метка уже занята другим элементом заметки.
    LabelTaken,
    /// Метка содержит недопустимые символы.
    LabelInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct FieldError {
    pub name: String,
    pub problem: FieldProblem,
}

/// Литерал ли узел для данного вида значения.
pub(crate) fn literal_matches(kind: &ParamKind, node: &SyntaxNode) -> bool {
    match kind {
        ParamKind::Choice { options } => match node.kind() {
            SyntaxKind::Str | SyntaxKind::Ident => {
                let text = node.full_text();
                options
                    .iter()
                    .any(|option| option.as_str() == text.as_str())
            }
            _ => false,
        },
        ParamKind::Bool => node.kind() == SyntaxKind::Bool,
        ParamKind::Int => node.kind() == SyntaxKind::Int,
        ParamKind::Float => matches!(node.kind(), SyntaxKind::Float | SyntaxKind::Int),
        ParamKind::Numeric { units } => {
            node.kind() == SyntaxKind::Numeric
                && units.iter().any(|unit| {
                    let text = node.full_text();
                    text.ends_with(unit.as_str())
                        && text[..text.len() - unit.len()]
                            .chars()
                            .all(|ch| ch.is_ascii_digit() || ch == '.')
                })
        }
        ParamKind::String => node.kind() == SyntaxKind::Str,
        ParamKind::Content => node.kind() == SyntaxKind::ContentBlock,
        ParamKind::AutoOr { inner } => {
            node.kind() == SyntaxKind::Auto || literal_matches(inner, node)
        }
        ParamKind::NoneOr { inner } => {
            node.kind() == SyntaxKind::None || literal_matches(inner, node)
        }
        ParamKind::Expression => false,
    }
}

/// Значение для поля формы: строка без кавычек, содержимое без скобок.
pub(crate) fn plain_of(node: &SyntaxNode, source: &str) -> String {
    match node.kind() {
        SyntaxKind::Str => node
            .cast::<ast::Str>()
            .map(|string| string.get().to_string())
            .unwrap_or_else(|| source.to_string()),
        SyntaxKind::ContentBlock => {
            let inner = source.strip_prefix('[').unwrap_or(source);
            inner.strip_suffix(']').unwrap_or(inner).to_string()
        }
        _ => source.to_string(),
    }
}

/// Строка Typst в кавычках, с экранированием по правилам языка.
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{{{:x}}}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Вид значения без обёрток «авто/нет» — чтобы решить, нужны ли кавычки.
fn base_kind(kind: &ParamKind) -> &ParamKind {
    match kind {
        ParamKind::AutoOr { inner } | ParamKind::NoneOr { inner } => base_kind(inner),
        other => other,
    }
}

/// Запись Typst по полю формы. `Ok(None)` — параметр на умолчании.
///
/// `math` — значение стоит внутри формулы: там свой синтаксис, и проверка
/// вида литерала по правилам кода к нему неприменима.
pub(crate) fn field_source(
    param: &ParamSpec,
    field: Option<&FieldValue>,
    math: bool,
) -> Result<Option<String>, FieldProblem> {
    let source = match field {
        Some(FieldValue {
            source: Some(source),
            ..
        }) => source.trim().to_string(),
        Some(FieldValue {
            plain: Some(plain), ..
        }) => {
            if plain.is_empty() {
                String::new()
            } else {
                match base_kind(&param.kind) {
                    ParamKind::String if !math => quote(plain),
                    ParamKind::Content if !math => format!("[{plain}]"),
                    _ => plain.trim().to_string(),
                }
            }
        }
        _ => String::new(),
    };

    if source.is_empty() {
        return if param.required && !param.variadic {
            Err(FieldProblem::Required)
        } else {
            Ok(None)
        };
    }
    check_source(param, &source, math)?;
    Ok(Some(source))
}

/// Годится ли запись как значение параметра (FR-015).
///
/// Литерал не того вида — ошибка. Выражение (`100% - 2cm`, `luma(240)`)
/// принимается: вычислить его форма не может, а запрещать — значит отнимать
/// у пользователя то, что Typst примет (FR-012a).
fn check_source(param: &ParamSpec, source: &str, math: bool) -> Result<(), FieldProblem> {
    let root = if math {
        typst::syntax::parse_math(source)
    } else if param.variadic {
        // Ячейки таблицы и прочие «сколько угодно значений» пишутся через
        // запятую — как список они и проверяются.
        typst::syntax::parse_code(&format!("({source},)"))
    } else {
        typst::syntax::parse_code(source)
    };
    if has_error(&root) {
        return Err(FieldProblem::Unparsable);
    }
    if math || param.variadic || param.kind == ParamKind::Expression {
        // Поле «как есть»: вид значения форма не проверяет, лишь бы разбиралось.
        return Ok(());
    }
    let exprs: Vec<&SyntaxNode> = root
        .children()
        .filter(|node| !node.kind().is_trivia())
        .collect();
    let [expr] = exprs.as_slice() else {
        return Err(FieldProblem::Unparsable);
    };
    if literal_matches(&param.kind, expr) || !is_literal_node(expr) {
        return Ok(());
    }
    // Литерал, но не того вида. Для выбора из списка — отдельная причина.
    match base_kind(&param.kind) {
        ParamKind::Choice { .. } if matches!(expr.kind(), SyntaxKind::Str | SyntaxKind::Ident) => {
            Err(FieldProblem::NotAnOption)
        }
        _ => Err(FieldProblem::WrongKind),
    }
}

/// Узел — литерал какого-нибудь вида (а не выражение).
pub(crate) fn is_literal_node(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::Int
            | SyntaxKind::Float
            | SyntaxKind::Numeric
            | SyntaxKind::Str
            | SyntaxKind::Bool
            | SyntaxKind::None
            | SyntaxKind::Auto
            | SyntaxKind::ContentBlock
    )
}

/// Есть ли в поддереве ошибка разбора.
pub(crate) fn has_error(node: &SyntaxNode) -> bool {
    node.kind().is_error() || node.children().any(has_error)
}

/// Годна ли метка: буквы, цифры, `-`, `_`, `.`, `:` — как в синтаксисе Typst.
pub(crate) fn label_is_valid(label: &str) -> bool {
    !label.is_empty()
        && label
            .chars()
            .all(|ch| ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
}
