//! Бенчмарк библиографии vault (фича 017, T054): разбор `.bib` на 10 000
//! записей, поиск по 10 000 источников (SC-002, ≤ 200 мс) и выдача индекса
//! цитат на 5 000 заметок — цитирующие заметки и ключи без источника (SC-004,
//! ≤ 1 с).
//!
//! `harness = false` и ручной замер — по тем же причинам, что у `vault_scale`.
//! Меряется лучший из нескольких прогонов: порог про скорость кода, а не про
//! шум планировщика. Разбор и проход по заметкам меряются одним прогоном и
//! только печатаются: бюджетов у них в спеке нет, они идут порциями.
//!
//! Прогон: `cargo bench --bench sources_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::sources::bibtex::read_bibtex;
    use typsidian_core::sources::SourceEngine;
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    const SOURCES: usize = 10_000;
    const NOTES: usize = 5_000;
    const NOTES_PER_FOLDER: usize = 100;
    const SEARCH_BUDGET: Duration = Duration::from_millis(200);
    const INDEX_BUDGET: Duration = Duration::from_secs(1);
    const RUNS: usize = 5;

    let mut bib = String::new();
    for i in 0..SOURCES {
        bib.push_str(&format!(
            "@article{{key{i:05},\n  author = {{Фамилия{}, Имя and Second, Author}},\n  \
             title = {{Заголовок статьи номер {i} о вёрстке}},\n  journal = {{Журнал {}}},\n  \
             year = {{{}}},\n}}\n\n",
            i % 997,
            i % 50,
            1950 + i % 75
        ));
    }

    let dir = tempfile::TempDir::new().expect("временный каталог");
    std::fs::write(dir.path().join("references.bib"), &bib).expect("запись .bib");
    // Каждая заметка цитирует три ключа; каждая десятая — ещё и несуществующий.
    for i in 0..NOTES {
        let folder = dir
            .path()
            .join(format!("folder-{:03}", i / NOTES_PER_FOLDER));
        std::fs::create_dir_all(&folder).expect("подпапка");
        let missing = if i % 10 == 0 {
            format!(" и @nokey{}", i % 40)
        } else {
            String::new()
        };
        std::fs::write(
            folder.join(format!("note-{i:05}.typ")),
            format!(
                "= Заметка {i}\n\nСм. @key{:05}, @key{:05} и #cite(<key{:05}>){missing}.\n\n\
                 #bibliography(\"/references.bib\")\n",
                i % SOURCES,
                (i * 7) % SOURCES,
                (i * 13) % SOURCES
            ),
        )
        .expect("запись заметки");
    }

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

    let started = Instant::now();
    let reading = read_bibtex("references.bib", &bib);
    let parse_took = started.elapsed();
    assert_eq!(reading.sources.len(), SOURCES);
    assert!(reading.problems.is_empty());

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let backend = runtime
        .block_on(NativeBackend::open(dir.path()))
        .expect("открытие vault");
    let mut engine = SourceEngine::new();
    let started = Instant::now();
    while runtime.block_on(engine.refresh(&backend, &[])).pending > 0 {}
    let refresh_took = started.elapsed();
    assert_eq!(engine.summary().source_count as usize, SOURCES);

    let notes = runtime.block_on(backend.list());
    let started = Instant::now();
    engine.scan_start(&notes);
    loop {
        let view = runtime.block_on(engine.scan_step(&backend, 50));
        if view.processed == view.total {
            break;
        }
    }
    let scan_took = started.elapsed();

    let mut hits = 0;
    let search_took = best(&mut || {
        hits = engine
            .search("фамилия12 вёрстке 1962", 200, false)
            .total_matches;
    });
    let search_broad_took = best(&mut || {
        engine.search("статьи", 200, false);
    });
    let search_uncited_took = best(&mut || {
        engine.search("", 200, true);
    });
    let mut citing = 0;
    let mut missing = 0;
    let index_took = best(&mut || {
        citing = engine.citing("key00042").len();
        missing = engine.missing_keys().len();
    });

    println!("sources_scale: {SOURCES} источников, {NOTES} заметок");
    println!("  read_bibtex         {parse_took:?}");
    println!("  refresh (весь файл) {refresh_took:?}");
    println!("  проход индекса      {scan_took:?}");
    println!("  search точный       {search_took:?} ({hits} совпадений)");
    println!("  search широкий      {search_broad_took:?}");
    println!("  search uncited      {search_uncited_took:?}");
    println!("  citing + missing    {index_took:?} ({citing} заметок, {missing} ключей)");

    // `nokey{i % 40}` у кратных десяти: остатки 0, 10, 20, 30.
    assert!(missing == 4, "ключей без источника: {missing}");
    assert!(citing > 0, "у key00042 есть цитирующие");
    assert!(hits > 0, "точный запрос находит источник");
    for (name, took) in [
        ("search точный", search_took),
        ("search широкий", search_broad_took),
        ("search uncited", search_uncited_took),
    ] {
        assert!(
            took <= SEARCH_BUDGET,
            "{name}: {took:?} > {SEARCH_BUDGET:?} (SC-002)"
        );
    }
    assert!(
        index_took <= INDEX_BUDGET,
        "citing + missing: {index_took:?} > {INDEX_BUDGET:?} (SC-004)"
    );
}

#[cfg(target_arch = "wasm32")]
fn main() {}
