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
pub mod keybindings;
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

/// Режим колонки заметки (фича 013, research.md §12). Фича 015 добавила
/// `Outline`, 020 — `Properties`, 025 — `Graph`, 017 добавит `Sources`; неизвестное значение из более новой версии
/// читается как умолчание ([`NotePanelState::lenient`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "camelCase")]
pub enum NotePanelMode {
    #[default]
    Links,
    /// Оглавление заметки (фича 015): в файле - `"outline"`.
    Outline,
    /// Свойства заметки (фича 020): в файле - `"properties"`.
    Properties,
    /// Локальный граф заметки (фича 025): в файле - `"graph"`.
    Graph,
}

/// Состояние колонки заметки — свойство устройства (фича 013, FR-021).
///
/// В `VaultOverrides` не попадает, сбросом к умолчаниям не затрагивается:
/// это положение окна, а не настройка внешнего вида.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(default)]
pub struct NotePanelState {
    /// Колонка показана (на шторку узкого окна не действует).
    pub shown: bool,
    /// Активный режим.
    pub mode: NotePanelMode,
    /// Раскрыт ли список упоминаний (FR-012).
    pub mentions_expanded: bool,
    /// Какие виды пунктов, кроме заголовков, показывает оглавление (фича 015,
    /// FR-010). Умолчание (оба выключены) не пишется: файл остаётся прежним, а
    /// прежний интерфейс - собирающимся.
    #[serde(skip_serializing_if = "outline_kinds_are_default")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub outline_kinds: crate::structure::OutlineKinds,
}

fn outline_kinds_are_default(kinds: &crate::structure::OutlineKinds) -> bool {
    *kinds == crate::structure::OutlineKinds::default()
}

impl NotePanelState {
    /// Разбор без права уронить файл: негодное поле читается как умолчание,
    /// остальные поля и остальные настройки сохраняются.
    pub fn lenient(value: &serde_json::Value) -> Self {
        let flag = |key: &str| value.get(key).and_then(serde_json::Value::as_bool);
        Self {
            shown: flag("shown").unwrap_or_default(),
            mode: value
                .get("mode")
                .and_then(|mode| serde_json::from_value(mode.clone()).ok())
                .unwrap_or_default(),
            mentions_expanded: flag("mentions_expanded").unwrap_or_default(),
            outline_kinds: value
                .get("outline_kinds")
                .map(|kinds| {
                    let flag = |key: &str| kinds.get(key).and_then(serde_json::Value::as_bool);
                    crate::structure::OutlineKinds {
                        figures: flag("figures").unwrap_or_default(),
                        labels: flag("labels").unwrap_or_default(),
                    }
                })
                .unwrap_or_default(),
        }
    }
}

fn lenient_note_panel<'de, D>(deserializer: D) -> Result<NotePanelState, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(NotePanelState::lenient(&value))
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
    /// `None` не пишется вовсе: команды Tauri отдали бы его как `null`, а UI
    /// ждёт `undefined`, как из wasm (тот же приём у `language`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string | undefined"))]
    pub editor_config_path: Option<PathBuf>,
    /// Язык интерфейса, выбранный явно (фича 010); `None` — «как в системе».
    ///
    /// Строкой, а не перечислением: какие языки поставляются, знает интерфейс,
    /// и язык из более новой версии обязан пережить запись других значений,
    /// а не превратиться в умолчание (FR-018). По той же причине значение из
    /// файла не проверяется при разборе — только новое, в [`apply::plan`].
    ///
    /// `None` не сериализуется: через команды Tauri он пришёл бы в UI как
    /// `null` мимо проверки `=== undefined`, и разбор языка уронил бы первый
    /// же рендер окна на desktop и Android.
    #[serde(
        deserialize_with = "lenient_language",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string | undefined"))]
    pub language: Option<String>,
    /// Тождество устройства (фича 011, research §4). Принадлежит устройству
    /// при любом раскладе: окажись оно в vault, все устройства стали бы одним.
    /// `None` — ещё не выдано; выдаёт [`crate::sync::device`] при первом
    /// обращении к синхронизации.
    #[serde(default, deserialize_with = "lenient_device")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub device: Option<crate::sync::DeviceIdentity>,
    /// Значения синхронизации, принадлежащие устройству, — по одному на vault
    /// (фича 011, data-model «DeviceVaultSync»): автор снимков, посредник для
    /// браузера, адрес Syncthing.
    #[serde(default, deserialize_with = "lenient_vault_sync")]
    pub vault_sync: Vec<crate::sync::DeviceVaultSyncEntry>,
    /// Состояние колонки заметки (фича 013): только устройству.
    #[serde(deserialize_with = "lenient_note_panel")]
    pub note_panel: NotePanelState,
    /// Наборы вкладок по vault (фича 014, FR-021…FR-023): только устройству.
    #[serde(deserialize_with = "lenient_workspaces")]
    pub workspaces: Vec<VaultWorkspaceEntry>,
}

/// Набор вкладок одного vault на этом устройстве (фича 014, data-model
/// «Сохранённый набор»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct VaultWorkspaceEntry {
    pub vault: VaultId,
    pub value: crate::workspace::SavedWorkspace,
}

/// Негодная запись набора вкладок отбрасывается, а не роняет файл настроек
/// (FR-023): вкладки восстанавливаются без неё, остальное читается как обычно.
fn lenient_workspaces<'de, D>(deserializer: D) -> Result<Vec<VaultWorkspaceEntry>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| serde_json::from_value(entry.clone()).ok())
                .collect()
        })
        .unwrap_or_default())
}

impl Settings {
    /// Сохранённый набор вкладок vault, если он есть.
    pub fn workspace_for(&self, vault: &VaultId) -> Option<crate::workspace::SavedWorkspace> {
        self.workspaces
            .iter()
            .find(|entry| &entry.vault == vault)
            .map(|entry| entry.value.clone())
    }

    /// Забывает набор вкладок vault, когда сам vault забыт (фича 014).
    pub fn forget_workspace(&mut self, vault: &VaultId) {
        self.workspaces.retain(|entry| &entry.vault != vault);
    }

    /// Значения синхронизации устройства для vault; умолчания, если их нет.
    pub fn vault_sync_for(&self, vault: &VaultId) -> crate::sync::DeviceVaultSync {
        self.vault_sync
            .iter()
            .find(|entry| &entry.vault == vault)
            .map(|entry| entry.value.clone())
            .unwrap_or_default()
    }
}

/// Тождество с негодным идентификатором — как если бы его не было: выдать
/// новое лучше, чем узнавать себя по чужому мусору.
fn lenient_device<'de, D>(deserializer: D) -> Result<Option<crate::sync::DeviceIdentity>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value::<crate::sync::DeviceIdentity>(value)
        .ok()
        .filter(|device| crate::sync::DeviceIdentity::is_valid_id(&device.id)))
}

/// Испорченная запись синхронизации отбрасывается, а не роняет файл
/// настроек (FR-013 фичи 004).
fn lenient_vault_sync<'de, D>(
    deserializer: D,
) -> Result<Vec<crate::sync::DeviceVaultSyncEntry>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| serde_json::from_value(entry.clone()).ok())
                .collect()
        })
        .unwrap_or_default())
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
            language: None,
            device: None,
            vault_sync: Vec::new(),
            note_panel: NotePanelState::default(),
            workspaces: Vec::new(),
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
    #[error("язык «{value}» не является тегом языка")]
    LanguageTag { value: String },
}

/// Допустим ли размер шрифта (FR-016, research.md §11). Общий код обеих
/// реализаций: диапазон не имеет права разойтись по таргету (принцип II) — тот
/// же приём, что `is_valid_note_name` в `vault::backend`.
pub fn is_valid_font_size(value: u8) -> bool {
    (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&value)
}

/// Разбор ключа `language` без права уронить файл (FR-018): не строка — как
/// «как в системе», остальные значения читаются как обычно.
fn lenient_language<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value.as_str().map(str::to_owned))
}

/// Похоже ли значение на тег языка BCP 47 (фича 010): основной подтег из 2–8 букв и
/// подтеги из 1–8 латинских букв или цифр через дефис, не длиннее 35 знаков.
///
/// Только синтаксис: поставляется ли такой язык, решает интерфейс.
pub fn is_valid_language_tag(value: &str) -> bool {
    if value.is_empty() || value.len() > 35 {
        return false;
    }
    let mut parts = value.split('-');
    let primary_ok = parts.next().is_some_and(|primary| {
        (2..=8).contains(&primary.len()) && primary.bytes().all(|b| b.is_ascii_alphabetic())
    });
    primary_ok
        && parts.all(|part| {
            (1..=8).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric())
        })
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
        editor_font_size: overrides
            .editor_font_size
            .unwrap_or(device.editor_font_size),
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
        language: device.language.clone(),
        device: device.device.clone(),
        vault_sync: device.vault_sync.clone(),
        note_panel: device.note_panel,
        workspaces: device.workspaces.clone(),
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
    fn absent_optional_strings_are_not_serialized_as_null() {
        // Команды Tauri отдают JSON: `null` прошёл бы в UI мимо `=== undefined`.
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert!(json.get("language").is_none());
        assert!(json.get("editor_config_path").is_none());

        let back: Settings = serde_json::from_value(json).unwrap();
        assert_eq!(back, Settings::default());
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
        assert_eq!(validate(&settings), Err(InvalidSettings::DuplicateVaultId));
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
            language: Some("en".to_string()),
            ..Settings::default()
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
        assert_eq!(
            effective.language, device.language,
            "язык только с устройства"
        );
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
        let device =
            format::parse(written_by_previous_version).expect("файл прежней версии читается новой");

        assert_eq!(device.theme, Theme::Dark);
        assert_eq!(device.editor_font_size, 20);
        assert_eq!(device.editor_mode_preference, ModePreference::FullBackend);
        assert_eq!(
            device.editor_config_path,
            Some("/home/user/init.lua".into())
        );
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
