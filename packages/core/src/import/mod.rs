//! Импорт папки Markdown (Obsidian) в vault (фича 021).
//!
//! Модуль делает пять вещей:
//!
//! 1. [`markdown`] переводит один файл Markdown в Typst: заголовки, списки,
//!    таблицы, сноски и прочее по таблице research.md §2, а всё, что
//!    перевести нельзя, оставляет видимым исходным текстом и записывает в
//!    потери;
//! 2. [`obsidian`] знает расширения Obsidian поверх CommonMark: YAML-шапку,
//!    выноски, размеры картинок, разбор цели вики-ссылки;
//! 3. [`math`] переводит формулы LaTeX → Typst;
//! 4. [`names`] строит план имён и разрешает ссылки по правилу Obsidian;
//! 5. [`batch`] ведёт пакет — шаг за шагом, с отменой между заметками,
//!    журналом продолжения и заметкой отчёта ([`report`]).
//!
//! Исходная папка берётся только через [`crate::vault::backend::ReadOnly`]:
//! запись в неё невозможна по типу (FR-002). Все правила — в ядре, оболочки
//! только ведут цикл шагов и показывают прогресс (принципы I, X).

pub mod batch;
pub mod markdown;
pub mod math;
pub mod names;
pub mod obsidian;
pub mod report;

pub use batch::{import_begin, import_finish, import_step, survey};
pub use markdown::{convert, Converted};
pub use names::ConvertContext;
pub use report::ReportTexts;

#[cfg(target_arch = "wasm32")]
pub mod web;

use serde::{Deserialize, Serialize};

/// Что делать, если в месте назначения уже есть файл с таким именем (FR-009).
/// Существующий файл не перезаписывается ни в одном из вариантов.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum OnConflict {
    /// Пропустить такой файл; он попадает в отчёт.
    #[default]
    Skip,
    /// Положить рядом под именем «имя (импорт).typ».
    Beside,
}

/// Задание на импорт (data-model.md «ImportJob»).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ImportJob {
    /// Папка vault, в которую ложится дерево; пусто — корень.
    pub destination: String,
    pub on_conflict: OnConflict,
    /// Копировать ли файлы, не являющиеся Markdown (картинки, на которые никто
    /// не ссылается, и всё прочее). Иначе они перечисляются как пропущенные.
    pub copy_other_files: bool,
    /// Время запуска, мс от эпохи Unix. Часов в ядре под wasm нет, поэтому
    /// время передаёт оболочка; ядро только запоминает его в журнале (экран
    /// продолжения). Ноль — неизвестно.
    #[serde(default)]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub started_ms: f64,
    /// Тексты заметки отчёта из переводов интерфейса. Без них — английские.
    #[serde(default)]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub texts: ReportTexts,
}

/// Продолжение прерванного импорта (экран C4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ResumeInfo {
    /// Сколько файлов перенесено прошлым запуском.
    pub done: u32,
    /// Сколько всего шагов в плане.
    pub total: u32,
    /// Время запуска, который оставил журнал, мс от эпохи Unix.
    pub at: f64,
}

/// Что нашлось в источнике (экран C1) и можно ли продолжить (C4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ImportSurvey {
    /// Файлов Markdown.
    pub notes: u32,
    /// Картинок форматов, которые Typst принимает.
    pub images: u32,
    /// Всего прочих файлов.
    pub other: u32,
    /// Служебных записей, которые не импортируются (`.obsidian`, `.trash`).
    pub excluded: u32,
    pub resume: Option<ResumeInfo>,
}

/// Вид потери при переводе (data-model.md «ReportEntry»).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum LossKind {
    /// HTML оставлен исходным текстом.
    Html,
    /// Встраивание (`![[Заметка]]`, файл не-картинка) заменено ссылкой или
    /// оставлено текстом.
    Embed,
    /// Блок плагина (dataview, tasks…) оставлен исходным текстом.
    PluginBlock,
    /// Диаграмма (mermaid…) оставлена исходным текстом.
    Diagram,
    /// Формулу перевести не удалось.
    Math,
    /// Ссылка на раздел заметки ведёт на заметку целиком.
    SectionLink,
    /// Ссылка на блок заметки ведёт на заметку целиком.
    BlockLink,
    /// Цель ссылки или картинки не найдена.
    MissingTarget,
    /// Имя ссылки подходит нескольким заметкам; выбрана ближайшая.
    AmbiguousTarget,
    /// YAML-шапка не вписалась в свойства (020) и оставлена комментарием.
    Properties,
    /// Заметка не прошла проверку синтаксиса Typst и оставлена исходным
    /// текстом целиком (страховка FR-008, не должна срабатывать).
    Syntax,
}

/// Потеря с местом в исходной заметке.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Loss {
    pub kind: LossKind,
    /// Строка исходного файла, с 1.
    pub line: u32,
}

/// Почему файл не перенесён.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SkipReason {
    TooLarge,
    NotUtf8,
    /// Не Markdown, и копирование прочих файлов не выбрано.
    OtherFile,
    /// В месте назначения уже есть такой файл.
    ExistsSkipped,
    /// Источник не прочитался.
    Unreadable,
    /// Назначение не записалось.
    WriteFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Outcome {
    Clean,
    Lossy,
    Skipped,
}

/// Строка отчёта об одном файле.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ReportEntry {
    /// Путь в источнике, через `/`.
    pub source: String,
    /// Путь в vault, если файл перенесён.
    pub destination: Option<String>,
    pub outcome: Outcome,
    pub losses: Vec<Loss>,
    pub skip_reason: Option<SkipReason>,
    /// Имя в vault отличается от исходного не только расширением.
    pub renamed: Option<String>,
}

/// Фаза пакета.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ImportPhase {
    /// Есть шаги.
    Running,
    /// Все шаги сделаны.
    Done,
}

/// Состояние пакета, которое оболочка носит между шагами (data-model.md
/// «ImportState»). План (тысячи файлов и карта ссылок) хранится в ядре и
/// между шагами не пересылается — иначе каждый шаг стоил бы сериализации всего
/// плана (research.md §7, Decision 2026-10-15); здесь только счётчики и
/// хвост отчёта для карточки хода.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ImportState {
    /// Ключ пакета в ядре: идентификатор журнала и номер запуска. Окно его не
    /// разбирает.
    pub id: String,
    pub destination: String,
    pub phase: ImportPhase,
    /// Следующий шаг; `next == total` — все сделаны.
    pub next: u32,
    pub total: u32,
    /// Перенесено чисто / с потерями / пропущено — в этом запуске.
    pub clean: u32,
    pub lossy: u32,
    pub skipped: u32,
    /// Сколько шагов пропущено, потому что перенесены прошлым запуском.
    pub resumed: u32,
    /// Источник последнего обработанного файла.
    pub current: Option<String>,
    /// Последние файлы с потерями или пропущенные (не больше 20).
    pub recent: Vec<ReportEntry>,
}

/// Отказ операции пакета.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ImportError {
    /// Источник не открыт: команда пришла раньше `import_open_source`.
    #[error("папка-источник не открыта")]
    NoSource,
    /// Ядро не знает такого пакета: страницу перезагрузили или состояние
    /// пришло от другого запуска. Начать заново — продолжение по журналу не
    /// даст дублей.
    #[error("пакет импорта не найден")]
    SessionLost,
    #[error("нет прав на запись")]
    AccessDenied,
    #[error("не удалось записать")]
    Io,
}

/// Журнал продолжения `.typsidian/imports/<id>.json` (research.md §8).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ImportJournal {
    pub version: u32,
    pub source_hint: String,
    pub destination: String,
    /// Время запуска, оставившего журнал, мс.
    #[serde(default)]
    pub started_ms: f64,
    /// Сколько шагов было в плане.
    #[serde(default)]
    pub total: u32,
    /// Перенесённые файлы: источник → назначение.
    pub done: Vec<JournalEntry>,
    /// Скопированные вложения: источник → назначение. Нужны, чтобы после
    /// продолжения общая картинка не копировалась второй раз.
    #[serde(default)]
    pub attachments: Vec<JournalEntry>,
    /// Файлы, которые пропущены окончательно (слишком большой, не UTF-8, не
    /// заметка, имя занято): продолжать за ними нечего.
    #[serde(default)]
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub source: String,
    pub destination: String,
}

/// Версия формата журнала.
pub const JOURNAL_VERSION: u32 = 1;
