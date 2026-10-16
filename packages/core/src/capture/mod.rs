//! Быстрый захват (фича 022): запись из «Поделиться» в конец заметки и
//! ежедневная заметка.
//!
//! Весь модуль — правила (принцип I): вид записи, дописывание, путь заметки по
//! дате, соседние дни, создание из заготовки, выбор «буфер или файл». Ни одно
//! из них не воспроизводится в интерфейсе или в нативном коде Android — тот
//! лишь принимает намерение системы и кладёт его в очередь.
//!
//! - [`entry`] — как захват выглядит в тексте и как дописывается;
//! - [`daily`] — путь ежедневной заметки, соседние дни, создание;
//! - [`write`] — запись захвата без потерь (правка буфера или файл).
//!
//! Часов в ядре нет ни на одном таргете (тот же довод, что у
//! `vault_settings::set_aside_invalid`): дату и время приносит окно по часам
//! устройства (спека, Edge Cases).

pub mod daily;
pub mod entry;
pub mod write;

#[cfg(target_arch = "wasm32")]
pub mod web;

use serde::{Deserialize, Serialize};

use crate::export::folder::{validate_folder, ExportFolderProblem};

pub use daily::{
    daily_date_of, daily_neighbours, daily_path, open_daily, DailyError, DailyNeighbours,
    DailyTemplateUse, OpenDaily,
};
pub use entry::{append, entry, Appended};
pub use write::{capture_target_path, capture_write};

/// Заметка входящих по умолчанию (research.md §1).
pub const DEFAULT_INBOX: &str = "входящие.typ";
/// Папка ежедневных заметок по умолчанию.
pub const DEFAULT_DAILY_FOLDER: &str = "дневник";

/// Вид названия ежедневной заметки (FR-010, FR-011).
///
/// Значение в файле — сам образец, а не имя варианта: файл читают глазами.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum DailyName {
    /// `дневник/2026-10-16.typ` — сортируется по алфавиту (умолчание).
    #[default]
    #[serde(rename = "YYYY-MM-DD")]
    Iso,
    /// `дневник/2026/10/2026-10-16.typ`.
    #[serde(rename = "YYYY/MM/YYYY-MM-DD")]
    IsoNested,
    /// `дневник/16.10.2026.typ`.
    #[serde(rename = "DD.MM.YYYY")]
    Dotted,
}

impl DailyName {
    /// Значение в файле настроек.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Iso => "YYYY-MM-DD",
            Self::IsoNested => "YYYY/MM/YYYY-MM-DD",
            Self::Dotted => "DD.MM.YYYY",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        [Self::Iso, Self::IsoNested, Self::Dotted]
            .into_iter()
            .find(|name| name.as_str() == text)
    }
}

/// Вид записи захвата.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "lowercase")]
pub enum EntryStyle {
    /// Пункт списка: `- 10:15 текст` (умолчание).
    #[default]
    List,
    /// Абзац: `10:15 текст`.
    Paragraph,
}

impl EntryStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Paragraph => "paragraph",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        [Self::List, Self::Paragraph]
            .into_iter()
            .find(|style| style.as_str() == text)
    }
}

/// Настройки захвата, записанные в `vault.json` (ключ `capture`).
///
/// Слой, а не копия, как и остальные [`crate::vault::vault_settings::VaultOverrides`]:
/// `None` — «не задано, действует умолчание» (см. [`CaptureSettings`]).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CaptureOverrides {
    /// Заметка входящих — путь от корня vault.
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub inbox: Option<String>,
    /// Папка ежедневных заметок.
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub daily_folder: Option<String>,
    /// Заготовка ежедневной заметки (003) — путь от корня vault.
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub daily_template: Option<String>,
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub daily_name: Option<DailyName>,
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub entry_style: Option<EntryStyle>,
    /// Писать ли время захвата (умолчание — да).
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub entry_time: Option<bool>,
}

impl CaptureOverrides {
    /// Ничего не задано — в файле таблицы `capture` быть не должно.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Действующие настройки захвата: умолчания применены (data-model.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CaptureSettings {
    pub inbox: String,
    pub daily_folder: String,
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub daily_template: Option<String>,
    pub daily_name: DailyName,
    pub entry_style: EntryStyle,
    pub entry_time: bool,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            inbox: DEFAULT_INBOX.to_owned(),
            daily_folder: DEFAULT_DAILY_FOLDER.to_owned(),
            daily_template: None,
            daily_name: DailyName::default(),
            entry_style: EntryStyle::default(),
            entry_time: true,
        }
    }
}

/// Какое поле настроек захвата не годится.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "camelCase")]
pub enum CaptureField {
    Inbox,
    DailyFolder,
    DailyTemplate,
}

/// Нормальная форма пути из настроек: части через `/`, без `..`, внутри vault.
///
/// Пустая строка — «не задано» (`Ok(None)`): очистка поля в панели возвращает
/// умолчание, а не делает путь пустым. Правило общее с папкой экспорта
/// ([`validate_folder`]) — второго разбора пути не заводим.
pub fn normalize_path(raw: &str) -> Result<Option<String>, ExportFolderProblem> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    validate_folder(raw).map(Some)
}

/// То же для файла заметки: расширение `.typ` дописывается, если его нет.
pub fn normalize_note_path(raw: &str) -> Result<Option<String>, ExportFolderProblem> {
    Ok(normalize_path(raw)?.map(|path| {
        if path.ends_with(".typ") {
            path
        } else {
            format!("{path}.typ")
        }
    }))
}

/// Проверяет и приводит настройки к виду, в котором они хранятся.
///
/// Пустое поле становится `None`. Первое негодное поле — отказ; файл при этом
/// не тронут (вызывается до записи).
pub fn normalize_overrides(
    overrides: &CaptureOverrides,
) -> Result<CaptureOverrides, (CaptureField, ExportFolderProblem)> {
    let mut result = overrides.clone();
    result.inbox = match overrides.inbox.as_deref() {
        Some(raw) => normalize_note_path(raw).map_err(|problem| (CaptureField::Inbox, problem))?,
        None => None,
    };
    result.daily_folder = match overrides.daily_folder.as_deref() {
        Some(raw) => normalize_path(raw).map_err(|problem| (CaptureField::DailyFolder, problem))?,
        None => None,
    };
    result.daily_template = match overrides.daily_template.as_deref() {
        Some(raw) => {
            normalize_path(raw).map_err(|problem| (CaptureField::DailyTemplate, problem))?
        }
        None => None,
    };
    Ok(result)
}

/// Настройки с умолчаниями (`captureSettings`).
///
/// Негодное значение здесь уже невозможно (разбор файла отбрасывает такое, а
/// запись не пропускает), но проверка повторяется: путь не имеет права выйти из
/// vault из-за того, что кто-то собрал настройки в обход разбора.
pub fn capture_settings(overrides: Option<&CaptureOverrides>) -> CaptureSettings {
    let defaults = CaptureSettings::default();
    let Some(set) = overrides else {
        return defaults;
    };
    CaptureSettings {
        inbox: set
            .inbox
            .as_deref()
            .and_then(|raw| normalize_note_path(raw).ok().flatten())
            .unwrap_or(defaults.inbox),
        daily_folder: set
            .daily_folder
            .as_deref()
            .and_then(|raw| normalize_path(raw).ok().flatten())
            .unwrap_or(defaults.daily_folder),
        daily_template: set
            .daily_template
            .as_deref()
            .and_then(|raw| normalize_path(raw).ok().flatten()),
        daily_name: set.daily_name.unwrap_or(defaults.daily_name),
        entry_style: set.entry_style.unwrap_or(defaults.entry_style),
        entry_time: set.entry_time.unwrap_or(defaults.entry_time),
    }
}

/// Календарная дата по часам устройства (Edge Cases: «сегодня» — на момент
/// нажатия).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DailyDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl DailyDate {
    /// Существует ли такой день (високосные годы учтены).
    pub fn is_valid(&self) -> bool {
        if self.year == 0 || self.year > 9999 || !(1..=12).contains(&self.month) {
            return false;
        }
        let leap = (self.year.is_multiple_of(4) && !self.year.is_multiple_of(100))
            || self.year.is_multiple_of(400);
        let days = match self.month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        (1..=days).contains(&self.day)
    }
}

/// Момент захвата: дата и время по часам устройства.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CaptureMoment {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
}

impl CaptureMoment {
    pub fn date(&self) -> DailyDate {
        DailyDate {
            year: self.year,
            month: self.month,
            day: self.day,
        }
    }
}

/// Что захвачено.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum CaptureContent {
    Text {
        text: String,
    },
    Link {
        url: String,
        #[serde(default)]
        #[cfg_attr(target_arch = "wasm32", tsify(optional))]
        title: Option<String>,
    },
    /// Изображение: байты приходят отдельным аргументом
    /// [`capture_write`]; `file` — имя файла в очереди оболочки, ядру
    /// непрозрачное.
    Image {
        file: String,
        mime: String,
        name: String,
    },
}

/// Куда дописать.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum CaptureTarget {
    /// Заметка входящих из настроек; создаётся, если её нет.
    Inbox,
    /// Ежедневная заметка на дату захвата; создаётся по заготовке.
    Daily,
    /// Выбранная заметка (из недавних); должна существовать.
    Note { path: String },
}

/// Захват: содержимое, приписка, время, назначение (data-model.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Capture {
    /// Идентификатор записи очереди оболочки.
    pub id: String,
    pub content: CaptureContent,
    /// Приписка пользователя.
    #[serde(default)]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub note: Option<String>,
    pub at: CaptureMoment,
    pub target: CaptureTarget,
}

/// Почему захват не записан. Захват при этом не теряется: оболочка оставляет
/// его в очереди (FR-005, SC-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "camelCase")]
pub enum CaptureFailure {
    /// Заметка стала бы больше предела размера (001 FR-013).
    TooLarge,
    /// Vault только для чтения.
    ReadOnly,
    /// Vault недоступен или не выбран.
    NoVault,
    /// Заметку три раза подряд меняли между чтением и записью.
    Conflict,
    /// У заметки несохранённые правки вне редактора: дописывать некуда.
    UnsavedElsewhere,
    /// Выбранной заметки больше нет.
    TargetMissing,
    /// Назначение — не заметка vault.
    InvalidTarget,
    /// Картинка не пришла или её вид не поддерживается.
    BadImage,
    /// Захватывать нечего: ни содержимого, ни приписки.
    Empty,
    /// Диск ответил отказом.
    Io,
}

/// Чем кончился захват. Отказ — вариант значения, а не ошибка вызова.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum CaptureOutcome {
    /// Заметка открыта в редакторе: применить правку к буферу, сохранение —
    /// обычным путём. На диск ничего не записано, кроме, возможно, картинки.
    EditBuffer {
        path: String,
        edit: crate::elements::edit::TextEdit,
    },
    /// Дописано в файл.
    Written {
        path: String,
    },
    Failed {
        reason: CaptureFailure,
    },
}
