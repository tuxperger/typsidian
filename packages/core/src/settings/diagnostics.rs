//! Сведения раздела «Дополнительно» (FR-039). Существуют только в рантайме, в
//! файл настроек не пишутся.
//!
//! Ограничения (FR-041, FR-042, принцип IX): содержимое заметок и досок сюда не
//! входит, пути — только относительно корня vault, автоматической отправки
//! куда-либо нет и быть не может — тип не несёт ни одного способа её выполнить.

use super::store::{StorageState, StoreCapabilities};
use serde::{Deserialize, Serialize};

/// Диагностика приложения (data-model.md «Diagnostics»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Diagnostics {
    pub app_version: String,
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
    /// `native` либо `web`.
    pub target: String,
    /// Семейство ОС (`unix`, `windows`, `unknown` для web).
    pub os_family: String,
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
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Environment {
            target: "native".to_string(),
            os_family: std::env::consts::FAMILY.to_string(),
        }
    }
}

/// Собирает диагностику (FR-039).
///
/// Функция намеренно чистая и ничего не отправляет: FR-042 и принцип IX
/// требуют, чтобы автоматической передачи не существовало, а не чтобы она была
/// выключена флагом. Передать диагностику может только пользователь, скопировав
/// её из интерфейса.
pub fn collect(capabilities: StoreCapabilities, state: StorageState) -> Diagnostics {
    Diagnostics {
        app_version: APP_VERSION.to_string(),
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

    fn capabilities() -> StoreCapabilities {
        StoreCapabilities {
            location: "/home/user/.config/typsidian/settings.json".to_string(),
            can_reveal: true,
            writable: true,
        }
    }

    #[test]
    fn diagnostics_carry_version_and_storage_state() {
        let diagnostics = collect(capabilities(), StorageState::ReadOnlyForSession);

        assert_eq!(diagnostics.app_version, APP_VERSION);
        assert_eq!(diagnostics.storage.state, StorageState::ReadOnlyForSession);
    }

    #[test]
    fn environment_names_the_target() {
        let environment = environment();
        #[cfg(target_arch = "wasm32")]
        assert_eq!(environment.target, "web");
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(environment.target, "native");
        assert!(!environment.os_family.is_empty());
    }
}
