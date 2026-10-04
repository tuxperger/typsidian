//! Тождество устройства (фича 011, data-model «DeviceIdentity»).
//!
//! Нужно ровно для одного: модели «хранитель истории» (research §4). Хранитель
//! записывается в файл vault, который Syncthing разносит по всем устройствам, и
//! каждое устройство обязано узнать в нём себя — или не себя. Имя хоста для
//! этого не годится: два ноутбука с одинаковым именем — обычное дело.
//!
//! Живёт в настройках **устройства** (004) и не переезжает вместе с vault:
//! окажись он в `vault.json`, все устройства получили бы один идентификатор, и
//! хранителем стали бы все сразу.

use serde::{Deserialize, Serialize};

/// Длина идентификатора в байтах до шестнадцатеричной записи.
const ID_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DeviceIdentity {
    /// 32 шестнадцатеричных знака, случайные.
    pub id: String,
    /// Имя, которое видит пользователь: «хранитель — ноутбук». Редактируемо.
    pub name: String,
}

impl DeviceIdentity {
    /// Новое тождество со случайным идентификатором.
    ///
    /// `None` — источник случайности недоступен. Это не повод падать: без
    /// тождества недоступна только модель хранителя, а не синхронизация вообще.
    pub fn generate(name: impl Into<String>) -> Option<Self> {
        let mut bytes = [0u8; ID_BYTES];
        getrandom::getrandom(&mut bytes).ok()?;
        Some(Self {
            id: to_hex(&bytes),
            name: name.into(),
        })
    }

    /// Имя устройства по умолчанию: имя хоста, если его удаётся узнать.
    pub fn default_name() -> String {
        default_name()
    }

    /// Похож ли идентификатор на выданный [`DeviceIdentity::generate`].
    pub fn is_valid_id(id: &str) -> bool {
        id.len() == ID_BYTES * 2 && id.bytes().all(|b| b.is_ascii_hexdigit())
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn default_name() -> String {
    // Без внешнего крейта: переменные окружения и файл, которые есть на
    // Linux/macOS/Windows; Android обычно не даёт ни того, ни другого.
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|text| text.trim().to_string())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "device".to_string())
}

#[cfg(target_arch = "wasm32")]
fn default_name() -> String {
    "browser".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_are_valid_and_distinct() {
        let a = DeviceIdentity::generate("a").unwrap();
        let b = DeviceIdentity::generate("b").unwrap();
        assert!(DeviceIdentity::is_valid_id(&a.id));
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn foreign_ids_are_rejected() {
        assert!(!DeviceIdentity::is_valid_id("laptop"));
        assert!(!DeviceIdentity::is_valid_id(&"g".repeat(32)));
    }
}
