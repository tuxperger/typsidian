//! Бенчмарк отбора по свойствам (фича 020, T012): SC-001 — отбор по тегу в
//! vault из 10 000 заметок показывает результат не позже чем через 300 мс.
//!
//! Меряется весь путь, который проходит команда `vault_select`: индекс
//! `NativeBackend` (SQLite-кэш, свойства в памяти), сбор сведений о заметках и
//! сам отбор. Для сравнения — чистый отбор над готовыми сведениями (его же
//! зовёт сборка блока выборки: бюджет плана ≤ 30 мс на входы).
//!
//! `harness = false` и ручной замер — по тем же причинам, что у `vault_scale`.
//! Меряется лучший из нескольких прогонов.
//!
//! Прогон: `cargo bench --bench properties_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::properties::{
        run_selection, select_inputs, select_notes, Condition, Selection,
    };
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    const BUDGET: Duration = Duration::from_millis(300);
    const INPUTS_BUDGET: Duration = Duration::from_millis(30);
    const NOTE_COUNT: usize = 10_000;
    const NOTES_PER_FOLDER: usize = 100;
    const RUNS: usize = 5;

    let dir = tempfile::TempDir::new().expect("временный каталог");
    for i in 0..NOTE_COUNT {
        let folder = dir
            .path()
            .join(format!("folder-{:03}", i / NOTES_PER_FOLDER));
        std::fs::create_dir_all(&folder).expect("подпапка");
        // Каждая пятая заметка помечена тегом, у каждой третьей есть статус.
        let record = if i % 5 == 0 {
            format!(
                "#metadata((tags: (\"проект\", \"к{}\"), статус: \"{}\", часы: {i})) <properties>\n",
                i % 7,
                if i % 3 == 0 { "готово" } else { "в работе" }
            )
        } else {
            String::new()
        };
        std::fs::write(
            folder.join(format!("note-{i:05}.typ")),
            format!("{record}= Заметка {i}\n\nТекст.\n"),
        )
        .expect("запись заметки");
    }

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let backend = runtime
        .block_on(NativeBackend::open(dir.path()))
        .expect("открытие vault");
    let by_tag = Selection {
        conditions: vec![Condition::TagHas {
            tag: "проект".into(),
        }],
        ..Selection::default()
    };

    let best = |mut work: Box<dyn FnMut() -> usize>| -> (Duration, usize) {
        let mut shown = 0;
        let took = (0..RUNS)
            .map(|_| {
                let started = Instant::now();
                shown = work();
                started.elapsed()
            })
            .min()
            .expect("прогоны");
        (took, shown)
    };

    let (full, total) = best(Box::new(|| {
        runtime.block_on(select_notes(&backend, &by_tag)).total as usize
    }));
    assert_eq!(total, NOTE_COUNT / 5, "тег есть у каждой пятой заметки");

    let notes = runtime.block_on(backend.indexed_notes());
    let (pure, _) = best(Box::new(|| {
        run_selection(&notes, &by_tag, 0).total as usize
    }));

    let block = "#import \"/.typsidian/lib/select.typ\": select\n#select(where: ((tag: \"проект\"), (key: \"статус\", op: \"=\", value: \"готово\")), sort: \"-часы\", columns: (\"название\", \"статус\", \"часы\"))\n";
    let (inputs, _) = best(Box::new(|| {
        select_inputs(&notes, block, 0).map_or(0, |json| json.len())
    }));

    println!("SC-001 отбор по тегу, {NOTE_COUNT} заметок, весь путь: {full:?} (бюджет {BUDGET:?})");
    println!("отбор над готовыми сведениями: {pure:?}");
    println!("входы блока выборки (500 строк): {inputs:?} (порог пересмотра {INPUTS_BUDGET:?})");

    let mut failures = Vec::new();
    if full > BUDGET {
        failures.push(format!("SC-001: отбор за {full:?}, бюджет {BUDGET:?}"));
    }
    if failures.is_empty() {
        println!("бюджеты выдержаны");
    } else {
        for failure in &failures {
            eprintln!("НЕ ВЫДЕРЖАН — {failure}");
        }
        std::process::exit(1);
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
