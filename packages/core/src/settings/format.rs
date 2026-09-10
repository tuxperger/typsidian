//! Чтение и запись файла настроек — JSON (research.md §2, пересмотрено).
//!
//! # Почему JSON, хотя research.md §2 выбирал TOML
//!
//! Исходное решение фичи 004 выбрало TOML ради **комментариев**: FR-036
//! приглашает открыть файл, FR-013 делает правку руками равноправной правке
//! через панель, а обычная serde-сериализация стирала бы то, что пользователь
//! написал сам. Формат сменён по прямому решению владельца проекта; цена
//! известна и здесь не замалчивается:
//!
//! - **комментариев в файле больше нет** — JSON их не поддерживает вовсе;
//! - **порядок ключей задаёт приложение**, а не пользователь: он
//!   алфавитный и детерминированный (см. ниже про `preserve_order`).
//!
//! Что при этом **сохранено** из прежних гарантий:
//!
//! - неизвестные ключи не являются ошибкой разбора и переживают запись
//!   приложения — то, что пользователь дописал руками, не выбрасывается
//!   (FR-013);
//! - сломанный синтаксис называется ошибкой, а не подменяется молча
//!   умолчаниями;
//! - отсутствующие ключи берут значения по умолчанию.
//!
//! # Переноса с прежнего формата нет
//!
//! Тоже по прямому решению владельца проекта. `settings.toml`, написанный
//! прежней версией, не читается: приложение поднимается на умолчаниях и создаёт
//! `settings.json` рядом. Прежний файл при этом **не трогается** — ни удаления,
//! ни переименования: перенести значения руками должно быть возможно.
//!
//! Это наблюдаемая потеря настроек при обновлении, и прятать её за словом
//! «формат» нельзя. Названа она и в `004/research.md §2`, и в
//! `004/quickstart.md` отдельным сценарием.
//!
//! # `preserve_order` у `serde_json` включать НЕЛЬЗЯ
//!
//! Соблазн очевиден: с этой feature `Map` сохранял бы порядок ключей
//! пользователя. Но `serde_json` в проекте общий, а `boards::digest`
//! рассчитывает ровно на обратное — что `Map` **сортирует** ключи, и именно это
//! даёт канонический порядок при нормализации доски без отдельного кода
//! (`packages/core/Cargo.toml`, фича 002 research.md §5, §9). Включение feature
//! изменило бы хэш каждой доски в проекте и уронило бы эталонный
//! `REFERENCE_DIGEST` в `boards_native.rs`/`boards_wasm.rs`.
//!
//! Поэтому порядок ключей здесь алфавитный — свойство `BTreeMap` под `Map`, а
//! не выбор. Файл от записи к записи стабилен, и это главное.

use super::{Settings, VaultRecord};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Ссылки на папки vault, пригодные к сериализации: `VaultId` → путь.
///
/// Заполняется **только на native** (research.md §10): в вебе ссылка — это
/// `FileSystemDirectoryHandle`, который в JSON не сериализуется, и её реестр
/// живёт в IndexedDB. Формат файла при этом один на оба таргета — веб просто
/// не пишет этот объект.
pub type FolderRefs = BTreeMap<String, String>;

/// Имя объекта со ссылками на папки.
const FOLDER_REFS_KEY: &str = "folder_refs";

/// Ключ с подсказкой о том, что файл можно править руками.
///
/// Заменяет комментарий-шапку, которого в JSON быть не может. Обычный ключ, а
/// не магия: пользователь волен его удалить, приложение не станет его
/// восстанавливать — [`render`] его не пишет, [`initial_document`] пишет один
/// раз при создании файла.
const HINT_KEY: &str = "_";

/// Текст подсказки в новом файле.
const HINT: &str = "Настройки Typsidian. Файл можно править вручную — приложение подхватит изменения без перезапуска. Эту строку можно удалить.";

/// Не удалось разобрать файл настроек. Само по себе не ошибка приложения:
/// вызывающая сторона поднимается на значениях по умолчанию (FR-013).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("файл настроек не разбирается: {message}")]
pub struct ParseError {
    pub message: String,
}

/// Разбирает содержимое файла настроек.
///
/// Неизвестные ключи не являются ошибкой: пользователь мог оставить их от
/// будущей или прошлой версии, и отказ разбирать файл из-за лишней строки
/// противоречил бы FR-013.
pub fn parse(text: &str) -> Result<Settings, ParseError> {
    // Пустой файл — это `Settings::default()`, а не синтаксическая ошибка:
    // так ведёт себя и только что созданный файл нулевой длины после сбоя
    // записи, и он не должен объявляться повреждённым.
    if text.trim().is_empty() {
        return Ok(Settings::default());
    }
    serde_json::from_str::<Settings>(text).map_err(|error| ParseError {
        message: error.to_string(),
    })
}

/// Записывает значения поверх существующего текста.
///
/// `previous` — прежнее содержимое файла (пустая строка, если файла ещё нет).
/// Возвращает новый текст целиком: запись на диск атомарна и выполняется
/// реализацией хранилища (research.md §5).
///
/// Ключи, которых приложение не знает, **переносятся как есть**: разбор идёт
/// через `serde_json::Value`, а не через `Settings`, и всё лишнее остаётся в
/// объекте нетронутым.
pub fn render(previous: &str, settings: &Settings, folder_refs: &FolderRefs) -> String {
    let mut document = match serde_json::from_str::<Value>(previous) {
        Ok(Value::Object(map)) => map,
        // Не объект или не разбирается — начинаем с чистого: сохранять нечего.
        _ => Map::new(),
    };

    document.insert("theme".into(), Value::from(theme_str(settings.theme)));
    document.insert(
        "editor_font_size".into(),
        Value::from(settings.editor_font_size),
    );
    document.insert(
        "editor_mode_preference".into(),
        Value::from(mode_str(settings.editor_mode_preference)),
    );

    set_or_remove(
        &mut document,
        "current_vault",
        settings
            .current_vault
            .as_ref()
            .map(|id| Value::from(id.as_str())),
    );
    set_or_remove(
        &mut document,
        "editor_config_path",
        settings
            .editor_config_path
            .as_ref()
            .map(|path| Value::from(path.to_string_lossy().into_owned())),
    );

    write_known_vaults(&mut document, &settings.known_vaults);
    write_folder_refs(&mut document, folder_refs);

    // С отступами, а не одной строкой: файл предназначен для чтения глазами
    // (FR-036), и `diff` по нему должен быть построчным.
    let mut text = serde_json::to_string_pretty(&Value::Object(document))
        .unwrap_or_else(|_| "{}".to_string());
    text.push('\n');
    text
}

/// Пишет значение либо убирает ключ, если значения нет.
fn set_or_remove(document: &mut Map<String, Value>, key: &str, new_value: Option<Value>) {
    match new_value {
        Some(value) => {
            document.insert(key.to_string(), value);
        }
        None => {
            document.remove(key);
        }
    }
}

/// Читает реестр ссылок на папки. Отсутствие объекта — норма (веб его не
/// пишет), а не признак повреждённого файла.
pub fn parse_folder_refs(text: &str) -> FolderRefs {
    let Ok(Value::Object(document)) = serde_json::from_str::<Value>(text) else {
        return FolderRefs::new();
    };
    let Some(Value::Object(table)) = document.get(FOLDER_REFS_KEY) else {
        return FolderRefs::new();
    };
    table
        .iter()
        .filter_map(|(key, item)| item.as_str().map(|path| (key.clone(), path.to_string())))
        .collect()
}

fn write_folder_refs(document: &mut Map<String, Value>, folder_refs: &FolderRefs) {
    if folder_refs.is_empty() {
        document.remove(FOLDER_REFS_KEY);
        return;
    }
    let mut table = Map::new();
    for (id, path) in folder_refs {
        table.insert(id.clone(), Value::from(path.clone()));
    }
    document.insert(FOLDER_REFS_KEY.to_string(), Value::Object(table));
}

fn write_known_vaults(document: &mut Map<String, Value>, records: &[VaultRecord]) {
    if records.is_empty() {
        document.remove("known_vaults");
        return;
    }

    let entries: Vec<Value> = records
        .iter()
        .map(|record| {
            let mut entry = Map::new();
            entry.insert("id".into(), Value::from(record.id.as_str()));
            entry.insert("location".into(), Value::from(record.location.clone()));
            // Отсутствующая метка не пишется вовсе, а не пишется как `null`:
            // `null` в файле выглядел бы значением, которого нет.
            if let Some(label) = &record.label {
                entry.insert("label".into(), Value::from(label.clone()));
            }
            Value::Object(entry)
        })
        .collect();
    document.insert("known_vaults".to_string(), Value::Array(entries));
}

fn theme_str(theme: super::Theme) -> &'static str {
    match theme {
        super::Theme::Light => "light",
        super::Theme::Dark => "dark",
    }
}

fn mode_str(preference: super::ModePreference) -> &'static str {
    match preference {
        super::ModePreference::FullBackend => "fullBackend",
        super::ModePreference::SimplifiedEmulation => "simplifiedEmulation",
    }
}

/// Новый файл настроек — со строкой-подсказкой вместо комментария-шапки.
pub fn initial_document(settings: &Settings) -> String {
    let mut seed = Map::new();
    seed.insert(HINT_KEY.to_string(), Value::from(HINT));
    let previous = serde_json::to_string(&Value::Object(seed)).unwrap_or_default();
    render(&previous, settings, &FolderRefs::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{ModePreference, Theme, VaultId, VaultRecord};

    #[test]
    fn round_trip_preserves_values() {
        let settings = Settings {
            theme: Theme::Dark,
            editor_font_size: 20,
            known_vaults: vec![VaultRecord::new(
                VaultId::from_raw("abc"),
                "/work/notes".to_string(),
            )],
            current_vault: Some(VaultId::from_raw("abc")),
            editor_mode_preference: ModePreference::FullBackend,
            editor_config_path: Some("/home/user/init.lua".into()),
        };

        let text = render("", &settings, &FolderRefs::new());
        let parsed = parse(&text).expect("собственный вывод должен разбираться");
        assert_eq!(parsed, settings);
    }

    #[test]
    fn the_output_is_json_a_person_can_read() {
        let text = render("", &Settings::default(), &FolderRefs::new());
        assert!(text.starts_with('{'), "объект, а не массив и не строка");
        assert!(text.ends_with("}\n"), "файл заканчивается переводом строки");
        assert!(
            text.lines().count() > 3,
            "с отступами, а не одной строкой: файл читают глазами (FR-036)"
        );
        assert!(serde_json::from_str::<serde_json::Value>(&text).is_ok());
    }

    #[test]
    fn unknown_keys_are_not_an_error() {
        // FR-013: файл принадлежит пользователю; лишний ключ не должен делать
        // его нечитаемым.
        let text = r#"{"theme": "dark", "my_future_option": 42}"#;
        let parsed = parse(text).expect("неизвестный ключ не является ошибкой");
        assert_eq!(parsed.theme, Theme::Dark);
    }

    #[test]
    fn unknown_key_survives_application_write() {
        // Единственная гарантия прежнего TOML-формата, которую JSON сохраняет
        // полностью: приложение не выбрасывает то, чего не понимает.
        let text = r#"{"theme": "dark", "my_future_option": 42}"#;
        let settings = parse(text).unwrap();
        let rewritten = render(text, &settings, &FolderRefs::new());
        assert!(rewritten.contains("my_future_option"));
        assert_eq!(
            serde_json::from_str::<Value>(&rewritten).unwrap()["my_future_option"],
            Value::from(42)
        );
    }

    #[test]
    fn the_key_order_is_stable_between_writes() {
        // Порядок задаёт приложение (алфавитный, свойство `BTreeMap` под
        // `serde_json::Map`), но он обязан быть ОДИНАКОВЫМ от записи к записи:
        // иначе каждая правка одной настройки давала бы diff на весь файл.
        let settings = Settings::default();
        let first = render("", &settings, &FolderRefs::new());
        let second = render(&first, &settings, &FolderRefs::new());
        assert_eq!(first, second);

        let keys: Vec<&str> = first
            .lines()
            .filter_map(|line| line.trim().split('"').nth(1))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted, "ключи идут в алфавитном порядке");
    }

    #[test]
    fn broken_syntax_is_reported_not_silently_defaulted() {
        let error = parse("{ theme: dark }").expect_err("сломанный JSON должен отклоняться");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn an_empty_file_is_defaults_not_a_failure() {
        // Файл нулевой длины остаётся после оборванной записи; объявлять его
        // повреждённым и уводить пользователя в «файл не читается» незачем.
        assert_eq!(parse("").unwrap(), Settings::default());
        assert_eq!(parse("   \n").unwrap(), Settings::default());
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let parsed = parse(r#"{"theme": "dark"}"#).unwrap();
        assert_eq!(parsed.theme, Theme::Dark);
        assert_eq!(parsed.editor_font_size, super::super::DEFAULT_FONT_SIZE);
        assert!(parsed.known_vaults.is_empty());
    }

    #[test]
    fn clearing_optional_values_removes_their_keys() {
        let settings = Settings {
            current_vault: None,
            editor_config_path: None,
            ..Settings::default()
        };
        let previous = r#"{"current_vault": "old", "editor_config_path": "/tmp/x"}"#;
        let rewritten = render(previous, &settings, &FolderRefs::new());
        assert!(!rewritten.contains("current_vault"));
        assert!(!rewritten.contains("editor_config_path"));
    }

    #[test]
    fn vault_labels_round_trip_and_absent_label_is_omitted() {
        let mut labelled = VaultRecord::new(VaultId::from_raw("a"), "notes".to_string());
        labelled.label = Some("Работа".to_string());
        let plain = VaultRecord::new(VaultId::from_raw("b"), "notes".to_string());
        let settings = Settings {
            known_vaults: vec![labelled, plain],
            ..Settings::default()
        };

        let text = render("", &settings, &FolderRefs::new());
        let parsed = parse(&text).unwrap();
        assert_eq!(parsed.known_vaults.len(), 2);
        assert_eq!(parsed.known_vaults[0].label.as_deref(), Some("Работа"));
        assert_eq!(parsed.known_vaults[1].label, None);
        // Две записи с одинаковым показываемым расположением остаются двумя
        // (research.md §10).
        assert_ne!(parsed.known_vaults[0].id, parsed.known_vaults[1].id);
    }

    #[test]
    fn folder_refs_round_trip_and_absent_object_is_not_an_error() {
        // Веб реестр не пишет — его отсутствие нормальный случай, а не
        // повреждённый файл (research.md §10).
        assert!(parse_folder_refs(r#"{"theme": "dark"}"#).is_empty());

        let mut refs = FolderRefs::new();
        refs.insert("a".to_string(), "/work/notes".to_string());
        refs.insert("b".to_string(), "/personal/notes".to_string());
        let text = render("", &Settings::default(), &refs);
        assert_eq!(parse_folder_refs(&text), refs);
        // Значения настроек от появления служебного объекта не меняются.
        assert_eq!(parse(&text).unwrap(), Settings::default());
    }

    #[test]
    fn initial_document_carries_a_hint_and_parses() {
        let text = initial_document(&Settings::default());
        assert!(text.contains("править вручную"));
        assert_eq!(parse(&text).unwrap(), Settings::default());
    }
}
