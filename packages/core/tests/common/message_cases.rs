//! Форма сообщений приложения на границе с интерфейсом — общая таблица для
//! обоих таргетов (фича 010, contracts/core-messages.md).
//!
//! JSON каждого варианта сверяется с эталоном: интерфейс разбирает его
//! исчерпывающим `switch` по `code`, и переименование поля на одном таргете
//! молча сломало бы перевод на нём (принцип II).

#![allow(dead_code)]

use serde_json::{json, Value};
use typsidian_core::messages::{AppMessage, FingerprintActual, FontProblem, ResolveReason};
use typsidian_core::preview::packages::{ArchiveProblem, InstallError};
use typsidian_core::preview::{Diagnostic, DiagnosticSeverity};

fn as_json(message: &AppMessage) -> Value {
    serde_json::to_value(message).expect("сообщение сериализуется")
}

/// Каждый вариант — с эталоном; имена пользователя остаются как есть (FR-005).
pub fn every_variant_has_a_stable_json_shape() {
    let table: Vec<(AppMessage, Value)> = vec![
        (
            AppMessage::DependencyUnreadable {
                path: "data/таблица.csv".to_string(),
                reason: ResolveReason::TooLarge,
            },
            json!({"code": "DependencyUnreadable", "path": "data/таблица.csv", "reason": "tooLarge"}),
        ),
        (
            AppMessage::TemplateMissing {
                spec: "@preview/codly:1.3.0".to_string(),
            },
            json!({"code": "TemplateMissing", "spec": "@preview/codly:1.3.0"}),
        ),
        (
            AppMessage::TemplateDownloaded {
                spec: "@preview/codly:1.3.0".to_string(),
                url: "https://packages.typst.org/preview/codly-1.3.0.tar.gz".to_string(),
                file_count: 12,
                fingerprint: "0123456789abcdef".to_string(),
            },
            json!({
                "code": "TemplateDownloaded",
                "spec": "@preview/codly:1.3.0",
                "url": "https://packages.typst.org/preview/codly-1.3.0.tar.gz",
                "fileCount": 12,
                "fingerprint": "0123456789abcdef"
            }),
        ),
        (
            AppMessage::TemplateInstallFailed {
                spec: "@preview/codly:1.3.0".to_string(),
                reason: InstallError::ArchiveRejected {
                    reason: ArchiveProblem::EscapesFolder {
                        entry: "../снаружи.typ".to_string(),
                    },
                },
            },
            json!({
                "code": "TemplateInstallFailed",
                "spec": "@preview/codly:1.3.0",
                "reason": {
                    "kind": "ArchiveRejected",
                    "reason": {"problem": "escapesFolder", "entry": "../снаружи.typ"}
                }
            }),
        ),
        (
            AppMessage::TemplateFingerprintDiverged {
                spec: "@preview/codly:1.3.0".to_string(),
                recorded: "aaaa".to_string(),
                actual: FingerprintActual::FileMissing {
                    name: "lib.typ".to_string(),
                },
            },
            json!({
                "code": "TemplateFingerprintDiverged",
                "spec": "@preview/codly:1.3.0",
                "recorded": "aaaa",
                "actual": {"kind": "fileMissing", "name": "lib.typ"}
            }),
        ),
        (
            AppMessage::VaultFontUnusable {
                path: "шрифты/битый.ttf".to_string(),
                reason: FontProblem::Unreadable {
                    reason: ResolveReason::AccessDenied,
                },
            },
            json!({
                "code": "VaultFontUnusable",
                "path": "шрифты/битый.ttf",
                "reason": {"kind": "unreadable", "reason": "accessDenied"}
            }),
        ),
        (
            AppMessage::NoteOutsideVault {
                path: "../вне.typ".to_string(),
            },
            json!({"code": "NoteOutsideVault", "path": "../вне.typ"}),
        ),
        (
            AppMessage::BuildTimedOut { seconds: 10 },
            json!({"code": "BuildTimedOut", "seconds": 10}),
        ),
        (
            AppMessage::VaultUnavailable,
            json!({"code": "VaultUnavailable"}),
        ),
    ];

    for (message, expected) in table {
        assert_eq!(as_json(&message), expected, "{message:?}");
        let back: AppMessage = serde_json::from_value(expected).expect("разбор обратно");
        assert_eq!(back, message);
    }
}

/// У замечания приложения `message` — текст для журналов, `app` — то, что
/// покажет интерфейс; у замечания Typst поля `app` в JSON нет вовсе.
pub fn a_diagnostic_carries_the_message_kind_only_when_the_app_made_it() {
    let message = AppMessage::TemplateMissing {
        spec: "@preview/codly:1.3.0".to_string(),
    };
    let app = Diagnostic::app(DiagnosticSeverity::Error, message.clone());
    assert_eq!(app.message, message.log_text());
    assert_eq!(app.app, Some(message));

    let typst = Diagnostic {
        severity: DiagnosticSeverity::Error,
        message: "unknown variable: x".to_string(),
        location: None,
        template: None,
        app: None,
    };
    let json = serde_json::to_value(&typst).expect("сериализация");
    assert!(json.get("app").is_none(), "{json}");
}

/// Журнал от языка интерфейса не зависит (FR-031): текст один и тот же при
/// любом выборе, потому что выбора ядро просто не знает.
pub fn log_text_names_the_user_values_verbatim() {
    let message = AppMessage::DependencyUnreadable {
        path: "картинки/схема.svg".to_string(),
        reason: ResolveReason::NotFound,
    };
    assert!(message.log_text().contains("картинки/схема.svg"));
}
