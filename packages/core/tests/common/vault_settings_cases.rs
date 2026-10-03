//! Проверки хранилища настроек уровня vault — общие для обоих таргетов
//! (фича 006, contracts/vault-settings.md).
//!
//! Работают поверх `MemoryVault`: логика лежит **над** `VaultBackend`, и
//! настоящий доступ к диску здесь только мешал бы — на wasm32 файловой системы
//! нет вовсе (принцип II).

#![allow(dead_code)]

use std::path::Path;
use typsidian_core::boards::placement::AssetsPlacement;
use typsidian_core::settings::keybindings::{self, CommandId, Shortcut, ShortcutProblem};
use typsidian_core::vault::events::VaultEvent;
use typsidian_core::vault::sections::SectionMark;
use typsidian_core::vault::service_dir;
use typsidian_core::vault::vault_settings::{
    read_vault_settings, reset_keybinding, service_dir_status, set_keybinding, set_section,
    write_vault_settings, SetKeybindingError, VaultOverrides, VaultSettingsError,
};

use super::MemoryVault;
use typsidian_core::vault::backend::VaultBackend;

const SETTINGS_PATH: &str = ".typsidian/vault.json";

/// Отсутствие файла — штатное состояние, а не ошибка (FR-023), и политика по
/// умолчанию при этом «рядом с заметкой» (FR-003).
pub async fn absent_file_is_not_an_error() {
    let vault = MemoryVault::new();
    let overrides = read_vault_settings(&vault)
        .await
        .expect("отсутствие файла ошибкой не является");
    assert_eq!(overrides, VaultOverrides::default());
    assert_eq!(overrides.placement(), AssetsPlacement::NextToNote);
}

/// Значения переживают круговой рейс через файл.
pub async fn values_survive_a_round_trip() {
    let mut vault = MemoryVault::new();
    let written = VaultOverrides {
        assets_placement: Some(AssetsPlacement::VaultRoot),
        sections: vec![SectionMark::new(Path::new("Работа")).unwrap()],
        editor_font_size: Some(16),
        ..VaultOverrides::default()
    };
    write_vault_settings(&mut vault, &written)
        .await
        .expect("запись в доступный vault");
    assert_eq!(read_vault_settings(&vault).await.unwrap(), written);
}

/// Файл лежит в служебном месте, а не в корне vault (FR-020).
pub async fn the_file_lives_in_the_service_dir() {
    let mut vault = MemoryVault::new();
    write_vault_settings(&mut vault, &VaultOverrides::default())
        .await
        .expect("запись в доступный vault");
    assert_eq!(service_dir::vault_settings_path(), Path::new(SETTINGS_PATH));
    assert!(
        vault.read(Path::new(SETTINGS_PATH)).await.is_ok(),
        "файл настроек обязан лежать внутри .typsidian/"
    );
}

/// Дописанное пользователем переживает запись из панели: файл принадлежит ему
/// (то же правило, что для `settings.json` в фиче 004).
///
/// Комментариев в JSON нет — эту половину прежней гарантии смена формата
/// оплатила и притворяться иначе нельзя. Уцелевшая половина проверяется здесь:
/// ключи, которых приложение не знает, не выбрасываются.
pub async fn hand_written_keys_survive_a_write() {
    let authored = "{\n  \"settings\": {\"editor_font_size\": 18},\n  \"my_future_option\": {\"nested\": true}\n}\n";
    let mut vault = MemoryVault::new().with_file(SETTINGS_PATH, authored.as_bytes().to_vec());

    let mut overrides = read_vault_settings(&vault).await.unwrap();
    overrides.editor_font_size = Some(19);
    write_vault_settings(&mut vault, &overrides).await.unwrap();

    let rewritten = vault.read(Path::new(SETTINGS_PATH)).await.unwrap();
    assert!(rewritten.contains("my_future_option"));
    assert!(rewritten.contains("nested"));
    assert_eq!(
        read_vault_settings(&vault).await.unwrap().editor_font_size,
        Some(19)
    );
}

/// Испорченный файл не перезаписывается молча (FR-034): он отличим от пустого,
/// иначе настройки пользователя молча заменились бы умолчаниями.
pub async fn a_broken_file_is_reported_not_overwritten() {
    let broken = "{ \"assets\": ";
    let vault = MemoryVault::new().with_file(SETTINGS_PATH, broken.as_bytes().to_vec());
    assert_eq!(
        read_vault_settings(&vault).await,
        Err(VaultSettingsError::Unreadable)
    );
    // Содержимое на месте: чтение ничего не переписало.
    assert_eq!(vault.read(Path::new(SETTINGS_PATH)).await.unwrap(), broken);
}

/// Пометка и снятие пометки идут через ядро и дают детерминированный порядок
/// (FR-013).
pub async fn marking_a_section_is_idempotent_and_ordered() {
    let mut vault = MemoryVault::new();

    let after = set_section(&mut vault, Path::new("Работа"), true)
        .await
        .unwrap();
    assert_eq!(after.sections.len(), 1);

    // Повторная пометка — не дубль и не ошибка.
    let again = set_section(&mut vault, Path::new("Работа"), true)
        .await
        .unwrap();
    assert_eq!(again.sections.len(), 1);

    let two = set_section(&mut vault, Path::new("Личное"), true)
        .await
        .unwrap();
    assert_eq!(
        two.sections
            .iter()
            .map(|s| s.path.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["Личное".to_string(), "Работа".to_string()],
        "порядок детерминирован, иначе каждая пометка переставляла бы строки в файле"
    );

    let removed = set_section(&mut vault, Path::new("Работа"), false)
        .await
        .unwrap();
    assert_eq!(removed.sections.len(), 1);
    assert_eq!(removed.sections[0].path, Path::new("Личное"));
}

/// FR-017: снятие пометки ничего не двигает на диске.
pub async fn unmarking_moves_no_files() {
    let snapshot = "Работа/assets/Идея/board-47dc7994.svg";
    let mut vault = MemoryVault::new().with_file(snapshot, b"<svg/>".to_vec());
    set_section(&mut vault, Path::new("Работа"), true)
        .await
        .unwrap();
    set_section(&mut vault, Path::new("Работа"), false)
        .await
        .unwrap();
    assert!(
        vault.read(Path::new(snapshot)).await.is_ok(),
        "файл вставки обязан остаться на месте"
    );
}

/// FR-025: vault только на чтение служебного места не получает, и причина
/// названа отдельным словом, а не общим «не удалось».
pub async fn a_read_only_vault_gets_no_service_dir() {
    let mut vault = MemoryVault::new().read_only();
    assert_eq!(
        write_vault_settings(&mut vault, &VaultOverrides::default()).await,
        Err(VaultSettingsError::NotWritable)
    );
    assert!(
        vault.read(Path::new(SETTINGS_PATH)).await.is_err(),
        "файл настроек не создан"
    );
    assert!(
        matches!(
            service_dir_status(&vault, true),
            Some(VaultEvent::VaultSettingsReadOnly)
        ),
        "недоступность записи — более сильный факт, чем отсутствие папки"
    );
}

/// FR-024: об утрате настроек сообщается только там, где было что терять.
///
/// Признак «было что терять» — локальный для vault: приложение оставляет в
/// папке ровно один вид следов, файлы вставок.
pub async fn settings_loss_is_reported_only_when_there_was_something_to_lose() {
    // Следы приложения есть, служебного места нет — его удалили.
    let seen_before = MemoryVault::new().without_service_dir();
    assert!(matches!(
        service_dir_status(&seen_before, true),
        Some(VaultEvent::VaultSettingsLost)
    ));

    // Свежая папка без единого снимка — терять было нечего, и ложное
    // сообщение об утрате хуже его отсутствия.
    assert!(
        service_dir_status(&seen_before, false).is_none(),
        "первое открытие не повод объявлять утрату"
    );

    // Служебное место на месте — сказать нечего в обоих случаях.
    let intact = MemoryVault::new();
    assert!(service_dir_status(&intact, true).is_none());
    assert!(service_dir_status(&intact, false).is_none());
}

/// FR-028: реестру vault в этом файле места нет — ключ не подхватывается даже
/// написанный руками.
pub async fn the_vault_registry_has_nowhere_to_live_here() {
    let text = r#"{"settings": {"known_vaults": [{"id": "a", "location": "/x"}]}}"#;
    let vault = MemoryVault::new().with_file(SETTINGS_PATH, text.as_bytes().to_vec());
    assert_eq!(
        read_vault_settings(&vault).await.unwrap(),
        VaultOverrides::default()
    );
}

// ── Сочетания клавиш ────────────────────────────────────────────────────────

/// Назначенное сочетание переживает круговой рейс через файл, а команда, которой
/// не касались, остаётся на умолчании ядра.
pub async fn a_keybinding_survives_a_round_trip() {
    let mut vault = MemoryVault::new();
    let mine = Shortcut::normalize(true, false, true, "p");

    let after = set_keybinding(&mut vault, CommandId::CommandPalette, Some(mine.clone()))
        .await
        .expect("назначение в доступный vault");
    assert_eq!(after.keybindings.len(), 1);

    let read_back = read_vault_settings(&vault).await.unwrap();
    assert_eq!(read_back.keybindings, after.keybindings);

    let resolved = keybindings::resolve(&read_back.keybindings, false);
    let palette = resolved
        .iter()
        .find(|binding| binding.command == CommandId::CommandPalette)
        .unwrap();
    assert_eq!(palette.shortcut.as_ref(), Some(&mine));
    assert!(!palette.is_default);

    // Соседняя команда не тронута.
    let search = resolved
        .iter()
        .find(|binding| binding.command == CommandId::Search)
        .unwrap();
    assert!(search.is_default);
}

/// Негодное сочетание отклоняется **до** записи: файл не должен содержать то,
/// чем нельзя пользоваться.
pub async fn an_unusable_shortcut_is_refused_before_the_file_is_touched() {
    let mut vault = MemoryVault::new();

    let refused = set_keybinding(
        &mut vault,
        CommandId::Search,
        // Escape занят интерфейсом.
        Some(Shortcut::normalize(true, false, false, "escape")),
    )
    .await;
    assert!(matches!(
        refused,
        Err(SetKeybindingError::Invalid(ShortcutProblem::ReservedKey))
    ));

    let refused = set_keybinding(
        &mut vault,
        CommandId::Search,
        // Без модификатора сочетание срабатывало бы при наборе текста.
        Some(Shortcut::normalize(false, false, false, "f")),
    )
    .await;
    assert!(matches!(
        refused,
        Err(SetKeybindingError::Invalid(ShortcutProblem::NoModifier))
    ));

    // Отказ не оставил следов: файла нет вовсе.
    assert_eq!(
        read_vault_settings(&vault).await.unwrap(),
        VaultOverrides::default()
    );
}

/// Снять сочетание и вернуть умолчание — разные действия, и одно не выражается
/// через другое.
pub async fn clearing_differs_from_resetting() {
    let mut vault = MemoryVault::new();

    // Снятое: запись есть, сочетания нет.
    let cleared = set_keybinding(&mut vault, CommandId::Search, None)
        .await
        .expect("снятие в доступный vault");
    assert_eq!(cleared.keybindings.len(), 1);
    let resolved = keybindings::resolve(&cleared.keybindings, false);
    let search = resolved
        .iter()
        .find(|binding| binding.command == CommandId::Search)
        .unwrap();
    assert_eq!(search.shortcut, None);
    assert!(!search.is_default);

    // Возврат к умолчанию: записи не остаётся вовсе, и сочетание снова есть.
    let reset = reset_keybinding(&mut vault, CommandId::Search)
        .await
        .expect("возврат к умолчанию");
    assert!(reset.keybindings.is_empty());
    let resolved = keybindings::resolve(&reset.keybindings, false);
    let search = resolved
        .iter()
        .find(|binding| binding.command == CommandId::Search)
        .unwrap();
    assert!(search.is_default);
    assert_eq!(
        search.shortcut.as_ref(),
        Some(&CommandId::Search.default_shortcut())
    );
}

/// Повторное назначение той же команде заменяет запись, а не добавляет вторую:
/// иначе файл рос бы при каждом изменении, а какая из записей действует, стало
/// бы вопросом порядка.
pub async fn reassigning_replaces_rather_than_appends() {
    let mut vault = MemoryVault::new();
    for key in ["p", "j", "u"] {
        set_keybinding(
            &mut vault,
            CommandId::CommandPalette,
            Some(Shortcut::normalize(true, false, false, key)),
        )
        .await
        .expect("назначение");
    }
    let overrides = read_vault_settings(&vault).await.unwrap();
    assert_eq!(overrides.keybindings.len(), 1);
    assert_eq!(
        overrides.keybindings[0]
            .shortcut
            .as_ref()
            .map(|s| s.key.as_str()),
        Some("u")
    );
}

/// Возврат команды, которую и не меняли, файл не трогает: лишняя запись — лишний
/// повод его испортить.
pub async fn resetting_an_untouched_command_writes_nothing() {
    let mut vault = MemoryVault::new();
    let overrides = reset_keybinding(&mut vault, CommandId::Search)
        .await
        .expect("возврат к умолчанию");
    assert_eq!(overrides, VaultOverrides::default());
    assert!(
        vault.read(Path::new(SETTINGS_PATH)).await.is_err(),
        "файл не должен появиться от возврата к тому, что и так действует"
    );
}

/// Фича 007, FR-004: адрес шаблона пишется только годным, и отказ ничего не
/// портит — ни эту запись, ни прежний файл.
pub async fn a_bad_template_source_is_refused_before_writing() {
    use typsidian_core::vault::vault_settings::TemplateSourceEntry;

    let mut vault = MemoryVault::new();
    let good = VaultOverrides {
        template_sources: vec![TemplateSourceEntry {
            spec: "@preview/codly:1.3.0".to_string(),
            url: "https://example.org/codly.tar.gz".to_string(),
        }],
        ..VaultOverrides::default()
    };
    write_vault_settings(&mut vault, &good)
        .await
        .expect("годный адрес записывается");

    for (spec, url) in [
        ("codly", "https://example.org/codly.tar.gz"),
        ("@preview/codly:1.3", "https://example.org/codly.tar.gz"),
        ("@preview/codly:1.3.0", "ftp://example.org/codly.tar.gz"),
        ("@preview/codly:1.3.0", "https://"),
        (
            "@preview/codly:1.3.0",
            "https://example.org/с пробелом.tar.gz",
        ),
    ] {
        let bad = VaultOverrides {
            template_sources: vec![TemplateSourceEntry {
                spec: spec.to_string(),
                url: url.to_string(),
            }],
            ..VaultOverrides::default()
        };
        assert_eq!(
            write_vault_settings(&mut vault, &bad).await,
            Err(VaultSettingsError::InvalidTemplateSource {
                spec: spec.to_string()
            }),
            "{spec} → {url}"
        );
    }

    assert_eq!(
        read_vault_settings(&vault).await.unwrap(),
        good,
        "прежние настройки не тронуты"
    );
}

/// Фича 009, FR-029: папка экспорта пишется только годной и в нормальной
/// форме, а отказ не портит прежний файл.
pub async fn the_export_folder_is_validated_and_normalized() {
    use typsidian_core::export::ExportFolderProblem;

    let mut vault = MemoryVault::new();
    let good = VaultOverrides {
        export_folder: Some(" Выгрузка\\2026/ ".to_string()),
        ..VaultOverrides::default()
    };
    write_vault_settings(&mut vault, &good)
        .await
        .expect("годная папка записывается");
    let stored = read_vault_settings(&vault).await.unwrap();
    assert_eq!(stored.export_folder.as_deref(), Some("Выгрузка/2026"));

    for (raw, problem) in [
        ("../вне", ExportFolderProblem::LeavesVault),
        ("/abs", ExportFolderProblem::Absolute),
        (".typsidian/pdf", ExportFolderProblem::ServiceFolder),
        ("  ", ExportFolderProblem::Empty),
    ] {
        let bad = VaultOverrides {
            export_folder: Some(raw.to_string()),
            ..VaultOverrides::default()
        };
        assert_eq!(
            write_vault_settings(&mut vault, &bad).await,
            Err(VaultSettingsError::InvalidExportFolder { problem }),
            "{raw}"
        );
    }
    assert_eq!(
        read_vault_settings(&vault).await.unwrap(),
        stored,
        "прежнее значение в силе"
    );
}

/// Испорченная руками папка в файле отбрасывается молча: экспорт идёт в
/// папку по умолчанию, остальные настройки читаются (как у пометок разделов).
pub async fn a_bad_export_folder_in_the_file_is_dropped() {
    let vault = MemoryVault::new().with_file(
        ".typsidian/vault.json",
        r#"{"export": {"folder": "../../etc"}, "settings": {"theme": "dark"}}"#,
    );
    let read = read_vault_settings(&vault).await.unwrap();
    assert_eq!(read.export_folder, None);
    assert!(read.theme.is_some());
}
