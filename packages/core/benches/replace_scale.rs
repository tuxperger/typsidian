//! Бенчмарк замены по vault (фича 027, T011): SC-001 — предпросмотр замены на
//! vault из 2 000 заметок появляется не позднее 2 с; заодно — замена одной
//! заметки (ход применения на 2 000 заметок складывается из этих вызовов).
//!
//! `harness = false` и ручной замер — по тем же причинам, что у `vault_scale`.
//! Меряется лучший из нескольких прогонов.
//!
//! Прогон: `cargo bench --bench replace_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::search::replace::{
        replace_in_note, replace_preview, ReplaceNoteOutcome, ReplaceNoteRequest, ReplaceQuery,
    };
    use typsidian_core::search::SearchQuery;
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    const BUDGET: Duration = Duration::from_secs(2);
    const NOTE_BUDGET: Duration = Duration::from_millis(50);
    const NOTE_COUNT: usize = 2_000;
    const RUNS: usize = 3;

    let dir = tempfile::TempDir::new().expect("временный каталог");
    for i in 0..NOTE_COUNT {
        let folder = dir.path().join(format!("folder-{:02}", i / 100));
        std::fs::create_dir_all(&folder).expect("подпапка");
        // Каждая заметка — сорок строк, четыре совпадения: 8 000 на vault, под
        // пределом предпросмотра в 10 000.
        let body: String = (0..40)
            .map(|line| {
                if line % 10 == 0 {
                    format!("Строка {line}: по данным Иванов и др. ({i}).\n")
                } else {
                    format!("Строка {line}: обычный текст без совпадений.\n")
                }
            })
            .collect();
        std::fs::write(folder.join(format!("note-{i:04}.typ")), body).expect("запись");
    }

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut backend = runtime
        .block_on(NativeBackend::open(dir.path()))
        .expect("открытие vault");
    let query = ReplaceQuery {
        search: SearchQuery {
            text: "Иванов".into(),
            ..SearchQuery::default()
        },
        replacement: "Иванова".into(),
    };

    let mut preview_took = Duration::MAX;
    let mut total = 0;
    for _ in 0..RUNS {
        let started = Instant::now();
        let preview = runtime
            .block_on(replace_preview(&backend, &query, &[]))
            .expect("предпросмотр");
        preview_took = preview_took.min(started.elapsed());
        total = preview.total_matches;
    }
    assert_eq!(total as usize, NOTE_COUNT * 4);

    let preview = runtime
        .block_on(replace_preview(&backend, &query, &[]))
        .expect("предпросмотр");
    let note = &preview.notes[0];
    let request = ReplaceNoteRequest {
        query: query.clone(),
        path: note.path.clone(),
        digest: note.digest.clone(),
        selected: note.matches.iter().map(|m| m.index).collect(),
    };
    let started = Instant::now();
    let outcome = runtime.block_on(replace_in_note(&mut backend, &request, None));
    let note_took = started.elapsed();
    assert!(
        matches!(outcome, ReplaceNoteOutcome::Written { .. }),
        "{outcome:?}"
    );

    println!("SC-001 предпросмотр, {NOTE_COUNT} заметок, {total} совпадений: {preview_took:?} (бюджет {BUDGET:?})");
    println!("замена в одной заметке: {note_took:?} (порог {NOTE_BUDGET:?})");

    let mut failures = Vec::new();
    if preview_took > BUDGET {
        failures.push(format!("SC-001: предпросмотр за {preview_took:?}"));
    }
    if note_took > NOTE_BUDGET {
        failures.push(format!("замена в заметке за {note_took:?}"));
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
