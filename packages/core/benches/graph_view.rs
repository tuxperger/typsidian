//! Бенчмарк графа связей (фича 025, T011): SC-001 — граф vault из 5 000
//! заметок и 15 000 связей строится одним запросом; бюджет `vault_graph`
//! по research.md §9 — не больше 100 мс на native.
//!
//! Меряется весь путь команды `vault_graph`: индекс `NativeBackend` (SQLite-кэш
//! ссылок и вставок, свойства в памяти), сбор источника и построение. Отдельно —
//! тяжёлый запрос: теги, вложения, фильтр, группы и локальный граф. Файлы при
//! запросе не читаются.
//!
//! Ручной замер и `harness = false` — по тем же причинам, что у `vault_scale`.
//! Меряется лучший из нескольких прогонов.
//!
//! Прогон: `cargo bench --bench graph_view`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::graph::view::{
        vault_graph, GraphGroup, GraphRequest, GroupColor, LocalGraph,
    };
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    const BUDGET: Duration = Duration::from_millis(100);
    const NOTE_COUNT: usize = 5_000;
    const LINKS_PER_NOTE: usize = 3;
    const NOTES_PER_FOLDER: usize = 100;
    const RUNS: usize = 5;

    let name = |i: usize| format!("folder-{:03}/note-{i:05}", i / NOTES_PER_FOLDER);

    let dir = tempfile::TempDir::new().expect("временный каталог");
    for i in 0..NOTE_COUNT {
        std::fs::create_dir_all(
            dir.path()
                .join(format!("folder-{:03}", i / NOTES_PER_FOLDER)),
        )
        .expect("подпапка");
        // Три исходящие ссылки на «случайные» заметки (детерминированно).
        let mut text = String::new();
        if i % 4 == 0 {
            text.push_str(&format!(
                "#metadata((tags: (\"тег{}\", \"общий\"))) <properties>\n",
                i % 40
            ));
        }
        for k in 1..=LINKS_PER_NOTE {
            let target = (i * 7 + k * 1_103) % NOTE_COUNT;
            text.push_str(&format!("[[/{}]] ", name(target)));
        }
        if i % 10 == 0 {
            text.push_str(&format!("#image(\"pic-{}.png\")\n", i % 200));
        }
        std::fs::write(dir.path().join(format!("{}.typ", name(i))), text).expect("запись заметки");
    }

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let opened = Instant::now();
    let backend = runtime
        .block_on(NativeBackend::open(dir.path()))
        .expect("открытие vault");
    let open_took = opened.elapsed();

    let plain = GraphRequest {
        filter: String::new(),
        show_orphans: true,
        show_attachments: false,
        show_tags: false,
        groups: Vec::new(),
        local: None,
    };
    let heavy = GraphRequest {
        filter: "folder".into(),
        show_orphans: false,
        show_attachments: true,
        show_tags: true,
        groups: vec![
            GraphGroup {
                query: "папка:folder-001".into(),
                color: GroupColor::Orange,
            },
            GraphGroup {
                query: "тег:общий".into(),
                color: GroupColor::Blue,
            },
        ],
        local: None,
    };
    let around = GraphRequest {
        local: Some(LocalGraph {
            center: format!("{}.typ", name(42)),
            depth: 3,
        }),
        ..plain.clone()
    };

    let best = |request: &GraphRequest| -> (Duration, usize, usize) {
        let mut shape = (0, 0);
        let took = (0..RUNS)
            .map(|_| {
                let started = Instant::now();
                let graph = runtime.block_on(vault_graph(&backend, request));
                let took = started.elapsed();
                shape = (graph.nodes.len(), graph.edges.len());
                took
            })
            .min()
            .expect("прогоны");
        (took, shape.0, shape.1)
    };

    let (plain_took, nodes, edges) = best(&plain);
    assert_eq!(nodes, NOTE_COUNT, "в графе все заметки");
    assert!(edges > 10_000, "связей около 15 000: {edges}");
    let (heavy_took, heavy_nodes, heavy_edges) = best(&heavy);
    let (local_took, local_nodes, _) = best(&around);

    println!("открытие vault ({NOTE_COUNT} заметок, холодный обход): {open_took:?}");
    println!(
        "SC-001 vault_graph, {nodes} узлов, {edges} связей: {plain_took:?} (бюджет {BUDGET:?})"
    );
    println!(
        "с тегами, вложениями, фильтром и группами: {heavy_took:?} ({heavy_nodes} узлов, {heavy_edges} связей)"
    );
    println!("локальный граф глубины 3: {local_took:?} ({local_nodes} узлов)");

    let mut failures = Vec::new();
    for (label, took) in [("простой", plain_took), ("тяжёлый", heavy_took)] {
        if took > BUDGET {
            failures.push(format!("{label} запрос за {took:?}, бюджет {BUDGET:?}"));
        }
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
