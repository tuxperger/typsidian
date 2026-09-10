//! Проверки хранилища настроек уровня vault — общие для обоих таргетов
//! (фича 006, contracts/vault-settings.md).
//!
//! Работают поверх `MemoryVault`: логика лежит **над** `VaultBackend`, и
//! настоящий доступ к диску здесь только мешал бы — на wasm32 файловой системы
//! нет вовсе (принцип II).

#![allow(dead_code)]

use std::path::Path;
use typsidian_core::boards::placement::AssetsPlacement;
use typsidian_core::vault::sections::SectionMark;
use typsidian_core::vault::service_dir;
use typsidian_core::vault::events::VaultEvent;
use typsidian_core::vault::vault_settings::{
    read_vault_settings, service_dir_status, set_section, write_vault_settings, VaultOverrides,
    VaultSettingsError,
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
    assert_eq!(read_vault_settings(&vault).await.unwrap().editor_font_size, Some(19));
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

    let after = set_section(&mut vault, Path::new("Работа"), true).await.unwrap();
    assert_eq!(after.sections.len(), 1);

    // Повторная пометка — не дубль и не ошибка.
    let again = set_section(&mut vault, Path::new("Работа"), true).await.unwrap();
    assert_eq!(again.sections.len(), 1);

    let two = set_section(&mut vault, Path::new("Личное"), true).await.unwrap();
    assert_eq!(
        two.sections.iter().map(|s| s.path.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        vec!["Личное".to_string(), "Работа".to_string()],
        "порядок детерминирован, иначе каждая пометка переставляла бы строки в файле"
    );

    let removed = set_section(&mut vault, Path::new("Работа"), false).await.unwrap();
    assert_eq!(removed.sections.len(), 1);
    assert_eq!(removed.sections[0].path, Path::new("Личное"));
}

/// FR-017: снятие пометки ничего не двигает на диске.
pub async fn unmarking_moves_no_files() {
    let snapshot = "Работа/assets/Идея/board-47dc7994.svg";
    let mut vault = MemoryVault::new().with_file(snapshot, b"<svg/>".to_vec());
    set_section(&mut vault, Path::new("Работа"), true).await.unwrap();
    set_section(&mut vault, Path::new("Работа"), false).await.unwrap();
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
