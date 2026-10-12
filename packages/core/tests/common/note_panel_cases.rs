//! Состояние колонки заметки в настройках устройства — проверки, общие для
//! обоих таргетов (фича 013, data-model.md «NotePanelState», FR-021).
//!
//! Правила чистые: разбор файла, раскладка по хранилищам, сброс. Одна таблица
//! на native и wasm32 (принцип II).

#![allow(dead_code)]

use typsidian_core::settings::apply::{
    device_defaults_change, reset_to_defaults, route, target_for, ChangeTarget, RoutedChange,
    SettingsChange,
};
use typsidian_core::settings::format::{self, FolderRefs};
use typsidian_core::settings::{resolve, NotePanelMode, NotePanelState, Settings, Theme};
use typsidian_core::structure::OutlineKinds;
use typsidian_core::vault::vault_settings::VaultOverrides;

fn open_panel() -> NotePanelState {
    NotePanelState {
        shown: true,
        mode: NotePanelMode::Links,
        mentions_expanded: true,
        outline_kinds: OutlineKinds::default(),
    }
}

fn outline_panel() -> NotePanelState {
    NotePanelState {
        shown: true,
        mode: NotePanelMode::Outline,
        mentions_expanded: false,
        outline_kinds: OutlineKinds {
            figures: true,
            labels: false,
        },
    }
}

/// Умолчания: колонка скрыта, режим «Связи», упоминания свёрнуты.
pub fn defaults_are_hidden_links_collapsed() {
    let state = Settings::default().note_panel;
    assert!(!state.shown);
    assert_eq!(state.mode, NotePanelMode::Links);
    assert!(!state.mentions_expanded);
    // Чистый файл не обзаводится ключом, пока колонку не трогали.
    let text = format::render("", &Settings::default(), &FolderRefs::new());
    assert!(!text.contains("note_panel"), "{text}");
}

/// Круговой путь через файл настроек.
pub fn state_round_trips_through_the_file() {
    let changed = SettingsChange::note_panel(open_panel()).apply_to(&Settings::default());
    let text = format::render("", &changed, &FolderRefs::new());
    assert!(text.contains("note_panel"), "{text}");
    assert_eq!(format::parse(&text).unwrap().note_panel, open_panel());

    // Возврат к умолчанию убирает ключ.
    let back = SettingsChange::note_panel(NotePanelState::default()).apply_to(&changed);
    let cleared = format::render(&text, &back, &FolderRefs::new());
    assert!(!cleared.contains("note_panel"), "{cleared}");
    assert_eq!(
        format::parse(&cleared).unwrap().note_panel,
        NotePanelState::default()
    );
}

/// Негодное значение не роняет остальные настройки и остальные поля.
pub fn a_bad_value_does_not_break_the_rest() {
    for written in [
        r#"{"theme": "dark", "editor_font_size": 20, "note_panel": 42}"#,
        r#"{"theme": "dark", "editor_font_size": 20, "note_panel": "shown"}"#,
        r#"{"theme": "dark", "editor_font_size": 20, "note_panel": null}"#,
        r#"{"theme": "dark", "editor_font_size": 20, "note_panel": {"shown": "да", "mode": 7}}"#,
    ] {
        let settings = format::parse(written).expect("файл разбирается");
        assert_eq!(settings.theme, Theme::Dark, "{written}");
        assert_eq!(settings.editor_font_size, 20, "{written}");
        assert_eq!(settings.note_panel, NotePanelState::default(), "{written}");
    }

    // Годное поле переживает соседнее негодное.
    let partial = format::parse(r#"{"note_panel": {"shown": true, "mentions_expanded": 3}}"#)
        .expect("файл разбирается");
    assert!(partial.note_panel.shown);
    assert!(!partial.note_panel.mentions_expanded);
}

/// Режим из более новой версии читается как «Связи».
pub fn an_unknown_mode_reads_as_links() {
    let settings = format::parse(
        r#"{"theme": "dark", "note_panel": {"shown": true, "mode": "from-the-future"}}"#,
    )
    .expect("файл разбирается");
    assert_eq!(settings.theme, Theme::Dark);
    assert!(settings.note_panel.shown);
    assert_eq!(settings.note_panel.mode, NotePanelMode::Links);
}

/// При открытом vault правка уходит в устройство и не попадает в vault.
pub fn the_state_goes_to_the_device_even_with_an_open_vault() {
    let change = SettingsChange::note_panel(open_panel());
    assert_eq!(target_for(&change, true), ChangeTarget::Device);
    assert_eq!(
        route(&change, true),
        vec![RoutedChange {
            target: ChangeTarget::Device,
            change: change.clone(),
        }]
    );

    // «Тема + колонка» при открытом vault — две записи, каждая в одно
    // хранилище: колонка не едет в vault вместе с темой.
    let mixed = SettingsChange {
        theme: Some(Theme::Dark),
        ..change
    };
    assert_eq!(
        route(&mixed, true),
        vec![
            RoutedChange {
                target: ChangeTarget::Device,
                change: SettingsChange::note_panel(open_panel()),
            },
            RoutedChange {
                target: ChangeTarget::OpenVault,
                change: SettingsChange::theme(Theme::Dark),
            },
        ]
    );

    // Слои vault состояние колонки не перекрывают.
    let device = Settings {
        note_panel: open_panel(),
        ..Settings::default()
    };
    let overrides = VaultOverrides {
        theme: Some(Theme::Dark),
        ..VaultOverrides::default()
    };
    assert_eq!(resolve(&device, &overrides).note_panel, open_panel());
}

/// FR-021: сброс к умолчаниям колонку не трогает.
pub fn reset_does_not_touch_the_state() {
    let reset = reset_to_defaults();
    assert_eq!(reset.note_panel, None);
    let shown = Settings {
        note_panel: open_panel(),
        ..Settings::default()
    };
    assert_eq!(reset.apply_to(&shown).note_panel, open_panel());

    // «Сделать значениями по умолчанию» тоже не переносит состояние колонки.
    assert_eq!(device_defaults_change(&shown).note_panel, None);
}

/// Форма JSON — контракт с интерфейсом.
pub fn state_json_shape() {
    assert_eq!(
        serde_json::to_value(open_panel()).unwrap(),
        serde_json::json!({"shown": true, "mode": "links", "mentions_expanded": true})
    );
    let change = SettingsChange::note_panel(open_panel());
    let json = serde_json::to_value(&change).unwrap();
    assert_eq!(json["note_panel"]["mode"], "links");
    // Отсутствующее поле набора не сериализуется: `null` прошёл бы в
    // интерфейс мимо проверки на `undefined`.
    assert!(serde_json::to_value(SettingsChange::theme(Theme::Dark))
        .unwrap()
        .get("note_panel")
        .is_none());
}

// ── Оглавление (фича 015, T007) ─────────────────────────────────────────────

/// Режим «Структура» и выбор видов пунктов переживают запись в файл и чтение.
pub fn the_outline_mode_and_kinds_round_trip() {
    let changed = SettingsChange::note_panel(outline_panel()).apply_to(&Settings::default());
    let text = format::render("", &changed, &FolderRefs::new());
    let back = format::parse(&text).unwrap().note_panel;
    assert_eq!(back, outline_panel());
    assert_eq!(back.mode, NotePanelMode::Outline);
    assert!(back.outline_kinds.figures);
    assert!(!back.outline_kinds.labels);
}

/// Форма JSON — контракт с интерфейсом: режим `outline`, виды вложенной
/// записью; умолчание видов не пишется, прежняя форма состояния не меняется.
pub fn the_outline_state_json_shape() {
    assert_eq!(
        serde_json::to_value(outline_panel()).unwrap(),
        serde_json::json!({
            "shown": true,
            "mode": "outline",
            "mentions_expanded": false,
            "outline_kinds": {"figures": true, "labels": false}
        })
    );
    assert!(serde_json::to_value(open_panel())
        .unwrap()
        .get("outline_kinds")
        .is_none());
}

/// Состояние без записи о видах (файл версии 013) читается с выключенными
/// видами; негодное поле видов не роняет остальные.
pub fn the_outline_kinds_are_tolerant_on_read() {
    let old = format::parse(r#"{"note_panel": {"shown": true, "mode": "links"}}"#)
        .expect("файл разбирается");
    assert_eq!(old.note_panel.outline_kinds, OutlineKinds::default());

    for written in [
        r#"{"note_panel": {"shown": true, "outline_kinds": 5}}"#,
        r#"{"note_panel": {"shown": true, "outline_kinds": {"figures": "да"}}}"#,
    ] {
        let settings = format::parse(written).expect("файл разбирается");
        assert!(settings.note_panel.shown, "{written}");
        assert_eq!(
            settings.note_panel.outline_kinds,
            OutlineKinds::default(),
            "{written}"
        );
    }

    let partial = format::parse(
        r#"{"note_panel": {"mode": "outline", "outline_kinds": {"labels": true, "figures": 1}}}"#,
    )
    .expect("файл разбирается");
    assert_eq!(partial.note_panel.mode, NotePanelMode::Outline);
    assert!(partial.note_panel.outline_kinds.labels);
    assert!(!partial.note_panel.outline_kinds.figures);

    // Режим из более новой версии по-прежнему читается как «Связи».
    let future = format::parse(r#"{"note_panel": {"shown": true, "mode": "sources-2030"}}"#)
        .expect("файл разбирается");
    assert_eq!(future.note_panel.mode, NotePanelMode::Links);
}
