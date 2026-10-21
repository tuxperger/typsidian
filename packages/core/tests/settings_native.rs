//! cargo test — только native (принцип II). Тесты организованы по user story
//! (см. `specs/004-app-settings/tasks.md`).
#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::path::Path;
use tempfile::TempDir;
use typsidian_core::settings::apply::{reset_to_defaults, ApplyPlan, RejectReason, SettingsChange};
use typsidian_core::settings::events::SettingsEvent;
use typsidian_core::settings::folder_ref::{FolderRegistry, Resolved};
use typsidian_core::settings::format::{self, FolderRefs};
use typsidian_core::settings::native::NativeStore;
use typsidian_core::settings::store::{SettingsStore, StorageState, SETTINGS_FILE_NAME};
use typsidian_core::settings::{Settings, Theme};

/// Открывает хранилище в отдельном каталоге: настоящий каталог конфигурации
/// пользователя тесты не трогают.
async fn store_in(dir: &TempDir) -> NativeStore {
    NativeStore::open_in(dir.path().to_path_buf())
        .await
        .expect("открытие хранилища")
}

fn settings_path(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join(SETTINGS_FILE_NAME)
}

// ---------------------------------------------------------------------------
// Phase 2 — формат файла (T018)
// ---------------------------------------------------------------------------

#[test]
fn what_the_user_wrote_by_hand_survives_an_application_write() {
    // Прежняя проверка research.md §2 требовала переживания **комментариев**.
    // В JSON комментариев нет, и притворяться иначе нельзя — но вторая половина
    // того же требования (FR-013) в силе: приложение не выбрасывает ключи,
    // которых не понимает.
    let authored = r#"{
  "theme": "dark",
  "editor_font_size": 18,
  "my_future_option": {"nested": true}
}"#;
    let mut settings = format::parse(authored).expect("файл должен разбираться");
    settings.editor_font_size = 21;

    let rewritten = format::render(authored, &settings, &FolderRefs::new());

    assert!(
        rewritten.contains("my_future_option"),
        "дописанное пользователем обязано пережить запись: {rewritten}"
    );
    assert!(
        rewritten.contains("nested"),
        "вместе со вложенным содержимым"
    );
    assert_eq!(format::parse(&rewritten).unwrap().editor_font_size, 21);
    assert_eq!(format::parse(&rewritten).unwrap().theme, Theme::Dark);
}

// ---------------------------------------------------------------------------
// Phase 2 — паритет open/load/apply (T021)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn open_on_empty_directory_starts_from_defaults() {
    let dir = TempDir::new().unwrap();
    let store = store_in(&dir).await;

    assert_eq!(store.settings(), &Settings::default());
    assert_eq!(store.storage_state(), StorageState::Loaded);
}

#[tokio::test]
async fn apply_persists_values_and_they_survive_reopen() {
    // FR-002, FR-007: изменение вступает в силу и переживает перезапуск.
    let dir = TempDir::new().unwrap();
    {
        let mut store = store_in(&dir).await;
        let plan = store.apply(&SettingsChange::theme(Theme::Dark)).await;
        assert_eq!(plan, ApplyPlan::Applied);
    }

    let reopened = store_in(&dir).await;
    assert_eq!(reopened.settings().theme, Theme::Dark);
}

#[tokio::test]
async fn out_of_range_font_size_is_rejected_with_a_reason() {
    // FR-016, research.md §11 — одинаковый отказ на обоих таргетах.
    let dir = TempDir::new().unwrap();
    let mut store = store_in(&dir).await;

    for value in [7u8, 33u8] {
        assert_eq!(
            store.apply(&SettingsChange::font_size(value)).await,
            ApplyPlan::Rejected {
                reason: RejectReason::ValueOutOfRange
            },
            "размер {value} должен отклоняться"
        );
    }
    for value in [8u8, 32u8] {
        assert_eq!(
            store.apply(&SettingsChange::font_size(value)).await,
            ApplyPlan::Applied,
            "размер {value} должен приниматься"
        );
    }
    // Отклонённое значение не должно осесть в файле.
    let on_disk = format::parse(&fs::read_to_string(settings_path(&dir)).unwrap()).unwrap();
    assert_eq!(on_disk.editor_font_size, 32);
}

#[tokio::test]
async fn write_is_atomic_and_leaves_no_partial_file() {
    // research.md §5: наблюдаемое состояние — либо прежнее целиком, либо новое
    // целиком. Временные файлы после записи не остаются.
    let dir = TempDir::new().unwrap();
    let mut store = store_in(&dir).await;
    for size in [10u8, 11, 12, 13, 14] {
        store.apply(&SettingsChange::font_size(size)).await;
    }

    let leftovers: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "после атомарной записи не должно оставаться временных файлов: {leftovers:?}"
    );

    let text = fs::read_to_string(settings_path(&dir)).unwrap();
    assert!(
        format::parse(&text).is_ok(),
        "файл всегда остаётся разбираемым целиком"
    );
}

#[tokio::test]
async fn capabilities_report_native_abilities_by_value() {
    // research.md §13: платформенные различия — значение в API, а не его форма.
    let dir = TempDir::new().unwrap();
    let store = store_in(&dir).await;
    let capabilities = store.capabilities();

    assert!(capabilities.can_reveal, "на native раскрытие доступно");
    assert!(capabilities.writable);
    assert!(
        capabilities.location.ends_with(SETTINGS_FILE_NAME),
        "расположение показывается пользователю как путь к файлу (FR-036)"
    );
}

#[tokio::test]
async fn reset_keeps_known_vaults_and_returns_values_to_defaults() {
    // FR-037, FR-038, SC-011.
    let dir = TempDir::new().unwrap();
    let vault_dir = TempDir::new().unwrap();
    let mut store = store_in(&dir).await;

    let id = store
        .add(vault_dir.path().to_path_buf(), "notes".to_string())
        .await
        .expect("добавление папки");
    store.apply(&SettingsChange::theme(Theme::Dark)).await;

    assert_eq!(store.apply(&reset_to_defaults()).await, ApplyPlan::Applied);

    assert_eq!(store.settings().theme, Theme::Light);
    assert_eq!(
        store.settings().known_vaults.len(),
        1,
        "сброс не удаляет список известных vault"
    );
    assert_eq!(store.settings().known_vaults[0].id, id);
    // Ни один файл внутри самой папки vault не появился и не изменился.
    assert_eq!(
        fs::read_dir(vault_dir.path()).unwrap().count(),
        0,
        "сброс не трогает содержимое vault"
    );
}

// ---------------------------------------------------------------------------
// Phase 2 — реестр папок (T014, research.md §10)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_same_folder_is_never_registered_twice() {
    // FR-020.
    let dir = TempDir::new().unwrap();
    let vault_dir = TempDir::new().unwrap();
    let mut store = store_in(&dir).await;

    let first = store
        .add(vault_dir.path().to_path_buf(), "notes".to_string())
        .await
        .unwrap();
    let second = store
        .add(vault_dir.path().to_path_buf(), "notes".to_string())
        .await
        .unwrap();

    assert_eq!(first, second, "та же папка переиспользует запись");
    assert_eq!(store.settings().known_vaults.len(), 1);
}

#[tokio::test]
async fn two_different_folders_with_the_same_name_stay_separate() {
    // Тест регрессии идентичности (FR-020, US2 AS9): именно этот случай
    // схлопнула бы идентичность по имени или показываемому расположению.
    let dir = TempDir::new().unwrap();
    let parent = TempDir::new().unwrap();
    let work = parent.path().join("work/notes");
    let personal = parent.path().join("personal/notes");
    fs::create_dir_all(&work).unwrap();
    fs::create_dir_all(&personal).unwrap();

    let mut store = store_in(&dir).await;
    let first = store.add(work.clone(), "notes".to_string()).await.unwrap();
    let second = store
        .add(personal.clone(), "notes".to_string())
        .await
        .unwrap();

    assert_ne!(first, second, "две разные папки дают две записи");
    assert_eq!(store.settings().known_vaults.len(), 2);

    // И каждая разрешается в свою папку.
    let resolved_first = store.resolve(&first).await;
    let resolved_second = store.resolve(&second).await;
    match (resolved_first, resolved_second) {
        (Resolved::Resolved(a), Resolved::Resolved(b)) => {
            assert_eq!(a, fs::canonicalize(&work).unwrap());
            assert_eq!(b, fs::canonicalize(&personal).unwrap());
        }
        other => panic!("обе записи должны разрешаться в свои папки: {other:?}"),
    }
}

#[tokio::test]
async fn label_is_shown_instead_of_location_without_affecting_identity() {
    // FR-018, US2 AS10, research.md §14.
    let dir = TempDir::new().unwrap();
    let parent = TempDir::new().unwrap();
    let work = parent.path().join("work/notes");
    let personal = parent.path().join("personal/notes");
    fs::create_dir_all(&work).unwrap();
    fs::create_dir_all(&personal).unwrap();

    let mut store = store_in(&dir).await;
    let first = store.add(work.clone(), "notes".to_string()).await.unwrap();
    let second = store.add(personal, "notes".to_string()).await.unwrap();

    store
        .set_label(&first, Some("Работа".to_string()))
        .expect("подпись задаётся");
    // Одинаковые подписи допустимы — уникальности подпись не требует.
    store
        .set_label(&second, Some("Работа".to_string()))
        .unwrap();

    let records = store.records_with_availability().await;
    assert_eq!(records[0].display_name(), "Работа");
    assert_eq!(records[1].display_name(), "Работа");
    // Тождество от подписи не изменилось: та же папка по-прежнему находится.
    assert_eq!(store.find_existing(&work).await, Some(first));
}

#[tokio::test]
async fn forgetting_a_vault_leaves_the_folder_and_its_notes_untouched() {
    // FR-024.
    let dir = TempDir::new().unwrap();
    let vault_dir = TempDir::new().unwrap();
    fs::write(vault_dir.path().join("note.typ"), "содержимое").unwrap();

    let mut store = store_in(&dir).await;
    let id = store
        .add(vault_dir.path().to_path_buf(), "notes".to_string())
        .await
        .unwrap();

    store.forget(&id).await.expect("забывание записи");

    assert!(store.settings().known_vaults.is_empty());
    assert_eq!(store.resolve(&id).await, Resolved::Missing);
    assert_eq!(
        fs::read_to_string(vault_dir.path().join("note.typ")).unwrap(),
        "содержимое",
        "папка и заметки на диске не тронуты"
    );
}

#[tokio::test]
async fn the_last_opened_vault_survives_a_restart_and_resolves_to_the_same_folder() {
    // FR-007 фичи 001: «запоминать ранее открытые vault и предлагать открыть
    // последний при следующем запуске». Проверяется целиком тот путь, которым
    // это делают оболочки: запись о папке, отметка её текущей, перезапуск
    // (новое открытие хранилища) и разрешение ссылки обратно в папку.
    let dir = TempDir::new().unwrap();
    let vault_dir = TempDir::new().unwrap();

    let remembered = {
        let mut store = store_in(&dir).await;
        let id = store
            .add(vault_dir.path().to_path_buf(), "notes".to_string())
            .await
            .unwrap();
        assert_eq!(
            store
                .apply(&SettingsChange::current_vault(id.clone()))
                .await,
            ApplyPlan::Applied
        );
        id
    };

    // Перезапуск приложения: то же хранилище открывается заново.
    let store = store_in(&dir).await;
    assert_eq!(
        store.settings().current_vault.as_ref(),
        Some(&remembered),
        "последний открытый vault обязан пережить перезапуск"
    );
    assert_eq!(
        store.resolve(&remembered).await,
        Resolved::Resolved(fs::canonicalize(vault_dir.path()).unwrap()),
        "ссылка обязана разрешиться в ту же самую папку"
    );
}

#[tokio::test]
async fn no_service_files_appear_inside_the_vault_folder() {
    // FR-020, принцип VI: тождество устанавливается средствами платформы, а не
    // меткой-файлом внутри vault.
    let dir = TempDir::new().unwrap();
    let vault_dir = TempDir::new().unwrap();
    let mut store = store_in(&dir).await;

    let id = store
        .add(vault_dir.path().to_path_buf(), "notes".to_string())
        .await
        .unwrap();
    store
        .apply(&SettingsChange::current_vault(id.clone()))
        .await;
    store.forget(&id).await.unwrap();

    assert_eq!(
        fs::read_dir(vault_dir.path()).unwrap().count(),
        0,
        "внутри папки vault не должно появиться ни одного файла приложения"
    );
}

#[tokio::test]
async fn a_moved_folder_is_not_hunted_down_and_becomes_unavailable() {
    // FR-023, research.md §10: розыск запрещён; сохранение связи не
    // гарантируется и как гарантия не проверяется.
    let dir = TempDir::new().unwrap();
    let parent = TempDir::new().unwrap();
    let original = parent.path().join("notes");
    fs::create_dir_all(&original).unwrap();

    let mut store = store_in(&dir).await;
    let id = store
        .add(original.clone(), "notes".to_string())
        .await
        .unwrap();
    assert!(store.resolve(&id).await.is_resolved());

    fs::rename(&original, parent.path().join("notes-moved")).unwrap();

    assert_eq!(
        store.resolve(&id).await,
        Resolved::Missing,
        "перенесённая папка не разыскивается по новому расположению"
    );
    let records = store.records_with_availability().await;
    assert_eq!(
        records[0].availability,
        typsidian_core::settings::Availability::Unavailable
    );
}

#[tokio::test]
async fn folder_refs_survive_reopen() {
    // Реестр ссылок на native живёт в том же файле рядом с записями
    // (contracts/settings-store-trait.md).
    let dir = TempDir::new().unwrap();
    let vault_dir = TempDir::new().unwrap();
    let id = {
        let mut store = store_in(&dir).await;
        store
            .add(vault_dir.path().to_path_buf(), "notes".to_string())
            .await
            .unwrap()
    };

    let reopened = store_in(&dir).await;
    assert!(reopened.resolve(&id).await.is_resolved());
    assert_eq!(
        reopened
            .find_existing(&vault_dir.path().to_path_buf())
            .await,
        Some(id)
    );
}

// ---------------------------------------------------------------------------
// Phase 2 — миграция с last_vault (research.md §10)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn legacy_last_vault_is_imported_and_its_file_removed() {
    // T052, research.md §10: `vault::last_vault` был вторым источником истины и
    // объявлен только под native — его роль поглощается списком известных
    // vault, а файл удаляется, чтобы двух источников не осталось.
    let dir = TempDir::new().unwrap();
    let legacy_dir = TempDir::new().unwrap();
    let legacy = legacy_dir.path().join("last_vault.txt");
    let vault_dir = TempDir::new().unwrap();
    fs::write(&legacy, vault_dir.path().to_string_lossy().as_bytes()).unwrap();

    let store = NativeStore::open_in_with_legacy(dir.path().to_path_buf(), Some(legacy.clone()))
        .await
        .unwrap();

    assert_eq!(
        store.settings().known_vaults.len(),
        1,
        "путь из last_vault.txt становится единственной известной записью"
    );
    assert_eq!(
        store.settings().current_vault,
        Some(store.settings().known_vaults[0].id.clone()),
        "и текущим vault"
    );
    assert!(
        store
            .resolve(&store.settings().known_vaults[0].id)
            .await
            .is_resolved(),
        "импортированная запись разрешается в ту же папку"
    );
    assert!(!legacy.exists(), "исходный файл удалён");
}

#[tokio::test]
async fn migration_does_not_run_when_settings_already_know_a_vault() {
    // Импорт — разовое действие «до обновления», а не постоянный второй
    // источник: если список уже непуст, наследие не подмешивается.
    let dir = TempDir::new().unwrap();
    let legacy_dir = TempDir::new().unwrap();
    let legacy = legacy_dir.path().join("last_vault.txt");
    let existing_vault = TempDir::new().unwrap();
    let legacy_vault = TempDir::new().unwrap();
    fs::write(&legacy, legacy_vault.path().to_string_lossy().as_bytes()).unwrap();

    {
        let mut store = store_in(&dir).await;
        store
            .add(existing_vault.path().to_path_buf(), "notes".to_string())
            .await
            .unwrap();
    }

    let store = NativeStore::open_in_with_legacy(dir.path().to_path_buf(), Some(legacy.clone()))
        .await
        .unwrap();

    assert_eq!(store.settings().known_vaults.len(), 1);
    assert!(
        legacy.exists(),
        "чужой файл не трогается, если импорт не нужен"
    );
}

#[tokio::test]
async fn settings_storage_lives_outside_any_vault() {
    // FR-011: потеря хранилища настроек не влияет на содержимое vault.
    let dir = TempDir::new().unwrap();
    let vault_dir = TempDir::new().unwrap();
    fs::write(vault_dir.path().join("note.typ"), "важная заметка").unwrap();

    {
        let mut store = store_in(&dir).await;
        store
            .add(vault_dir.path().to_path_buf(), "notes".to_string())
            .await
            .unwrap();
        store.apply(&SettingsChange::theme(Theme::Dark)).await;
    }

    // Хранилище настроек удалено целиком.
    fs::remove_file(settings_path(&dir)).unwrap();

    let store = store_in(&dir).await;
    assert_eq!(
        store.settings().theme,
        Theme::Light,
        "приложение поднимается на значениях по умолчанию"
    );
    assert_eq!(
        fs::read_to_string(vault_dir.path().join("note.typ")).unwrap(),
        "важная заметка",
        "заметки читаются как прежде"
    );
}

#[tokio::test]
async fn switching_vault_does_not_change_appearance() {
    // FR-010, SC-007.
    let dir = TempDir::new().unwrap();
    let first_vault = TempDir::new().unwrap();
    let second_vault = TempDir::new().unwrap();
    let mut store = store_in(&dir).await;

    store
        .apply(&SettingsChange {
            theme: Some(Theme::Dark),
            editor_font_size: Some(20),
            ..SettingsChange::default()
        })
        .await;

    let first = store
        .add(first_vault.path().to_path_buf(), "first".to_string())
        .await
        .unwrap();
    let second = store
        .add(second_vault.path().to_path_buf(), "second".to_string())
        .await
        .unwrap();
    store.apply(&SettingsChange::current_vault(first)).await;
    store
        .apply(&SettingsChange::current_vault(second.clone()))
        .await;

    let settings = store.settings();
    assert_eq!(settings.current_vault, Some(second));
    assert_eq!(settings.theme, Theme::Dark);
    assert_eq!(settings.editor_font_size, 20);
}

// ---------------------------------------------------------------------------
// Phase 2 — повреждённое и недоступное хранилище (FR-013, FR-014)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn broken_file_falls_back_to_defaults_without_overwriting_it() {
    // FR-013, research.md §6.
    let dir = TempDir::new().unwrap();
    let path = settings_path(&dir);
    fs::write(&path, "theme = = = сломано").unwrap();

    let mut store = store_in(&dir).await;

    assert_eq!(store.storage_state(), StorageState::DefaultsAfterUnreadable);
    assert_eq!(store.settings(), &Settings::default());
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "theme = = = сломано",
        "испорченный файл не перезаписывается при открытии"
    );

    let events = store.poll_events().await;
    assert!(events.contains(&SettingsEvent::StorageUnreadable));
}

#[tokio::test]
async fn invalid_file_is_set_aside_before_the_first_write() {
    // FR-013, research.md §6: переименован рядом, пользователь уведомлён куда.
    let dir = TempDir::new().unwrap();
    let path = settings_path(&dir);
    fs::write(&path, "сломано = = =").unwrap();

    let mut store = store_in(&dir).await;
    let _ = store.poll_events().await;

    store.apply(&SettingsChange::theme(Theme::Dark)).await;

    let events = store.poll_events().await;
    let moved_to = events.iter().find_map(|event| match event {
        SettingsEvent::InvalidFileSetAside { moved_to } => Some(moved_to.clone()),
        _ => None,
    });
    let moved_to = moved_to.expect("пользователь должен узнать, куда перенесён испорченный файл");

    assert!(moved_to.exists(), "испорченный файл сохранён рядом");
    assert_eq!(fs::read_to_string(&moved_to).unwrap(), "сломано = = =");
    assert_eq!(
        format::parse(&fs::read_to_string(&path).unwrap())
            .unwrap()
            .theme,
        Theme::Dark,
        "на месте исходного файла появились рабочие настройки"
    );
}

#[tokio::test]
async fn externally_repaired_file_recovers_without_restart() {
    // FR-013: починенный руками файл возвращает приложение к нему.
    let dir = TempDir::new().unwrap();
    let path = settings_path(&dir);
    fs::write(&path, "сломано = = =").unwrap();

    let mut store = store_in(&dir).await;
    assert_eq!(store.storage_state(), StorageState::DefaultsAfterUnreadable);
    let _ = store.poll_events().await;

    fs::write(&path, "{\"theme\": \"dark\"}\n").unwrap();
    let settings = store.load().await;

    assert_eq!(settings.theme, Theme::Dark);
    assert_eq!(store.storage_state(), StorageState::Loaded);
    let events = store.poll_events().await;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SettingsEvent::StorageRecovered { .. })),
        "восстановление должно сообщаться отдельным событием: {events:?}"
    );
}

#[tokio::test]
async fn external_edit_is_picked_up_as_values_changed() {
    // FR-013, FR-006: правка руками равноправна правке через панель.
    let dir = TempDir::new().unwrap();
    let mut store = store_in(&dir).await;
    store.apply(&SettingsChange::theme(Theme::Light)).await;
    let _ = store.poll_events().await;

    let path = settings_path(&dir);
    let text = fs::read_to_string(&path).unwrap().replace("light", "dark");
    fs::write(&path, text).unwrap();

    let settings = store.load().await;
    assert_eq!(settings.theme, Theme::Dark);
    let events = store.poll_events().await;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SettingsEvent::ValuesChangedExternally { .. })),
        "внешнее изменение должно сообщаться событием: {events:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn read_only_directory_keeps_changes_for_the_session_only() {
    // FR-014: изменения действуют до конца сессии, пользователь предупреждён.
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    // Под root биты доступа не действуют — тест был бы недетерминирован.
    let probe = dir.path().join("probe");
    fs::create_dir(&probe).unwrap();
    let mut perms = fs::metadata(&probe).unwrap().permissions();
    perms.set_mode(0o555);
    fs::set_permissions(&probe, perms).unwrap();
    let bypasses = fs::write(probe.join("x"), "").is_ok();
    let mut restore = fs::metadata(&probe).unwrap().permissions();
    restore.set_mode(0o755);
    fs::set_permissions(&probe, restore).unwrap();
    fs::remove_dir_all(&probe).unwrap();
    if bypasses {
        eprintln!("пропуск read_only_directory_keeps_changes_for_the_session_only: процесс обходит права доступа");
        return;
    }

    let mut perms = fs::metadata(dir.path()).unwrap().permissions();
    perms.set_mode(0o555);
    fs::set_permissions(dir.path(), perms).unwrap();

    let mut store = NativeStore::open_in(dir.path().to_path_buf())
        .await
        .expect("нечитаемый на запись каталог не должен ломать открытие");

    assert_eq!(store.storage_state(), StorageState::ReadOnlyForSession);
    let events = store.poll_events().await;
    assert!(events.contains(&SettingsEvent::StorageBecameReadOnly));

    // Изменение всё равно применяется — до конца сессии.
    assert_eq!(
        store.apply(&SettingsChange::theme(Theme::Dark)).await,
        ApplyPlan::Applied
    );
    assert_eq!(store.settings().theme, Theme::Dark);
    assert!(!store.capabilities().writable);
    assert!(
        !settings_path(&dir).exists(),
        "на диск при этом ничего не записалось"
    );

    let mut restore = fs::metadata(dir.path()).unwrap().permissions();
    restore.set_mode(0o755);
    fs::set_permissions(dir.path(), restore).unwrap();
}

/// Ждёт события, удовлетворяющего предикату, не дольше `budget`. Возвращает
/// затраченное время — тестам US1 важен не только факт, но и бюджет SC-014.
async fn wait_for_event<F>(
    store: &mut NativeStore,
    budget: std::time::Duration,
    predicate: F,
) -> Option<std::time::Duration>
where
    F: Fn(&SettingsEvent) -> bool,
{
    let started = tokio::time::Instant::now();
    let deadline = started + budget;
    while tokio::time::Instant::now() < deadline {
        if store.poll_events().await.iter().any(&predicate) {
            return Some(started.elapsed());
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    None
}

#[tokio::test]
async fn external_change_is_reflected_within_the_budget() {
    // T026, FR-013, SC-014: правка файла сторонним редактором подхватывается
    // ≤2 с, без перезапуска и без действий пользователя.
    let dir = TempDir::new().unwrap();
    let mut store = store_in(&dir).await;
    store.apply(&SettingsChange::theme(Theme::Light)).await;
    let _ = store.poll_events().await;

    let path = settings_path(&dir);
    let text = fs::read_to_string(&path).unwrap().replace("light", "dark");
    fs::write(&path, text).unwrap();

    let elapsed = wait_for_event(&mut store, std::time::Duration::from_secs(2), |event| {
        matches!(event, SettingsEvent::ValuesChangedExternally { .. })
    })
    .await
    .expect("внешнее изменение должно быть замечено в пределах бюджета SC-014 (2 с)");

    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "обнаружение заняло {elapsed:?}, бюджет SC-014 — 2 с"
    );
    assert_eq!(
        store.settings().theme,
        Theme::Dark,
        "значение применено, а не только замечено"
    );
}

#[tokio::test]
async fn a_change_in_one_window_reaches_another_through_the_file() {
    // T043, FR-006: согласованность между окнами обеспечивается тем же
    // наблюдением за файлом — отдельного канала между окнами не существует
    // (research.md §4). Два экземпляра хранилища на одном каталоге — это и есть
    // модель двух окон.
    let dir = TempDir::new().unwrap();
    let mut window_a = store_in(&dir).await;
    let mut window_b = store_in(&dir).await;
    let _ = window_a.poll_events().await;
    let _ = window_b.poll_events().await;

    window_a.apply(&SettingsChange::theme(Theme::Dark)).await;

    let elapsed = wait_for_event(&mut window_b, std::time::Duration::from_secs(2), |event| {
        matches!(
            event,
            SettingsEvent::ValuesChangedExternally { settings } if settings.theme == Theme::Dark
        )
    })
    .await
    .expect("изменение из окна A должно дойти до окна B как обычное внешнее");

    assert!(elapsed < std::time::Duration::from_secs(2));
    assert_eq!(window_b.settings().theme, Theme::Dark);
}

#[tokio::test]
async fn a_change_takes_effect_without_any_explicit_save_step() {
    // T024, FR-002, FR-003: между действием пользователя и действующим
    // значением нет промежуточного шага — `apply` и есть всё взаимодействие.
    let dir = TempDir::new().unwrap();
    let mut store = store_in(&dir).await;

    store.apply(&SettingsChange::font_size(20)).await;

    // Значение действует немедленно…
    assert_eq!(store.settings().editor_font_size, 20);
    // …и уже лежит на диске: отдельной команды сохранения не существует.
    let on_disk = format::parse(&fs::read_to_string(settings_path(&dir)).unwrap()).unwrap();
    assert_eq!(on_disk.editor_font_size, 20);
}

#[tokio::test]
async fn store_never_writes_inside_the_vault_directory() {
    // FR-011: хранилище настроек лежит вне любого vault.
    let dir = TempDir::new().unwrap();
    let store = store_in(&dir).await;
    assert!(
        store.settings_path().starts_with(dir.path()),
        "файл настроек лежит в каталоге настроек, а не в vault"
    );
    assert!(Path::new(store.settings_path())
        .file_name()
        .is_some_and(|name| name == SETTINGS_FILE_NAME));
}

// ---------------------------------------------------------------------------
// Фича 010 — язык интерфейса (T008/T009): общие проверки из common/language_cases.rs
// ---------------------------------------------------------------------------

mod common;
use common::language_cases as language;

#[test]
fn language_round_trips_through_the_file() {
    language::language_round_trips_through_the_file();
}

#[test]
fn unrecognised_tag_survives_writing_another_value() {
    language::unrecognised_tag_survives_writing_another_value();
}

#[test]
fn malformed_tag_in_file_keeps_other_values() {
    language::malformed_tag_in_file_keeps_other_values();
}

#[test]
fn invalid_tag_rejects_the_whole_change() {
    language::invalid_tag_rejects_the_whole_change();
}

#[test]
fn stored_malformed_tag_does_not_block_other_changes() {
    language::stored_malformed_tag_does_not_block_other_changes();
}

#[test]
fn language_goes_to_the_device_even_with_an_open_vault() {
    language::language_goes_to_the_device_even_with_an_open_vault();
}

#[test]
fn language_and_theme_split_between_stores() {
    language::language_and_theme_split_between_stores();
}

#[test]
fn reset_returns_to_the_system_language() {
    language::reset_returns_to_the_system_language();
}

#[test]
fn language_choice_json_shape() {
    language::language_choice_json_shape();
}

#[tokio::test]
async fn chosen_language_survives_reopen_and_reset_clears_it() {
    // FR-015, FR-017: выбор переживает перезапуск; сброс — «как в системе».
    use typsidian_core::settings::apply::LanguageChoice;
    let dir = TempDir::new().unwrap();
    {
        let mut store = store_in(&dir).await;
        let change = SettingsChange::language(LanguageChoice::Tag("en".to_string()));
        assert_eq!(store.apply(&change).await, ApplyPlan::Applied);
    }
    let mut reopened = store_in(&dir).await;
    assert_eq!(reopened.settings().language.as_deref(), Some("en"));

    reopened.apply(&reset_to_defaults()).await;
    let on_disk = fs::read_to_string(settings_path(&dir)).unwrap();
    assert!(!on_disk.contains("language"), "{on_disk}");
}

#[tokio::test]
async fn the_journal_records_events_without_values_and_survives_a_restart() {
    // 004 FR-040, FR-041, FR-043 (T093, T112): журнал пишется рядом с
    // настройками, называет событие и поле, но не значение и не путь, и
    // продолжается после перезапуска.
    let dir = TempDir::new().unwrap();
    let vault = TempDir::new().unwrap();
    {
        let mut store = store_in(&dir).await;
        store.apply(&SettingsChange::theme(Theme::Dark)).await;
        store
            .add(
                vault.path().to_path_buf(),
                vault.path().to_string_lossy().into_owned(),
            )
            .await
            .unwrap();

        let journal = store.journal_text();
        assert!(journal.contains("SettingsLoaded"), "{journal}");
        assert!(journal.contains("SettingsChanged"), "{journal}");
        assert!(journal.contains("theme"), "{journal}");
        assert!(journal.contains("VaultAdded"), "{journal}");
        // Ни значения, ни пути: только имена событий и полей.
        assert!(
            !journal.contains("Dark") && !journal.contains("dark"),
            "{journal}"
        );
        assert!(
            !journal.contains(&*vault.path().to_string_lossy()),
            "{journal}"
        );
    }

    let on_disk = fs::read_to_string(dir.path().join("typsidian.log")).unwrap();
    assert!(on_disk.contains("VaultAdded"));

    let reopened = store_in(&dir).await;
    let journal = reopened.journal_text();
    assert!(journal.contains("VaultAdded"), "прежние записи на месте");
    assert_eq!(journal.matches("SettingsLoaded").count(), 2);
}

// ---------------------------------------------------------------------------
// Фича 013 — состояние колонки заметки (T010): common/note_panel_cases.rs
// ---------------------------------------------------------------------------

use common::note_panel_cases as note_panel;

#[test]
fn note_panel_defaults_are_hidden_links_collapsed() {
    note_panel::defaults_are_hidden_links_collapsed();
}

#[test]
fn note_panel_state_round_trips_through_the_file() {
    note_panel::state_round_trips_through_the_file();
}

#[test]
fn note_panel_bad_value_does_not_break_the_rest() {
    note_panel::a_bad_value_does_not_break_the_rest();
}

#[test]
fn note_panel_unknown_mode_reads_as_links() {
    note_panel::an_unknown_mode_reads_as_links();
}

#[test]
fn note_panel_goes_to_the_device_even_with_an_open_vault() {
    note_panel::the_state_goes_to_the_device_even_with_an_open_vault();
}

#[test]
fn note_panel_reset_does_not_touch_the_state() {
    note_panel::reset_does_not_touch_the_state();
}

#[test]
fn note_panel_state_json_shape() {
    note_panel::state_json_shape();
}

#[test]
fn note_panel_outline_mode_and_kinds_round_trip() {
    note_panel::the_outline_mode_and_kinds_round_trip();
}

#[test]
fn note_panel_properties_mode_round_trips() {
    note_panel::the_properties_mode_round_trips();
}

#[test]
fn note_panel_graph_mode_round_trips() {
    note_panel::the_graph_mode_round_trips();
}

#[test]
fn note_panel_outline_state_json_shape() {
    note_panel::the_outline_state_json_shape();
}

#[test]
fn note_panel_outline_kinds_are_tolerant_on_read() {
    note_panel::the_outline_kinds_are_tolerant_on_read();
}

// Фича 014 (T004, T055): common/vault_settings_cases.rs
use common::vault_settings_cases as workspace_settings;

#[test]
fn a_punctuation_shortcut_carries_the_key_code() {
    workspace_settings::a_punctuation_shortcut_carries_the_key_code();
}

// Фича 015 (T007).
#[test]
fn the_outline_command_has_its_name_and_default() {
    workspace_settings::the_outline_command_has_its_name_and_default();
}

// Фича 019 (T007).
#[test]
fn the_start_show_command_has_its_name_and_default() {
    workspace_settings::the_start_show_command_has_its_name_and_default();
}

// Фича 022 (T008).
#[test]
fn the_today_command_has_its_name_and_default() {
    workspace_settings::the_today_command_has_its_name_and_default();
}

#[test]
fn a_workspace_change_replaces_only_its_own_vault() {
    workspace_settings::a_workspace_change_replaces_only_its_own_vault();
}

#[test]
fn a_workspace_change_goes_to_the_device() {
    workspace_settings::a_workspace_change_goes_to_the_device();
}

#[test]
fn the_workspaces_key_round_trips_and_tolerates_garbage() {
    workspace_settings::the_workspaces_key_round_trips_and_tolerates_garbage();
}

#[test]
fn a_workspace_change_is_not_journaled() {
    workspace_settings::a_workspace_change_is_not_journaled();
}

#[test]
fn note_panel_sources_mode_and_uncited_flag_round_trip() {
    note_panel::the_sources_mode_and_uncited_flag_round_trip();
}

// Фича 018 — дополнение при наборе и команды (T024): common/completion_cases.rs

#[test]
fn completion_setting_defaults_to_on_and_lives_on_the_device() {
    common::completion_cases::completion_setting_defaults_to_on_and_lives_on_the_device();
}

#[test]
fn completion_commands_have_free_defaults() {
    common::completion_cases::completion_commands_have_free_defaults();
}
