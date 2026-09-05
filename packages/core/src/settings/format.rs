//! Чтение и **форматосохраняющая** запись файла настроек (research.md §2).
//!
//! Файл настроек — пользовательский артефакт: FR-036 приглашает его открыть,
//! FR-013 делает правку руками равноправной правке через панель. Приложение
//! перезаписывает файл при каждом изменении настройки, поэтому обычная
//! serde-сериализация молча стирала бы комментарии и порядок ключей, которые
//! пользователь написал сам.
//!
//! Отсюда разделение: чтение — `toml` + `serde` (простое и строгое), запись —
//! `toml_edit` поверх **существующего текста**, правящий только затронутые
//! значения.

use super::{Settings, VaultRecord};
use std::collections::BTreeMap;
use toml_edit::{value, Array, DocumentMut, Item, Value};

/// Ссылки на папки vault, пригодные к сериализации: `VaultId` → путь.
///
/// Заполняется **только на native** (research.md §10): в вебе ссылка — это
/// `FileSystemDirectoryHandle`, который в TOML не сериализуется, и её реестр
/// живёт в IndexedDB. Формат файла при этом один на оба таргета — веб просто
/// не пишет эту таблицу. `BTreeMap` ради детерминированного порядка: иначе
/// каждая запись переставляла бы строки в пользовательском файле.
pub type FolderRefs = BTreeMap<String, String>;

/// Имя таблицы со ссылками на папки.
const FOLDER_REFS_TABLE: &str = "folder_refs";

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
    toml::from_str::<Settings>(text).map_err(|error| ParseError {
        message: error.to_string(),
    })
}

/// Записывает значения в существующий текст, сохраняя комментарии, порядок
/// ключей и форматирование пользователя.
///
/// `previous` — прежнее содержимое файла (пустая строка, если файла ещё нет).
/// Возвращает новый текст целиком: запись на диск атомарна и выполняется
/// реализацией хранилища (research.md §5).
pub fn render(previous: &str, settings: &Settings, folder_refs: &FolderRefs) -> String {
    let mut document = previous.parse::<DocumentMut>().unwrap_or_default();

    set_value(&mut document, "theme", Value::from(theme_str(settings.theme)));
    set_value(
        &mut document,
        "editor_font_size",
        Value::from(i64::from(settings.editor_font_size)),
    );
    set_value(
        &mut document,
        "editor_mode_preference",
        Value::from(mode_str(settings.editor_mode_preference)),
    );

    match &settings.current_vault {
        Some(id) => set_value(&mut document, "current_vault", Value::from(id.as_str())),
        None => {
            document.remove("current_vault");
        }
    }
    match &settings.editor_config_path {
        Some(path) => set_value(
            &mut document,
            "editor_config_path",
            Value::from(path.to_string_lossy().into_owned()),
        ),
        None => {
            document.remove("editor_config_path");
        }
    }

    write_known_vaults(&mut document, &settings.known_vaults);
    write_folder_refs(&mut document, folder_refs);
    document.to_string()
}

/// Записывает значение, **сохраняя его декор** — отступы и комментарии вокруг.
///
/// Присваивание `document[key] = value(...)` подменяет весь `Item` вместе с
/// декором, из-за чего комментарий в конце строки (`ключ = 18  # почему`)
/// молча исчезал бы при первой же записи приложения. Именно это research.md §2
/// и требует не допустить: файл — пользовательский артефакт.
fn set_value(document: &mut DocumentMut, key: &str, new_value: Value) {
    match document.get_mut(key).and_then(Item::as_value_mut) {
        Some(existing) => {
            let decor = existing.decor().clone();
            *existing = new_value;
            *existing.decor_mut() = decor;
        }
        None => document[key] = value(new_value),
    }
}

/// Читает реестр ссылок на папки. Отсутствие таблицы — норма (веб её не
/// пишет), а не признак повреждённого файла.
pub fn parse_folder_refs(text: &str) -> FolderRefs {
    let Ok(document) = text.parse::<DocumentMut>() else {
        return FolderRefs::new();
    };
    let Some(table) = document.get(FOLDER_REFS_TABLE).and_then(Item::as_table) else {
        return FolderRefs::new();
    };
    table
        .iter()
        .filter_map(|(key, item)| item.as_str().map(|path| (key.to_string(), path.to_string())))
        .collect()
}

fn write_folder_refs(document: &mut DocumentMut, folder_refs: &FolderRefs) {
    if folder_refs.is_empty() {
        document.remove(FOLDER_REFS_TABLE);
        return;
    }
    let mut table = toml_edit::Table::new();
    for (id, path) in folder_refs {
        table.insert(id, value(path.clone()));
    }
    // Таблица идёт последней и не смешивается со значениями настроек: это
    // служебная часть файла, а не то, что пользователь правит руками.
    table.set_implicit(false);
    document[FOLDER_REFS_TABLE] = Item::Table(table);
}

/// Список vault пишется массивом инлайн-таблиц, а не `[[known_vaults]]`:
/// повторяющиеся секции `toml_edit` пришлось бы пересобирать целиком на каждое
/// изменение, теряя ровно то форматирование, ради которого он и взят.
fn write_known_vaults(document: &mut DocumentMut, records: &[VaultRecord]) {
    if records.is_empty() {
        document.remove("known_vaults");
        return;
    }

    let mut array = Array::new();
    for record in records {
        let mut entry = toml_edit::InlineTable::new();
        entry.insert("id", Value::from(record.id.as_str()));
        entry.insert("location", Value::from(record.location.clone()));
        if let Some(label) = &record.label {
            entry.insert("label", Value::from(label.clone()));
        }
        array.push(Value::InlineTable(entry));
    }
    document["known_vaults"] = Item::Value(Value::Array(array));
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

/// Пустой документ настроек по умолчанию — с пояснением, что файл можно
/// править руками. Комментарий пишется один раз, при создании файла, и дальше
/// переживает все записи приложения наравне с пользовательскими.
pub fn initial_document(settings: &Settings) -> String {
    let header = "# Настройки Typsidian. Файл можно править вручную —\n\
                  # приложение подхватит изменения без перезапуска\n\
                  # и сохранит ваши комментарии при следующей записи.\n";
    render(header, settings, &FolderRefs::new())
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
    fn user_comments_and_key_order_survive_a_write() {
        // Ключевая проверка research.md §2: обычная serde-сериализация здесь
        // провалилась бы.
        let authored = "\
# мои настройки
editor_font_size = 18  # нравится покрупнее

# тема
theme = \"dark\"
";
        let mut settings = parse(authored).unwrap();
        settings.editor_font_size = 19;

        let rewritten = render(authored, &settings, &FolderRefs::new());

        assert!(rewritten.contains("# мои настройки"));
        assert!(rewritten.contains("# нравится покрупнее"));
        assert!(rewritten.contains("# тема"));
        assert!(
            rewritten.find("editor_font_size").is_some()
                && rewritten.find("editor_font_size") < rewritten.find("theme"),
            "порядок ключей пользователя сохраняется"
        );
        assert!(rewritten.contains("19"));
        assert_eq!(parse(&rewritten).unwrap().editor_font_size, 19);
        assert_eq!(parse(&rewritten).unwrap().theme, Theme::Dark);
    }

    #[test]
    fn unknown_keys_are_not_an_error() {
        // FR-013: файл принадлежит пользователю; лишняя строка не должна
        // делать его нечитаемым.
        let text = "theme = \"dark\"\nmy_future_option = 42\n";
        let parsed = parse(text).expect("неизвестный ключ не является ошибкой");
        assert_eq!(parsed.theme, Theme::Dark);
    }

    #[test]
    fn unknown_key_survives_application_write() {
        let text = "theme = \"dark\"\nmy_future_option = 42\n";
        let settings = parse(text).unwrap();
        let rewritten = render(text, &settings, &FolderRefs::new());
        assert!(
            rewritten.contains("my_future_option"),
            "приложение не выбрасывает то, чего не понимает"
        );
    }

    #[test]
    fn broken_syntax_is_reported_not_silently_defaulted() {
        let error = parse("theme = = = \"dark\"").expect_err("сломанный TOML должен отклоняться");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let parsed = parse("theme = \"dark\"").unwrap();
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
        let previous = "current_vault = \"old\"\neditor_config_path = \"/tmp/x\"\n";
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
    fn folder_refs_round_trip_and_absent_table_is_not_an_error() {
        // Веб таблицу не пишет — её отсутствие нормальный случай, а не
        // повреждённый файл (research.md §10).
        assert!(parse_folder_refs("theme = \"dark\"").is_empty());

        let mut refs = FolderRefs::new();
        refs.insert("a".to_string(), "/work/notes".to_string());
        refs.insert("b".to_string(), "/personal/notes".to_string());
        let text = render("", &Settings::default(), &refs);
        assert_eq!(parse_folder_refs(&text), refs);
        // Значения настроек от появления служебной таблицы не меняются.
        assert_eq!(parse(&text).unwrap(), Settings::default());
    }

    #[test]
    fn initial_document_carries_a_hint_and_parses() {
        let text = initial_document(&Settings::default());
        assert!(text.contains("править вручную"));
        assert_eq!(parse(&text).unwrap(), Settings::default());
    }
}
