//! Бенчмарк массового внешнего изменения (T093): подтверждает SC-005 —
//! 1000 файлов, изменённых вне приложения, приводят индекс в соответствие с
//! диском ≤15с, а интерфейс остаётся отзывчивым (задержка отклика ≤100 мс).
//!
//! `harness = false` и ручной замер — по той же причине, что и в
//! `vault_scale.rs`: это сквозной сценарий, а не микрооперация.
//!
//! Прогон: `cargo bench --bench vault_bulk_change`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::path::Path;
    use std::time::{Duration, Instant};
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    /// SC-005: сходимость к состоянию диска после массового изменения.
    const CONVERGENCE_BUDGET: Duration = Duration::from_secs(15);
    /// SC-005/FR-019: отклик на действие пользователя во время обработки.
    const RESPONSIVENESS_BUDGET: Duration = Duration::from_millis(100);
    const NOTE_COUNT: usize = 1000;

    let dir = tempfile::TempDir::new().expect("временный каталог");
    let root = dir.path();
    for i in 0..NOTE_COUNT {
        std::fs::write(
            root.join(format!("note-{i:04}.typ")),
            format!("исходное содержимое {i}\n"),
        )
        .expect("запись заметки");
    }

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let mut backend = NativeBackend::open(root).await.expect("открытие vault");
        assert_eq!(backend.list().await.len(), NOTE_COUNT);
        // Дать осесть событиям самого открытия, чтобы измерять именно
        // реакцию на массовое изменение.
        tokio::time::sleep(Duration::from_millis(600)).await;
        backend.poll_events();

        // Массовое изменение вне приложения (аналог `git checkout` между
        // ветками с массовыми правками).
        let change_started = Instant::now();
        for i in 0..NOTE_COUNT {
            std::fs::write(
                root.join(format!("note-{i:04}.typ")),
                format!("содержимое после массового изменения {i}\n"),
            )
            .expect("массовая перезапись");
        }

        // Отзывчивость измеряется в момент, когда обработка заведомо идёт:
        // обычный пользовательский вызов не должен ждать её завершения.
        let responsiveness_started = Instant::now();
        let _ = backend.list().await;
        let responsiveness = responsiveness_started.elapsed();

        let mut converged = None;
        let mut saw_bulk_markers = (false, false);
        let deadline = Instant::now() + CONVERGENCE_BUDGET + Duration::from_secs(5);
        while Instant::now() < deadline {
            for event in backend.poll_events() {
                match event {
                    typsidian_core::vault::events::VaultEvent::BulkChangeStarted => {
                        saw_bulk_markers.0 = true;
                    }
                    typsidian_core::vault::events::VaultEvent::BulkChangeSettled => {
                        saw_bulk_markers.1 = true;
                    }
                    _ => {}
                }
            }
            let all_updated = backend
                .read(Path::new(&format!("note-{:04}.typ", NOTE_COUNT - 1)))
                .await
                .map(|content| content.contains("после массового изменения"))
                .unwrap_or(false);
            if all_updated && backend.list().await.len() == NOTE_COUNT && saw_bulk_markers.1 {
                converged = Some(change_started.elapsed());
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        println!(
            "SC-005 отзывчивость во время обработки: {responsiveness:?} (бюджет {RESPONSIVENESS_BUDGET:?})"
        );
        match converged {
            Some(elapsed) => println!(
                "SC-005 сходимость {NOTE_COUNT} изменений: {elapsed:?} (бюджет {CONVERGENCE_BUDGET:?})"
            ),
            None => println!("SC-005 сходимость: НЕ ДОСТИГНУТА в отведённое время"),
        }
        println!(
            "US3 AS5 маркеры пачки: started={} settled={}",
            saw_bulk_markers.0, saw_bulk_markers.1
        );

        let mut failures = Vec::new();
        if responsiveness > RESPONSIVENESS_BUDGET {
            failures.push(format!(
                "SC-005: отклик {responsiveness:?}, бюджет {RESPONSIVENESS_BUDGET:?}"
            ));
        }
        match converged {
            Some(elapsed) if elapsed <= CONVERGENCE_BUDGET => {}
            Some(elapsed) => failures.push(format!(
                "SC-005: сходимость {elapsed:?}, бюджет {CONVERGENCE_BUDGET:?}"
            )),
            None => failures.push("SC-005: индекс не сошёлся к состоянию диска".to_string()),
        }
        if !saw_bulk_markers.0 || !saw_bulk_markers.1 {
            failures.push("US3 AS5: пачка не обрамлена BulkChangeStarted/Settled".to_string());
        }

        if failures.is_empty() {
            println!("все бюджеты выдержаны");
        } else {
            for failure in &failures {
                eprintln!("НЕ ВЫДЕРЖАН — {failure}");
            }
            std::process::exit(1);
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // Массовое изменение на web проверяется сценарием 7 quickstart.md:
    // у File System Access API нет FS-событий, и эквивалент замера — частота
    // вызова `pollForChanges()` из JS (research.md §3).
}
