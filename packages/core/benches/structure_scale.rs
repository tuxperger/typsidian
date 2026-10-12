//! Бенчмарк оглавления заметки (фича 015, T045): подтверждает бюджеты
//! research.md §9 на заметке в 10 000 строк с 2 000 заголовков.
//!
//! `harness = false` и ручной замер - по тем же причинам, что у `vault_scale`.
//! Бюджеты native: `outline` ≤ 20 мс, `outline_view` ≤ 15 мс, `outline_active`
//! ≤ 8 мс. Меряется лучший из нескольких прогонов: порог про скорость кода, а
//! не про шум планировщика.
//!
//! Прогон: `cargo bench --bench structure_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::structure::{
        outline, outline_active, outline_view, OutlineKinds, OutlineRequest,
    };

    const LINES: usize = 10_000;
    const HEADINGS: usize = 2_000;
    const RUNS: usize = 7;

    // Каждый пятый ряд - заголовок трёх чередующихся уровней, между ними
    // обычный текст, рисунки с подписью и метки.
    let mut text = String::new();
    let mut heading = 0;
    for line in 0..LINES {
        if line % (LINES / HEADINGS) == 0 {
            heading += 1;
            let depth = 1 + (heading % 3);
            text.push_str(&format!(
                "{} Раздел номер {heading} с *оформлением* <h{heading}>\n",
                "=".repeat(depth)
            ));
        } else if line % 37 == 0 {
            text.push_str(&format!(
                "#figure(rect(), caption: [Рисунок {line}]) <f{line}>\n"
            ));
        } else {
            text.push_str("Обычная строка текста заметки для объёма, мир, привет.\n");
        }
    }

    let best = |mut work: Box<dyn FnMut()>| -> Duration {
        (0..RUNS)
            .map(|_| {
                let started = Instant::now();
                work();
                started.elapsed()
            })
            .min()
            .expect("прогоны")
    };

    let tree = outline(&text);
    assert_eq!(
        tree.items
            .iter()
            .filter(|item| item.kind == typsidian_core::structure::OutlineKind::Heading)
            .count(),
        HEADINGS
    );

    let all = OutlineKinds {
        figures: true,
        labels: true,
    };
    let request = OutlineRequest {
        kinds: all,
        collapsed: tree
            .items
            .iter()
            .step_by(10)
            .map(|item| item.key.clone())
            .collect(),
        filter: "номер 7".to_string(),
        caret: Some(text.encode_utf16().count() as u32 / 2),
    };

    let parse_took = best(Box::new({
        let text = text.clone();
        move || {
            std::hint::black_box(outline(&text));
        }
    }));
    let view_took = best(Box::new({
        let (tree, request) = (tree.clone(), request.clone());
        move || {
            std::hint::black_box(outline_view(&tree, &request));
        }
    }));
    let active_took = best(Box::new({
        let (tree, request) = (tree.clone(), request.clone());
        move || {
            std::hint::black_box(outline_active(&tree, &request));
        }
    }));

    let report = |name: &str, took: Duration, budget_ms: u64| {
        let ok = took <= Duration::from_millis(budget_ms);
        println!(
            "{name:<16} {:>8.2} мс (бюджет {budget_ms} мс) {}",
            took.as_secs_f64() * 1000.0,
            if ok { "OK" } else { "ПРЕВЫШЕН" }
        );
        ok
    };
    println!(
        "заметка: {LINES} строк, {HEADINGS} заголовков, {} пунктов",
        tree.items.len()
    );
    let results = [
        report("outline", parse_took, 20),
        report("outline_view", view_took, 15),
        report("outline_active", active_took, 8),
    ];
    if results.iter().any(|ok| !ok) {
        std::process::exit(1);
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
