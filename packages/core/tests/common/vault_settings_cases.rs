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

// ── Вложения (фича 016, T017) ───────────────────────────────────────────────

/// Размещение и папка вложений записываются и читаются; негодное название
/// отклоняется до записи, и файл не тронут (FR-017, FR-018).
pub async fn the_attachments_settings_are_validated_and_survive_a_round_trip() {
    use typsidian_core::attachments::{AttachmentsFolderProblem, AttachmentsPlacement};

    let mut vault = MemoryVault::new();
    let good = VaultOverrides {
        attachments_placement: Some(AttachmentsPlacement::VaultFolder),
        attachments_folder: Some(" файлы\\картинки/ ".to_string()),
        ..VaultOverrides::default()
    };
    write_vault_settings(&mut vault, &good)
        .await
        .expect("годная папка записывается");
    let stored = read_vault_settings(&vault).await.unwrap();
    assert_eq!(
        stored.attachments_placement,
        Some(AttachmentsPlacement::VaultFolder)
    );
    assert_eq!(stored.attachments_folder.as_deref(), Some("файлы/картинки"));
    let text = String::from_utf8(vault.bytes_of(SETTINGS_PATH).unwrap()).unwrap();
    assert!(text.contains("\"vault-folder\""), "{text}");

    let before = vault.bytes_of(SETTINGS_PATH);
    for (raw, problem) in [
        ("../вне", AttachmentsFolderProblem::LeavesVault),
        ("/abs", AttachmentsFolderProblem::Absolute),
        (".typsidian/x", AttachmentsFolderProblem::ServiceFolder),
        ("", AttachmentsFolderProblem::Empty),
    ] {
        let bad = VaultOverrides {
            attachments_folder: Some(raw.to_string()),
            ..VaultOverrides::default()
        };
        assert_eq!(
            write_vault_settings(&mut vault, &bad).await,
            Err(VaultSettingsError::InvalidAttachmentsFolder { problem }),
            "{raw:?}"
        );
    }
    assert_eq!(vault.bytes_of(SETTINGS_PATH), before, "файл не тронут");
}

/// Негодные значения в файле отбрасываются молча, неизвестные ключи таблицы
/// `attachments` переживают запись из панели.
pub async fn bad_attachments_values_are_dropped_and_unknown_keys_kept() {
    let mut vault = MemoryVault::new().with_file(
        SETTINGS_PATH,
        r#"{"attachments": {"placement": "под-кроватью", "folder": "../../etc", "своё": 1}, "settings": {"theme": "dark"}}"#,
    );
    let read = read_vault_settings(&vault).await.unwrap();
    assert_eq!(read.attachments_placement, None);
    assert_eq!(read.attachments_folder, None);
    assert!(read.theme.is_some(), "остальные настройки читаются");

    let next = VaultOverrides {
        attachments_folder: Some("файлы".to_string()),
        ..read
    };
    write_vault_settings(&mut vault, &next).await.unwrap();
    let text = String::from_utf8(vault.bytes_of(SETTINGS_PATH).unwrap()).unwrap();
    assert!(text.contains("\"своё\""), "чужой ключ сохранён: {text}");
    assert_eq!(
        read_vault_settings(&vault)
            .await
            .unwrap()
            .attachments_folder
            .as_deref(),
        Some("файлы")
    );
}

// ── Сочетания со знаками (фича 014, T004) ───────────────────────────────────

/// Сочетание со знаком несёт имя физической клавиши в `ResolvedBinding.code`:
/// по нему интерфейс сверяет нажатие в любой раскладке и с `Shift` (research.md §12).
pub fn a_punctuation_shortcut_carries_the_key_code() {
    let resolved = keybindings::resolve(&[], false);
    let code_of = |command: CommandId| {
        resolved
            .iter()
            .find(|binding| binding.command == command)
            .map(|binding| binding.code.clone())
            .unwrap()
    };
    assert_eq!(code_of(CommandId::TabNext), "Period");
    assert_eq!(code_of(CommandId::TabPrevious), "Comma");
    assert_eq!(code_of(CommandId::SplitEditor), "Backslash");
    assert_eq!(code_of(CommandId::TabMoveToOtherGroup), "Backslash");
    assert_eq!(code_of(CommandId::NavigateBack), "BracketLeft");
    assert_eq!(code_of(CommandId::NavigateForward), "BracketRight");
    // Буква, как и прежде.
    assert_eq!(code_of(CommandId::TabClose), "KeyW");

    // Два сочетания на `\` различаются Shift: из коробки конфликта нет.
    for binding in &resolved {
        assert!(binding.conflicts_with.is_empty(), "{:?}", binding.command);
    }
    let shifted = resolved
        .iter()
        .find(|binding| binding.command == CommandId::TabMoveToOtherGroup)
        .unwrap();
    assert_eq!(shifted.display, "Ctrl+Alt+Shift+\\");
}

// ── Команда оглавления (фича 015, T007) ─────────────────────────────────────

/// `toggle-outline` стоит шестой с конца (за ней 019, 022, 016 и две 017), имя и
/// умолчание зафиксированы, а `problem()`
/// сочетание не отвергает; ни одна другая команда умолчания не делит.
pub fn the_outline_command_has_its_name_and_default() {
    assert_eq!(
        keybindings::COMMANDS[keybindings::COMMANDS.len() - 8],
        CommandId::ToggleOutline
    );
    assert_eq!(CommandId::ToggleOutline.as_str(), "toggle-outline");
    assert_eq!(
        CommandId::parse("toggle-outline"),
        Some(CommandId::ToggleOutline)
    );
    let shortcut = CommandId::ToggleOutline.default_shortcut();
    assert_eq!(shortcut.to_text(), "Mod+Shift+O");
    assert_eq!(shortcut.problem(), None);

    let resolved = keybindings::resolve(&[], false);
    let binding = resolved
        .iter()
        .find(|binding| binding.command == CommandId::ToggleOutline)
        .unwrap();
    assert_eq!(binding.code, "KeyO");
    assert_eq!(binding.display, "Ctrl+Shift+O");
    assert!(binding.conflicts_with.is_empty());
}

// ── Команда показа слайдов (фича 019, T007) ─────────────────────────────────

/// `start-show` стоит пятой с конца, имя и умолчание `Mod+Alt+P` зафиксированы,
/// сочетание не отвергается и не делится ни с одной другой командой.
pub fn the_start_show_command_has_its_name_and_default() {
    assert_eq!(
        keybindings::COMMANDS[keybindings::COMMANDS.len() - 7],
        CommandId::StartShow
    );
    assert_eq!(CommandId::StartShow.as_str(), "start-show");
    assert_eq!(CommandId::parse("start-show"), Some(CommandId::StartShow));
    let shortcut = CommandId::StartShow.default_shortcut();
    assert_eq!(shortcut.to_text(), "Mod+Alt+P");
    assert_eq!(shortcut.problem(), None);

    let resolved = keybindings::resolve(&[], false);
    let binding = resolved
        .iter()
        .find(|binding| binding.command == CommandId::StartShow)
        .unwrap();
    assert_eq!(binding.code, "KeyP");
    assert_eq!(binding.display, "Ctrl+Alt+P");
    assert!(binding.is_default);
    assert!(binding.conflicts_with.is_empty());
}

// ── Команда «Сегодня» (фича 022, T008) ───────────────────────────────────────

/// `today` стоит четвёртой с конца (за ней 016 и две команды 017), имя и
/// свободное умолчание `Mod+Alt+J` зафиксированы,
/// сочетание не отвергается и не делится ни с одной другой командой.
pub fn the_today_command_has_its_name_and_default() {
    assert_eq!(
        keybindings::COMMANDS[keybindings::COMMANDS.len() - 6],
        CommandId::Today
    );
    assert_eq!(CommandId::Today.as_str(), "today");
    assert_eq!(CommandId::parse("today"), Some(CommandId::Today));
    let shortcut = CommandId::Today.default_shortcut();
    assert_eq!(shortcut.to_text(), "Mod+Alt+J");
    assert_eq!(shortcut.problem(), None);

    let resolved = keybindings::resolve(&[], false);
    let binding = resolved
        .iter()
        .find(|binding| binding.command == CommandId::Today)
        .unwrap();
    assert_eq!(binding.code, "KeyJ");
    assert_eq!(binding.display, "Ctrl+Alt+J");
    assert!(binding.conflicts_with.is_empty());
}

// ── Набор вкладок в настройках устройства (фича 014, T055) ──────────────────

fn saved_set(paths: &[&str]) -> typsidian_core::workspace::SavedWorkspace {
    use typsidian_core::workspace::{SavedGroup, SavedWorkspace};
    SavedWorkspace {
        groups: vec![SavedGroup {
            tabs: paths.iter().map(|p| p.to_string()).collect(),
            active: Some(0),
        }],
        active_group: typsidian_core::workspace::GroupSlot::First,
        ratio_permille: 500,
    }
}

fn workspace_change(
    vault: &str,
    paths: &[&str],
) -> typsidian_core::settings::apply::SettingsChange {
    use typsidian_core::settings::apply::SettingsChange;
    use typsidian_core::settings::{VaultId, VaultWorkspaceEntry};
    SettingsChange {
        workspace: Some(VaultWorkspaceEntry {
            vault: VaultId::from_raw(vault),
            value: saved_set(paths),
        }),
        ..SettingsChange::default()
    }
}

/// Набор заменяет запись своего vault и не трогает чужую.
pub fn a_workspace_change_replaces_only_its_own_vault() {
    use typsidian_core::settings::{Settings, VaultId};
    let a = VaultId::from_raw("a");
    let b = VaultId::from_raw("b");
    let s = workspace_change("a", &["x.typ"]).apply_to(&Settings::default());
    let s = workspace_change("b", &["y.typ"]).apply_to(&s);
    let s = workspace_change("a", &["z.typ", "w.typ"]).apply_to(&s);
    assert_eq!(s.workspaces.len(), 2);
    assert_eq!(s.workspace_for(&a), Some(saved_set(&["z.typ", "w.typ"])));
    assert_eq!(s.workspace_for(&b), Some(saved_set(&["y.typ"])));
    // Пустой набор убирает запись.
    let s = workspace_change("a", &[]).apply_to(&s);
    assert_eq!(s.workspace_for(&a), None);
    assert_eq!(s.workspaces.len(), 1);
    // Забытый vault теряет свой набор.
    let mut s = s;
    s.forget_workspace(&b);
    assert!(s.workspaces.is_empty());
}

/// Маршрут: набор всегда в устройство, даже при открытом vault.
pub fn a_workspace_change_goes_to_the_device() {
    use typsidian_core::settings::apply::{route, ChangeTarget, SettingsChange};
    use typsidian_core::settings::Theme;
    let change = workspace_change("a", &["x.typ"]);
    for open in [true, false] {
        let routed = route(&change, open);
        assert_eq!(routed.len(), 1);
        assert_eq!(routed[0].target, ChangeTarget::Device);
    }
    let mixed = SettingsChange {
        theme: Some(Theme::Dark),
        ..change
    };
    let routed = route(&mixed, true);
    assert_eq!(routed.len(), 2);
    assert_eq!(routed[0].target, ChangeTarget::Device);
    assert!(routed[0].change.workspace.is_some());
    assert_eq!(routed[1].target, ChangeTarget::OpenVault);
    assert!(routed[1].change.workspace.is_none());
}

/// Ключ `workspaces` переживает круг записи; негодное читается как пустое.
pub fn the_workspaces_key_round_trips_and_tolerates_garbage() {
    use typsidian_core::settings::format::{self, FolderRefs};
    use typsidian_core::settings::{Settings, Theme, VaultId};
    let s = workspace_change("a", &["x.typ"]).apply_to(&Settings::default());
    let text = format::render("", &s, &FolderRefs::new());
    assert!(text.contains("workspaces"), "{text}");
    assert_eq!(
        format::parse(&text)
            .unwrap()
            .workspace_for(&VaultId::from_raw("a")),
        Some(saved_set(&["x.typ"]))
    );
    let cleared = format::render(&text, &Settings::default(), &FolderRefs::new());
    assert!(!cleared.contains("workspaces"), "{cleared}");

    for written in [
        r#"{"theme": "dark", "workspaces": 42}"#,
        r#"{"theme": "dark", "workspaces": "x"}"#,
        r#"{"theme": "dark", "workspaces": [{"vault": "a", "value": {"groups": 3}}]}"#,
    ] {
        let parsed = format::parse(written).expect("файл разбирается");
        assert_eq!(parsed.theme, Theme::Dark, "{written}");
        assert!(parsed.workspaces.is_empty(), "{written}");
    }
}

/// Набор из одного `workspace` не даёт записи журнала.
pub fn a_workspace_change_is_not_journaled() {
    use typsidian_core::settings::logging::Journal;
    let mut journal = Journal::default();
    journal.record_change(0, &workspace_change("a", &["x.typ"]));
    assert_eq!(journal.text(), "");
}
