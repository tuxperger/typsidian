//! Бенчмарк задач (фича 033, T008).
//!
//! SC-001: окно «Задачи» на 5 000 заметок открывается не дольше 1 с. Первый
//! показ строит индекс задач (читает все заметки), повторный — только
//! сверяет отпечатки и читает индекс: он и есть обычное открытие окна.
//! Меряется лучший из нескольких повторных показов; первый — для сведения.
//!
//! Ручной замер и `harness = false` — по тем же причинам, что у `vault_scale`.
//!
//! Прогон: `cargo bench --bench tasks_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::tasks::{tasks_view, TaskFilter};
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    const BUDGET: Duration = Duration::from_secs(1);
    const NOTES: usize = 5_000;
    const RUNS: usize = 5;

    let dir = tempfile::TempDir::new().expect("временный каталог");
    for n in 0..NOTES {
        let folder = dir.path().join(format!("папка-{:02}", n / 100));
        std::fs::create_dir_all(&folder).expect("папка");
        let mut text = format!("= Заметка {n}\n\nТекст заметки.\n\n");
        if n % 5 == 0 {
            text.push_str(
                "- [ ] Первая задача 📅 2026-10-30\n- [x] Готовая задача\n  - [ ] Вложенная\n",
            );
        }
        std::fs::write(folder.join(format!("заметка {n}.typ")), text).expect("запись");
    }

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut backend = runtime
        .block_on(NativeBackend::open(dir.path()))
        .expect("открытие vault");
    let filter = TaskFilter::default();

    let started = Instant::now();
    let first = runtime.block_on(tasks_view(&mut backend, &filter, "2026-10-23"));
    let cold = started.elapsed();
    assert_eq!(first.open, (NOTES / 5 * 2) as u32);

    let took = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            let view = runtime.block_on(tasks_view(&mut backend, &filter, "2026-10-23"));
            let took = started.elapsed();
            assert_eq!(view.open, first.open);
            took
        })
        .min()
        .expect("прогоны");
    println!("первый показ (строит индекс) {NOTES} заметок: {cold:?}");
    println!("SC-001 окно «Задачи», {NOTES} заметок: {took:?} (бюджет {BUDGET:?})");
    if took > BUDGET {
        eprintln!("НЕ ВЫДЕРЖАН — {took:?}, бюджет {BUDGET:?}");
        std::process::exit(1);
    }
    println!("бюджет выдержан");
}

#[cfg(target_arch = "wasm32")]
fn main() {}
