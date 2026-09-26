//! Параметры элементов — общая таблица для обоих таргетов (фича 008,
//! contracts/element-params.md → «Проверяемые сценарии»).
//!
//! Все функции чистые, поэтому таблица одна и гоняется `cargo test` и
//! `wasm-bindgen-test` без подмен: расхождение между таргетами — дефект.

#![allow(dead_code)]

use typsidian_core::elements::schema::schema_problems;
use typsidian_core::elements::{
    element_at, element_schema, plan_element_edit, render_element, schema, ElementAtResult,
    ElementSyntax, FieldProblem, FieldValue, ParsedElement, PlanEditError, TextEdit,
    UnavailableReason,
};
use typst::syntax::parse;

// ── Помощники ───────────────────────────────────────────────────────────────

fn utf16(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Позиция первого вхождения `needle` в UTF-16.
fn at(text: &str, needle: &str) -> usize {
    let byte = text.find(needle).unwrap_or_else(|| panic!("нет «{needle}» в «{text}»"));
    utf16(&text[..byte])
}

fn found(text: &str, offset: usize) -> ParsedElement {
    match element_at(text, offset) {
        ElementAtResult::Found { element } => element,
        other => panic!("ожидался элемент в «{text}» на {offset}, получено {other:?}"),
    }
}

/// Поля формы ровно такими, какими их прочитала правка, — «ничего не меняли».
fn fields_as_read(element: &ParsedElement) -> Vec<FieldValue> {
    element
        .values
        .iter()
        .map(|value| FieldValue {
            name: value.name.clone(),
            source: None,
            plain: Some(value.plain.clone()),
        })
        .collect()
}

fn source(name: &str, value: &str) -> FieldValue {
    FieldValue {
        name: name.into(),
        source: Some(value.into()),
        plain: None,
    }
}

fn plain(name: &str, value: &str) -> FieldValue {
    FieldValue {
        name: name.into(),
        source: None,
        plain: Some(value.into()),
    }
}

/// Применить замены к тексту — так же, как это сделает редактор.
fn apply(text: &str, edits: &[TextEdit]) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut out: Vec<u16> = Vec::new();
    let mut cursor = 0;
    for edit in edits {
        assert!(edit.from >= cursor, "замены перекрываются: {edits:?}");
        out.extend_from_slice(&units[cursor..edit.from]);
        out.extend(edit.insert.encode_utf16());
        cursor = edit.to;
    }
    out.extend_from_slice(&units[cursor..]);
    String::from_utf16(&out).expect("UTF-16")
}

fn edit(text: &str, element: &ParsedElement, fields: &[FieldValue]) -> String {
    let edits = plan_element_edit(text, element, fields, element.label.as_deref())
        .unwrap_or_else(|error| panic!("правка «{text}» не удалась: {error:?}"));
    apply(text, &edits)
}

fn has_syntax_error(text: &str) -> bool {
    fn walk(node: &typst::syntax::SyntaxNode) -> bool {
        node.kind().is_error() || node.children().any(walk)
    }
    walk(&parse(text))
}

/// Текст элемента в том окружении, где он живёт: формулы — внутри `$ … $`.
fn in_context(id: &str, text: &str) -> String {
    let spec = element_schema(id).expect("схема");
    if spec.syntax == ElementSyntax::Call && spec.typst.starts_with("math.") {
        format!("$ {text} $")
    } else {
        text.to_string()
    }
}

/// Поля вставки по начальным значениям схемы.
fn prefilled(id: &str) -> Vec<FieldValue> {
    element_schema(id)
        .expect("схема")
        .prefill
        .iter()
        .map(|prefill| source(&prefill.name, &prefill.value))
        .collect()
}

// ── Схема (SC-001a) ─────────────────────────────────────────────────────────

pub fn the_schema_matches_typst() {
    let problems = schema_problems();
    assert!(problems.is_empty(), "схема разошлась с Typst:\n{}", problems.join("\n"));
}

pub fn every_element_has_a_titled_main_parameter() {
    for element in &schema().elements {
        let Some(spec) = element_schema(&element.id) else { continue };
        assert!(spec.params.iter().any(|p| p.main), "{}: нет основного", spec.id);
        assert!(spec.params.iter().all(|p| !p.title.trim().is_empty()), "{}", spec.id);
    }
}

pub fn a_heading_inserts_without_a_form_and_a_code_block_with_one() {
    // contracts § «Когда форма».
    assert!(!element_schema("heading-2").unwrap().opens_form_on_insert());
    assert!(element_schema("code-block").unwrap().opens_form_on_insert());
    assert!(element_schema("figure").unwrap().opens_form_on_insert());
    assert!(element_schema("columns").unwrap().opens_form_on_insert());
    assert_eq!(element_schema("no-such-element"), None, "без схемы — сниппет (FR-003)");
}

// ── Вставка (US1) ───────────────────────────────────────────────────────────

/// Круг «вставка → разбор → правка без изменений» для **каждого** элемента
/// схемы (SC-003, SC-007, FR-009).
pub fn every_element_round_trips() {
    for element in &schema().elements {
        let id = element.id.as_str();
        let rendered = render_element(id, &prefilled(id), None, &[])
            .unwrap_or_else(|errors| panic!("{id}: вставка не собралась: {errors:?}"));
        let text = in_context(id, &rendered.text);
        assert!(!has_syntax_error(&text), "{id}: синтаксическая ошибка в «{text}»");

        let offset = at(&text, rendered.text.trim_start_matches('#').split('(').next().unwrap_or(""));
        let parsed = found(&text, offset);
        let canonical = element.same_as.as_deref().unwrap_or(id);
        assert!(
            parsed.id == canonical || parsed.id == id,
            "{id}: разобран как {} в «{text}»",
            parsed.id
        );
        let edits = plan_element_edit(&text, &parsed, &fields_as_read(&parsed), parsed.label.as_deref())
            .unwrap_or_else(|error| panic!("{id}: {error:?}"));
        assert!(edits.is_empty(), "{id}: правка без изменений дала {edits:?} на «{text}»");
    }
}

pub fn defaults_are_not_written() {
    // FR-013: зазор до подписи на умолчании в текст не попадает.
    let mut fields = prefilled("figure");
    fields.push(source("gap", "0.65em"));
    let text = render_element("figure", &fields, None, &[]).unwrap().text;
    assert!(!text.contains("gap"), "{text}");
    assert!(text.starts_with("#figure(\n  image(\"data/график.png\", width: 80%),"), "{text}");
}

pub fn strings_and_content_are_quoted_by_the_core() {
    let text = render_element(
        "external-link",
        &[plain("dest", "https://a.b/\"x\""), plain("body", "сайт")],
        None,
        &[],
    )
    .unwrap()
    .text;
    assert_eq!(text, "#link(\"https://a.b/\\\"x\\\"\")[сайт]");
}

pub fn required_and_wrong_values_are_named() {
    // FR-015.
    let errors = render_element("image", &[plain("width", "80%")], None, &[]).unwrap_err();
    assert!(errors.iter().any(|e| e.name == "source" && e.problem == FieldProblem::Required));

    let errors = render_element(
        "image",
        &[plain("source", "a.png"), source("width", "\"широко\"")],
        None,
        &[],
    )
    .unwrap_err();
    assert!(errors.iter().any(|e| e.name == "width" && e.problem == FieldProblem::WrongKind));

    let errors = render_element(
        "image",
        &[plain("source", "a.png"), source("fit", "\"squash\"")],
        None,
        &[],
    )
    .unwrap_err();
    assert!(errors.iter().any(|e| e.name == "fit" && e.problem == FieldProblem::NotAnOption));

    let errors = render_element("image", &[plain("source", "a.png"), source("width", "80% +")], None, &[])
        .unwrap_err();
    assert!(errors.iter().any(|e| e.name == "width" && e.problem == FieldProblem::Unparsable));

    // Выражение принимается: вычислить его форма не может (FR-012a).
    assert!(render_element("image", &[plain("source", "a.png"), source("width", "100% - 2cm")], None, &[]).is_ok());
}

pub fn labels_are_written_and_checked() {
    let taken = vec!["fig:taken".to_string()];
    let text = render_element("figure", &prefilled("figure"), Some("fig:new"), &taken).unwrap().text;
    assert!(text.ends_with(") <fig:new>"), "{text}");

    let errors = render_element("figure", &prefilled("figure"), Some("fig:taken"), &taken).unwrap_err();
    assert!(errors.iter().any(|e| e.problem == FieldProblem::LabelTaken));
    let errors = render_element("figure", &prefilled("figure"), Some("fig bad"), &taken).unwrap_err();
    assert!(errors.iter().any(|e| e.problem == FieldProblem::LabelInvalid));
}

// ── Поиск элемента (US2) ────────────────────────────────────────────────────

pub fn values_are_read_from_the_text() {
    let text = "#image(\"a.png\", width: 100% - 2cm, fit: \"contain\")";
    let element = found(text, 3);
    let get = |name: &str| element.values.iter().find(|v| v.name == name).unwrap().clone();
    assert_eq!(get("source").plain, "a.png");
    assert!(!get("source").expression);
    assert!(get("width").expression, "вычисление — поле «как есть»");
    assert_eq!(get("width").source, "100% - 2cm");
    assert!(!get("fit").expression);
}

pub fn the_caret_on_the_boundary_is_inside() {
    let text = "до #columns(2)[x] после";
    assert_eq!(found(text, at(text, "#")).id, "columns", "перед #");
    assert_eq!(found(text, at(text, "] после") + 1).id, "columns", "после ]");
    assert!(matches!(element_at(text, 1), ElementAtResult::None), "вне элемента");
}

pub fn offsets_are_utf16() {
    let text = "Ж𝔸 #columns(3)[x]";
    let element = found(text, at(text, "columns"));
    assert_eq!(element.from, at(text, "#"));
    let count = element.values.iter().find(|v| v.name == "count").unwrap();
    assert_eq!(count.from, at(text, "3"));
    assert_eq!(count.to, at(text, "3") + 1);
}

pub fn old_snippet_text_reads_as_ordinary_values() {
    // research.md §11: подстановок в файле нет — только текст.
    let text = "#outline(title: [Содержание], depth: 2)";
    let element = found(text, 2);
    assert_eq!(element.values.iter().find(|v| v.name == "depth").unwrap().plain, "2");
}

// ── Правка (US2) ────────────────────────────────────────────────────────────

pub fn an_unchanged_form_changes_nothing() {
    for text in [
        "#figure(\n  image(\"a.png\", width: 80%),\n  caption: [a (b) [c]],\n) <fig:x>",
        "#set text(size: 11pt, lang: \"ru\")",
        "== Итоги",
        "```rust\nfn main() {}\n```",
        "$ frac(a, b) $",
        "#table(columns: 3, [A], [B], [C])",
    ] {
        let element = found(text, at(text, text.chars().nth(2).unwrap().to_string().as_str()).max(1));
        let edits = plan_element_edit(text, &element, &fields_as_read(&element), element.label.as_deref())
            .unwrap();
        assert!(edits.is_empty(), "«{text}»: {edits:?}");
    }
}

pub fn a_changed_value_is_replaced_in_place() {
    let text = "до #columns(2, gutter: 8pt)[Текст] после";
    let element = found(text, at(text, "columns"));
    let changed = edit(text, &element, &[plain("count", "3")]);
    assert_eq!(changed, "до #columns(3, gutter: 8pt)[Текст] после");
}

pub fn a_new_named_argument_follows_the_call_style() {
    let text = "#columns(2)[x]";
    let changed = edit(text, &found(text, 2), &[source("gutter", "1em")]);
    assert_eq!(changed, "#columns(2, gutter: 1em)[x]");

    let text = "#figure(\n  image(\"a.png\"),\n  caption: [a],\n)";
    let changed = edit(text, &found(text, 2), &[source("gap", "1em")]);
    assert_eq!(changed, "#figure(\n  image(\"a.png\"),\n  caption: [a],\n  gap: 1em,\n)");

    let text = "#quote[q]";
    let changed = edit(text, &found(text, 2), &[source("block", "true")]);
    assert_eq!(changed, "#quote(block: true)[q]");
}

pub fn a_value_reset_to_default_is_removed_with_its_comma() {
    let text = "#image(\"a.png\", width: 80%, height: 2cm)";
    let changed = edit(text, &found(text, 2), &[plain("width", "")]);
    assert_eq!(changed, "#image(\"a.png\", height: 2cm)");
    let changed = edit(text, &found(text, 2), &[plain("height", "")]);
    assert_eq!(changed, "#image(\"a.png\", width: 80%)");

    let text = "#figure(\n  image(\"a.png\"),\n  caption: [a],\n  gap: 1em,\n)";
    let changed = edit(text, &found(text, 2), &[plain("gap", "")]);
    assert_eq!(changed, "#figure(\n  image(\"a.png\"),\n  caption: [a],\n)");
}

pub fn the_inner_element_is_edited_through_the_outer_form() {
    let text = "#figure(image(\"a.png\", width: 80%), caption: [a]) <fig:x>";
    let element = found(text, 2);
    assert_eq!(element.id, "figure");
    assert!(element.inner);
    assert!(element.values.iter().any(|v| v.name == "body.width" && v.source == "80%"));
    let changed = edit(text, &element, &[source("body.width", "60%")]);
    assert_eq!(changed, "#figure(image(\"a.png\", width: 60%), caption: [a]) <fig:x>");

    let table = "#figure(table(columns: 2, [A], [B]), caption: [t])";
    assert_eq!(found(table, 2).id, "figure-table");
}

pub fn labels_are_edited() {
    let text = "#figure(image(\"a.png\")) <fig:old>";
    let element = found(text, 2);
    assert_eq!(element.label.as_deref(), Some("fig:old"));
    let apply_label = |label: Option<&str>| {
        apply(text, &plan_element_edit(text, &element, &[], label).unwrap())
    };
    assert_eq!(apply_label(Some("fig:new")), "#figure(image(\"a.png\")) <fig:new>");
    assert_eq!(apply_label(None), "#figure(image(\"a.png\"))");

    let bare = "#figure(image(\"a.png\"))\n\nсм. <fig:busy>";
    let element = found(bare, 2);
    assert_eq!(element.taken_labels, vec!["fig:busy".to_string()]);
    let added = apply(bare, &plan_element_edit(bare, &element, &[], Some("fig:a")).unwrap());
    assert!(added.starts_with("#figure(image(\"a.png\")) <fig:a>"), "{added}");
    let error = plan_element_edit(bare, &element, &[], Some("fig:busy")).unwrap_err();
    assert!(matches!(error, PlanEditError::Invalid { .. }));
}

pub fn markup_elements_are_edited_in_their_own_syntax() {
    let text = "== Итоги\n";
    assert_eq!(edit(text, &found(text, 3), &[plain("level", "3")]), "=== Итоги\n");

    let text = "```rust\nfn main() {}\n```";
    let element = found(text, 4);
    assert_eq!(edit(text, &element, &[plain("lang", "python")]), "```python\nfn main() {}\n```");
    assert_eq!(edit(text, &element, &[plain("lang", "")]), "```\nfn main() {}\n```");

    let bare = "```\ncode\n```";
    assert_eq!(edit(bare, &found(bare, 1), &[plain("lang", "js")]), "```js\ncode\n```");

    let inline = "формула $x^2$ тут";
    let element = found(inline, at(inline, "x"));
    assert_eq!(edit(inline, &element, &[source("block", "true")]), "формула $ x^2 $ тут");
    let block = "$ x^2 $";
    assert_eq!(edit(block, &found(block, 2), &[source("block", "false")]), "$x^2$");
}

pub fn a_changed_text_is_not_overwritten() {
    // FR-020: форма открыта, текст элемента поменяли — правка не пишется поверх.
    let text = "#columns(2)[x]";
    let element = found(text, 2);
    let changed = "#columns(4)[x]";
    assert_eq!(
        plan_element_edit(changed, &element, &[plain("count", "3")], None),
        Err(PlanEditError::Stale)
    );
}

// ── Непонятое не теряется (US3) ─────────────────────────────────────────────

pub fn unknown_arguments_and_comments_survive() {
    // FR-012.
    let text = "#figure(\n  image(\"a.png\"), // картинка\n  caption: [a (b) [c]],\n  foo: 1,\n)";
    let element = found(text, 2);
    let changed = edit(text, &element, &[plain("caption", "Новая подпись")]);
    assert_eq!(
        changed,
        "#figure(\n  image(\"a.png\"), // картинка\n  caption: [Новая подпись],\n  foo: 1,\n)"
    );
}

pub fn expressions_are_kept_until_replaced() {
    // FR-012a.
    let text = "#columns(n * 2, gutter: 8pt)[x]";
    let element = found(text, 2);
    let count = element.values.iter().find(|v| v.name == "count").unwrap();
    assert!(count.expression);
    let changed = edit(text, &element, &[plain("count", "n * 2"), source("gutter", "1em")]);
    assert_eq!(changed, "#columns(n * 2, gutter: 1em)[x]", "выражение не тронуто");
    let replaced = edit(text, &element, &[plain("count", "3")]);
    assert_eq!(replaced, "#columns(3, gutter: 8pt)[x]");
}

pub fn ambiguous_markup_is_refused_with_a_reason() {
    // FR-011.
    let cases: [(&str, UnavailableReason); 4] = [
        (
            "#image(\"a.png\", width: 40%, width: 60%)",
            UnavailableReason::DuplicateParam { name: "width".into() },
        ),
        ("#figure(image(\"a.png\"", UnavailableReason::Unclosed),
        ("#image(..args)", UnavailableReason::SpreadArgs),
        ("#columns(2, 3)[x]", UnavailableReason::UnknownPositional),
    ];
    for (text, expected) in cases {
        match element_at(text, 2) {
            ElementAtResult::Unavailable { reason, .. } => assert_eq!(reason, expected, "«{text}»"),
            other => panic!("«{text}»: ожидался отказ, получено {other:?}"),
        }
    }
}

// ── Вложенность (US5) ───────────────────────────────────────────────────────

pub fn the_innermost_element_is_chosen() {
    // FR-016.
    let text = "#columns(2)[ #figure(image(\"a.png\", width: 50%)) текст ]";
    assert_eq!(found(text, at(text, "a.png")).id, "image");
    assert_eq!(found(text, at(text, "figure")).id, "figure");
    assert_eq!(found(text, at(text, "текст")).id, "columns");

    // Правка внутреннего не трогает внешний.
    let inner = found(text, at(text, "a.png"));
    let changed = edit(text, &inner, &[source("width", "70%")]);
    assert_eq!(changed, "#columns(2)[ #figure(image(\"a.png\", width: 70%)) текст ]");
}

pub fn math_calls_are_found_inside_formulas() {
    let text = "$ frac(a, b) + 1 $";
    let element = found(text, at(text, "frac") + 1);
    assert_eq!(element.id, "math-frac");
    let changed = edit(text, &element, &[plain("denom", "c + d")]);
    assert_eq!(changed, "$ frac(a, c + d) + 1 $");
}

pub fn initial_values_come_ready_from_the_core() {
    // Форма заполняется при вставке так же, как при правке: строки без
    // кавычек, содержимое без скобок, вложенный элемент — полями `body.*`.
    let figure = element_schema("figure").unwrap();
    assert!(figure.initial_inner);
    let get = |name: &str| figure.initial.iter().find(|v| v.name == name).cloned();
    assert_eq!(get("body.source").unwrap().plain, "data/график.png");
    assert_eq!(get("caption").unwrap().plain, "Подпись");
    assert_eq!(figure.form_on_insert, Some(true));

    let code = element_schema("code-block").unwrap();
    assert_eq!(code.initial.iter().find(|v| v.name == "lang").unwrap().plain, "rust");

    let frac = element_schema("math-frac").unwrap();
    assert_eq!(frac.initial.iter().find(|v| v.name == "denom").unwrap().source, "b");
}
