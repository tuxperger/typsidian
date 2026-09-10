//! Настройки приложения (фича 004): значения, принадлежащие устройству, а не
//! vault (FR-010, FR-011). Хранилище — один файл вне любого vault, за трейтом
//! [`store::SettingsStore`] с двумя реализациями, по образцу `vault::backend`.
//!
//! Раскладка модуля повторяет `vault/`: типы и общая валидация здесь,
//! платформенное — в `native`/`web` за трейтом, условная компиляция только
//! внутри реализаций (принцип II).

pub mod apply;
pub mod diagnostics;
pub mod editor_mode;
pub mod events;
pub mod folder_ref;
pub mod format;
pub mod logging;
pub mod store;
pub mod vaults;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(target_arch = "wasm32")]
pub mod web;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use vaults::{Availability, VaultId, VaultRecord};

/// Тема оформления (FR-015). Ровно два варианта: следование системной теме
/// вынесено за границы фичи спецификацией.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

/// **Выбор пользователя** по режиму редактирования — не текущее состояние
/// (data-model.md). Хранить их одним полем нельзя: `PlainInput` наступает от
/// отказа подсистемы и не должен закрепляться на следующий запуск, поэтому
/// персистентное предпочтение и рантайм-состояние [`editor_mode::EditorMode`]
/// разделены намеренно.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "camelCase")]
pub enum ModePreference {
    FullBackend,
    #[default]
    SimplifiedEmulation,
}

/// Версия приложения — из манифеста ядра, на этапе сборки.
///
/// Показывается в подвале панели настроек (макет «Утилиты» § Настройки). Здесь,
/// а не литералом в интерфейсе: две копии версии разошлись бы при первом же
/// выпуске, и подвал начал бы врать.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Наименьший допустимый размер шрифта редактора (research.md §11): предел
/// читаемости на обычной плотности пикселей.
pub const MIN_FONT_SIZE: u8 = 8;
/// Наибольший допустимый (research.md §11): покрывает потребности
/// слабовидящих, не ломая раскладку панелей.
pub const MAX_FONT_SIZE: u8 = 32;
/// Значение по умолчанию (data-model.md).
pub const DEFAULT_FONT_SIZE: u8 = 14;

/// Полный набор значений, переживающих перезапуск (data-model.md «Settings»).
/// Сериализуется в один JSON-файл.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(default, rename_all = "snake_case")]
pub struct Settings {
    pub theme: Theme,
    pub editor_font_size: u8,
    pub known_vaults: Vec<VaultRecord>,
    pub current_vault: Option<VaultId>,
    pub editor_mode_preference: ModePreference,
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string | undefined"))]
    pub editor_config_path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            editor_font_size: DEFAULT_FONT_SIZE,
            known_vaults: Vec::new(),
            current_vault: None,
            editor_mode_preference: ModePreference::default(),
            editor_config_path: None,
        }
    }
}

/// Почему набор значений не является допустимым. Отдельно от
/// [`apply::RejectReason`]: это проверка самих значений, не зависящая ни от
/// состояния хранилища, ни от открытой заметки.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidSettings {
    #[error("размер шрифта {value} вне допустимого диапазона {MIN_FONT_SIZE}..={MAX_FONT_SIZE}")]
    FontSizeOutOfRange { value: u8 },
    #[error("текущий vault ссылается на запись, которой нет в списке известных")]
    CurrentVaultNotKnown,
    #[error("одна и та же папка встречается в списке дважды")]
    DuplicateVaultId,
}

/// Допустим ли размер шрифта (FR-016, research.md §11). Общий код обеих
/// реализаций: диапазон не имеет права разойтись по таргету (принцип II) — тот
/// же приём, что `is_valid_note_name` в `vault::backend`.
pub fn is_valid_font_size(value: u8) -> bool {
    (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&value)
}

/// Значения, действующие при открытом vault: устройство как слой по умолчанию,
/// vault перекрывает только то, что в нём записано (фича 006, FR-030).
///
/// Чистая функция, поле за полем. Слой, а не копия: `None` в перекрытии
/// означает «спросить устройство», и разница между «значение не задано» и
/// «задано и равно умолчанию» выражена в системе типов, а не в соглашении.
///
/// Список известных vault и текущий vault перекрытию **не подлежат** и в
/// [`crate::vault::vault_settings::VaultOverrides`] отсутствуют физически
/// (FR-028): без них неоткуда узнать, какую папку открывать, поэтому положить
/// их внутрь одной из папок нельзя даже по ошибке.
pub fn resolve(
    device: &Settings,
    overrides: &crate::vault::vault_settings::VaultOverrides,
) -> Settings {
    Settings {
        theme: overrides.theme.unwrap_or(device.theme),
        editor_font_size: overrides.editor_font_size.unwrap_or(device.editor_font_size),
        editor_mode_preference: overrides
            .editor_mode_preference
            .unwrap_or(device.editor_mode_preference),
        editor_config_path: overrides
            .editor_config_path
            .clone()
            .or_else(|| device.editor_config_path.clone()),
        // Принадлежат устройству и слоями не перекрываются.
        known_vaults: device.known_vaults.clone(),
        current_vault: device.current_vault.clone(),
    }
}

/// Инварианты набора настроек целиком.
///
/// Платформенным остаётся **только** сравнение самих папок (research.md §10):
/// здесь дубли ловятся по уже выданному [`VaultId`], а не по пути, имени или
/// показываемому расположению — их сравнение запрещено. Подпись в тождестве не
/// участвует и уникальности не требует (research.md §14).
pub fn validate(settings: &Settings) -> Result<(), InvalidSettings> {
    if !is_valid_font_size(settings.editor_font_size) {
        return Err(InvalidSettings::FontSizeOutOfRange {
            value: settings.editor_font_size,
        });
    }

    let mut seen: Vec<&VaultId> = Vec::with_capacity(settings.known_vaults.len());
    for record in &settings.known_vaults {
        if seen.contains(&&record.id) {
            return Err(InvalidSettings::DuplicateVaultId);
        }
        seen.push(&record.id);
    }

    if let Some(current) = &settings.current_vault {
        if !settings.known_vaults.iter().any(|r| &r.id == current) {
            return Err(InvalidSettings::CurrentVaultNotKnown);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_data_model() {
        let settings = Settings::default();
        assert_eq!(settings.theme, Theme::Light);
        assert_eq!(settings.editor_font_size, 14);
        assert_eq!(settings.current_vault, None);
        assert!(settings.known_vaults.is_empty());
        assert_eq!(
            settings.editor_mode_preference,
            ModePreference::SimplifiedEmulation
        );
        assert_eq!(settings.editor_config_path, None);
        assert!(validate(&settings).is_ok());
    }

    #[test]
    fn font_size_range_is_closed_and_inclusive() {
        // research.md §11: 7 и 33 отклоняются, 8 и 32 принимаются.
        assert!(!is_valid_font_size(7));
        assert!(is_valid_font_size(8));
        assert!(is_valid_font_size(32));
        assert!(!is_valid_font_size(33));
    }

    #[test]
    fn duplicate_vault_id_is_rejected() {
        let record = VaultRecord::new(VaultId::from_raw("a"), "/tmp/notes".to_string());
        let settings = Settings {
            known_vaults: vec![record.clone(), record],
            ..Settings::default()
        };
        assert_eq!(
            validate(&settings),
            Err(InvalidSettings::DuplicateVaultId)
        );
    }

    #[test]
    fn label_does_not_participate_in_identity() {
        // research.md §14: одинаковые подписи у разных записей допустимы.
        let mut first = VaultRecord::new(VaultId::from_raw("a"), "/work/notes".to_string());
        let mut second = VaultRecord::new(VaultId::from_raw("b"), "/personal/notes".to_string());
        first.label = Some("Заметки".to_string());
        second.label = Some("Заметки".to_string());
        let settings = Settings {
            known_vaults: vec![first, second],
            ..Settings::default()
        };
        assert!(validate(&settings).is_ok());
    }

    #[test]
    fn the_vault_layer_overrides_only_what_it_records() {
        // FR-030: `None` пропускает значение устройства, `Some` перекрывает.
        use crate::vault::vault_settings::VaultOverrides;

        let device = Settings {
            theme: Theme::Light,
            editor_font_size: 14,
            editor_mode_preference: ModePreference::SimplifiedEmulation,
            editor_config_path: Some("/device/init.lua".into()),
            known_vaults: vec![VaultRecord::new(
                VaultId::from_raw("a"),
                "/notes".to_string(),
            )],
            current_vault: Some(VaultId::from_raw("a")),
        };
        let overrides = VaultOverrides {
            theme: Some(Theme::Dark),
            ..VaultOverrides::default()
        };

        let effective = resolve(&device, &overrides);
        assert_eq!(effective.theme, Theme::Dark, "перекрыто");
        assert_eq!(effective.editor_font_size, 14, "взято с устройства");
        assert_eq!(
            effective.editor_config_path,
            Some("/device/init.lua".into()),
            "взято с устройства"
        );
        // FR-028: реестр остаётся свойством устройства при любом перекрытии.
        assert_eq!(effective.known_vaults, device.known_vaults);
        assert_eq!(effective.current_vault, device.current_vault);
    }

    #[test]
    fn values_written_by_the_previous_version_become_the_device_layer() {
        // FR-032, SC-005: при первом запуске новой версии не теряется ни одно
        // ранее заданное значение. Файл настроек устройства **и есть** слой по
        // умолчанию после переезда настроек внутрь vault (research.md §4) —
        // отдельного кода миграции для этого не нужно.
        //
        // Речь именно о файле, написанном **этой же** версией формата: смена
        // формата на JSON переносом с прежнего TOML не сопровождается вовсе
        // (research.md §2, пересмотрено).
        use crate::vault::vault_settings::VaultOverrides;

        let written_by_previous_version = r#"{
  "theme": "dark",
  "editor_font_size": 20,
  "editor_mode_preference": "fullBackend",
  "editor_config_path": "/home/user/init.lua",
  "known_vaults": [{ "id": "a", "location": "/work/notes" }],
  "current_vault": "a"
}"#;
        let device = format::parse(written_by_previous_version)
            .expect("файл прежней версии читается новой");

        assert_eq!(device.theme, Theme::Dark);
        assert_eq!(device.editor_font_size, 20);
        assert_eq!(device.editor_mode_preference, ModePreference::FullBackend);
        assert_eq!(device.editor_config_path, Some("/home/user/init.lua".into()));
        assert_eq!(device.known_vaults.len(), 1);
        assert_eq!(device.current_vault, Some(VaultId::from_raw("a")));

        // И они действуют как слой по умолчанию для папки без своих значений.
        assert_eq!(resolve(&device, &VaultOverrides::default()), device);
    }

    #[test]
    fn an_empty_vault_layer_changes_nothing() {
        use crate::vault::vault_settings::VaultOverrides;
        let device = Settings {
            theme: Theme::Dark,
            editor_font_size: 20,
            ..Settings::default()
        };
        assert_eq!(resolve(&device, &VaultOverrides::default()), device);
    }

    #[test]
    fn current_vault_must_point_at_a_known_record() {
        let settings = Settings {
            current_vault: Some(VaultId::from_raw("missing")),
            ..Settings::default()
        };
        assert_eq!(
            validate(&settings),
            Err(InvalidSettings::CurrentVaultNotKnown)
        );
    }
}
