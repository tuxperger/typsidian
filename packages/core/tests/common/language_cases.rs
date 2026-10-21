//! Язык интерфейса в настройках устройства — проверки, общие для обоих таргетов
//! (фича 010, data-model.md «LanguageChoice», contracts/core-messages.md).
//!
//! Правила чистые: разбор файла, раскладка набора по хранилищам, проверка тега.
//! Одна таблица на native и wasm32 — иначе при правке контракта один таргет
//! остался бы проверенным по устаревшему правилу (принцип II).

#![allow(dead_code)]

use typsidian_core::settings::apply::{
    plan, reset_to_defaults, route, target_for, ApplyPlan, ChangeTarget, LanguageChoice,
    RejectReason, RoutedChange, SettingsChange,
};
use typsidian_core::settings::format::{self, FolderRefs};
use typsidian_core::settings::{is_valid_language_tag, Settings, Theme};

fn tag(value: &str) -> LanguageChoice {
    LanguageChoice::Tag(value.to_string())
}

/// Явный выбор пишется ключом `language`, «как в системе» ключ убирает.
pub fn language_round_trips_through_the_file() {
    let chosen = SettingsChange::language(tag("en")).apply_to(&Settings::default());
    let text = format::render("", &chosen, &FolderRefs::new());
    assert!(text.contains(r#""language": "en""#), "{text}");
    assert_eq!(
        format::parse(&text).unwrap().language.as_deref(),
        Some("en")
    );

    let system = SettingsChange::language(LanguageChoice::System).apply_to(&chosen);
    let cleared = format::render(&text, &system, &FolderRefs::new());
    assert!(!cleared.contains("language"), "{cleared}");
}

/// FR-018: язык, который эта версия не поставляет, хранится и переживает
/// запись другой настройки.
pub fn unrecognised_tag_survives_writing_another_value() {
    let written = r#"{"theme": "light", "language": "xx"}"#;
    let settings = format::parse(written).unwrap();
    let after_theme = SettingsChange::theme(Theme::Dark).apply_to(&settings);
    let rewritten = format::render(written, &after_theme, &FolderRefs::new());
    let reread = format::parse(&rewritten).unwrap();
    assert_eq!(reread.language.as_deref(), Some("xx"));
    assert_eq!(reread.theme, Theme::Dark);
}

/// FR-018: кривой тег, вписанный руками, не обнуляет остальные значения.
pub fn malformed_tag_in_file_keeps_other_values() {
    for written in [
        r#"{"theme": "dark", "editor_font_size": 20, "language": "not a tag!"}"#,
        r#"{"theme": "dark", "editor_font_size": 20, "language": 42}"#,
    ] {
        let settings = format::parse(written).expect("файл разбирается");
        assert_eq!(settings.theme, Theme::Dark, "{written}");
        assert_eq!(settings.editor_font_size, 20, "{written}");
    }
}

/// Синтаксически неверный тег отклоняет набор целиком, и тема из того же
/// набора тоже не применяется.
pub fn invalid_tag_rejects_the_whole_change() {
    for bad in ["", "e", "english language", "ru_RU", "123", &"a".repeat(36)] {
        assert!(!is_valid_language_tag(bad), "{bad:?}");
        let change = SettingsChange {
            theme: Some(Theme::Dark),
            ..SettingsChange::language(tag(bad))
        };
        assert_eq!(
            plan(&Settings::default(), &change),
            ApplyPlan::Rejected {
                reason: RejectReason::ValueOutOfRange
            },
            "{bad:?}"
        );
    }
    for good in ["en", "ru", "en-GB", "zh-Hant-TW", "xx"] {
        assert!(is_valid_language_tag(good), "{good}");
        assert_eq!(
            plan(&Settings::default(), &SettingsChange::language(tag(good))),
            ApplyPlan::Applied,
            "{good}"
        );
    }
}

/// Кривой тег, уже лежащий в файле, не мешает сменить тему (FR-018).
pub fn stored_malformed_tag_does_not_block_other_changes() {
    let base = Settings {
        language: Some("not a tag!".to_string()),
        ..Settings::default()
    };
    assert_eq!(
        plan(&base, &SettingsChange::theme(Theme::Dark)),
        ApplyPlan::Applied
    );
}

/// FR-016: язык — свойство устройства, даже при открытом vault.
pub fn language_goes_to_the_device_even_with_an_open_vault() {
    let change = SettingsChange::language(tag("en"));
    assert_eq!(target_for(&change, true), ChangeTarget::Device);
    assert_eq!(
        route(&change, true),
        vec![RoutedChange {
            target: ChangeTarget::Device,
            change: change.clone(),
        }]
    );
}

/// «Язык + тема» при открытом vault — две записи, каждая в одно хранилище.
pub fn language_and_theme_split_between_stores() {
    let change = SettingsChange {
        theme: Some(Theme::Dark),
        ..SettingsChange::language(tag("ru"))
    };
    assert_eq!(
        route(&change, true),
        vec![
            RoutedChange {
                target: ChangeTarget::Device,
                change: SettingsChange::language(tag("ru")),
            },
            RoutedChange {
                target: ChangeTarget::OpenVault,
                change: SettingsChange::theme(Theme::Dark),
            },
        ]
    );
    // Без открытого vault — обе записи в устройство, но по-прежнему по одной.
    let closed = route(&change, false);
    assert_eq!(closed.len(), 2);
    assert!(closed.iter().all(|r| r.target == ChangeTarget::Device));
    // Пустой набор ничего не пишет.
    assert!(route(&SettingsChange::default(), true).is_empty());
}

/// FR-017: сброс возвращает «как в системе».
pub fn reset_returns_to_the_system_language() {
    let reset = reset_to_defaults();
    assert_eq!(reset.language, Some(LanguageChoice::System));
    let chosen = Settings {
        language: Some("en".to_string()),
        ..Settings::default()
    };
    assert_eq!(reset.apply_to(&chosen).language, None);
}

/// Форма JSON выбора — контракт с интерфейсом.
pub fn language_choice_json_shape() {
    assert_eq!(
        serde_json::to_string(&LanguageChoice::System).unwrap(),
        r#""system""#
    );
    assert_eq!(
        serde_json::to_string(&tag("en")).unwrap(),
        r#"{"tag":"en"}"#
    );
}
