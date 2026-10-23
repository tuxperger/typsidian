//! Бенчмарк снимков заметок (фича 029, T011).
//!
//! - SC-003: история заметки из 200 снимков открывается не дольше 500 мс —
//!   меряется то, что делает окно истории при открытии: `snapshot_history` и
//!   различия выбранного (новейшего) снимка с текущим текстом.
//! - SC-001: снимок заметки на 10 000 строк не задерживает набор (≤ 50 мс).
//!   Снимок идёт после сохранения и мимо набора, но и сам вызов
//!   `snapshot_take` укладывается в бюджет задержки, так что даже на одном
//!   потоке (web) он не заметен.
//!
//! Ручной замер и `harness = false` — по тем же причинам, что у `vault_scale`.
//! Меряется лучший из нескольких прогонов поверх `NativeBackend`.
//!
//! Прогон: `cargo bench --bench snapshots_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::snapshots::{
        snapshot_diff, snapshot_history, snapshot_take, SnapshotReason, TakeOutcome,
    };
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    const HISTORY_BUDGET: Duration = Duration::from_millis(500);
    const TAKE_BUDGET: Duration = Duration::from_millis(50);
    const SNAPSHOTS: usize = 200;
    const BIG_LINES: usize = 10_000;
    const RUNS: usize = 5;
    const MINUTE: i64 = 60_000;
    /// 2026-10-22 12:00 UTC.
    const T0: i64 = 1_792_670_400_000;

    // Заметка в 300 строк, у каждой версии правится своя строка.
    let version = |n: usize| {
        (0..300)
            .map(|line| {
                if line == n % 300 {
                    format!("Строка {line}: правка номер {n}.\n")
                } else {
                    format!("Строка {line}: обычный текст заметки.\n")
                }
            })
            .collect::<String>()
    };

    let dir = tempfile::TempDir::new().expect("временный каталог");
    std::fs::write(dir.path().join("note.typ"), version(0)).expect("запись заметки");
    std::fs::write(dir.path().join("big.typ"), "").expect("запись заметки");

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut backend = runtime
        .block_on(NativeBackend::open(dir.path()))
        .expect("открытие vault");

    // Снимки раз в 6 минут: интервал по умолчанию — 5, срок — 7 дней.
    let mut now = T0;
    for n in 0..SNAPSHOTS {
        let outcome = runtime.block_on(snapshot_take(
            &mut backend,
            "note.typ",
            &version(n),
            SnapshotReason::Save,
            now,
        ));
        assert!(
            matches!(outcome, TakeOutcome::Taken { .. }),
            "снимок {n}: {outcome:?}"
        );
        now += 6 * MINUTE;
    }

    let current = version(SNAPSHOTS);
    let mut entries = 0;
    let history_took = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            let history = runtime.block_on(snapshot_history(&backend, "note.typ", now));
            let newest = &history.entries[0];
            let diff = runtime
                .block_on(snapshot_diff(&backend, &newest.id, &current))
                .expect("различия");
            let took = started.elapsed();
            assert!(!diff.is_empty());
            entries = history.entries.len();
            took
        })
        .min()
        .expect("прогоны");
    assert_eq!(entries, SNAPSHOTS, "в истории все снимки");
    println!("SC-003 история из {entries} снимков: {history_took:?} (бюджет {HISTORY_BUDGET:?})");

    // Каждый прогон — новый текст, иначе ядро пропустит дубль; `Open` не ждёт
    // интервала.
    let big = |run: usize| {
        (0..BIG_LINES)
            .map(|line| format!("Строка {line} большой заметки, прогон {run}.\n"))
            .collect::<String>()
    };
    let take_took = (0..RUNS)
        .map(|run| {
            let text = big(run);
            now += MINUTE;
            let started = Instant::now();
            let outcome = runtime.block_on(snapshot_take(
                &mut backend,
                "big.typ",
                &text,
                SnapshotReason::Open,
                now,
            ));
            let took = started.elapsed();
            assert!(
                matches!(outcome, TakeOutcome::Taken { .. }),
                "снимок большой заметки: {outcome:?}"
            );
            took
        })
        .min()
        .expect("прогоны");
    println!("SC-001 снимок заметки на {BIG_LINES} строк: {take_took:?} (бюджет {TAKE_BUDGET:?})");

    let mut failed = false;
    if history_took > HISTORY_BUDGET {
        eprintln!("НЕ ВЫДЕРЖАН SC-003 — {history_took:?}, бюджет {HISTORY_BUDGET:?}");
        failed = true;
    }
    if take_took > TAKE_BUDGET {
        eprintln!("НЕ ВЫДЕРЖАН SC-001 — {take_took:?}, бюджет {TAKE_BUDGET:?}");
        failed = true;
    }
    if failed {
        std::process::exit(1);
    }
    println!("бюджет выдержан");
}

#[cfg(target_arch = "wasm32")]
fn main() {}
