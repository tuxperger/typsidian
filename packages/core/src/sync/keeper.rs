//! Хранитель истории (фича 011, research §4, FR-029…FR-032).
//!
//! Чистая логика: кто хранитель, является ли им это устройство и не спорят ли
//! устройства о нём. Спор возникает, когда назначение поменяли на двух
//! устройствах почти одновременно: Syncthing оставляет одно значение в
//! `vault.json`, а второе кладёт в конфликтную копию. Каждое из двух устройств
//! тогда видит «хранитель — я», и обе истории начали бы расходиться — ровно
//! то, от чего модель хранителя защищает.

use serde::{Deserialize, Serialize};

use super::settings::Keeper;
use super::DeviceIdentity;

/// Положение этого устройства относительно хранителя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum KeeperState {
    /// Хранитель не назначен: снимки может делать любое устройство (vault без
    /// Syncthing — обычный репозиторий).
    Unassigned,
    ThisDevice,
    OtherDevice,
    /// Назначение неоднозначно — автоснимки остановлены везде (FR-032).
    Disputed,
}

impl KeeperState {
    /// Может ли устройство делать снимки и обмениваться с сервером без
    /// отдельного подтверждения (FR-030).
    pub fn may_write_history(&self) -> bool {
        matches!(self, KeeperState::Unassigned | KeeperState::ThisDevice)
    }
}

/// Что известно для оценки.
pub struct KeeperFacts<'a> {
    pub keeper: Option<&'a Keeper>,
    pub device: Option<&'a DeviceIdentity>,
    /// Хранитель, которого устройство видело в прошлый раз.
    pub last_seen: Option<&'a str>,
    /// В vault есть конфликтная копия файла настроек.
    pub settings_conflict_copy: bool,
}

pub fn assess(facts: &KeeperFacts<'_>) -> KeeperState {
    if facts.settings_conflict_copy && facts.keeper.is_some() {
        return KeeperState::Disputed;
    }
    let Some(keeper) = facts.keeper else {
        return KeeperState::Unassigned;
    };
    match facts.device {
        Some(device) if device.id == keeper.device_id => KeeperState::ThisDevice,
        // Без своего тождества устройство не может быть хранителем.
        _ => KeeperState::OtherDevice,
    }
}

/// Стало ли устройство не-хранителем «за спиной»: в прошлый раз хранителем было
/// оно само, а теперь в файле — другое. Это не спор (назначение однозначно), но
/// пользователь должен увидеть, что его устройство больше не делает снимков.
pub fn lost_keeper_role(facts: &KeeperFacts<'_>) -> bool {
    match (facts.device, facts.last_seen, facts.keeper) {
        (Some(device), Some(last_seen), Some(keeper)) => {
            last_seen == device.id && keeper.device_id != device.id
        }
        (Some(device), Some(last_seen), None) => last_seen == device.id,
        _ => false,
    }
}

/// Новое назначение хранителем этого устройства.
pub fn appoint(device: &DeviceIdentity, now: i64) -> Keeper {
    Keeper {
        device_id: device.id.clone(),
        device_name: device.name.clone(),
        since: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str) -> DeviceIdentity {
        DeviceIdentity {
            id: id.repeat(32),
            name: id.to_string(),
        }
    }

    #[test]
    fn states_follow_the_assignment() {
        let me = device("a");
        let other = device("b");
        let keeper_me = appoint(&me, 1);
        let keeper_other = appoint(&other, 1);
        let facts = |keeper| KeeperFacts {
            keeper,
            device: Some(&me),
            last_seen: None,
            settings_conflict_copy: false,
        };
        assert_eq!(assess(&facts(None)), KeeperState::Unassigned);
        assert_eq!(assess(&facts(Some(&keeper_me))), KeeperState::ThisDevice);
        assert_eq!(
            assess(&facts(Some(&keeper_other))),
            KeeperState::OtherDevice
        );
    }

    #[test]
    fn a_conflict_copy_of_the_settings_is_a_dispute() {
        let me = device("a");
        let keeper = appoint(&me, 1);
        let facts = KeeperFacts {
            keeper: Some(&keeper),
            device: Some(&me),
            last_seen: Some(&keeper.device_id),
            settings_conflict_copy: true,
        };
        assert_eq!(assess(&facts), KeeperState::Disputed);
        assert!(!assess(&facts).may_write_history());
    }

    #[test]
    fn losing_the_role_is_noticed() {
        let me = device("a");
        let other = appoint(&device("b"), 2);
        let facts = KeeperFacts {
            keeper: Some(&other),
            device: Some(&me),
            last_seen: Some(&me.id),
            settings_conflict_copy: false,
        };
        assert!(lost_keeper_role(&facts));
    }
}
