//! Текст элемента по значениям формы — «Будет вставлено» (FR-004, FR-005).
//!
//! Одна функция на вставку и на показ: форма показывает ровно то, что попадёт
//! в заметку, потому что это один и тот же вызов.

use serde::{Deserialize, Serialize};

use super::schema::{raw_spec, ElementSpec, ElementSyntax, ParamSpec};
use super::values::{field_source, label_is_valid, FieldError, FieldProblem, FieldValue};

/// Текст элемента и позиция курсора после вставки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RenderedElement {
    pub text: String,
    /// Первое пустое место внутри текста (UTF-16) — туда встаёт курсор
    /// (US1 AS3). `None` — пустых мест нет, курсор встаёт после элемента.
    pub caret: Option<usize>,
}

/// Порог, после которого аргументы пишутся по строке на каждый — так их
/// удобнее читать и править руками, как в прежних заготовках каталога.
const INLINE_LIMIT: usize = 50;

/// Текст элемента по значениям полей. Параметры на умолчании не пишутся
/// (FR-013). `label` — метка целиком, без угловых скобок.
pub fn render_element(
    id: &str,
    fields: &[FieldValue],
    label: Option<&str>,
    taken_labels: &[String],
) -> Result<RenderedElement, Vec<FieldError>> {
    let Some(spec) = raw_spec(id) else {
        return Err(vec![FieldError {
            name: String::new(),
            problem: FieldProblem::Unparsable,
        }]);
    };
    let mut errors = Vec::new();

    let mut text = match spec.syntax {
        ElementSyntax::Call | ElementSyntax::Set => match call_text(&spec, fields, "", true) {
            Ok(text) => text,
            Err(mut problems) => {
                errors.append(&mut problems);
                String::new()
            }
        },
        ElementSyntax::MarkupHeading => {
            let level = number_field(&spec, fields, "level", &mut errors)
                .unwrap_or(1)
                .clamp(1, 6);
            format!("{} {}", "=".repeat(level), pseudo(fields, "@body"))
        }
        ElementSyntax::MarkupRaw => {
            let lang = fields
                .iter()
                .find(|field| field.name == "lang")
                .and_then(|field| {
                    field
                        .plain
                        .clone()
                        .or_else(|| field.source.clone().map(unquote))
                })
                .unwrap_or_default();
            format!("```{lang}\n{}\n```", pseudo(fields, "@body"))
        }
        ElementSyntax::MarkupEquation => {
            let block = fields
                .iter()
                .find(|field| field.name == "block")
                .and_then(|field| field.source.as_deref().or(field.plain.as_deref()))
                == Some("true");
            let body = pseudo(fields, "@body");
            if block {
                format!("$ {body} $")
            } else {
                format!("${body}$")
            }
        }
    };

    if let Some(label) = label.filter(|label| !label.is_empty()) {
        if !label_is_valid(label) {
            errors.push(FieldError {
                name: "label".into(),
                problem: FieldProblem::LabelInvalid,
            });
        } else if taken_labels.iter().any(|taken| taken == label) {
            errors.push(FieldError {
                name: "label".into(),
                problem: FieldProblem::LabelTaken,
            });
        } else {
            text.push_str(&format!(" <{label}>"));
        }
    }

    if errors.is_empty() {
        Ok(RenderedElement { text, caret: None })
    } else {
        Err(errors)
    }
}

/// Значение служебного поля (`@body` — текст заголовка, кода, формулы).
fn pseudo(fields: &[FieldValue], name: &str) -> String {
    fields
        .iter()
        .find(|field| field.name == name)
        .and_then(|field| field.plain.clone().or_else(|| field.source.clone()))
        .unwrap_or_default()
}

fn unquote(source: String) -> String {
    source
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_string)
        .unwrap_or(source)
}

fn number_field(
    spec: &ElementSpec,
    fields: &[FieldValue],
    name: &str,
    errors: &mut Vec<FieldError>,
) -> Option<usize> {
    let text = fields
        .iter()
        .find(|field| field.name == name)
        .and_then(|field| field.source.clone().or_else(|| field.plain.clone()))?;
    match text.trim().parse::<usize>() {
        Ok(value) if (1..=6).contains(&value) => Some(value),
        _ => {
            let _ = spec;
            errors.push(FieldError {
                name: name.into(),
                problem: FieldProblem::WrongKind,
            });
            None
        }
    }
}

/// Вызов функции: `#name(args)[body]`, `#set name(args)` или `name(args)` в
/// формуле. `prefix` — приставка имён полей вложенного элемента (`body.`).
pub(crate) fn call_text(
    spec: &ElementSpec,
    fields: &[FieldValue],
    prefix: &str,
    top: bool,
) -> Result<String, Vec<FieldError>> {
    // Вызов внутри формулы: без `#` и без приставки `math.`. `#set
    // math.equation(…)` — код, а не формула, приставка там нужна.
    let math = spec.syntax == ElementSyntax::Call && spec.typst.starts_with("math.");
    let mut errors = Vec::new();
    let mut positional: Vec<String> = Vec::new();
    let mut named: Vec<String> = Vec::new();
    let mut variadic: Vec<String> = Vec::new();
    let mut trailing: Option<String> = None;
    let mut multiline = false;

    let positional_params: Vec<&ParamSpec> = spec
        .params
        .iter()
        .filter(|param| param.positional)
        .collect();
    let last_positional = positional_params.last().map(|param| param.name.clone());

    for param in &spec.params {
        let full = format!("{prefix}{}", param.name);
        let source = match inner_source(spec, param, fields, &mut errors) {
            Some(source) => Some(source),
            None => {
                let field = fields.iter().find(|field| field.name == full);
                match field_source(param, field, math) {
                    Ok(source) => source,
                    Err(problem) => {
                        errors.push(FieldError {
                            name: full,
                            problem,
                        });
                        None
                    }
                }
            }
        };
        let Some(source) = source else { continue };
        // Умолчание в текст не пишется (FR-013).
        if !param.positional && param.default.as_deref() == Some(source.as_str()) {
            continue;
        }
        if source.contains('\n') || (param.variadic && !math) {
            multiline = true;
        }
        if param.positional {
            let is_body = Some(&param.name) == last_positional.as_ref()
                && !math
                && !param.variadic
                && source.starts_with('[')
                && source.ends_with(']');
            if is_body {
                trailing = Some(source);
            } else if param.variadic {
                variadic.push(source);
            } else {
                positional.push(source);
            }
        } else {
            named.push(format!("{}: {source}", param.name));
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let name = if math {
        spec.typst.trim_start_matches("math.").to_string()
    } else {
        spec.typst.clone()
    };
    let head = match (spec.syntax, top && !math) {
        (ElementSyntax::Set, _) => format!("#set {name}"),
        (_, true) => format!("#{name}"),
        _ => name,
    };

    // Порядок, принятый в Typst: позиционные, именованные, затем ячейки и
    // прочие «сколько угодно значений» — `#table(columns: 3, [A], [B])`.
    let args: Vec<String> = positional
        .into_iter()
        .chain(named)
        .chain(variadic)
        .collect();
    let joined = args.join(", ");
    let mut text = head;
    if multiline || joined.chars().count() > INLINE_LIMIT {
        text.push_str("(\n");
        for arg in &args {
            text.push_str("  ");
            text.push_str(&arg.replace('\n', "\n  "));
            text.push_str(",\n");
        }
        text.push(')');
    } else if !args.is_empty() || trailing.is_none() {
        text.push('(');
        text.push_str(&joined);
        text.push(')');
    }
    if let Some(body) = trailing {
        text.push_str(&body);
    }
    Ok(text)
}

/// Значение параметра, в котором стоит вложенный элемент (`body` рисунка):
/// собирается из полей `body.*`, если хоть одно из них задано.
fn inner_source(
    spec: &ElementSpec,
    param: &ParamSpec,
    fields: &[FieldValue],
    errors: &mut Vec<FieldError>,
) -> Option<String> {
    let inner = spec
        .inner
        .as_ref()
        .filter(|inner| inner.param == param.name)?;
    let prefix = format!("{}.", inner.param);
    if !fields.iter().any(|field| field.name.starts_with(&prefix)) {
        return None;
    }
    let target = raw_spec(&inner.element)?;
    match call_text(&target, fields, &prefix, false) {
        Ok(text) => Some(text),
        Err(mut problems) => {
            errors.append(&mut problems);
            None
        }
    }
}
