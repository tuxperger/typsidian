//! Схема параметров элементов (data-model.md «Схема параметров», research.md §1).
//!
//! Схемы как файла нет: параметры каждого элемента **читаются из самого Typst**
//! в момент обращения — `Func::params()` отдаёт имя, вид значения, умолчание,
//! позиционность, обязательность и описание любого параметра встроенной
//! функции. Поэтому полнота не выверяется и не проверяется тестом — она
//! следует из построения (SC-001a): после обновления Typst форма сама
//! показывает новый набор параметров.
//!
//! Элемент называется **ключом** — синтаксис и путь функции Typst:
//! `call:figure`, `set:text`, `markup-heading:heading`, `call:math.frac`.
//! Рисунок с вложенным элементом несёт его в ключе: `call:figure>body=image`.
//! Какой элемент вставляет запись каталога, ядро узнаёт, разобрав её шаблон
//! парсером Typst ([`element_schema_for`]), — отдельного списка элементов в
//! ядре нет.
//!
//! В коде остаются только правила, которых в описании Typst нет по природе,
//! каждое с причиной рядом: профиль разметки (research.md §7), поправка вида
//! у `text`, позиционность `factor` у `scale` и вложенный элемент рисунка с
//! префиксом его метки.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

/// Как элемент выглядит в тексте заметки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl ElementSyntax {
    fn tag(self) -> &'static str {
        match self {
            ElementSyntax::Call => "call",
            ElementSyntax::Set => "set",
            ElementSyntax::MarkupHeading => "markup-heading",
            ElementSyntax::MarkupRaw => "markup-raw",
            ElementSyntax::MarkupEquation => "markup-equation",
        }
    }

    fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "call" => ElementSyntax::Call,
            "set" => ElementSyntax::Set,
            "markup-heading" => ElementSyntax::MarkupHeading,
            "markup-raw" => ElementSyntax::MarkupRaw,
            "markup-equation" => ElementSyntax::MarkupEquation,
            _ => return None,
        })
    }
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
    /// Ключ вложенного элемента: `call:image`.
    pub element: String,
    /// Параметры вложенного элемента.
    #[serde(default)]
    pub params: Vec<ParamSpec>,
}

/// Служебное значение, которое форма не показывает, но отдаёт ядру как есть:
/// `@body` — текст заголовка, кода или формулы.
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
    Choice {
        options: Vec<String>,
    },
    Bool,
    Int,
    Float,
    /// Число с единицей: `2cm`, `80%`, `1fr`, `45deg`.
    Numeric {
        units: Vec<String>,
    },
    String,
    /// Содержимое `[…]`.
    Content,
    AutoOr {
        inner: Box<ParamKind>,
    },
    NoneOr {
        inner: Box<ParamKind>,
    },
    /// Поле «как есть»: тип, который форма не рисует виджетом (массивы,
    /// словари, функции, обводки) — исходный текст Typst.
    Expression,
    /// Условия отбора блока выборки (фича 020). Значение поля — JSON списка
    /// [`crate::properties::Condition`]; запись Typst строит ядро.
    Conditions,
    /// Порядок блока выборки: JSON `null` или [`crate::properties::SortKey`].
    Order,
    /// Столбцы блока выборки: JSON списка имён.
    Columns,
}

/// Параметр элемента (data-model.md «ParamSpec»).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ParamSpec {
    pub name: String,
    /// Как параметр называется пользователю — его имя в Typst: то же слово,
    /// что он пишет в тексте заметки и видит в справочнике.
    pub title: String,
    /// Первый абзац описания параметра из Typst — подсказка к полю.
    #[serde(default)]
    pub docs: String,
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
}

/// Элемент со своими параметрами.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ElementSpec {
    /// Ключ элемента: `call:figure`, `set:text`, `call:figure>body=image`.
    pub id: String,
    /// Путь функции Typst: `figure`, `math.frac`, `polygon.regular`.
    pub typst: String,
    pub syntax: ElementSyntax,
    #[serde(default)]
    pub label: Option<LabelSpec>,
    #[serde(default)]
    pub inner: Option<InnerSpec>,
    /// Служебные значения вставки (`@body`) — из шаблона каталога.
    #[serde(default)]
    pub prefill: Vec<Prefill>,
    #[serde(default)]
    pub params: Vec<ParamSpec>,
    /// Открывать ли форму при вставке — уже решённое ядром правило
    /// (contracts § «Когда форма»).
    #[serde(default)]
    pub form_on_insert: Option<bool>,
    /// Начальные значения полей при вставке — шаблон каталога, прочитанный
    /// тем же разбором, что и при правке: форма заполняется одинаково в обоих
    /// режимах и не разбирает запись Typst сама (принцип I).
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
        match self.syntax {
            // Язык кода — единственное, что стоит выбрать при вставке блока,
            // и выбирается он в форме, а не набором после вставки.
            ElementSyntax::MarkupRaw => true,
            // Уровень заголовка и вид формулы задаёт сама запись каталога.
            ElementSyntax::MarkupHeading | ElementSyntax::MarkupEquation => false,
            ElementSyntax::Call | ElementSyntax::Set => {
                self.params.len() >= 2
                    || self
                        .params
                        .iter()
                        .any(|param| param.required && param.default.is_none() && !param.variadic)
            }
        }
    }
}

// ── Ключ элемента ──────────────────────────────────────────────────────────

/// Ключ элемента по синтаксису, пути функции и вложенному элементу.
pub fn key_for(syntax: ElementSyntax, path: &str, inner: Option<(&str, &str)>) -> String {
    match inner {
        Some((param, element)) => format!("{}:{path}>{param}={element}", syntax.tag()),
        None => format!("{}:{path}", syntax.tag()),
    }
}

/// Разбор ключа обратно: синтаксис, путь, вложенный (`param`, путь).
fn split_key(key: &str) -> Option<(ElementSyntax, &str, Option<(&str, &str)>)> {
    let (tag, rest) = key.split_once(':')?;
    let syntax = ElementSyntax::from_tag(tag)?;
    match rest.split_once('>') {
        Some((path, inner)) => {
            let (param, element) = inner.split_once('=')?;
            Some((syntax, path, Some((param, element))))
        }
        None => Some((syntax, rest, None)),
    }
}

// ── Правила, которых нет в описании Typst ──────────────────────────────────

/// Профиль разметки (research.md §7): в записи `== Текст`, ```` ```lang ````
/// и `$…$` физически выражается ровно один параметр функции — остальные
/// пишутся только вызовом, и форма разметки их не предлагает.
fn markup_profile(syntax: ElementSyntax) -> Option<(&'static str, &'static str)> {
    match syntax {
        ElementSyntax::MarkupHeading => Some(("heading", "level")),
        ElementSyntax::MarkupRaw => Some(("raw", "lang")),
        ElementSyntax::MarkupEquation => Some(("math.equation", "block")),
        ElementSyntax::Call | ElementSyntax::Set => None,
    }
}

/// Поправка вида значения там, где описание Typst расходится с тем, что
/// функция принимает на деле.
///
/// `text(…)[содержимое]`: позиционный `text` описан строкой, но собственный
/// конструктор `text` принимает и `[…]` — без поправки форма взяла бы
/// содержимое в кавычки и сломала бы разметку внутри него.
fn corrected_kind(path: &str, param: &str, kind: ParamKind) -> ParamKind {
    match (path, param) {
        ("text", "text") => ParamKind::Content,
        _ => kind,
    }
}

/// Поправка позиционности там, где описание Typst её теряет.
///
/// `scale(150%)`: в исходнике Typst `factor` помечен `#[positional]`, но поле
/// объявлено `#[external]`, и `Func::params()` отдаёт его только именованным.
/// Без поправки обычная запись `#scale(150%)[…]` не разбиралась бы вовсе.
fn corrected_positional(path: &str, param: &str, positional: bool) -> bool {
    positional || matches!((path, param), ("scale", "factor"))
}

/// У каких функций содержимое — вложенный элемент со своими параметрами в той
/// же форме: рисунок держит `image(…)` или `table(…)`, и править размер
/// картинки отдельной формой было бы неудобно.
pub(crate) fn inner_param_of(path: &str) -> Option<&'static str> {
    match path {
        "figure" => Some("body"),
        _ => None,
    }
}

/// Префикс метки элемента (research.md §6). Метка у рисунка — договорённость
/// оформления, а не часть языка: `fig:` у картинки, `tbl:` у таблицы, `lst:` у
/// листинга.
fn label_of(path: &str, inner: Option<&str>) -> Option<LabelSpec> {
    if path != "figure" {
        return None;
    }
    let prefix = match inner {
        Some("table") => "tbl",
        Some("raw") => "lst",
        _ => "fig",
    };
    Some(LabelSpec {
        prefix: prefix.to_string(),
    })
}

// ── Сборка из Typst ────────────────────────────────────────────────────────

/// Элемент по ключу. Собирается из Typst при первом обращении и дальше
/// берётся из кэша: описание функций за время работы не меняется. `None` —
/// такой функции в Typst нет.
pub fn spec(key: &str) -> Option<&'static ElementSpec> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<&'static ElementSpec>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(found) = cache.lock().ok()?.get(key) {
        return *found;
    }
    // Сборка — без блокировки: вложенный элемент собирается тем же путём.
    let built: Option<&'static ElementSpec> = build(key).map(|spec| &*Box::leak(Box::new(spec)));
    let mut guard = cache.lock().ok()?;
    // Число ключей ограничено числом функций Typst — утечка конечна.
    Some(*guard.entry(key.to_string()).or_insert(built)).flatten()
}

fn build(key: &str) -> Option<ElementSpec> {
    let (syntax, path, inner) = split_key(key)?;
    let params: Vec<ParamSpec> = match markup_profile(syntax) {
        Some((function, only)) => {
            if path != function {
                return None;
            }
            params_of(function, syntax)?
                .into_iter()
                .filter(|param| param.name == only)
                // Единственный параметр разметки и есть основной.
                .map(|param| ParamSpec {
                    main: true,
                    ..param
                })
                .collect()
        }
        // `select` — функция библиотеки vault, а не встроенная: описания Typst
        // у неё нет, схему задаёт ядро свойств (фича 020).
        None if path == "select" && syntax == ElementSyntax::Call => {
            crate::properties::select::form_params()
        }
        None => params_of(path, syntax)?,
    };

    let inner = match inner {
        Some((param, element)) => {
            if inner_param_of(path) != Some(param) {
                return None;
            }
            let element = key_for(ElementSyntax::Call, element, None);
            let target = spec(&element)?;
            Some(InnerSpec {
                param: param.to_string(),
                element,
                params: target.params.clone(),
            })
        }
        None => None,
    };
    let inner_path = inner
        .as_ref()
        .and_then(|inner| split_key(&inner.element).map(|(_, path, _)| path.to_string()));

    Some(ElementSpec {
        id: key.to_string(),
        typst: path.to_string(),
        syntax,
        label: if syntax == ElementSyntax::Call {
            label_of(path, inner_path.as_deref())
        } else {
            None
        },
        inner,
        prefill: Vec::new(),
        params,
        form_on_insert: None,
        initial: Vec::new(),
        initial_inner: false,
    })
}

/// Параметры функции так, как их описывает Typst.
fn params_of(path: &str, syntax: ElementSyntax) -> Option<Vec<ParamSpec>> {
    let machine = machine_params(path, syntax)?;
    Some(
        machine
            .into_iter()
            .map(|param| {
                let positional = corrected_positional(path, &param.name, param.positional);
                ParamSpec {
                    title: param.name.clone(),
                    kind: corrected_kind(path, &param.name, param.kind),
                    // Без позиционного и обязательного элемент не записать —
                    // они видны сразу; остальное в «Все параметры» (FR-012b).
                    main: positional || param.required,
                    docs: param.docs,
                    name: param.name,
                    default: param.default,
                    positional,
                    required: param.required,
                    settable: param.settable,
                    variadic: param.variadic,
                }
            })
            .collect(),
    )
}

/// Схема элемента для правки (FR-001a). `None` — такого элемента нет или у
/// него нет параметров: он вставляется сразу (FR-003).
pub fn element_schema(key: &str) -> Option<ElementSpec> {
    let spec = spec(key).filter(|spec| !spec.params.is_empty())?;
    let mut spec = spec.clone();
    // Правило «открывать ли форму» отдаётся уже решённым: интерфейс его не
    // повторяет (принцип I).
    spec.form_on_insert = Some(spec.opens_form_on_insert());
    ensure_main(&mut spec.params);
    Some(spec)
}

/// Порог, до которого параметры без явного основного показываются все сразу.
const SHOW_ALL_UP_TO: usize = 3;

/// Форма без единого видимого поля бесполезна: у `#pagebreak()` нет ни
/// позиционных, ни обязательных параметров. Когда их немного, основными
/// становятся все; когда много (`set text` — три десятка), основные задаёт
/// текст элемента, а остальное ждёт в «Все параметры» (FR-012b).
fn ensure_main(params: &mut [ParamSpec]) {
    if params.len() <= SHOW_ALL_UP_TO && !params.iter().any(|param| param.main) {
        for param in params {
            param.main = true;
        }
    }
}

/// Схема элемента, который вставляет запись каталога, — по тексту её шаблона
/// (без полей сниппета).
///
/// Шаблон разбирается тем же разбором, что и элемент в заметке: из него ядро
/// узнаёт функцию Typst, а значения шаблона становятся начальными значениями
/// формы. Вызов из формулы (`frac(a, b)`) читается внутри `$ … $`. `None` —
/// шаблон не элемент с параметрами (список, выделение), и он вставляется
/// сниппетом (FR-003).
pub fn element_schema_for(template: &str) -> Option<ElementSpec> {
    use super::locate::{element_at, ElementAtResult};

    let text = template.trim();
    if text.is_empty() {
        return None;
    }
    let whole = |text: &str, offset: usize, end: usize| match element_at(text, offset) {
        ElementAtResult::Found { element } if element.from == offset && element.to == end => {
            Some(element)
        }
        _ => None,
    };
    let units = text.encode_utf16().count();
    let parsed = whole(text, 0, units).or_else(|| {
        let wrapped = format!("$ {text} $");
        whole(&wrapped, 2, 2 + units).filter(|parsed| parsed.id.starts_with("call:math."))
    })?;

    let mut spec = element_schema(&parsed.id)?;
    // Что шаблон задал — видно сразу: без этого `#set text(font: …)` открыл
    // бы форму, где выбранный шаблоном шрифт спрятан под «Все параметры».
    for value in &parsed.values {
        match value.name.split_once('.') {
            Some((_, name)) => {
                if let Some(inner) = spec.inner.as_mut() {
                    if let Some(param) = inner.params.iter_mut().find(|p| p.name == name) {
                        param.main = true;
                    }
                }
            }
            None => {
                if let Some(param) = spec.params.iter_mut().find(|p| p.name == value.name) {
                    param.main = true;
                }
            }
        }
    }
    if let Some(body) = markup_body(spec.syntax, text) {
        spec.prefill.push(Prefill {
            name: "@body".into(),
            value: body,
        });
    }
    spec.initial_inner = parsed.inner;
    spec.initial = parsed.values;
    Some(spec)
}

/// Текст внутри разметки — то, что форма не правит, но обязана вставить.
fn markup_body(syntax: ElementSyntax, text: &str) -> Option<String> {
    match syntax {
        ElementSyntax::MarkupHeading => Some(text.trim_start_matches('=').trim().to_string()),
        ElementSyntax::MarkupRaw => {
            let inside = text.trim_start_matches('`');
            let (_, rest) = inside.split_once('\n')?;
            Some(
                rest.trim_end_matches('`')
                    .trim_end_matches('\n')
                    .to_string(),
            )
        }
        ElementSyntax::MarkupEquation => Some(text.trim_matches('$').trim().to_string()),
        ElementSyntax::Call | ElementSyntax::Set => None,
    }
}

/// Схема без вычисленных полей — для сборки текста.
pub(crate) fn raw_spec(key: &str) -> Option<&'static ElementSpec> {
    spec(key).filter(|spec| !spec.params.is_empty())
}

// ── Описание функций Typst ─────────────────────────────────────────────────

/// Машинная часть параметра — то, что говорит о нём сам Typst.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineParam {
    pub name: String,
    pub docs: String,
    pub kind: ParamKind,
    pub default: Option<String>,
    pub positional: bool,
    pub required: bool,
    pub settable: bool,
    pub variadic: bool,
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

/// Что Typst говорит о параметрах функции. Для `set` — только допускаемые в
/// `set` (research.md §7).
pub fn machine_params(path: &str, syntax: ElementSyntax) -> Option<Vec<MachineParam>> {
    use typst::foundations::Repr;

    let func = typst_func(path)?;
    let params = func
        .params()
        .filter_map(|info| info.to_native())
        .filter(|info| syntax != ElementSyntax::Set || info.settable)
        .map(|info| MachineParam {
            name: info.name.to_string(),
            docs: first_paragraph(info.docs),
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

/// Первый абзац описания из справочника Typst одной строкой.
///
/// Ссылки справочника `@image[картинка]` становятся своим текстом, голые
/// `@image` — именем: в подсказке у поля ссылаться некуда.
fn first_paragraph(docs: &str) -> String {
    let paragraph = docs.trim().split("\n\n").next().unwrap_or_default();
    let flat = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = String::with_capacity(flat.len());
    let mut chars = flat.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '@' {
            out.push(ch);
            continue;
        }
        let mut target = String::new();
        while let Some(&next) = chars.peek() {
            if next.is_alphanumeric() || matches!(next, '.' | ':' | '-' | '_') {
                target.push(next);
                chars.next();
            } else {
                break;
            }
        }
        // Точка в конце предложения попала в имя — она не часть ссылки.
        let dots = target.len() - target.trim_end_matches('.').len();
        let target = target.trim_end_matches('.');
        if dots == 0 && chars.peek() == Some(&'[') {
            chars.next();
            for inner in chars.by_ref() {
                if inner == ']' {
                    break;
                }
                out.push(inner);
            }
        } else {
            out.push_str(target.rsplit(':').next().unwrap_or(target));
            out.push_str(&".".repeat(dots));
        }
    }
    out
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
    walk(
        info,
        &mut any,
        &mut auto,
        &mut none,
        &mut options,
        &mut types,
    );
    // Типы, которые в тексте заметки литералом не пишутся: их значение всегда
    // выражение (`it => …`, `read(…, encoding: none)`). Выбрать поле для
    // остальных они не мешают — выражение в таком параметре всё равно
    // сохранится как есть (FR-012a). Без этого `numbering: "1"` (строка или
    // функция) и путь к картинке (строка или байты) остались бы без поля.
    const NOT_LITERAL: [&str; 13] = [
        "function",
        "bytes",
        "selector",
        "dictionary",
        "array",
        "module",
        "type",
        "location",
        "regex",
        "arguments",
        "counter",
        "state",
        "label",
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
