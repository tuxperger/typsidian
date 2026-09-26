//! Схема параметров элементов (data-model.md «Схема параметров», research.md §1).
//!
//! Схема — файл `schema.json` рядом с этим модулем. В нём **явно перечислены**
//! все параметры каждого элемента каталога вставки: так их видно, их правят, и
//! каждое изменение заметно в diff. Машинные поля параметра (вид значения,
//! умолчание, позиционный ли он) порождаются из самого Typst генератором
//! `examples/element-schema.rs`; подписи и признак «основной» пишутся руками.
//! Расхождение файла с Typst ловит тест [`machine_params`] — полнота схемы
//! доказывается, а не декларируется (SC-001a).
//!
//! Схема поставляется в составе ядра и одна на все клиенты (FR-001c): её
//! читает только ядро, пользователь её не меняет.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Как элемент выглядит в тексте заметки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "kebab-case")]
pub enum ElementSyntax {
    /// `#name(args)[body]` в разметке либо `name(args)` в коде и формулах.
    Call,
    /// `#set name(args)` — только параметры, допускаемые в `set`.
    Set,
    /// `== Текст` — уровень заголовка числом знаков `=` (research.md §7).
    MarkupHeading,
    /// ```` ```lang ```` — язык кодового блока.
    MarkupRaw,
    /// `$ … $` / `$…$` — выключная или строчная формула.
    MarkupEquation,
}

/// Метка `<prefix:…>` после элемента (research.md §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LabelSpec {
    pub prefix: String,
}

/// Вложенный элемент: у рисунка содержимое — вызов `image(…)` или `table(…)`,
/// и его параметры показываются в той же форме.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct InnerSpec {
    /// Параметр внешнего элемента, в котором стоит вложенный: `body`.
    pub param: String,
    /// Id вложенного элемента схемы.
    pub element: String,
    /// Параметры вложенного элемента — подставляются при загрузке схемы, в
    /// файле их нет (одно описание на элемент).
    #[serde(default)]
    pub params: Vec<ParamSpec>,
}

/// Начальное значение поля при вставке — запись Typst.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Prefill {
    pub name: String,
    pub value: String,
}

/// Вид значения параметра — по нему форма выбирает поле.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ParamKind {
    /// Выбор из списка (FR-014). Варианты — записью Typst: `"top"`, `center`.
    Choice { options: Vec<String> },
    Bool,
    Int,
    Float,
    /// Число с единицей: `2cm`, `80%`, `1fr`, `45deg`.
    Numeric { units: Vec<String> },
    String,
    /// Содержимое `[…]`.
    Content,
    AutoOr { inner: Box<ParamKind> },
    NoneOr { inner: Box<ParamKind> },
    /// Поле «как есть»: тип, который форма не рисует виджетом (массивы,
    /// словари, функции, обводки) — исходный текст Typst.
    Expression,
}

/// Параметр элемента (data-model.md «ParamSpec»).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ParamSpec {
    pub name: String,
    /// Как параметр называется пользователю.
    pub title: String,
    /// Основной — виден в форме сразу (FR-012b).
    #[serde(default)]
    pub main: bool,
    pub kind: ParamKind,
    /// Умолчание записью Typst; `None` — умолчания нет.
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub positional: bool,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub settable: bool,
    /// Принимает сколько угодно позиционных значений (ячейки таблицы).
    #[serde(default)]
    pub variadic: bool,
    /// Поправка к виду значения там, где описание Typst расходится с тем, что
    /// функция принимает на деле: `text(…)[содержимое]` описан как строка, но
    /// собственный конструктор принимает `[…]`. Пишется руками, с причиной рядом
    /// в `override_reason`; сверка с Typst идёт по исходному `kind`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_kind: Option<ParamKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_reason: Option<String>,
    /// Вид значения по описанию Typst, если он поправлен (`override_kind`).
    #[serde(skip)]
    pub typst_kind: Option<ParamKind>,
}

/// Элемент каталога со своими параметрами.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ElementSpec {
    /// Тот же id, что у записи каталога вставки в UI.
    pub id: String,
    /// Путь функции Typst: `figure`, `math.frac`, `polygon.regular`.
    pub typst: String,
    pub syntax: ElementSyntax,
    #[serde(default)]
    pub label: Option<LabelSpec>,
    #[serde(default)]
    pub inner: Option<InnerSpec>,
    #[serde(default)]
    pub prefill: Vec<Prefill>,
    #[serde(default)]
    pub params: Vec<ParamSpec>,
    /// Открывать ли форму при вставке. В файле необязателен: по умолчанию —
    /// правило SC-001 (≥ 2 параметров или обязательный без умолчания), схема
    /// может переопределить его явно (contracts § «Когда форма»).
    #[serde(default)]
    pub form_on_insert: Option<bool>,
    /// Запись только для вставки: при правке этот вид элемента разбирается
    /// под записью, на которую указывает `same_as`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_as: Option<String>,
    /// Начальные значения полей при вставке — `prefill`, прочитанный тем же
    /// разбором, что и при правке: форма заполняется одинаково в обоих режимах
    /// и не разбирает запись Typst сама (принцип I). В файле их нет, их
    /// вычисляет [`element_schema`].
    #[serde(default)]
    pub initial: Vec<super::values::ParamValue>,
    /// Вложенный элемент показан при вставке (`initial` содержит его поля).
    #[serde(default)]
    pub initial_inner: bool,
}

impl ElementSpec {
    pub fn param(&self, name: &str) -> Option<&ParamSpec> {
        self.params.iter().find(|param| param.name == name)
    }

    /// Открывает ли вставка форму (contracts § «Когда форма»).
    pub fn opens_form_on_insert(&self) -> bool {
        self.form_on_insert.unwrap_or_else(|| {
            self.params.len() >= 2
                || self
                    .params
                    .iter()
                    .any(|param| param.required && param.default.is_none() && !param.variadic)
        })
    }
}

#[derive(Debug, Deserialize)]
struct SchemaFile {
    typst: String,
    elements: Vec<ElementSpec>,
}

/// Вся схема: версия Typst, для которой она собрана, и элементы.
#[derive(Debug)]
pub struct Schema {
    pub typst: String,
    pub elements: Vec<ElementSpec>,
}

impl Schema {
    pub fn get(&self, id: &str) -> Option<&ElementSpec> {
        self.elements.iter().find(|element| element.id == id)
    }
}

const SCHEMA_JSON: &str = include_str!("schema.json");

/// Схема, прочитанная один раз за процесс.
///
/// Файл — часть сборки: не разобрался — сборка неверна, а не данные
/// пользователя, поэтому паника здесь уместна и ловится первым же тестом.
pub fn schema() -> &'static Schema {
    static SCHEMA: OnceLock<Schema> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        let file: SchemaFile =
            serde_json::from_str(SCHEMA_JSON).expect("schema.json обязан разбираться");
        let mut elements = file.elements;

        // Поправки к виду значения: форма пользуется поправкой, сверка с Typst —
        // исходным видом.
        for element in &mut elements {
            for param in &mut element.params {
                if let Some(kind) = param.override_kind.clone() {
                    param.typst_kind = Some(std::mem::replace(&mut param.kind, kind));
                }
            }
        }

        // Записи `same_as` получают параметры и метку своей основной записи:
        // одно описание параметров на функцию Typst.
        let originals = elements.clone();
        for element in &mut elements {
            if let Some(base) = element.same_as.as_deref() {
                let base = originals
                    .iter()
                    .find(|candidate| candidate.id == base)
                    .unwrap_or_else(|| panic!("same_as «{base}» не найден"));
                element.params = base.params.clone();
                if element.label.is_none() {
                    element.label = base.label.clone();
                }
            }
        }
        // Вложенному элементу — параметры его записи.
        let resolved = elements.clone();
        for element in &mut elements {
            if let Some(inner) = element.inner.as_mut() {
                let target = resolved
                    .iter()
                    .find(|candidate| candidate.id == inner.element)
                    .unwrap_or_else(|| panic!("inner «{}» не найден", inner.element));
                inner.params = target.params.clone();
            }
        }
        Schema {
            typst: file.typst,
            elements,
        }
    })
}

/// Схема элемента каталога (FR-001a). `None` — у элемента нет параметров:
/// он вставляется сразу (FR-003).
pub fn element_schema(id: &str) -> Option<ElementSpec> {
    schema()
        .get(id)
        .filter(|element| !element.params.is_empty())
        .map(|element| {
            // Правило «открывать ли форму» отдаётся уже решённым: интерфейс его
            // не повторяет (принцип I).
            let mut element = element.clone();
            element.form_on_insert = Some(element.opens_form_on_insert());
            if let Some(parsed) = initial_values(&element) {
                element.initial = parsed.values;
                element.initial_inner = parsed.inner;
            }
            element
        })
}

// ── Сверка с Typst ─────────────────────────────────────────────────────────

/// Машинная часть параметра — то, что говорит о нём сам Typst.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineParam {
    pub name: String,
    pub kind: ParamKind,
    pub default: Option<String>,
    pub positional: bool,
    pub required: bool,
    pub settable: bool,
    pub variadic: bool,
}

impl MachineParam {
    pub fn of(param: &ParamSpec) -> Self {
        MachineParam {
            name: param.name.clone(),
            kind: param.typst_kind.clone().unwrap_or_else(|| param.kind.clone()),
            default: param.default.clone(),
            positional: param.positional,
            required: param.required,
            settable: param.settable,
            variadic: param.variadic,
        }
    }
}

/// Функция Typst по пути: `figure`, `math.frac`, `polygon.regular`.
pub fn typst_func(path: &str) -> Option<typst::foundations::Func> {
    use typst::foundations::Value;
    use typst::{Library, LibraryExt};

    static LIBRARY: OnceLock<Library> = OnceLock::new();
    let library = LIBRARY.get_or_init(<Library as LibraryExt>::default);

    let mut segments = path.split('.');
    let first = segments.next()?;
    let mut value = library.global.scope().get(first)?.read().clone();
    for segment in segments {
        let scope = match &value {
            Value::Module(module) => module.scope(),
            Value::Func(func) => func.scope()?,
            Value::Type(ty) => ty.scope(),
            _ => return None,
        };
        value = scope.get(segment)?.read().clone();
    }
    match value {
        Value::Func(func) => Some(func),
        Value::Type(ty) => ty.constructor().ok(),
        _ => None,
    }
}

/// Что Typst говорит о параметрах функции — в той форме, в какой они лежат в
/// схеме. Для `set` — только допускаемые в `set` (research.md §7).
pub fn machine_params(path: &str, syntax: ElementSyntax) -> Option<Vec<MachineParam>> {
    use typst::foundations::Repr;

    let func = typst_func(path)?;
    let params = func
        .params()
        .filter_map(|info| info.to_native())
        .filter(|info| syntax != ElementSyntax::Set || info.settable)
        .map(|info| MachineParam {
            name: info.name.to_string(),
            kind: kind_of(&info.input),
            default: info.default.map(|default| default().repr().to_string()),
            positional: info.positional,
            required: info.required,
            settable: info.settable,
            variadic: info.variadic,
        })
        .collect();
    Some(params)
}

/// Варианты значения выравнивания и направления — идентификаторы Typst.
const ALIGNMENTS: [&str; 8] = [
    "left", "center", "right", "start", "end", "top", "horizon", "bottom",
];
const DIRECTIONS: [&str; 4] = ["ltr", "rtl", "ttb", "btt"];

/// Вид значения по описанию приведения типов Typst.
pub fn kind_of(info: &typst::foundations::CastInfo) -> ParamKind {
    use typst::foundations::{CastInfo, Repr, Value};

    fn walk(
        info: &CastInfo,
        any: &mut bool,
        auto: &mut bool,
        none: &mut bool,
        options: &mut Vec<String>,
        types: &mut Vec<&'static str>,
    ) {
        match info {
            CastInfo::Any => *any = true,
            CastInfo::Value(Value::Auto, _) => *auto = true,
            CastInfo::Value(Value::None, _) => *none = true,
            CastInfo::Value(value, _) => options.push(value.repr().to_string()),
            CastInfo::Type(ty) => match ty.short_name() {
                "auto" => *auto = true,
                "none" => *none = true,
                name => {
                    if !types.contains(&name) {
                        types.push(name);
                    }
                }
            },
            CastInfo::Union(parts) => {
                for part in parts {
                    walk(part, any, auto, none, options, types);
                }
            }
        }
    }

    let (mut any, mut auto, mut none) = (false, false, false);
    let mut options = Vec::new();
    let mut types = Vec::new();
    walk(info, &mut any, &mut auto, &mut none, &mut options, &mut types);
    // Типы, которые в тексте заметки литералом не пишутся: их значение всегда
    // выражение (`it => …`, `read(…, encoding: none)`). Выбрать поле для
    // остальных они не мешают — выражение в таком параметре всё равно
    // сохранится как есть (FR-012a). Без этого `numbering: "1"` (строка или
    // функция) и путь к картинке (строка или байты) остались бы без поля.
    const NOT_LITERAL: [&str; 13] = [
        "function", "bytes", "selector", "dictionary", "array", "module", "type",
        "location", "regex", "arguments", "counter", "state", "label",
    ];
    let before = types.len();
    types.retain(|ty| !NOT_LITERAL.contains(ty));
    if before > 0 && types.is_empty() && options.is_empty() {
        return ParamKind::Expression;
    }
    types.sort_unstable();

    let numeric_units = |types: &[&str]| -> Option<Vec<String>> {
        let mut units: Vec<String> = Vec::new();
        for ty in types {
            let add: &[&str] = match *ty {
                "length" => &["pt", "mm", "cm", "in", "em"],
                "ratio" => &["%"],
                "relative" => &["pt", "mm", "cm", "in", "em", "%"],
                "fraction" => &["fr"],
                "angle" => &["deg", "rad"],
                _ => return None,
            };
            for unit in add {
                if !units.iter().any(|known| known == unit) {
                    units.push((*unit).to_string());
                }
            }
        }
        Some(units)
    };

    let base = if any {
        ParamKind::Expression
    } else if types.is_empty() && !options.is_empty() {
        ParamKind::Choice { options }
    } else if !options.is_empty() {
        ParamKind::Expression
    } else {
        match types.as_slice() {
            [] => ParamKind::Expression,
            ["bool"] => ParamKind::Bool,
            ["int"] => ParamKind::Int,
            ["float"] | ["float", "int"] => ParamKind::Float,
            ["str"] | ["path"] | ["path", "str"] => ParamKind::String,
            ["content"] | ["content", "str"] => ParamKind::Content,
            ["alignment"] => ParamKind::Choice {
                options: ALIGNMENTS.iter().map(|name| name.to_string()).collect(),
            },
            ["direction"] => ParamKind::Choice {
                options: DIRECTIONS.iter().map(|name| name.to_string()).collect(),
            },
            other => match numeric_units(other) {
                Some(units) => ParamKind::Numeric { units },
                None => ParamKind::Expression,
            },
        }
    };

    if base == ParamKind::Expression {
        return base;
    }
    // Порядок обёрток фиксирован, чтобы вид значения был однозначен.
    let mut kind = base;
    if none {
        kind = ParamKind::NoneOr {
            inner: Box::new(kind),
        };
    }
    if auto {
        kind = ParamKind::AutoOr {
            inner: Box::new(kind),
        };
    }
    kind
}

/// Расхождения схемы с Typst — пустой список, если схема полна и точна
/// (SC-001a). Общий для теста и генератора.
pub fn schema_problems() -> Vec<String> {
    let mut problems = Vec::new();
    let schema = schema();
    let mut seen = std::collections::HashSet::new();

    for element in &schema.elements {
        if !seen.insert(element.id.as_str()) {
            problems.push(format!("{}: id повторён", element.id));
        }
        if element.same_as.is_some() {
            continue;
        }
        let Some(machine) = machine_params(&element.typst, element.syntax) else {
            problems.push(format!("{}: в Typst нет функции {}", element.id, element.typst));
            continue;
        };
        let markup = matches!(
            element.syntax,
            ElementSyntax::MarkupHeading | ElementSyntax::MarkupRaw | ElementSyntax::MarkupEquation
        );

        for param in &element.params {
            if param.title.trim().is_empty() {
                problems.push(format!("{}.{}: нет подписи", element.id, param.name));
            }
            match machine.iter().find(|known| known.name == param.name) {
                None => problems.push(format!(
                    "{}.{}: такого параметра у {} нет",
                    element.id, param.name, element.typst
                )),
                Some(known) if *known != MachineParam::of(param) => problems.push(format!(
                    "{}.{}: в схеме {:?}, в Typst {:?}",
                    element.id,
                    param.name,
                    MachineParam::of(param),
                    known
                )),
                Some(_) => {}
            }
        }
        // Профиль разметки — подмножество по замыслу (research.md §7); у
        // остальных — полный набор, в порядке справочника Typst.
        if !markup {
            let names: Vec<&str> = element.params.iter().map(|p| p.name.as_str()).collect();
            let expected: Vec<&str> = machine.iter().map(|p| p.name.as_str()).collect();
            if names != expected {
                problems.push(format!(
                    "{}: параметры {:?}, в Typst {:?}",
                    element.id, names, expected
                ));
            }
        }
        if !element.params.is_empty() && !element.params.iter().any(|p| p.main) {
            problems.push(format!("{}: нет ни одного основного параметра", element.id));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schema_matches_typst() {
        let problems = schema_problems();
        assert!(problems.is_empty(), "схема разошлась с Typst:\n{}", problems.join("\n"));
    }
}

/// Начальные значения: элемент собирается из `prefill` и читается обратно.
fn initial_values(element: &ElementSpec) -> Option<super::locate::ParsedElement> {
    use super::locate::{element_at, ElementAtResult};
    use super::values::FieldValue;

    let fields: Vec<FieldValue> = element
        .prefill
        .iter()
        .map(|prefill| FieldValue {
            name: prefill.name.clone(),
            source: Some(prefill.value.clone()),
            plain: None,
        })
        .collect();
    let rendered = super::render::render_element(&element.id, &fields, None, &[]).ok()?;
    // Вызов из формулы читается только внутри формулы.
    let math = element.syntax == ElementSyntax::Call && element.typst.starts_with("math.");
    let (text, offset) = if math {
        (format!("$ {} $", rendered.text), 2)
    } else {
        (rendered.text, 0)
    };
    match element_at(&text, offset) {
        ElementAtResult::Found { element } => Some(element),
        _ => None,
    }
}

/// Схема элемента без вычисленных полей — для сборки текста, которой
/// начальные значения не нужны (и которая сама их вычисляет).
pub(crate) fn raw_spec(id: &str) -> Option<ElementSpec> {
    schema()
        .get(id)
        .filter(|element| !element.params.is_empty())
        .cloned()
}
