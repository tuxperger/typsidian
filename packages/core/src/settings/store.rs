//! Трейт хранилища настроек (contracts/settings-store-trait.md).
//!
//! Единственная точка доступа к хранилищу: один трейт, две реализации —
//! `NativeStore` (файл в каталоге конфигурации ОС) и `WebStore` (OPFS), — по
//! образцу уже работающего `VaultBackend`. Условная компиляция допустима только
//! **внутри** реализаций; вызывающий код по платформе не ветвится.

use super::apply::{ApplyContext, ApplyPlan, SettingsChange};
use super::events::SettingsEvent;
use super::Settings;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Возможности хранилища на текущем таргете — **значение, а не форма API**
/// (research.md §13, принцип II). UI ветвится по значению, а не по таргету;
/// метод, существующий только под native, развалил бы контракт.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct StoreCapabilities {
    /// Путь к файлу настроек (native) либо обозначение хранилища OPFS (web).
    pub location: String,
    /// Можно ли раскрыть хранилище средствами ОС (FR-036). На web — `false`:
    /// расположение показывается справочным текстом без действия.
    pub can_reveal: bool,
    /// Доступно ли хранилище на запись прямо сейчас.
    pub writable: bool,
}

/// Состояние хранилища как источника значений (FR-013, FR-014).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "state")]
pub enum StorageState {
    /// Файл прочитан и разобран — обычная работа.
    #[default]
    Loaded,
    /// Файл повреждён, нечитаем или удалён: работа идёт на значениях по
    /// умолчанию, испорченный файл **не перезаписан** (research.md §6).
    DefaultsAfterUnreadable,
    /// Каталог недоступен на запись: изменения действуют до конца сессии и не
    /// переживут перезапуск (FR-014).
    ReadOnlyForSession,
}

/// Отказ операции хранилища. Нечитаемый файл сюда **не относится** — это не
/// ошибка, а состояние [`StorageState::DefaultsAfterUnreadable`] (FR-013).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum StoreError {
    #[error("не удалось определить каталог настроек на этой платформе")]
    NoConfigDirectory,
    #[error("не удалось записать хранилище настроек")]
    Io,
    #[error("раскрытие хранилища средствами системы здесь недоступно")]
    RevealUnsupported,
}

/// Хранилище настроек (contracts/settings-store-trait.md).
///
/// Чего трейт **не** делает: не вызывает `VaultBackend` (там, где смена режима
/// требует сохранить заметку, ядро возвращает инструкцию — research.md §9), не
/// знает о msgpack-rpc и не решает, что показать пользователю — отдаёт
/// перечислимые события и причины.
#[allow(async_fn_in_trait)]
pub trait SettingsStore: Sized {
    /// Открыть или создать хранилище. Нечитаемый файл — не ошибка: возвращается
    /// хранилище в состоянии [`StorageState::DefaultsAfterUnreadable`]
    /// (FR-013).
    async fn open() -> Result<Self, StoreError>;

    /// Текущие значения.
    fn settings(&self) -> &Settings;

    /// Перечитать значения с диска.
    async fn load(&mut self) -> Settings;

    fn storage_state(&self) -> StorageState;

    /// Применить набор изменений **атомарно**: либо весь набор, либо ничего
    /// (FR-037, research.md §5). Возвращает план, вернувшийся из ядра: при
    /// [`ApplyPlan::Applied`] изменения уже на диске, при остальных исходах
    /// файл не тронут.
    async fn apply(&mut self, change: &SettingsChange, context: &ApplyContext) -> ApplyPlan;

    fn capabilities(&self) -> StoreCapabilities;

    /// Раскрыть хранилище средствами ОС. При `can_reveal = false` возвращает
    /// отказ, а не отсутствует как метод (research.md §13).
    fn reveal(&self) -> Result<(), StoreError>;

    /// Забрать накопившиеся с прошлого вызова события (FR-013). Pull-опрос
    /// вместо push-потока — тот же приём, что `VaultBackend::poll_events()`.
    async fn poll_events(&mut self) -> Vec<SettingsEvent>;
}

/// Имя файла настроек, общее для обоих таргетов.
pub const SETTINGS_FILE_NAME: &str = "settings.json";

/// Как называется отложенный испорченный файл (research.md §6). Метка — UTC,
/// чтобы имена не зависели от часового пояса и сортировались лексикографически.
pub fn invalid_file_name(utc_millis: i64) -> String {
    format!("{SETTINGS_FILE_NAME}.invalid-{utc_millis}")
}

/// Путь, куда будет отложен испорченный файл, рядом с исходным.
pub fn invalid_file_path(settings_path: &std::path::Path, utc_millis: i64) -> PathBuf {
    settings_path.with_file_name(invalid_file_name(utc_millis))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn invalid_file_sits_next_to_the_original_and_keeps_it() {
        let path = invalid_file_path(Path::new("/home/user/.config/typsidian/settings.json"), 42);
        assert_eq!(
            path,
            Path::new("/home/user/.config/typsidian/settings.json.invalid-42")
        );
        assert_ne!(path.file_name(), Some(std::ffi::OsStr::new(SETTINGS_FILE_NAME)));
    }

    #[test]
    fn storage_state_defaults_to_loaded() {
        assert_eq!(StorageState::default(), StorageState::Loaded);
    }
}
