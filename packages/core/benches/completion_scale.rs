//! Бенчмарк дополнения (фича 018, T067): заметка в 10 000 строк с 500 метками —
//! список по `@` не дольше 20 мс (SC-001, бюджет 100 мс на весь путь до показа),
//! отметки не дольше 50 мс (FR-006) на нативном таргете.
//!
//! `harness = false` и ручной замер — по тем же причинам, что у `vault_scale`.
//! Меряется лучший из нескольких прогонов.
//!
//! Прогон: `cargo bench --bench completion_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::completion::{
        completions, reference_marks, CompletionRequest, LinkedIndex, MarksRequest,
    };
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    const LINES: usize = 10_000;
    const LABELS: usize = 500;
    const COMPLETIONS_BUDGET: Duration = Duration::from_millis(20);
    const MARKS_BUDGET: Duration = Duration::from_millis(50);
    const RUNS: usize = 7;

    let mut text = String::new();
    for line in 0..LINES {
        if line % (LINES / LABELS) == 0 {
            let index = line / (LINES / LABELS);
            text.push_str(&format!("= Раздел {index} <раздел-{index}>\n"));
        } else if line % 7 == 0 {
            text.push_str(&format!(
                "См. @раздел-{} и [[Заметка {}]].\n",
                line % LABELS,
                line % 30
            ));
        } else {
            text.push_str("Обычная строка текста заметки для объёма, мир, привет.\n");
        }
    }
    text.push_str("Набор @разд");
    let offset = text.encode_utf16().count() as u32;

    let dir = tempfile::TempDir::new().expect("временный каталог");
    std::fs::write(dir.path().join("главная.typ"), &text).expect("заметка");
    for index in 0..30 {
        std::fs::write(dir.path().join(format!("Заметка {index}.typ")), "= Текст\n")
            .expect("заметка");
    }
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let backend = runtime
        .block_on(NativeBackend::open(dir.path()))
        .expect("открытие vault");

    let linked = LinkedIndex {
        note_path: "главная.typ".to_owned(),
        ..LinkedIndex::default()
    };
    let request = CompletionRequest {
        note_path: "главная.typ".to_owned(),
        text: text.clone(),
        offset,
        manual: false,
        linked: linked.clone(),
        sources: Vec::new(),
    };
    let marks = MarksRequest {
        note_path: "главная.typ".to_owned(),
        text: text.clone(),
        linked,
        evidence: None,
        sources: Vec::new(),
        claimed: Vec::new(),
        held: false,
        previously_missing: Vec::new(),
    };

    let best = |work: &mut dyn FnMut()| -> Duration {
        (0..RUNS)
            .map(|_| {
                let started = Instant::now();
                work();
                started.elapsed()
            })
            .min()
            .expect("прогоны")
    };
    let mut total = 0;
    let completions_took = best(&mut || {
        total = runtime.block_on(completions(&backend, &request)).groups[0].total;
    });
    let mut found = 0;
    let marks_took = best(&mut || {
        found = runtime.block_on(reference_marks(&backend, &marks)).len();
    });

    println!("completion_scale: {LINES} строк, {LABELS} меток");
    println!("  completions      {completions_took:?} ({total} подходящих)");
    println!("  reference_marks  {marks_took:?} ({found} отметок)");
    assert_eq!(total as usize, LABELS);
    // Ссылки текста ведут к меткам и заметкам; отмечен только набираемый `@разд`.
    assert_eq!(found, 1);
    assert!(
        completions_took <= COMPLETIONS_BUDGET,
        "completions: {completions_took:?}"
    );
    assert!(
        marks_took <= MARKS_BUDGET,
        "reference_marks: {marks_took:?}"
    );
}

#[cfg(target_arch = "wasm32")]
fn main() {}
