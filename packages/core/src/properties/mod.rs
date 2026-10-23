//! Свойства заметок и выборки (фича 020).
//!
//! Свойства записаны в самой заметке обычным Typst — первым элементом
//! `#metadata((tags: ("проект",), статус: "черновик")) <properties>` — и в
//! документ не выводятся (research.md §1). Модуль делает четыре вещи:
//!
//! 1. [`read`] разбирает запись и говорит, однозначна ли она;
//! 2. [`write`] строит точечную правку записи — тем же `EditOutcome`, что и
//!    параметры элементов 008;
//! 3. [`query`] отбирает и упорядочивает заметки по свойствам, а [`index`]
//!    поставляет ему сведения из индекса vault;
//! 4. [`select`] ведёт блоки `#select(…)`: находит их в тексте, считает входы
//!    сборки и хранит текст библиотеки `select.typ`.
//!
//! Все функции разбора и отбора чистые: ни файловой системы, ни сети, ни
//! состояния между вызовами (принципы I, II). Смещения в тексте наружу — в
//! кодовых единицах UTF-16, как у остального ядра. **Значения свойств не
//! попадают ни в сообщения об ошибках, ни в журнал** (принцип IX): ошибки
//! называют ключ или вид, но не значение.

pub mod index;
pub mod query;
pub mod read;
pub mod select;
pub mod write;

#[cfg(target_arch = "wasm32")]
pub mod web;

use serde::{Deserialize, Serialize};

pub use index::{
    property_keys, reindex_properties, reindex_step, select_notes, IndexedNote, PropertyKeyInfo,
    ReindexOutcome, ReindexStep,
};
pub use query::{
    run_selection, CompareOp, Condition, DateRange, SelectView, Selection, SelectionResult,
    SelectionRow, SortKey, Ymd,
};
pub use read::read;
pub(crate) use select::import_position;
pub use select::{
    compile_inputs, ensure_select_library, inputs_digest, select_blocks, select_import_edits,
    select_inputs, select_library, LibraryStatus, SelectBlock, SelectLibrary, SELECT_INPUT_KEY,
};
pub use write::edit;

use crate::structure::SourcePlace;

/// Формат сведений о свойствах в индексе. Растёт при любом изменении того, что
/// и как кладётся в кэш: кэш с меньшим номером пересчитывается целиком.
/// 2 — добавлены вставки файлов `#image` (фича 025).
pub const PROPERTIES_FORMAT_VERSION: u32 = 2;

/// Метка записи свойств в тексте заметки (research.md §1).
pub const RECORD_LABEL: &str = "properties";

/// Ключ свойства-списка тегов.
pub const TAGS_KEY: &str = "tags";

/// Значение свойства (data-model.md «PropertyValue»).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum PropertyValue {
    Text {
        value: String,
    },
    Number {
        value: f64,
    },
    /// Календарная дата без времени и пояса.
    Date {
        year: i32,
        month: u8,
        day: u8,
    },
    Flag {
        value: bool,
    },
    List {
        values: Vec<String>,
    },
    /// Не литерал: только показ, правка выключена (макет B2).
    Unknown {
        source: String,
    },
}

/// Вид значения — для подсказок ключей и сравнения «внутри одного вида».
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ValueKind {
    Text,
    Number,
    Date,
    Flag,
    List,
    Unknown,
}

impl PropertyValue {
    pub fn kind(&self) -> ValueKind {
        match self {
            PropertyValue::Text { .. } => ValueKind::Text,
            PropertyValue::Number { .. } => ValueKind::Number,
            PropertyValue::Date { .. } => ValueKind::Date,
            PropertyValue::Flag { .. } => ValueKind::Flag,
            PropertyValue::List { .. } => ValueKind::List,
            PropertyValue::Unknown { .. } => ValueKind::Unknown,
        }
    }

    pub fn text(value: impl Into<String>) -> Self {
        PropertyValue::Text {
            value: value.into(),
        }
    }

    pub fn list<S: Into<String>>(values: impl IntoIterator<Item = S>) -> Self {
        PropertyValue::List {
            values: values.into_iter().map(Into::into).collect(),
        }
    }

    pub fn date(year: i32, month: u8, day: u8) -> Self {
        PropertyValue::Date { year, month, day }
    }

    /// Годится ли значение для записи: дата существует, число конечно.
    pub fn is_writable(&self) -> bool {
        match self {
            PropertyValue::Number { value } => value.is_finite(),
            PropertyValue::Date { year, month, day } => valid_date(*year, *month, *day),
            PropertyValue::Unknown { .. } => false,
            _ => true,
        }
    }
}

/// Свойство в порядке записи (data-model.md «Property»).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Property {
    /// Имя как в записи; `tags` — теги.
    pub key: String,
    pub value: PropertyValue,
    /// Место значения в тексте (переход из панели).
    pub place: SourcePlace,
}

/// Свойство без места в тексте — то, что лежит в индексе.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PropertyEntry {
    pub key: String,
    pub value: PropertyValue,
}

/// Почему запись не разбирается однозначно (FR-004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum PropertiesProblem {
    /// Запись с ошибкой разбора: незакрытая скобка или строка.
    Malformed,
    /// Вторая запись `<properties>` в заметке.
    SecondRecord,
    /// Аргумент `metadata` — не словарь.
    NotADictionary,
    /// В словаре есть `..разворот`.
    Spread,
    /// Ключ повторён.
    DuplicateKey { key: String },
    /// Значение ключа — не литерал.
    NotLiteral { key: String },
}

impl PropertiesProblem {
    /// Чем раньше, тем важнее: сообщается первая из найденных по этому порядку.
    fn rank(&self) -> u8 {
        match self {
            PropertiesProblem::SecondRecord => 0,
            // Повтор ключа сам Typst считает ошибкой разбора, но называть его
            // надо точнее, чем «ошибка в записи».
            PropertiesProblem::DuplicateKey { .. } => 1,
            PropertiesProblem::Malformed => 2,
            PropertiesProblem::NotADictionary => 3,
            PropertiesProblem::Spread => 4,
            PropertiesProblem::NotLiteral { .. } => 5,
        }
    }
}

/// Свойства одной заметки (data-model.md «NoteProperties»).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct NoteProperties {
    pub properties: Vec<Property>,
    /// Запись однозначна: форма может её править. Записи нет вовсе — тоже
    /// правка возможна (она создастся).
    pub editable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub problem: Option<PropertiesProblem>,
    /// Где запись целиком; `None` — записи нет.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub record: Option<SourcePlace>,
}

impl NoteProperties {
    /// Свойства для индекса: читаемые литералы, без нераспознанных значений.
    pub fn entries(&self) -> Vec<PropertyEntry> {
        self.properties
            .iter()
            .filter(|property| !matches!(property.value, PropertyValue::Unknown { .. }))
            .map(|property| PropertyEntry {
                key: property.key.clone(),
                value: property.value.clone(),
            })
            .collect()
    }
}

/// Правка свойств формой (data-model.md «PropertyEdit»).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum PropertyEdit {
    Set { key: String, value: PropertyValue },
    Remove { key: String },
    Rename { from: String, to: String },
}

/// Свойства из текста заметки — ровно то, что кладётся в индекс.
pub fn entries_of(text: &str) -> Vec<PropertyEntry> {
    // Дешёвый предфильтр: заметок без записи подавляющее большинство, а полный
    // разбор Typst на 10 000 заметок при открытии стоит заметного времени.
    if !text.contains(RECORD_LABEL) {
        return Vec::new();
    }
    read(text).entries()
}

// ── Время и календарь ──────────────────────────────────────────────────────

/// Сейчас, мс от эпохи Unix. Одинаково на обоих таргетах: на wasm32 часов
/// `std` нет, берутся часы браузера.
pub fn now_ms() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as i64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_millis() as i64)
            .unwrap_or(0)
    }
}

/// Существует ли такая календарная дата (григорианский календарь).
pub fn valid_date(year: i32, month: u8, day: u8) -> bool {
    if !(1..=12).contains(&month) || day == 0 {
        return false;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let last = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    day <= last
}

/// Дни от 1970-01-01 → дата (алгоритм Хиннанта, григорианский календарь).
pub(crate) fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    (year as i32, month as u8, day as u8)
}

/// Дата → дни от 1970-01-01.
pub(crate) fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let year = i64::from(year) - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let month = i64::from(month);
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Миллисекунды от эпохи → дата по UTC.
pub(crate) fn date_of_ms(ms: i64) -> (i32, u8, u8) {
    civil_from_days(ms.div_euclid(86_400_000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_round_trip() {
        for days in [-1000_i64, 0, 365, 19_000, 20_000, 30_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(date_of_ms(86_400_000 * 20_000), civil_from_days(20_000));
    }

    #[test]
    fn dates_are_validated() {
        assert!(valid_date(2024, 2, 29));
        assert!(!valid_date(2023, 2, 29));
        assert!(!valid_date(2026, 13, 1));
        assert!(!valid_date(2026, 4, 31));
    }
}
