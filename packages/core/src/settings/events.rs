//! События хранилища настроек ядро → UI (contracts/settings-store-trait.md
//! «События», contracts/frontend-settings.md).
//!
//! Типы порождаются из Rust через `serde` + `tsify-next` (принцип IV): ручных
//! дубликатов на стороне TS не существует. Разбор на стороне UI обязан быть
//! исчерпывающим (`never` в `default`).

use super::editor_mode::{ConfigDiskState, EditorMode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Что вернул `poll_events()` (FR-013, FR-006, FR-014).
///
/// Источник изменения — другое окно приложения или сторонний редактор — на
/// уровне типа не различается: согласованность между окнами обеспечивается тем
/// же наблюдением за файлом, отдельного канала между окнами нет
/// (research.md §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "type")]
pub enum SettingsEvent {
    /// Файл изменён вне текущего окна — применить новые значения без
    /// перезапуска (FR-013, FR-006).
    ValuesChangedExternally { settings: super::Settings },
    /// Файл повреждён, нечитаем или удалён (FR-013).
    StorageUnreadable,
    /// Ранее нечитаемый файл снова разбирается — пользователь починил его
    /// руками, перезапуск не потребовался.
    StorageRecovered { settings: super::Settings },
    /// Каталог потерял доступ на запись (FR-014).
    StorageBecameReadOnly,
    /// Испорченный файл переименован рядом перед первой записью
    /// (research.md §6).
    InvalidFileSetAside {
        #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
        moved_to: PathBuf,
    },
    /// Состояние режима редактирования изменилось (FR-025, FR-035).
    EditorModeChanged { mode: EditorMode },
    /// Признак «изменён, не применён» у конфига (FR-026). UI показывает
    /// действие применения и **не** применяет конфиг самостоятельно.
    EditorConfigDiskStateChanged { disk_state: ConfigDiskState },
}
