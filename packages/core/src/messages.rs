//! Сообщения, которые формирует само приложение (фича 010, FR-002, FR-014).
//!
//! Ядро не знает языка интерфейса и знать не должно: оно отдаёт **вид**
//! сообщения и подставляемые значения, а слова на языке пользователя собирает
//! интерфейс из своих каталогов. Отсюда две гарантии:
//!
//! - сообщение, уже показанное на экране, перерисовывается на новом языке без
//!   повторения действия, которое его вызвало (FR-014);
//! - имена пользователя — пути, спецификации шаблонов, адреса — приходят
//!   параметрами и не меняются ни на каком языке (FR-005).
//!
//! Сообщения компилятора Typst сюда не входят: их текст формулирует сам Typst,
//! и они показываются как есть (FR-004).
//!
//! Для журналов и тестов у каждого сообщения есть [`AppMessage::log_text`] —
//! текст, не зависящий от языка интерфейса (FR-031).

use crate::capture::CaptureFailure;
use crate::preview::packages::InstallError;
use crate::preview::resolve::ResolveError;
use serde::{Deserialize, Serialize};

/// Сообщение приложения: вид и параметры.
///
/// Тег `code` — стабильный идентификатор вида, по нему интерфейс выбирает
/// текст. Новый вариант без перевода в интерфейсе — ошибка `tsc`: разбор там
/// исчерпывающий.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "code", rename_all_fields = "camelCase")]
pub enum AppMessage {
    /// Связанный с заметкой файл не прочитался (005 FR-021).
    DependencyUnreadable { path: String, reason: ResolveReason },
    /// Внешний шаблон не установлен в папке заметок (006 FR-037).
    TemplateMissing { spec: String },
    /// Шаблон загружен — что и откуда (007 FR-006).
    TemplateDownloaded {
        spec: String,
        url: String,
        file_count: u32,
        fingerprint: String,
    },
    /// Шаблон не установился; причина — та же, что у явной загрузки (007 FR-007).
    TemplateInstallFailed { spec: String, reason: InstallError },
    /// Содержимое установленного шаблона разошлось с записанным отпечатком
    /// (007 FR-023).
    TemplateFingerprintDiverged {
        spec: String,
        recorded: String,
        actual: FingerprintActual,
    },
    /// Файл шрифта из папки vault не вошёл в набор (005, Edge Cases).
    VaultFontUnusable { path: String, reason: FontProblem },
    /// Путь заметки не выражается относительно корня vault (005 FR-023).
    NoteOutsideVault { path: String },
    /// Сборка оборвана по пределу времени (005 FR-026).
    BuildTimedOut { seconds: u32 },
    /// Папка vault недоступна во время сборки (005 FR-025).
    VaultUnavailable,
    /// Показывать нечего: у сборки нет ни одной страницы (019, A5).
    SlidesEmptyDeck,
    /// Захват не записан; он остаётся в очереди (022 FR-005).
    CaptureRefused { reason: CaptureFailure },
}

/// Почему связанный файл не прочитался. Зеркало [`ResolveError`] без шаблонов:
/// отсутствующий шаблон — отдельное сообщение со своим именем.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "camelCase")]
pub enum ResolveReason {
    NotFound,
    Unreadable,
    AccessDenied,
    OutsideVault,
    TooLarge,
}

/// Что сейчас лежит на месте шаблона, разошедшегося с отпечатком.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FingerprintActual {
    /// Содержимое на месте, но отпечаток другой.
    Digest { digest: String },
    /// Одного из записанных файлов больше нет.
    FileMissing { name: String },
}

/// Почему шрифт из папки vault не используется.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FontProblem {
    /// Файл не прочитался.
    Unreadable { reason: ResolveReason },
    /// Прочитался, но шрифтом не разобрался.
    Invalid,
}

impl ResolveReason {
    /// Причина чтения без шаблона. `None` — у ошибки есть своё сообщение
    /// ([`AppMessage::TemplateMissing`]).
    pub fn of(error: &ResolveError) -> Option<Self> {
        Some(match error {
            ResolveError::NotFound => ResolveReason::NotFound,
            ResolveError::Unreadable => ResolveReason::Unreadable,
            ResolveError::AccessDenied => ResolveReason::AccessDenied,
            ResolveError::OutsideVault => ResolveReason::OutsideVault,
            ResolveError::TooLarge => ResolveReason::TooLarge,
            ResolveError::PackageMissing { .. } => return None,
        })
    }

    fn log_text(self) -> &'static str {
        match self {
            ResolveReason::NotFound => "file not found",
            ResolveReason::Unreadable => "file is unreadable",
            ResolveReason::AccessDenied => "no read permission",
            ResolveReason::OutsideVault => "file is outside the vault",
            ResolveReason::TooLarge => "file exceeds the supported size",
        }
    }
}

impl CaptureFailure {
    fn log_text(self) -> &'static str {
        match self {
            CaptureFailure::TooLarge => "the note would exceed the supported size",
            CaptureFailure::ReadOnly => "the vault is read-only",
            CaptureFailure::NoVault => "the vault is unavailable",
            CaptureFailure::Conflict => "the note kept changing while saving",
            CaptureFailure::UnsavedElsewhere => "the note has unsaved changes elsewhere",
            CaptureFailure::TargetMissing => "the target note is gone",
            CaptureFailure::InvalidTarget => "the target is not a note of the vault",
            CaptureFailure::BadImage => "the image is missing or not supported",
            CaptureFailure::Empty => "there is nothing to capture",
            CaptureFailure::Io => "the disk refused the write",
        }
    }
}

impl AppMessage {
    /// Сообщение об отказе чтения связанного файла.
    pub fn from_resolve(path: &str, error: &ResolveError) -> Self {
        match (error, ResolveReason::of(error)) {
            (ResolveError::PackageMissing { spec }, _) => {
                AppMessage::TemplateMissing { spec: spec.clone() }
            }
            (_, Some(reason)) => AppMessage::DependencyUnreadable {
                path: path.to_owned(),
                reason,
            },
            // `of` возвращает `None` только для `PackageMissing`, разобранного
            // выше.
            (_, None) => AppMessage::TemplateMissing {
                spec: path.to_owned(),
            },
        }
    }

    /// Текст для журналов и тестов (FR-031). В интерфейс не идёт: там текст
    /// собирается из каталога на языке пользователя.
    ///
    /// Английский — эталонный язык проекта; от выбранного пользователем языка
    /// он не зависит.
    pub fn log_text(&self) -> String {
        match self {
            AppMessage::DependencyUnreadable { path, reason } => {
                format!("{path}: {}", reason.log_text())
            }
            AppMessage::TemplateMissing { spec } => {
                format!("template {spec} is not installed in the vault")
            }
            AppMessage::TemplateDownloaded {
                spec,
                url,
                file_count,
                fingerprint,
            } => format!(
                "template {spec} downloaded from {url} ({file_count} files, fingerprint {})",
                short(fingerprint)
            ),
            AppMessage::TemplateInstallFailed { spec, reason } => {
                format!("template {spec} not installed: {reason}")
            }
            AppMessage::TemplateFingerprintDiverged {
                spec,
                recorded,
                actual,
            } => {
                let actual = match actual {
                    FingerprintActual::Digest { digest } => short(digest).to_owned(),
                    FingerprintActual::FileMissing { name } => format!("file {name} is gone"),
                };
                format!(
                    "template {spec} diverged from its recorded fingerprint ({actual} instead of {}); \
                     not reinstalled",
                    short(recorded)
                )
            }
            AppMessage::VaultFontUnusable { path, reason } => match reason {
                FontProblem::Unreadable { reason } => format!("font {path}: {}", reason.log_text()),
                FontProblem::Invalid => format!("font {path}: not a font file"),
            },
            AppMessage::NoteOutsideVault { path } => {
                format!("{path}: the note path is outside the vault")
            }
            AppMessage::BuildTimedOut { seconds } => {
                format!("the build was stopped after {seconds} seconds")
            }
            AppMessage::VaultUnavailable => {
                "the vault folder is unavailable; linked files were not read".to_owned()
            }
            AppMessage::SlidesEmptyDeck => "the document has no pages to show".to_owned(),
            AppMessage::CaptureRefused { reason } => {
                format!("the capture was not saved: {}", reason.log_text())
            }
        }
    }
}

/// Первые 12 знаков отпечатка — столько показывается пользователю.
pub fn short(digest: &str) -> &str {
    &digest[..12.min(digest.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_template_is_its_own_message() {
        let error = ResolveError::PackageMissing {
            spec: "@preview/codly:1.3.0".to_string(),
        };
        assert_eq!(
            AppMessage::from_resolve("lib.typ", &error),
            AppMessage::TemplateMissing {
                spec: "@preview/codly:1.3.0".to_string()
            }
        );
    }

    #[test]
    fn log_text_does_not_depend_on_anything_but_the_message() {
        let message = AppMessage::DependencyUnreadable {
            path: "data/таблица.csv".to_string(),
            reason: ResolveReason::NotFound,
        };
        assert_eq!(message.log_text(), "data/таблица.csv: file not found");
    }
}
