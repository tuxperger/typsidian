//! Блок выборки `#select(…)` в заметке (research.md §7–§10, contracts/select-library.md).
//!
//! Блок — обычный вызов функции из файла `.typsidian/lib/select.typ`. Результат
//! выборки в файлы vault **не пишется**: перед сборкой ядро находит блоки в
//! тексте заметки ([`select_blocks`]), считает их по индексу и кладёт одной
//! JSON-строкой в `sys.inputs["typsidian-select"]` ([`select_inputs`]); сама
//! `select` ищет свой результат по ключу — каноническому тексту аргументов.
//! Без приложения входов нет, и `select` печатает замещающий текст с
//! условиями (Clarifications 2026-10-15).
//!
//! # Язык аргументов
//!
//! Все аргументы — литералы, чтобы ядро читало их без вычисления Typst:
//!
//! ```text
//! #select(
//!   where: ((tag: "проект"), (key: "статус", op: "=", value: "черновик"),
//!           (modified: 30)),
//!   sort: "-срок", columns: ("название", "статус"), limit: 20, view: "table")
//! ```
//!
//! Условие — словарь одной из форм: `tag`, `folder`, `key`+`exists`,
//! `key`+`op`+(`value` | `date`), `modified`/`created` (число суток или пара
//! дат `"ГГГГ-ММ-ДД"`). Даты — строками, потому что ключ строится через JSON, а
//! JSON даты Typst не кодирует.
//!
//! # Ключ
//!
//! Ключ — JSON всех пяти аргументов (с умолчаниями) с ключами словарей по
//! алфавиту. Ту же строку строит `select.typ`: `json.encode` над теми же
//! значениями. Ключ не зависит от порядка аргументов в вызове.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use typst::syntax::ast::AstNode;
use typst::syntax::{ast, Lines, LinkedNode, SyntaxKind, SyntaxNode};

use super::index::IndexedNote;
use super::query::{
    is_name_column, run_selection, CompareOp, Condition, DateRange, SelectView, Selection,
    SelectionResult, SortKey, Ymd, DEFAULT_COLUMN, DEFAULT_LIMIT,
};
use super::read::{find_records, place_of};
use super::write::render_key;
use super::{now_ms, valid_date, PropertyValue};
use crate::elements::edit::TextEdit;
use crate::elements::utf16_at;
use crate::elements::values::quote;
use crate::structure::SourcePlace;
use crate::vault::backend::{VaultBackend, WriteError};

/// Ключ в `sys.inputs`.
pub const SELECT_INPUT_KEY: &str = "typsidian-select";

/// Путь библиотеки внутри vault.
pub const LIBRARY_PATH: &str = ".typsidian/lib/select.typ";

/// Путь для `#import`: от корня vault.
pub const LIBRARY_IMPORT: &str = "/.typsidian/lib/select.typ";

/// Версия текста библиотеки (первая строка файла).
pub const LIBRARY_VERSION: u32 = 1;

/// Блок `#select(…)` в тексте заметки (data-model.md «SelectBlock»).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SelectBlock {
    /// Канонический текст аргументов; пустой, если аргументы не литералы.
    pub key: String,
    /// Блок целиком, вместе с `#`.
    pub place: SourcePlace,
    /// Прочитанная выборка; `None` — аргументы не литералы.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub selection: Option<Selection>,
    /// Форма может править блок (аргументы — литералы).
    pub editable: bool,
}

// ── JSON ↔ Typst ───────────────────────────────────────────────────────────

fn values_of<'a>(node: &LinkedNode<'a>) -> impl Iterator<Item = LinkedNode<'a>> {
    node.children().filter(|child| {
        !child.kind().is_trivia()
            && !matches!(
                child.kind(),
                SyntaxKind::LeftParen | SyntaxKind::RightParen | SyntaxKind::Comma
            )
    })
}

fn json_number(negative: bool, node: &SyntaxNode) -> Option<Value> {
    match node.kind() {
        SyntaxKind::Int => {
            let n = node.cast::<ast::Int>()?.get();
            Some(Value::from(if negative { n.checked_neg()? } else { n }))
        }
        SyntaxKind::Float => {
            let f = node.cast::<ast::Float>()?.get();
            Number::from_f64(if negative { -f } else { f }).map(Value::Number)
        }
        _ => None,
    }
}

/// Литерал Typst → JSON. `None` — выражение, которое без вычисления не прочесть.
fn json_of(node: &LinkedNode) -> Option<Value> {
    match node.kind() {
        SyntaxKind::Str => Some(Value::String(node.cast::<ast::Str>()?.get().to_string())),
        SyntaxKind::Int | SyntaxKind::Float => json_number(false, node.get()),
        SyntaxKind::Bool => Some(Value::Bool(node.cast::<ast::Bool>()?.get())),
        SyntaxKind::None => Some(Value::Null),
        SyntaxKind::Unary => {
            let unary = node.cast::<ast::Unary>()?;
            matches!(unary.op(), ast::UnOp::Neg)
                .then(|| json_number(true, unary.expr().to_untyped()))
                .flatten()
        }
        SyntaxKind::Parenthesized => {
            let mut inner = values_of(node);
            let only = inner.next()?;
            inner.next().is_none().then(|| json_of(&only)).flatten()
        }
        SyntaxKind::Array => values_of(node)
            .map(|item| json_of(&item))
            .collect::<Option<Vec<_>>>()
            .map(Value::Array),
        SyntaxKind::Dict => {
            let mut map = Map::new();
            for item in node.children() {
                match item.kind() {
                    SyntaxKind::Named | SyntaxKind::Keyed => {
                        let mut parts = item.children().filter(|n| !n.kind().is_trivia());
                        let key_node = parts.next()?;
                        let key = match key_node.kind() {
                            SyntaxKind::Ident => key_node.get().full_text().to_string(),
                            SyntaxKind::Str => key_node.cast::<ast::Str>()?.get().to_string(),
                            _ => return None,
                        };
                        let value = item.children().rfind(|n| !n.kind().is_trivia())?;
                        map.insert(key, json_of(&value)?);
                    }
                    SyntaxKind::Spread => return None,
                    _ => {}
                }
            }
            Some(Value::Object(map))
        }
        _ => None,
    }
}

/// JSON → запись Typst (для текста блока).
pub(crate) fn json_to_typst(value: &Value) -> String {
    match value {
        Value::Null => "none".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => quote(s),
        Value::Array(items) => match items.as_slice() {
            [] => "()".to_string(),
            [one] => format!("({},)", json_to_typst(one)),
            many => format!(
                "({})",
                many.iter()
                    .map(json_to_typst)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        },
        Value::Object(map) if map.is_empty() => "(:)".to_string(),
        Value::Object(map) => format!(
            "({})",
            map.iter()
                .map(|(key, value)| format!("{}: {}", render_key(key), json_to_typst(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn ymd_text(date: &Ymd) -> String {
    format!("{:04}-{:02}-{:02}", date.year, date.month, date.day)
}

fn parse_ymd(text: &str) -> Option<Ymd> {
    let mut parts = text.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !valid_date(year, month, day) {
        return None;
    }
    Some(Ymd { year, month, day })
}

fn op_text(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Eq => "=",
        CompareOp::Ne => "!=",
        CompareOp::Lt => "<",
        CompareOp::Gt => ">",
        CompareOp::Le => "<=",
        CompareOp::Ge => ">=",
    }
}

fn parse_op(text: &str) -> Option<CompareOp> {
    Some(match text {
        "=" => CompareOp::Eq,
        "!=" => CompareOp::Ne,
        "<" => CompareOp::Lt,
        ">" => CompareOp::Gt,
        "<=" => CompareOp::Le,
        ">=" => CompareOp::Ge,
        _ => return None,
    })
}

fn number_json(value: f64) -> Option<Value> {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        Some(Value::from(value as i64))
    } else {
        Number::from_f64(value).map(Value::Number)
    }
}

/// Условие → словарь аргумента `where`. `None` — условие записать нельзя.
pub fn condition_to_json(condition: &Condition) -> Option<Value> {
    let mut map = Map::new();
    let range_json = |range: &DateRange| match range {
        DateRange::LastDays { days } => Value::from(*days),
        DateRange::Between { from, to } => Value::Array(vec![
            Value::String(ymd_text(from)),
            Value::String(ymd_text(to)),
        ]),
    };
    match condition {
        Condition::TagHas { tag } => {
            map.insert("tag".into(), Value::String(tag.clone()));
        }
        Condition::Folder { folder } => {
            map.insert("folder".into(), Value::String(folder.clone()));
        }
        Condition::Exists { key } => {
            map.insert("key".into(), Value::String(key.clone()));
            map.insert("exists".into(), Value::Bool(true));
        }
        Condition::Modified { range } => {
            map.insert("modified".into(), range_json(range));
        }
        Condition::Created { range } => {
            map.insert("created".into(), range_json(range));
        }
        Condition::Compare { key, op, value } => {
            map.insert("key".into(), Value::String(key.clone()));
            map.insert("op".into(), Value::String(op_text(*op).into()));
            match value {
                PropertyValue::Text { value } => {
                    map.insert("value".into(), Value::String(value.clone()));
                }
                PropertyValue::Number { value } => {
                    map.insert("value".into(), number_json(*value)?);
                }
                PropertyValue::Flag { value } => {
                    map.insert("value".into(), Value::Bool(*value));
                }
                PropertyValue::Date { year, month, day } => {
                    let date = Ymd {
                        year: *year,
                        month: *month,
                        day: *day,
                    };
                    map.insert("date".into(), Value::String(ymd_text(&date)));
                }
                PropertyValue::List { .. } | PropertyValue::Unknown { .. } => return None,
            }
        }
    }
    Some(Value::Object(map))
}

fn range_from_json(value: &Value) -> Option<DateRange> {
    match value {
        Value::Number(n) => Some(DateRange::LastDays {
            days: u32::try_from(n.as_u64()?).ok()?,
        }),
        Value::Array(pair) => match pair.as_slice() {
            [Value::String(from), Value::String(to)] => Some(DateRange::Between {
                from: parse_ymd(from)?,
                to: parse_ymd(to)?,
            }),
            _ => None,
        },
        _ => None,
    }
}

/// Словарь аргумента `where` → условие. Лишние ключи — отказ: форма не должна
/// молча терять то, что не поняла.
pub fn condition_from_json(value: &Value) -> Option<Condition> {
    let map = value.as_object()?;
    let text = |key: &str| map.get(key).and_then(Value::as_str).map(str::to_string);
    let only = |keys: &[&str]| map.len() == keys.len() && keys.iter().all(|k| map.contains_key(*k));
    if only(&["tag"]) {
        return Some(Condition::TagHas { tag: text("tag")? });
    }
    if only(&["folder"]) {
        return Some(Condition::Folder {
            folder: text("folder")?,
        });
    }
    if only(&["modified"]) {
        return Some(Condition::Modified {
            range: range_from_json(&map["modified"])?,
        });
    }
    if only(&["created"]) {
        return Some(Condition::Created {
            range: range_from_json(&map["created"])?,
        });
    }
    if only(&["key", "exists"]) {
        return (map["exists"] == Value::Bool(true))
            .then(|| text("key").map(|key| Condition::Exists { key }))
            .flatten();
    }
    if only(&["key", "op", "value"]) {
        let value = match &map["value"] {
            Value::String(s) => PropertyValue::Text { value: s.clone() },
            Value::Number(n) => PropertyValue::Number { value: n.as_f64()? },
            Value::Bool(b) => PropertyValue::Flag { value: *b },
            _ => return None,
        };
        return Some(Condition::Compare {
            key: text("key")?,
            op: parse_op(&text("op")?)?,
            value,
        });
    }
    if only(&["key", "op", "date"]) {
        let date = parse_ymd(&text("date")?)?;
        return Some(Condition::Compare {
            key: text("key")?,
            op: parse_op(&text("op")?)?,
            value: PropertyValue::Date {
                year: date.year,
                month: date.month,
                day: date.day,
            },
        });
    }
    None
}

/// Условия → значение аргумента `where`.
pub fn conditions_to_json(conditions: &[Condition]) -> Option<Value> {
    conditions
        .iter()
        .map(condition_to_json)
        .collect::<Option<Vec<_>>>()
        .map(Value::Array)
}

/// Порядок → значение аргумента `sort`: `"-ключ"` по убыванию, `none` — нет.
pub fn sort_to_json(sort: Option<&SortKey>) -> Value {
    match sort {
        None => Value::Null,
        Some(sort) if sort.descending => Value::String(format!("-{}", sort.key)),
        Some(sort) => Value::String(sort.key.clone()),
    }
}

fn sort_from_json(value: &Value) -> Option<Option<SortKey>> {
    match value {
        Value::Null => Some(None),
        Value::String(s) => {
            let (descending, key) = match s.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, s.as_str()),
            };
            (!key.is_empty()).then(|| {
                Some(SortKey {
                    key: key.to_string(),
                    descending,
                })
            })
        }
        _ => None,
    }
}

/// Канонические аргументы блока с подставленными умолчаниями.
fn canonical_args(given: &BTreeMap<String, Value>) -> Value {
    let mut map = Map::new();
    map.insert(
        "where".into(),
        given.get("where").cloned().unwrap_or(Value::Array(vec![])),
    );
    map.insert(
        "sort".into(),
        given.get("sort").cloned().unwrap_or(Value::Null),
    );
    map.insert(
        "columns".into(),
        given
            .get("columns")
            .cloned()
            .unwrap_or_else(|| Value::Array(vec![Value::String(DEFAULT_COLUMN.into())])),
    );
    map.insert(
        "limit".into(),
        given
            .get("limit")
            .cloned()
            .unwrap_or_else(|| Value::from(DEFAULT_LIMIT)),
    );
    map.insert(
        "view".into(),
        given
            .get("view")
            .cloned()
            .unwrap_or_else(|| Value::String("table".into())),
    );
    Value::Object(map)
}

/// Канонические аргументы → выборка. `None` — значения не того вида.
pub fn selection_from_args(args: &Value) -> Option<Selection> {
    let map = args.as_object()?;
    let conditions = map
        .get("where")?
        .as_array()?
        .iter()
        .map(condition_from_json)
        .collect::<Option<Vec<_>>>()?;
    let sort = sort_from_json(map.get("sort")?)?;
    let columns = map
        .get("columns")?
        .as_array()?
        .iter()
        .map(|c| c.as_str().map(str::to_string))
        .collect::<Option<Vec<_>>>()?;
    let limit = u32::try_from(map.get("limit")?.as_u64()?).ok()?;
    let view = match map.get("view")?.as_str()? {
        "table" => SelectView::Table,
        "list" => SelectView::List,
        _ => return None,
    };
    Some(Selection {
        conditions,
        sort,
        columns,
        limit,
        view,
    })
}

/// Выборка → канонические аргументы (для текста блока и ключа).
pub fn selection_to_args(selection: &Selection) -> Option<Value> {
    let mut given = BTreeMap::new();
    given.insert(
        "where".to_string(),
        conditions_to_json(&selection.conditions)?,
    );
    given.insert("sort".to_string(), sort_to_json(selection.sort.as_ref()));
    given.insert(
        "columns".to_string(),
        Value::Array(
            selection
                .effective_columns()
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    given.insert(
        "limit".to_string(),
        Value::from(match selection.limit {
            0 => DEFAULT_LIMIT,
            n => n,
        }),
    );
    given.insert(
        "view".to_string(),
        Value::String(
            match selection.view {
                SelectView::Table => "table",
                SelectView::List => "list",
            }
            .into(),
        ),
    );
    Some(canonical_args(&given))
}

/// Ключ блока по каноническим аргументам.
pub fn key_of(args: &Value) -> String {
    serde_json::to_string(args).unwrap_or_default()
}

/// Текст вызова `#select(…)` по выборке: параметры на умолчании не пишутся.
pub fn render_call(selection: &Selection) -> Option<String> {
    let args = selection_to_args(selection)?;
    let map = args.as_object()?;
    let mut parts = Vec::new();
    if map["where"] != Value::Array(vec![]) {
        parts.push(format!("where: {}", json_to_typst(&map["where"])));
    }
    if map["sort"] != Value::Null {
        parts.push(format!("sort: {}", json_to_typst(&map["sort"])));
    }
    if map["columns"] != Value::Array(vec![Value::String(DEFAULT_COLUMN.into())]) {
        parts.push(format!("columns: {}", json_to_typst(&map["columns"])));
    }
    if map["limit"].as_u64() != Some(u64::from(DEFAULT_LIMIT)) {
        parts.push(format!("limit: {}", json_to_typst(&map["limit"])));
    }
    if map["view"] != Value::String("table".into()) {
        parts.push(format!("view: {}", json_to_typst(&map["view"])));
    }
    Some(format!("#select({})", parts.join(", ")))
}

// ── Блоки в тексте ─────────────────────────────────────────────────────────

fn is_select_call(node: &LinkedNode) -> bool {
    node.kind() == SyntaxKind::FuncCall
        && node.children().next().is_some_and(|callee| {
            callee.kind() == SyntaxKind::Ident && callee.get().full_text() == "select"
        })
}

/// Читает аргументы вызова: именованные литералы известных имён.
fn read_args(call: &LinkedNode) -> Option<(Value, Selection)> {
    let args = call.children().find(|n| n.kind() == SyntaxKind::Args)?;
    let mut given = BTreeMap::new();
    for arg in values_of(&args) {
        // Содержимое `[…]` после скобок и позиционные аргументы блок не задают.
        if arg.kind() != SyntaxKind::Named {
            return None;
        }
        let name = arg.children().next()?.get().full_text().to_string();
        if !matches!(
            name.as_str(),
            "where" | "sort" | "columns" | "limit" | "view"
        ) {
            return None;
        }
        let value = arg.children().rfind(|n| !n.kind().is_trivia())?;
        if given.insert(name, json_of(&value)?).is_some() {
            return None;
        }
    }
    let canonical = canonical_args(&given);
    let selection = selection_from_args(&canonical)?;
    Some((canonical, selection))
}

fn collect_blocks(node: &LinkedNode, lines: &Lines<&str>, out: &mut Vec<SelectBlock>) {
    let mut hash: Option<usize> = None;
    for child in node.children() {
        if child.kind() == SyntaxKind::Hash {
            hash = Some(child.offset());
            continue;
        }
        if is_select_call(&child) {
            let start = hash.take().unwrap_or_else(|| child.offset());
            let place = place_of(lines, start..child.range().end);
            match read_args(&child) {
                Some((canonical, selection)) => out.push(SelectBlock {
                    key: key_of(&canonical),
                    place,
                    selection: Some(selection),
                    editable: true,
                }),
                None => out.push(SelectBlock {
                    key: String::new(),
                    place,
                    selection: None,
                    editable: false,
                }),
            }
            continue;
        }
        if !child.kind().is_trivia() {
            hash = None;
        }
        collect_blocks(&child, lines, out);
    }
}

/// Блоки `#select(…)` в тексте заметки, по порядку.
pub fn select_blocks(text: &str) -> Vec<SelectBlock> {
    // Предфильтр: заметок без `select` подавляющее большинство.
    if !text.contains("select") {
        return Vec::new();
    }
    let root = typst::syntax::parse(text);
    let lines = Lines::new(text);
    let mut out = Vec::new();
    collect_blocks(&LinkedNode::new(&root), &lines, &mut out);
    out
}

// ── Входы сборки ───────────────────────────────────────────────────────────

fn display(value: Option<&PropertyValue>) -> String {
    match value {
        None => "—".to_string(),
        Some(PropertyValue::Text { value }) => value.clone(),
        Some(PropertyValue::Number { value }) => number_json(*value)
            .map(|n| n.to_string())
            .unwrap_or_else(|| value.to_string()),
        Some(PropertyValue::Date { year, month, day }) => format!("{day:02}.{month:02}.{year:04}"),
        Some(PropertyValue::Flag { value }) => if *value { "✓" } else { "" }.to_string(),
        Some(PropertyValue::List { values }) => values.join(", "),
        Some(PropertyValue::Unknown { source }) => source.clone(),
    }
}

/// Ссылка на заметку в форме vault `[[/путь]]`, от корня; `None`, если путь
/// так записать нельзя.
fn note_link(path: &str) -> Option<String> {
    let shown = path.strip_suffix(".typ").unwrap_or(path);
    let text = format!("[[/{shown}]]");
    let sites = crate::graph::link_sites(&text);
    let [site] = sites.as_slice() else {
        return None;
    };
    (site.raw == format!("/{shown}")
        && crate::graph::resolve_link_target(Path::new(""), &site.raw).as_deref()
            == Some(Path::new(path)))
    .then_some(text)
}

/// Результат выборки в виде, который читает `select.typ`: ячейки уже
/// превращены в тексты.
fn result_json(selection: &Selection, result: &SelectionResult) -> Value {
    let rows: Vec<Value> = result
        .rows
        .iter()
        .map(|row| {
            let cells: Vec<Value> = row
                .cells
                .iter()
                .zip(&result.columns)
                .map(|(cell, column)| {
                    let mut map = Map::new();
                    map.insert("s".into(), Value::String(display(cell.as_ref())));
                    let link = if is_name_column(column) {
                        note_link(&row.path).map(Value::String)
                    } else {
                        None
                    };
                    map.insert("l".into(), link.unwrap_or(Value::Null));
                    Value::Object(map)
                })
                .collect();
            let mut map = Map::new();
            map.insert("cells".into(), Value::Array(cells));
            Value::Object(map)
        })
        .collect();
    let mut map = Map::new();
    map.insert("rows".into(), Value::Array(rows));
    map.insert(
        "columns".into(),
        Value::Array(result.columns.iter().cloned().map(Value::String).collect()),
    );
    map.insert("total".into(), Value::from(result.total));
    map.insert("truncated".into(), Value::Bool(result.truncated));
    let _ = selection;
    Value::Object(map)
}

/// Значение `sys.inputs["typsidian-select"]` для заметки: JSON `{ключ: результат}`
/// по всем блокам с литеральными аргументами. `None` — блоков нет.
pub fn select_inputs(notes: &[IndexedNote], text: &str, now_ms: i64) -> Option<String> {
    let blocks = select_blocks(text);
    let mut all = Map::new();
    for block in &blocks {
        let Some(selection) = &block.selection else {
            continue;
        };
        if all.contains_key(&block.key) {
            continue;
        }
        let result = run_selection(notes, selection, now_ms);
        all.insert(block.key.clone(), result_json(selection, &result));
    }
    if all.is_empty() {
        return None;
    }
    serde_json::to_string(&Value::Object(all)).ok()
}

/// Входы сборки заметки по индексу backend. Заметка без блоков индекс не
/// трогает вовсе.
pub async fn compile_inputs<B: VaultBackend>(backend: &B, text: &str) -> Option<String> {
    if select_blocks(text).iter().all(|b| b.selection.is_none()) {
        return None;
    }
    select_inputs(&backend.indexed_notes().await, text, now_ms())
}

/// Отпечаток входов заметки: изменился — открытую сборку пора обновить
/// (SC-002). `None` — блоков нет.
pub async fn inputs_digest<B: VaultBackend>(backend: &B, text: &str) -> Option<String> {
    compile_inputs(backend, text)
        .await
        .map(|inputs| blake3::hash(inputs.as_bytes()).to_hex().to_string())
}

// ── Форма блока (элемент 008 «select») ──────────────────────────────────────

use crate::elements::schema::{ParamKind, ParamSpec};

fn form_param(name: &str, kind: ParamKind, default: &str, main: bool) -> ParamSpec {
    ParamSpec {
        name: name.to_string(),
        title: name.to_string(),
        docs: String::new(),
        main,
        kind,
        default: Some(default.to_string()),
        positional: false,
        required: false,
        settable: false,
        variadic: false,
    }
}

/// Параметры блока для формы 008: условия, порядок, столбцы, предел, вид.
pub(crate) fn form_params() -> Vec<ParamSpec> {
    vec![
        form_param("where", ParamKind::Conditions, "()", true),
        form_param("sort", ParamKind::Order, "none", true),
        form_param("columns", ParamKind::Columns, "(\"название\",)", true),
        form_param("limit", ParamKind::Int, &DEFAULT_LIMIT.to_string(), false),
        form_param(
            "view",
            ParamKind::Choice {
                options: vec!["\"table\"".into(), "\"list\"".into()],
            },
            "\"table\"",
            true,
        ),
    ]
}

/// Литерал параметра → значение поля формы (JSON). `None` — не литерал.
pub(crate) fn form_plain(kind: &ParamKind, node: &SyntaxNode) -> Option<String> {
    let json = json_of(&LinkedNode::new(node))?;
    match kind {
        ParamKind::Conditions => {
            let conditions = json
                .as_array()?
                .iter()
                .map(condition_from_json)
                .collect::<Option<Vec<_>>>()?;
            serde_json::to_string(&conditions).ok()
        }
        ParamKind::Order => serde_json::to_string(&sort_from_json(&json)?).ok(),
        ParamKind::Columns => {
            let columns = json
                .as_array()?
                .iter()
                .map(|c| c.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()?;
            serde_json::to_string(&columns).ok()
        }
        _ => None,
    }
}

/// Значение поля формы (JSON) → запись Typst. Пустая строка — параметр на
/// умолчании и в текст не пишется.
pub(crate) fn form_source(
    kind: &ParamKind,
    plain: &str,
) -> Result<String, crate::elements::values::FieldProblem> {
    use crate::elements::values::FieldProblem;
    let wrong = |_| FieldProblem::WrongKind;
    match kind {
        ParamKind::Conditions => {
            let conditions: Vec<Condition> = serde_json::from_str(plain).map_err(wrong)?;
            if conditions.is_empty() {
                return Ok(String::new());
            }
            let json = conditions_to_json(&conditions).ok_or(FieldProblem::WrongKind)?;
            Ok(json_to_typst(&json))
        }
        ParamKind::Order => {
            let sort: Option<SortKey> = serde_json::from_str(plain).map_err(wrong)?;
            Ok(match sort {
                None => String::new(),
                Some(sort) => json_to_typst(&sort_to_json(Some(&sort))),
            })
        }
        ParamKind::Columns => {
            let columns: Vec<String> = serde_json::from_str(plain).map_err(wrong)?;
            if columns.is_empty() || columns == [DEFAULT_COLUMN] {
                return Ok(String::new());
            }
            Ok(json_to_typst(&Value::Array(
                columns.into_iter().map(Value::String).collect(),
            )))
        }
        _ => Err(FieldProblem::WrongKind),
    }
}

// ── Строка #import ─────────────────────────────────────────────────────────

/// Правка, добавляющая `#import` библиотеки, если его ещё нет. Строка встаёт
/// после записи свойств (она остаётся первым элементом), иначе — в начало.
pub fn select_import_edits(text: &str) -> Vec<TextEdit> {
    let root = typst::syntax::parse(text);
    let linked = LinkedNode::new(&root);
    let imported = linked.children().any(|node| {
        node.cast::<ast::ModuleImport>().is_some_and(
            |import| matches!(import.source(), ast::Expr::Str(s) if s.get() == LIBRARY_IMPORT),
        )
    });
    if imported {
        return Vec::new();
    }
    let at = match find_records(&linked).records.first() {
        Some(record) => {
            let rest = &text[record.range.end..];
            record.range.end
                + if rest.starts_with("\r\n") {
                    2
                } else {
                    usize::from(rest.starts_with('\n'))
                }
        }
        None => 0,
    };
    let at16 = utf16_at(text, at);
    vec![TextEdit {
        from: at16,
        to: at16,
        insert: format!("#import {}: select\n", quote(LIBRARY_IMPORT)),
    }]
}

// ── Библиотека ─────────────────────────────────────────────────────────────

/// Файл библиотеки: путь, текст и версия.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SelectLibrary {
    pub path: String,
    pub text: String,
    pub version: u32,
}

/// Эталонный файл `select.typ` (contracts/select-library.md).
pub fn select_library() -> SelectLibrary {
    SelectLibrary {
        path: LIBRARY_PATH.to_string(),
        text: LIBRARY_TEXT.to_string(),
        version: LIBRARY_VERSION,
    }
}

/// Что стало с файлом библиотеки (research.md §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum LibraryStatus {
    /// Файла не было (или он старой версии без правок) — записан эталон.
    Written,
    /// Файл уже эталонный (или новее) — не тронут.
    UpToDate,
    /// Версия совпадает, а текст правили руками — не перезаписан; в колонке
    /// свойств показывается предупреждение.
    Customized,
}

/// Версия из первой строки `// typsidian-select vN`.
fn version_of(text: &str) -> Option<u32> {
    text.lines()
        .next()?
        .trim()
        .strip_prefix("// typsidian-select v")?
        .trim()
        .parse()
        .ok()
}

/// Кладёт `.typsidian/lib/select.typ` в vault, если его нет или он старой
/// версии без правок; пользовательскую правку не затирает (research.md §10).
/// Запись — атомарная, через `write_bytes` (фича 009).
pub async fn ensure_select_library<B: VaultBackend>(
    backend: &mut B,
) -> Result<LibraryStatus, WriteError> {
    let path = Path::new(LIBRARY_PATH);
    if let Ok(existing) = backend.read(path).await {
        return Ok(match version_of(&existing) {
            Some(v) if v > LIBRARY_VERSION => LibraryStatus::UpToDate,
            Some(v) if v == LIBRARY_VERSION && existing == LIBRARY_TEXT => LibraryStatus::UpToDate,
            // Версия та же, а текст другой, — правили руками. Старые версии
            // заменялись бы, только если совпадают с эталоном той версии;
            // эталонов старше текущего пока нет, значит файл чужой.
            _ => LibraryStatus::Customized,
        });
    }
    backend.ensure_folder(Path::new(".typsidian/lib")).await?;
    backend.write_bytes(path, LIBRARY_TEXT.as_bytes()).await?;
    Ok(LibraryStatus::Written)
}

/// Текст `select.typ` (contracts/select-library.md).
pub const LIBRARY_TEXT: &str = include_str!("select.typ");
