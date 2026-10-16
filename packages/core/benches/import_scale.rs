//! Бенчмарк импорта из Obsidian (фича 021, T023, SC-004): 2 000 заметок,
//! полный цикл `import_begin` → `import_step` × N → `import_finish` над
//! `NativeBackend` на диске.
//!
//! Это путь ядра нативно, а не окно рабочего стола. Отмена проверяется между
//! шагами, поэтому наибольшее время шага — верхняя оценка отклика отмены
//! (бюджет SC-004 — 2 с). Бюджет всего импорта — 2 мин.
//!
//! Источник готовит `scripts/dev/gen-obsidian.sh`. Отдельно меряется шаг на
//! заметке в 1 МБ: порог плана для wasm — 50 мс (в wasm здесь не меряется).
//!
//! Прогон: `cargo bench --bench import_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::path::Path;
    use std::process::Command;
    use std::time::{Duration, Instant};

    use typsidian_core::import::{import_begin, import_finish, import_step, ImportJob};
    use typsidian_core::vault::backend::{ReadOnly, VaultBackend};
    use typsidian_core::vault::native::NativeBackend;

    const NOTES: usize = 2000;
    const TOTAL_BUDGET: Duration = Duration::from_secs(120);
    const CANCEL_BUDGET: Duration = Duration::from_secs(2);

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

    fn job() -> ImportJob {
        ImportJob {
            destination: "Импорт".into(),
            copy_other_files: true,
            ..ImportJob::default()
        }
    }

    // Источник — настоящий вывод генератора.
    let source_dir = tempfile::TempDir::new().expect("временный каталог");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/dev/gen-obsidian.sh");
    let generated = source_dir.path().join("vault");
    let status = Command::new("bash")
        .arg(&script)
        .arg(&generated)
        .arg(NOTES.to_string())
        .status()
        .expect("запуск генератора");
    assert!(status.success(), "генератор не отработал");

    let dest_dir = tempfile::TempDir::new().expect("временный каталог");
    let (begin, steps, finish, total_steps) = runtime.block_on(async {
        let source = ReadOnly::new(NativeBackend::open_read_only(&generated).expect("источник"));
        let mut vault = NativeBackend::open(dest_dir.path()).await.expect("vault");

        let started = Instant::now();
        let mut state = import_begin(&source, &vault, &job()).await;
        let begin = started.elapsed();
        let total = state.total;

        let mut steps = Vec::with_capacity(total as usize);
        while state.next < state.total {
            let started = Instant::now();
            state = import_step(&source, &mut vault, state).await.expect("шаг");
            steps.push(started.elapsed());
        }
        let started = Instant::now();
        import_finish(&mut vault, &state).await.expect("итог");
        println!(
            "итог: чисто {}, с потерями {}, пропущено {}",
            state.clean, state.lossy, state.skipped
        );
        (begin, steps, started.elapsed(), total)
    });

    let sum: Duration = steps.iter().sum();
    let max = *steps.iter().max().expect("шаги были");
    let mut sorted = steps.clone();
    sorted.sort();
    let median = sorted[sorted.len() / 2];
    let p99 = sorted[sorted.len() * 99 / 100];
    let all = begin + sum + finish;
    println!("шагов в плане:               {total_steps} ({NOTES} заметок и картинки)");
    println!("import_begin (план):         {begin:?}");
    println!(
        "шаги, сумма:                 {sum:?}; среднее {:?}, медиана {median:?}, p99 {p99:?}, максимум {max:?}",
        sum / steps.len() as u32
    );
    println!("import_finish (журнал, отчёт): {finish:?}");
    println!("весь цикл:                   {all:?} — бюджет {TOTAL_BUDGET:?}");
    println!("отклик отмены (максимум шага): {max:?} — бюджет {CANCEL_BUDGET:?}");

    // Шаг на заметке в 1 МБ (порог плана для wasm — 50 мс).
    let big_source = tempfile::TempDir::new().expect("временный каталог");
    let mut big = String::from("# Большая заметка\n\n");
    let mut n = 0;
    while big.len() < 1024 * 1024 {
        big.push_str(&format!(
            "Абзац {n} с **жирным**, ссылкой [[Другая]] и формулой $x_{n}^2$.\n\n- пункт {n}\n\n"
        ));
        n += 1;
    }
    std::fs::write(big_source.path().join("Большая.md"), &big).expect("большая заметка");
    let big_dest = tempfile::TempDir::new().expect("временный каталог");
    let big_step = runtime.block_on(async {
        let source =
            ReadOnly::new(NativeBackend::open_read_only(big_source.path()).expect("источник"));
        let mut vault = NativeBackend::open(big_dest.path()).await.expect("vault");
        let state = import_begin(&source, &vault, &job()).await;
        let started = Instant::now();
        let state = import_step(&source, &mut vault, state).await.expect("шаг");
        let took = started.elapsed();
        import_finish(&mut vault, &state).await.expect("итог");
        took
    });
    println!(
        "шаг на заметке {} КБ:        {big_step:?} (порог плана для wasm — 50 мс, в wasm не мерялось)",
        big.len() / 1024
    );

    assert!(all <= TOTAL_BUDGET, "SC-004: импорт {all:?}");
    assert!(max <= CANCEL_BUDGET, "SC-004: отмена {max:?}");
    println!("пороги SC-004 (нативно) выдержаны");
}

#[cfg(target_arch = "wasm32")]
fn main() {}
