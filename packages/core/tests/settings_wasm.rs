//! wasm-bindgen-test — только wasm32-unknown-unknown (принцип II).
//!
//! В отличие от тестов vault, здесь хранилище проверяется **по-настоящему**:
//! OPFS доступен в headless-браузере без пользовательского жеста, поэтому
//! `WebStore` открывается, пишет и перечитывает реальный файл. Жеста требует
//! только выбор папки vault (`showDirectoryPicker`), поэтому реестр папок
//! проверяется на уровне общих правил, а сквозной прогон — в quickstart (T097).
#![cfg(target_arch = "wasm32")]

use typsidian_core::settings::apply::{reset_to_defaults, ApplyPlan, RejectReason, SettingsChange};
use typsidian_core::settings::events::SettingsEvent;
use typsidian_core::settings::format::{self, FolderRefs};
use typsidian_core::settings::store::{SettingsStore, StorageState, SETTINGS_FILE_NAME};
use typsidian_core::settings::web::WebStore;
use typsidian_core::settings::{Settings, Theme, VaultId, VaultRecord};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// OPFS — общее хранилище на всё происхождение, и тесты в одном браузере его
/// разделяют. Каждый тест начинает с заведомо известного состояния.
async fn fresh_store() -> WebStore {
    WebStore::overwrite_raw_for_test("")
        .await
        .expect("сброс файла настроек");
    <WebStore as SettingsStore>::open()
        .await
        .expect("открытие хранилища в OPFS")
}

// ---------------------------------------------------------------------------
// Формат файла (T018) — общий код обоих таргетов
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn what_the_user_wrote_by_hand_survives_an_application_write() {
    // Тот же тест, что и в settings_native.rs: правило одно на оба таргета
    // (research.md §2), и расходиться оно не имеет права. Комментариев в JSON
    // нет, поэтому проверяется то, что осталось от требования, — дописанное
    // пользователем не выбрасывается (FR-013).
    let authored = r#"{
  "theme": "dark",
  "editor_font_size": 18,
  "my_future_option": {"nested": true}
}"#;
    let mut settings = format::parse(authored).expect("файл должен разбираться");
    settings.editor_font_size = 21;

    let rewritten = format::render(authored, &settings, &FolderRefs::new());

    assert!(rewritten.contains("my_future_option"));
    assert!(rewritten.contains("nested"));
    assert_eq!(format::parse(&rewritten).unwrap().editor_font_size, 21);
    assert_eq!(
        format::parse(&rewritten).unwrap().theme,
        typsidian_core::settings::Theme::Dark
    );
}

#[wasm_bindgen_test]
fn the_key_order_is_stable_between_writes() {
    // Порядок задаёт приложение, но он обязан быть одинаковым от записи к
    // записи — иначе правка одной настройки давала бы diff на весь файл.
    let settings = typsidian_core::settings::Settings::default();
    let first = format::render("", &settings, &FolderRefs::new());
    let second = format::render(&first, &settings, &FolderRefs::new());
    assert_eq!(first, second);
}

#[wasm_bindgen_test]
fn unknown_keys_survive_and_are_not_an_error() {
    // FR-013: файл принадлежит пользователю.
    let text = r#"{"theme": "dark", "my_future_option": 42}"#;
    let settings = format::parse(text).expect("неизвестный ключ не ошибка");
    let rewritten = format::render(text, &settings, &FolderRefs::new());
    assert!(rewritten.contains("my_future_option"));
}

// ---------------------------------------------------------------------------
// Паритет open/load/apply (T021) — на реальном OPFS
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
async fn open_on_empty_storage_starts_from_defaults() {
    let store = fresh_store().await;
    assert_eq!(store.settings(), &Settings::default());
    assert_eq!(store.storage_state(), StorageState::Loaded);
}

#[wasm_bindgen_test]
async fn apply_persists_values_and_they_survive_reopen() {
    // FR-002, FR-007 — тот же результат, что и на native.
    let mut store = fresh_store().await;
    assert_eq!(
        store.apply(&SettingsChange::theme(Theme::Dark)).await,
        ApplyPlan::Applied
    );

    let reopened = <WebStore as SettingsStore>::open()
        .await
        .expect("повторное открытие");
    assert_eq!(reopened.settings().theme, Theme::Dark);
}

#[wasm_bindgen_test]
async fn out_of_range_font_size_is_rejected_with_the_same_reason_as_native() {
    // FR-016, research.md §11: одинаковые отказы на обоих таргетах.
    let mut store = fresh_store().await;
    for value in [7u8, 33u8] {
        assert_eq!(
            store.apply(&SettingsChange::font_size(value)).await,
            ApplyPlan::Rejected {
                reason: RejectReason::ValueOutOfRange
            }
        );
    }
    for value in [8u8, 32u8] {
        assert_eq!(
            store.apply(&SettingsChange::font_size(value)).await,
            ApplyPlan::Applied
        );
    }
    assert_eq!(store.settings().editor_font_size, 32);
}

#[wasm_bindgen_test]
async fn write_is_atomic_and_file_is_always_parseable() {
    // research.md §5: `FileSystemWritableFileStream` фиксирует изменения только
    // при close(), поэтому обрезанного JSON не бывает.
    let mut store = fresh_store().await;
    for size in [10u8, 11, 12, 13, 14] {
        store.apply(&SettingsChange::font_size(size)).await;
    }
    let raw = WebStore::read_raw_for_test()
        .await
        .expect("файл настроек должен существовать");
    assert!(format::parse(&raw).is_ok());
    assert_eq!(format::parse(&raw).unwrap().editor_font_size, 14);
}

#[wasm_bindgen_test]
async fn capabilities_express_platform_limits_as_values_not_missing_methods() {
    // research.md §13, принцип II: форма API одна, различается значение.
    let store = fresh_store().await;
    let capabilities = store.capabilities();

    assert!(
        !capabilities.can_reveal,
        "в вебе раскрытие средствами ОС невозможно"
    );
    assert!(capabilities.location.contains(SETTINGS_FILE_NAME));
    // Метод существует и честно отказывает, а не отсутствует.
    assert!(SettingsStore::reveal(&store).is_err());
}

#[wasm_bindgen_test]
async fn broken_file_falls_back_to_defaults_without_overwriting_it() {
    // FR-013, research.md §6.
    WebStore::overwrite_raw_for_test("сломано = = =")
        .await
        .unwrap();
    let mut store = <WebStore as SettingsStore>::open().await.unwrap();

    assert_eq!(store.storage_state(), StorageState::DefaultsAfterUnreadable);
    assert_eq!(store.settings(), &Settings::default());
    assert_eq!(
        WebStore::read_raw_for_test().await.as_deref(),
        Some("сломано = = ="),
        "испорченный файл не перезаписывается при открытии"
    );

    let events = store.poll_events().await;
    assert!(events.contains(&SettingsEvent::StorageUnreadable));
}

#[wasm_bindgen_test]
async fn invalid_file_is_set_aside_before_the_first_write() {
    // FR-013, research.md §6 — то же правило, что и на native.
    WebStore::overwrite_raw_for_test("тоже сломано = = =")
        .await
        .unwrap();
    let mut store = <WebStore as SettingsStore>::open().await.unwrap();
    let _ = store.poll_events().await;

    store.apply(&SettingsChange::theme(Theme::Dark)).await;

    let events = store.poll_events().await;
    let moved_to = events
        .iter()
        .find_map(|event| match event {
            SettingsEvent::InvalidFileSetAside { moved_to } => Some(moved_to.clone()),
            _ => None,
        })
        .expect("пользователь должен узнать, куда перенесён испорченный файл");

    assert!(
        WebStore::set_aside_exists_for_test(&moved_to.to_string_lossy()).await,
        "копия испорченного файла сохранена рядом"
    );
    let raw = WebStore::read_raw_for_test().await.unwrap();
    assert_eq!(format::parse(&raw).unwrap().theme, Theme::Dark);
}

#[wasm_bindgen_test]
async fn reset_keeps_known_vaults_and_returns_values_to_defaults() {
    // FR-037, FR-038, SC-011 — паритет с native.
    let mut store = fresh_store().await;
    store.apply(&SettingsChange::theme(Theme::Dark)).await;

    assert_eq!(store.apply(&reset_to_defaults()).await, ApplyPlan::Applied);
    assert_eq!(store.settings().theme, Theme::Light);
}

#[wasm_bindgen_test]
async fn external_change_from_another_tab_is_picked_up() {
    // FR-006, FR-013: единственный источник внешнего изменения в вебе — другая
    // вкладка того же происхождения (research.md §3). Здесь она моделируется
    // прямой записью в тот же файл OPFS.
    let mut store = fresh_store().await;
    store.apply(&SettingsChange::theme(Theme::Light)).await;
    let _ = store.poll_events().await;

    WebStore::overwrite_raw_for_test("{\"theme\": \"dark\"}\n")
        .await
        .unwrap();

    let settings = store.load().await;
    assert_eq!(settings.theme, Theme::Dark);
    let events = store.poll_events().await;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SettingsEvent::ValuesChangedExternally { .. })),
        "внешнее изменение сообщается тем же событием, что и на native: {events:?}"
    );
}

#[wasm_bindgen_test]
async fn externally_repaired_file_recovers_without_restart() {
    // FR-013.
    WebStore::overwrite_raw_for_test("опять сломано = = =")
        .await
        .unwrap();
    let mut store = <WebStore as SettingsStore>::open().await.unwrap();
    assert_eq!(store.storage_state(), StorageState::DefaultsAfterUnreadable);
    let _ = store.poll_events().await;

    WebStore::overwrite_raw_for_test("{\"theme\": \"dark\"}\n")
        .await
        .unwrap();
    let settings = store.load().await;

    assert_eq!(settings.theme, Theme::Dark);
    assert_eq!(store.storage_state(), StorageState::Loaded);
    let events = store.poll_events().await;
    assert!(events
        .iter()
        .any(|event| matches!(event, SettingsEvent::StorageRecovered { .. })));
}

// ---------------------------------------------------------------------------
// Правила списка vault — общий код, платформенным остаётся только сравнение
// самих папок (research.md §10)
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn two_records_with_identical_location_stay_separate() {
    // Регрессия research.md §10: в вебе показывается только имя папки, поэтому
    // сравнение по нему схлопнуло бы две разные папки `notes` в одну запись.
    let first = VaultRecord::new(VaultId::from_raw("a"), "notes".to_string());
    let second = VaultRecord::new(VaultId::from_raw("b"), "notes".to_string());
    let settings = Settings {
        known_vaults: vec![first, second],
        ..Settings::default()
    };
    assert!(typsidian_core::settings::validate(&settings).is_ok());
    assert_eq!(settings.known_vaults.len(), 2);
}

#[wasm_bindgen_test]
fn the_same_record_is_never_added_twice() {
    // T045, FR-020. Само сравнение папок здесь не участвует: решение «та же ли
    // это папка» принимает платформа (`isSameEntry`) до вызова, а список лишь
    // обязан не заводить вторую запись под уже выданным идентификатором.
    let mut records = vec![VaultRecord::new(
        VaultId::from_raw("a"),
        "notes".to_string(),
    )];
    typsidian_core::settings::vaults::upsert(
        &mut records,
        VaultRecord::new(VaultId::from_raw("a"), "notes".to_string()),
    );
    assert_eq!(records.len(), 1);
}

#[wasm_bindgen_test]
fn forgetting_a_record_removes_it_and_nothing_else() {
    // T048, FR-024: убирается запись, папка на диске не трогается — сюда она и
    // не попадает, ссылка живёт в реестре IndexedDB.
    let mut records = vec![
        VaultRecord::new(VaultId::from_raw("a"), "work".to_string()),
        VaultRecord::new(VaultId::from_raw("b"), "personal".to_string()),
    ];
    assert!(typsidian_core::settings::vaults::forget(
        &mut records,
        &VaultId::from_raw("a")
    ));
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, VaultId::from_raw("b"));
}

#[wasm_bindgen_test]
fn switching_vault_changes_nothing_but_the_current_vault() {
    // T049, FR-010, SC-007. Проверяется на уровне набора изменений, а не через
    // `WebStore`: чтобы положить запись в `known_vaults` на вебе, нужен живой
    // `FileSystemDirectoryHandle`, а он выдаётся только по жесту пользователя.
    // Само правило «переключение не трогает внешний вид и режим» — общий код
    // обоих таргетов, и проверяется здесь именно оно.
    let first = VaultRecord::new(VaultId::from_raw("a"), "work".to_string());
    let second = VaultRecord::new(VaultId::from_raw("b"), "personal".to_string());
    let before = Settings {
        theme: Theme::Dark,
        editor_font_size: 20,
        known_vaults: vec![first, second],
        current_vault: Some(VaultId::from_raw("a")),
        language: Some("en".to_string()),
        ..Settings::default()
    };

    let after = SettingsChange::current_vault(VaultId::from_raw("b")).apply_to(&before);

    assert_eq!(after.current_vault, Some(VaultId::from_raw("b")));
    assert_eq!(after.theme, before.theme);
    assert_eq!(after.editor_font_size, before.editor_font_size);
    assert_eq!(after.language, before.language);
    assert_eq!(after.known_vaults, before.known_vaults);
    assert!(typsidian_core::settings::validate(&after).is_ok());
}

#[wasm_bindgen_test]
fn switching_to_an_unknown_vault_is_rejected() {
    // Инвариант из data-model.md: `current_vault` всегда указывает на запись из
    // списка либо отсутствует.
    let before = Settings::default();
    let after = SettingsChange::current_vault(VaultId::from_raw("missing")).apply_to(&before);
    assert!(typsidian_core::settings::validate(&after).is_err());
}

#[wasm_bindgen_test]
fn label_is_shown_instead_of_location_and_need_not_be_unique() {
    // FR-018, research.md §14.
    let mut first = VaultRecord::new(VaultId::from_raw("a"), "notes".to_string());
    let mut second = VaultRecord::new(VaultId::from_raw("b"), "notes".to_string());
    first.label = Some("Работа".to_string());
    second.label = Some("Работа".to_string());

    assert_eq!(first.display_name(), "Работа");
    assert_eq!(second.display_name(), "Работа");
    let settings = Settings {
        known_vaults: vec![first, second],
        ..Settings::default()
    };
    assert!(typsidian_core::settings::validate(&settings).is_ok());
}

// ---------------------------------------------------------------------------
// Биндинги сброса и диагностики (T108, T109, T113)
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn reset_binding_returns_the_same_change_as_the_core() {
    // FR-037. Биндинг обязан отдавать ровно тот набор, что собирает ядро:
    // если UI когда-нибудь начнёт строить его сам, эти два значения разойдутся,
    // и умолчания появятся во втором месте (принцип I).
    assert_eq!(WebStore::reset_to_defaults_js(), reset_to_defaults());
}

#[wasm_bindgen_test]
fn reset_binding_leaves_the_current_vault_alone() {
    // FR-038, SC-011: сброс не закрывает открытый vault и не трогает список
    // известных. Проверяется на самом наборе, а не на последствиях: поле,
    // которого в наборе нет, применить невозможно.
    assert!(WebStore::reset_to_defaults_js().current_vault.is_none());
}

#[wasm_bindgen_test]
async fn reset_through_apply_returns_values_to_defaults_and_keeps_known_vaults() {
    // Ключевая проверка: сброс проходит тем же `apply`, что и обычное
    // изменение, а не отдельным путём (FR-037).
    let mut store = fresh_store().await;
    let mut known = VaultRecord::new(VaultId::from_raw("kept"), "notes".to_string());
    known.label = Some("Работа".to_string());

    let seeded = Settings {
        theme: Theme::Dark,
        editor_font_size: 20,
        known_vaults: vec![known.clone()],
        ..Settings::default()
    };
    store
        .apply(&SettingsChange {
            theme: Some(seeded.theme),
            editor_font_size: Some(seeded.editor_font_size),
            current_vault: None,
            language: None,
            ..SettingsChange::default()
        })
        .await;

    let plan = store.apply(&WebStore::reset_to_defaults_js()).await;
    assert!(matches!(plan, ApplyPlan::Applied { .. }), "план: {plan:?}");

    let after = SettingsStore::settings(&store);
    assert_eq!(after.theme, Settings::default().theme);
    assert_eq!(after.editor_font_size, Settings::default().editor_font_size);
}

#[wasm_bindgen_test]
async fn diagnostics_report_the_web_target() {
    // FR-039: значение вычисляет ядро, а не называет интерфейс.
    let store = fresh_store().await;
    let report = store.diagnostics_js();

    assert!(!report.app_version.is_empty());
    assert_eq!(report.environment.target, "web");
}

// ---------------------------------------------------------------------------
// Фича 010 — язык интерфейса (T008/T009): общие проверки из common/language_cases.rs
// ---------------------------------------------------------------------------

mod common;
use common::language_cases as language;

#[wasm_bindgen_test]
fn language_round_trips_through_the_file() {
    language::language_round_trips_through_the_file();
}

#[wasm_bindgen_test]
fn unrecognised_tag_survives_writing_another_value() {
    language::unrecognised_tag_survives_writing_another_value();
}

#[wasm_bindgen_test]
fn malformed_tag_in_file_keeps_other_values() {
    language::malformed_tag_in_file_keeps_other_values();
}

#[wasm_bindgen_test]
fn invalid_tag_rejects_the_whole_change() {
    language::invalid_tag_rejects_the_whole_change();
}

#[wasm_bindgen_test]
fn stored_malformed_tag_does_not_block_other_changes() {
    language::stored_malformed_tag_does_not_block_other_changes();
}

#[wasm_bindgen_test]
fn language_goes_to_the_device_even_with_an_open_vault() {
    language::language_goes_to_the_device_even_with_an_open_vault();
}

#[wasm_bindgen_test]
fn language_and_theme_split_between_stores() {
    language::language_and_theme_split_between_stores();
}

#[wasm_bindgen_test]
fn reset_returns_to_the_system_language() {
    language::reset_returns_to_the_system_language();
}

#[wasm_bindgen_test]
fn language_choice_json_shape() {
    language::language_choice_json_shape();
}

#[wasm_bindgen_test]
async fn chosen_language_survives_reopen_and_reset_clears_it() {
    // FR-015, FR-017 — тот же сценарий, что в settings_native.rs, поверх OPFS.
    use typsidian_core::settings::apply::LanguageChoice;
    let mut store = fresh_store().await;
    let change = SettingsChange::language(LanguageChoice::Tag("en".to_string()));
    assert_eq!(store.apply(&change).await, ApplyPlan::Applied);
    let mut reopened = <WebStore as SettingsStore>::open().await.unwrap();
    assert_eq!(reopened.settings().language.as_deref(), Some("en"));

    reopened.apply(&reset_to_defaults()).await;
    let again = <WebStore as SettingsStore>::open().await.unwrap();
    assert_eq!(again.settings().language, None);
}

#[wasm_bindgen_test]
async fn the_journal_records_events_without_values_and_survives_a_restart() {
    // 004 FR-040, FR-041 — тот же журнал, что и на native, только в OPFS.
    // Файлы журнала переживают соседние тесты, поэтому проверяется прирост.
    let mut store = fresh_store().await;
    let before = store.journal_text().matches("SettingsChanged").count();
    store.apply(&SettingsChange::theme(Theme::Dark)).await;

    let journal = store.journal_text();
    assert_eq!(journal.matches("SettingsChanged").count(), before + 1);
    assert!(journal.contains("theme"));
    assert!(!journal.contains("Dark") && !journal.contains("dark"));

    let reopened = <WebStore as SettingsStore>::open()
        .await
        .expect("повторное открытие");
    assert_eq!(
        reopened.journal_text().matches("SettingsChanged").count(),
        before + 1
    );
}

// ---------------------------------------------------------------------------
// Фича 013 — состояние колонки заметки (T010): common/note_panel_cases.rs
// ---------------------------------------------------------------------------

use common::note_panel_cases as note_panel;

#[wasm_bindgen_test]
fn note_panel_defaults_are_hidden_links_collapsed() {
    note_panel::defaults_are_hidden_links_collapsed();
}

#[wasm_bindgen_test]
fn note_panel_state_round_trips_through_the_file() {
    note_panel::state_round_trips_through_the_file();
}

#[wasm_bindgen_test]
fn note_panel_bad_value_does_not_break_the_rest() {
    note_panel::a_bad_value_does_not_break_the_rest();
}

#[wasm_bindgen_test]
fn note_panel_unknown_mode_reads_as_links() {
    note_panel::an_unknown_mode_reads_as_links();
}

#[wasm_bindgen_test]
fn note_panel_goes_to_the_device_even_with_an_open_vault() {
    note_panel::the_state_goes_to_the_device_even_with_an_open_vault();
}

#[wasm_bindgen_test]
fn note_panel_reset_does_not_touch_the_state() {
    note_panel::reset_does_not_touch_the_state();
}

#[wasm_bindgen_test]
fn note_panel_state_json_shape() {
    note_panel::state_json_shape();
}

#[wasm_bindgen_test]
fn note_panel_outline_mode_and_kinds_round_trip() {
    note_panel::the_outline_mode_and_kinds_round_trip();
}

#[wasm_bindgen_test]
fn note_panel_properties_mode_round_trips() {
    note_panel::the_properties_mode_round_trips();
}

#[wasm_bindgen_test]
fn note_panel_graph_mode_round_trips() {
    note_panel::the_graph_mode_round_trips();
}

#[wasm_bindgen_test]
fn note_panel_outline_state_json_shape() {
    note_panel::the_outline_state_json_shape();
}

#[wasm_bindgen_test]
fn note_panel_outline_kinds_are_tolerant_on_read() {
    note_panel::the_outline_kinds_are_tolerant_on_read();
}

// Фича 014 (T004, T055): common/vault_settings_cases.rs
use common::vault_settings_cases as workspace_settings;

#[wasm_bindgen_test]
fn a_punctuation_shortcut_carries_the_key_code() {
    workspace_settings::a_punctuation_shortcut_carries_the_key_code();
}

// Фича 015 (T007).
#[wasm_bindgen_test]
fn the_outline_command_has_its_name_and_default() {
    workspace_settings::the_outline_command_has_its_name_and_default();
}

// Фича 019 (T007).
#[wasm_bindgen_test]
fn the_start_show_command_has_its_name_and_default() {
    workspace_settings::the_start_show_command_has_its_name_and_default();
}

// Фича 022 (T008).
#[wasm_bindgen_test]
fn the_today_command_has_its_name_and_default() {
    workspace_settings::the_today_command_has_its_name_and_default();
}

#[wasm_bindgen_test]
fn a_workspace_change_replaces_only_its_own_vault() {
    workspace_settings::a_workspace_change_replaces_only_its_own_vault();
}

#[wasm_bindgen_test]
fn a_workspace_change_goes_to_the_device() {
    workspace_settings::a_workspace_change_goes_to_the_device();
}

#[wasm_bindgen_test]
fn the_workspaces_key_round_trips_and_tolerates_garbage() {
    workspace_settings::the_workspaces_key_round_trips_and_tolerates_garbage();
}

#[wasm_bindgen_test]
fn a_workspace_change_is_not_journaled() {
    workspace_settings::a_workspace_change_is_not_journaled();
}

#[wasm_bindgen_test]
fn note_panel_sources_mode_and_uncited_flag_round_trip() {
    note_panel::the_sources_mode_and_uncited_flag_round_trip();
}

// Фича 018 — дополнение при наборе и команды (T024): common/completion_cases.rs

#[wasm_bindgen_test]
fn completion_setting_defaults_to_on_and_lives_on_the_device() {
    common::completion_cases::completion_setting_defaults_to_on_and_lives_on_the_device();
}

#[wasm_bindgen_test]
fn completion_commands_have_free_defaults() {
    common::completion_cases::completion_commands_have_free_defaults();
}
