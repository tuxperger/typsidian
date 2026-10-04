//! Когда делать автоматический снимок и обмен (фича 011, research §12,
//! FR-019, FR-031, FR-032).
//!
//! Чистая функция от фактов и времени. Таймер живёт в оболочке и только
//! спрашивает ядро — так правило проверяется без часов, а платформенные
//! таймеры (tokio на native, `setTimeout` в браузере) остаются связующим слоем.

use serde::{Deserialize, Serialize};

use super::keeper::KeeperState;
use super::syncthing::FolderState;
use super::Millis;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum AutoDecision {
    /// Ждать до указанного времени и спросить снова.
    Wait {
        until: Millis,
    },
    Snapshot,
    Blocked {
        reason: BlockReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum BlockReason {
    Disabled,
    NoHistory,
    NotKeeper,
    KeeperDisputed,
    /// Syncthing ещё доставляет файлы: снимок поймал бы половину правок.
    SyncthingBusy,
    PullConflictsPending,
    InProgressOperation,
    NothingChanged,
}

/// Факты для решения об автоснимке.
#[derive(Debug, Clone)]
pub struct SnapshotFacts {
    pub enabled: bool,
    pub has_history: bool,
    pub keeper: KeeperState,
    /// Пусто — состояние Syncthing неизвестно; тогда не ждём его.
    pub syncthing: Option<FolderState>,
    pub pull_conflicts: bool,
    pub in_progress: bool,
    pub has_changes: bool,
    /// Когда файлы vault менялись последний раз (своими правками или
    /// доставкой Syncthing — FR-031 не различает).
    pub last_change_at: Option<Millis>,
    pub pause_ms: Millis,
}

pub fn decide(now: Millis, facts: &SnapshotFacts) -> AutoDecision {
    use BlockReason::*;
    let blocked = |reason| AutoDecision::Blocked { reason };
    if !facts.enabled {
        return blocked(Disabled);
    }
    if !facts.has_history {
        return blocked(NoHistory);
    }
    match facts.keeper {
        KeeperState::Disputed => return blocked(KeeperDisputed),
        // Автоснимки — только на хранителе (FR-030, FR-031). Без назначенного
        // хранителя их делать некому: «любое устройство» здесь значило бы
        // «все сразу».
        KeeperState::OtherDevice | KeeperState::Unassigned => return blocked(NotKeeper),
        KeeperState::ThisDevice => {}
    }
    if facts.in_progress {
        return blocked(InProgressOperation);
    }
    if facts.pull_conflicts {
        return blocked(PullConflictsPending);
    }
    if !facts.has_changes {
        return blocked(NothingChanged);
    }
    if matches!(
        facts.syncthing,
        Some(FolderState::Syncing | FolderState::Scanning)
    ) {
        return blocked(SyncthingBusy);
    }
    let quiet_since = facts.last_change_at.unwrap_or(now);
    let due = quiet_since + facts.pause_ms;
    if now >= due {
        AutoDecision::Snapshot
    } else {
        AutoDecision::Wait { until: due }
    }
}

/// Пора ли автоматически получать снимки с сервера (FR-019).
pub fn pull_due(now: Millis, last_fetch: Option<Millis>, interval_ms: Option<Millis>) -> bool {
    match (interval_ms, last_fetch) {
        (Some(_), None) => true,
        (Some(interval), Some(last)) => now >= last + interval,
        (None, _) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> SnapshotFacts {
        SnapshotFacts {
            enabled: true,
            has_history: true,
            keeper: KeeperState::ThisDevice,
            syncthing: Some(FolderState::Idle),
            pull_conflicts: false,
            in_progress: false,
            has_changes: true,
            last_change_at: Some(1_000),
            pause_ms: 300_000,
        }
    }

    #[test]
    fn waits_for_the_pause_then_snapshots() {
        assert_eq!(
            decide(2_000, &facts()),
            AutoDecision::Wait { until: 301_000 }
        );
        assert_eq!(decide(301_000, &facts()), AutoDecision::Snapshot);
    }

    #[test]
    fn every_block_reason_is_reachable() {
        let cases: Vec<(Box<dyn Fn(&mut SnapshotFacts)>, BlockReason)> = vec![
            (Box::new(|f| f.enabled = false), BlockReason::Disabled),
            (Box::new(|f| f.has_history = false), BlockReason::NoHistory),
            (
                Box::new(|f| f.keeper = KeeperState::OtherDevice),
                BlockReason::NotKeeper,
            ),
            (
                Box::new(|f| f.keeper = KeeperState::Unassigned),
                BlockReason::NotKeeper,
            ),
            (
                Box::new(|f| f.keeper = KeeperState::Disputed),
                BlockReason::KeeperDisputed,
            ),
            (
                Box::new(|f| f.in_progress = true),
                BlockReason::InProgressOperation,
            ),
            (
                Box::new(|f| f.pull_conflicts = true),
                BlockReason::PullConflictsPending,
            ),
            (
                Box::new(|f| f.has_changes = false),
                BlockReason::NothingChanged,
            ),
            (
                Box::new(|f| f.syncthing = Some(FolderState::Syncing)),
                BlockReason::SyncthingBusy,
            ),
        ];
        for (tweak, reason) in cases {
            let mut f = facts();
            tweak(&mut f);
            assert_eq!(decide(10_000_000, &f), AutoDecision::Blocked { reason });
        }
    }

    #[test]
    fn unknown_syncthing_state_does_not_block() {
        let mut f = facts();
        f.syncthing = None;
        assert_eq!(decide(10_000_000, &f), AutoDecision::Snapshot);
    }

    #[test]
    fn pull_interval() {
        assert!(!pull_due(10, Some(5), None));
        assert!(pull_due(10, None, Some(60_000)));
        assert!(!pull_due(10, Some(5), Some(60_000)));
        assert!(pull_due(60_005, Some(5), Some(60_000)));
    }
}
