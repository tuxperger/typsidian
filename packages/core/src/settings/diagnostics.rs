//! Сведения раздела «Дополнительно» (FR-039). Существуют только в рантайме, в
//! файл настроек не пишутся.
//!
//! Ограничения (FR-041, FR-042, принцип IX): содержимое заметок и досок сюда не
//! входит, пути — только относительно корня vault, автоматической отправки
//! куда-либо нет и быть не может — тип не несёт ни одного способа её выполнить.

use super::editor_mode::{BridgeAvailability, EditorMode};
use super::store::{StorageState, StoreCapabilities};
use serde::{Deserialize, Serialize};

/// Диагностика приложения (data-model.md «Diagnostics»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Diagnostics {
    pub app_version: String,
    /// Текущее состояние моста, включая причину отказа.
    pub editor_mode: EditorMode,
    /// Доступен ли полноценный режим на этом устройстве и почему нет.
    pub editor_mode_availability: BridgeAvailability,
    /// Сведения об окружении, влияющие на доступность полноценного режима.
    pub environment: Environment,
    pub storage: StorageDiagnostics,
}

/// Сведения об окружении. Перечислимые поля, а не свободный дамп: всё, что
/// сюда попадает, пользователь может отправить в поддержку вручную (FR-042),
/// поэтому состав ограничен осознанно и не включает ни путей, ни имён.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Environment {
    /// `native` либо `web` — определяет, возможен ли полноценный режим вообще.
    pub target: String,
    /// Семейство ОС (`linux`, `macos`, `windows`, `unknown` для web).
    pub os_family: String,
    /// Может ли платформа порождать внешние процессы (research.md §7).
    pub can_spawn_processes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct StorageDiagnostics {
    pub capabilities: StoreCapabilities,
    pub state: StorageState,
}

/// Версия приложения, взятая из манифеста крейта.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Сведения об окружении текущей сборки.
pub fn environment() -> Environment {
    #[cfg(target_arch = "wasm32")]
    {
        Environment {
            target: "web".to_string(),
            os_family: "unknown".to_string(),
            // Веб-клиент не может порождать внешние процессы — отсюда
            // постоянная недоступность полноценного режима (research.md §7).
            can_spawn_processes: false,
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Environment {
            target: "native".to_string(),
            os_family: std::env::consts::FAMILY.to_string(),
            can_spawn_processes: true,
        }
    }
}

/// Собирает диагностику (FR-039).
///
/// Функция намеренно чистая и ничего не отправляет: FR-042 и принцип IX
/// требуют, чтобы автоматической передачи не существовало, а не чтобы она была
/// выключена флагом. Передать диагностику может только пользователь, скопировав
/// её из интерфейса.
pub fn collect(
    editor_mode: EditorMode,
    editor_mode_availability: BridgeAvailability,
    capabilities: StoreCapabilities,
    state: StorageState,
) -> Diagnostics {
    Diagnostics {
        app_version: APP_VERSION.to_string(),
        editor_mode,
        editor_mode_availability,
        environment: environment(),
        storage: StorageDiagnostics {
            capabilities,
            state,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::editor_mode::OutageReason;

    fn capabilities() -> StoreCapabilities {
        StoreCapabilities {
            location: "/home/user/.config/typsidian/settings.json".to_string(),
            can_reveal: true,
            writable: true,
        }
    }

    #[test]
    fn diagnostics_carry_mode_reason_and_storage_state() {
        let diagnostics = collect(
            EditorMode::PlainInput {
                reason: OutageReason::BackendStopped,
            },
            BridgeAvailability::Unavailable {
                reason: OutageReason::BackendStopped,
            },
            capabilities(),
            StorageState::ReadOnlyForSession,
        );

        assert_eq!(diagnostics.app_version, APP_VERSION);
        assert!(matches!(
            diagnostics.editor_mode,
            EditorMode::PlainInput { .. }
        ));
        assert_eq!(diagnostics.storage.state, StorageState::ReadOnlyForSession);
    }

    #[test]
    fn environment_reports_process_capability_per_target() {
        let environment = environment();
        #[cfg(target_arch = "wasm32")]
        assert!(!environment.can_spawn_processes);
        #[cfg(not(target_arch = "wasm32"))]
        assert!(environment.can_spawn_processes);
        assert!(!environment.target.is_empty());
    }
}
