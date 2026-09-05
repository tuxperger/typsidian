use crate::vault::index::FileStamp;
use crate::vault::AccessState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Единый тип события ядро -> UI (data-model.md «Внешнее изменение»,
/// contracts/frontend-events.md). Источник (другое окно того же приложения или
/// сторонняя программа) не различается на уровне этого типа — оба обрабатываются
/// одинаково (FR-034).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "type")]
pub enum VaultEvent {
    NoteCreated {
        path: PathBuf,
    },
    /// Только если у заметки нет несохранённых правок (FR-017).
    NoteUpdated {
        path: PathBuf,
    },
    NoteRenamed {
        from_path: PathBuf,
        to_path: PathBuf,
    },
    NoteDeleted {
        path: PathBuf,
    },
    /// Вместо `NoteUpdated`, если есть несохранённые правки (FR-021).
    ConflictDetected {
        note_path: PathBuf,
        disk_snapshot: FileStamp,
    },
    /// Вместо `NoteDeleted`, если есть несохранённые правки (FR-025).
    NoteDeletedWithUnsavedEdits {
        note_path: PathBuf,
    },
    BulkChangeStarted,
    BulkChangeSettled,
    VaultAccessChanged {
        access_state: AccessState,
    },
    LinksInvalidated {
        affected_paths: Vec<PathBuf>,
    },
}

/// Порог, начиная с которого пачка обнаруженных изменений считается
/// массовой (US3 AS5, FR-019, SC-005) и оборачивается
/// `BulkChangeStarted`/`BulkChangeSettled` вместо потока отдельных событий
/// (T055). Значение выбрано эвристически: заметно больше типичной
/// единичной операции пользователя (rename каталога с несколькими
/// вложенными заметками, atomic-write через tmp+rename и т.п. — обычно
/// единицы событий за один цикл обработки), но многократно меньше
/// эталонного бенчмарка массового изменения из SC-005 (1000 файлов) — так
/// что настоящий bulk-сценарий гарантированно пересекает порог, а обычные
/// одиночные операции пользователя — нет. Общая точка для `NativeBackend`
/// (`watch()`/сверка при открытии) и `WebBackend` (`pollForChanges`), чтобы
/// эвристика не расходилась между backend (принцип II).
pub const BULK_CHANGE_THRESHOLD: usize = 20;

/// Собирает событие FR-030 (T087) — «набор битых ссылок изменился». Вызывается,
/// когда заметка появилась, исчезла или переехала: ровно эти три случая меняют
/// статус входящих на неё ссылок. `None` для пустого списка, чтобы вызывающий
/// код не проверял `is_empty()` отдельно и не публиковал пустое событие.
pub fn links_invalidated(mut affected_paths: Vec<PathBuf>) -> Option<VaultEvent> {
    affected_paths.sort();
    affected_paths.dedup();
    if affected_paths.is_empty() {
        None
    } else {
        Some(VaultEvent::LinksInvalidated { affected_paths })
    }
}

/// Оборачивает пачку событий `BulkChangeStarted`/`BulkChangeSettled`, если её
/// размер достиг [`BULK_CHANGE_THRESHOLD`]. Пустая пачка остаётся пустой
/// (вызывающий код не обязан отдельно проверять `is_empty()` перед вызовом).
pub fn wrap_bulk_if_needed(mut events: Vec<VaultEvent>) -> Vec<VaultEvent> {
    if events.len() >= BULK_CHANGE_THRESHOLD {
        let mut wrapped = Vec::with_capacity(events.len() + 2);
        wrapped.push(VaultEvent::BulkChangeStarted);
        wrapped.append(&mut events);
        wrapped.push(VaultEvent::BulkChangeSettled);
        wrapped
    } else {
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_batch_is_not_wrapped() {
        let events = vec![VaultEvent::NoteCreated {
            path: PathBuf::from("a.typ"),
        }];
        let wrapped = wrap_bulk_if_needed(events.clone());
        assert_eq!(wrapped.len(), events.len());
        assert!(!matches!(wrapped[0], VaultEvent::BulkChangeStarted));
    }

    #[test]
    fn batch_at_threshold_is_wrapped_with_started_and_settled() {
        let events: Vec<VaultEvent> = (0..BULK_CHANGE_THRESHOLD)
            .map(|i| VaultEvent::NoteCreated {
                path: PathBuf::from(format!("{i}.typ")),
            })
            .collect();
        let wrapped = wrap_bulk_if_needed(events);
        assert!(matches!(
            wrapped.first(),
            Some(VaultEvent::BulkChangeStarted)
        ));
        assert!(matches!(
            wrapped.last(),
            Some(VaultEvent::BulkChangeSettled)
        ));
        assert_eq!(wrapped.len(), BULK_CHANGE_THRESHOLD + 2);
    }

    #[test]
    fn links_invalidated_is_deduplicated_and_empty_stays_silent() {
        assert!(links_invalidated(Vec::new()).is_none());
        let event = links_invalidated(vec![
            PathBuf::from("b.typ"),
            PathBuf::from("a.typ"),
            PathBuf::from("b.typ"),
        ])
        .expect("непустой список должен дать событие");
        match event {
            VaultEvent::LinksInvalidated { affected_paths } => {
                assert_eq!(
                    affected_paths,
                    vec![PathBuf::from("a.typ"), PathBuf::from("b.typ")]
                );
            }
            other => panic!("ожидалось LinksInvalidated, получено {other:?}"),
        }
    }

    #[test]
    fn batch_below_threshold_by_one_is_not_wrapped() {
        let events: Vec<VaultEvent> = (0..BULK_CHANGE_THRESHOLD - 1)
            .map(|i| VaultEvent::NoteCreated {
                path: PathBuf::from(format!("{i}.typ")),
            })
            .collect();
        let wrapped = wrap_bulk_if_needed(events);
        assert!(!wrapped
            .iter()
            .any(|e| matches!(e, VaultEvent::BulkChangeStarted)));
    }
}
