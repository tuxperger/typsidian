//! Диагностики сборки (data-model.md «Диагностика»).
//!
//! Причина и место приходят от самого компилятора: `SourceDiagnostic` несёт
//! текст и спан, а `WorldExt::range` переводит спан в байтовый диапазон внутри
//! конкретного файла (research.md §7). Разделение «ошибка/предупреждение»
//! берётся из формы `Warned`, а не из разбора текста сообщения (FR-018).

use crate::messages::AppMessage;
use serde::{Deserialize, Serialize};
use typst::diag::{Severity, SourceDiagnostic};
use typst::syntax::FileId;
use typst::{World, WorldExt};

/// Насколько серьёзно замечание.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// Место замечания в файле vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DiagnosticLocation {
    /// Путь внутри vault.
    pub path: String,
    /// Это сама открытая заметка или связанный с ней файл.
    pub is_open_note: bool,
    /// Строка, 1-based — для показа пользователю.
    pub line: u32,
    /// Колонка, 1-based.
    pub column: u32,
    /// Байтовый диапазон — для точной установки курсора (FR-015).
    pub byte_start: u32,
    pub byte_end: u32,
}

/// Одно замечание к тексту.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    /// Причина словами. Общий текст «не удалось собрать» запрещён (FR-012).
    ///
    /// У замечаний компилятора — его собственный текст, он показывается как
    /// есть (010 FR-004). У замечаний приложения — [`AppMessage::log_text`],
    /// текст для журналов: показывается не он, а [`Diagnostic::app`].
    pub message: String,
    /// Место; `None` для замечания без привязки к файлу.
    pub location: Option<DiagnosticLocation>,
    /// Внешний шаблон, который можно попросить загрузить явно (фича 007,
    /// FR-010), — спецификацией строкой: `@preview/codly:1.3.0`.
    ///
    /// Стоит только у замечаний о **неудавшейся или не начатой** загрузке
    /// шаблона, у которого есть источник. Поле, а не разбор текста в
    /// интерфейсе: формулировка сообщения меняется, а спецификация — нет
    /// (принцип I).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub template: Option<String>,
    /// Сообщение приложения — вид и параметры (фича 010, FR-002).
    ///
    /// `Some` ⇔ замечание сформировало само приложение, и интерфейс собирает
    /// его текст на языке пользователя; `None` — это замечание Typst.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub app: Option<AppMessage>,
}

impl Diagnostic {
    /// Замечание приложения без места в тексте.
    pub fn app(severity: DiagnosticSeverity, message: AppMessage) -> Self {
        Self {
            severity,
            message: message.log_text(),
            location: None,
            template: None,
            app: Some(message),
        }
    }
}

/// Переводит диагностику компилятора в форму для UI.
pub fn translate(world: &dyn World, main: FileId, diagnostic: &SourceDiagnostic) -> Diagnostic {
    let severity = match diagnostic.severity {
        Severity::Error => DiagnosticSeverity::Error,
        Severity::Warning => DiagnosticSeverity::Warning,
    };

    Diagnostic {
        severity,
        message: diagnostic.message.to_string(),
        location: locate(world, main, diagnostic),
        template: None,
        app: None,
    }
}

fn locate(
    world: &dyn World,
    main: FileId,
    diagnostic: &SourceDiagnostic,
) -> Option<DiagnosticLocation> {
    let id = diagnostic.span.id()?;
    let range = world.range(diagnostic.span)?;
    let source = world.source(id).ok()?;
    let (line, column) = source.lines().byte_to_line_column(range.start)?;

    Some(DiagnosticLocation {
        path: id.vpath().get_without_slash().to_owned(),
        is_open_note: id == main,
        // Компилятор считает от нуля, пользователю показываем от единицы.
        line: line as u32 + 1,
        column: column as u32 + 1,
        byte_start: range.start as u32,
        byte_end: range.end as u32,
    })
}

/// Порядок показа — по месту в тексте (FR-016).
pub fn sort_by_location(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| match (&a.location, &b.location) {
        (Some(x), Some(y)) => x.path.cmp(&y.path).then(x.byte_start.cmp(&y.byte_start)),
        // Замечания без места идут последними: их некуда поставить в тексте.
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
}
