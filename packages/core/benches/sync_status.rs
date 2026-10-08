//! Бенчмарк сводки и списка изменений истории (фича 011, T067): подтверждает
//! SC-002 — обзор и изменения за 2 с на 5 000 заметок.
//!
//! `harness = false` и ручной замер — по тем же причинам, что у `vault_scale`:
//! измеряется сквозной сценарий с диском (настоящий libgit2 поверх настоящей
//! папки), а не микрооперация.
//!
//! Прогон: `cargo bench --bench sync_status`. Порог взят из spec.md и зависит
//! от оборудования: на медленном диске бенч честно сообщит о непрохождении.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::sync::engine::native::NativeGitEngine;
    use typsidian_core::sync::{
        Author, DeviceIdentity, DeviceVaultSync, HistoryState, ShellCapabilities, SyncContext,
        SyncSession,
    };
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    /// SC-002: сводка и список изменений.
    const BUDGET: Duration = Duration::from_secs(2);
    const NOTE_COUNT: usize = 5_000;
    const NOTES_PER_FOLDER: usize = 100;
    /// Сколько заметок изменено между снимком и замером — обычный рабочий
    /// день, а не «всё сразу»: статус обязан быть быстрым именно тогда, когда
    /// почти ничего не менялось.
    const CHANGED: usize = 25;

    let dir = tempfile::TempDir::new().expect("временный каталог");
    let root = dir.path();
    let note_path = |i: usize| {
        root.join(format!("folder-{:03}", i / NOTES_PER_FOLDER))
            .join(format!("note-{i:05}.typ"))
    };
    for i in 0..NOTE_COUNT {
        let path = note_path(i);
        std::fs::create_dir_all(path.parent().expect("папка заметки")).expect("подпапка");
        std::fs::write(&path, format!("= Заметка {i}\n\nтекст\n")).expect("запись заметки");
    }

    let ctx = |now: i64| SyncContext {
        device: Some(DeviceIdentity {
            id: "a".repeat(32),
            name: "бенчмарк".into(),
        }),
        device_sync: DeviceVaultSync {
            author: Some(Author {
                name: "Бенчмарк".into(),
                email: "bench@example.com".into(),
            }),
            ..DeviceVaultSync::default()
        },
        now,
        capabilities: ShellCapabilities::current(false),
    };

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let (overview_took, changes_took, seen) = runtime.block_on(async {
        let mut backend = NativeBackend::open(root).await.expect("открытие vault");
        let mut session = SyncSession::new(NativeGitEngine::open(root), None, "bench");

        let started = Instant::now();
        session
            .enable_history(&mut backend, &ctx(1_700_000_000_000), "Первый снимок")
            .await
            .expect("включение истории");
        eprintln!(
            "первый снимок {NOTE_COUNT} заметок: {:?}",
            started.elapsed()
        );

        for i in 0..CHANGED {
            std::fs::write(
                note_path(i * (NOTE_COUNT / CHANGED)),
                format!("= Заметка {i}\n\nправка\n"),
            )
            .expect("правка заметки");
        }

        let started = Instant::now();
        let overview = session.overview(&backend, &ctx(1_700_000_100_000)).await;
        let overview_took = started.elapsed();

        let started = Instant::now();
        let changes = session.changes(&backend).await.expect("список изменений");
        let changes_took = started.elapsed();

        let counted = match overview.history {
            HistoryState::Local { changes, .. } => changes as usize,
            other => panic!("история не включилась: {other:?}"),
        };
        assert_eq!(counted, changes.len(), "сводка и список расходятся");
        (overview_took, changes_took, changes.len())
    });

    assert_eq!(seen, CHANGED, "замечены не все изменённые заметки");
    eprintln!("сводка: {overview_took:?} (порог {BUDGET:?})");
    eprintln!("список изменений: {changes_took:?} (порог {BUDGET:?})");

    let mut failed = false;
    for (name, took) in [
        ("сводка", overview_took),
        ("список изменений", changes_took),
    ] {
        if took > BUDGET {
            eprintln!("SC-002 НЕ ВЫПОЛНЕН: {name} — {took:?} > {BUDGET:?}");
            failed = true;
        }
    }
    if failed {
        std::process::exit(1);
    }
    eprintln!("SC-002 выполнен на {NOTE_COUNT} заметках");
}

#[cfg(target_arch = "wasm32")]
fn main() {}
