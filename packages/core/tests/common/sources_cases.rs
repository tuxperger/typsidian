//! Библиография vault (фича 017): общие случаи для `sources_native.rs` и
//! `sources_wasm.rs`. Логика лежит над `VaultBackend`, поэтому проверяется на
//! бэкенде в памяти одинаково на обоих таргетах (принцип II); сети здесь нет
//! вовсе — тесты сами по себе проверка FR-009.

#![allow(dead_code)]

use std::path::Path;

use typsidian_core::sources::{
    bibtex::read_bibtex, create_source_file, plan_citation, yaml::read_hayagriva,
    BibliographyOffer, CitationMarkKind, CitationPlanError, CitationScanPhase,
    CreateSourceFileError, SourceEngine, SourceReadProblemKind,
};
use typsidian_core::structure::{mode_at, parse, TextMode};
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::events::VaultEvent;

use super::MemoryVault;

const BIB: &str = "@book{knuth1984,\n  author = {Knuth, Donald E.},\n  title = {The {\\TeX}book},\n  year = {1984},\n}\n\n@article{lamport1994,\n  author = {Lamport, Leslie},\n  title = {LaTeX: A Document Preparation System},\n  journal = {Addison-Wesley},\n  year = {1994},\n}\n\n@book{кнут2001,\n  author = {Кнут, Дональд},\n  title = {Искусство программирования},\n  year = {2001},\n}\n";

const YML: &str =
    "knuth1984:\n  type: book\n  title: The TeXbook\n  author: Knuth, Donald E.\n  date: 1984\n";

async fn engine_with(vault: &MemoryVault) -> SourceEngine {
    let mut engine = SourceEngine::new();
    while engine.refresh(vault, &[]).await.pending > 0 {}
    engine
}

async fn scanned(vault: &MemoryVault, engine: &mut SourceEngine) {
    let notes = vault.list().await;
    engine.scan_start(&notes);
    while engine.scan_step(vault, 2).await.phase == CitationScanPhase::Running {}
}

// ── Зонд API крейтов (T001, research.md §2) ────────────────────────────────

pub fn the_reading_crates_behave_as_researched() {
    // `RawBibliography::parse` + `from_raw` по одной записи с общими сокращениями.
    let raw = biblatex::RawBibliography::parse("@string{a = {X}}\n@misc{k, title = a}\n").unwrap();
    let one = biblatex::RawBibliography {
        preamble: String::new(),
        entries: vec![raw.entries[0].clone()],
        abbreviations: raw.abbreviations.clone(),
    };
    let bibliography = biblatex::Bibliography::from_raw(one).unwrap();
    let entry = bibliography.iter().next().unwrap();
    // `TryFrom<&biblatex::Entry>` по одной записи.
    let converted = hayagriva::Entry::try_from(entry).unwrap();
    assert_eq!(converted.key(), "k");
    // `from_value::<Library>` на карте из одной записи.
    let value: serde_yaml::Value = serde_yaml::from_str(YML).unwrap();
    let library: hayagriva::Library = serde_yaml::from_value(value).unwrap();
    assert_eq!(library.iter().count(), 1);
}

// ── Расширение structure/ (T007) ───────────────────────────────────────────

pub fn structure_finds_cites_and_bibliographies() {
    let text = "= Глава <гл>\nСм. @knuth1984 и #cite(<lamport1994>), #cite(label(\"с пробелом\")).\n#bibliography((\"/a.bib\", \"b.yml\"))\n#let f = \"x.bib\"\n#bibliography(f)\n";
    let found = parse(text);
    let keys: Vec<&str> = found.cites.iter().map(|site| site.key.as_str()).collect();
    assert_eq!(keys, vec!["lamport1994", "с пробелом"]);
    assert_eq!(found.refs[0].target, "knuth1984");
    assert_eq!(found.bibliographies.len(), 2);
    assert_eq!(found.bibliographies[0].paths, vec!["/a.bib", "b.yml"]);
    assert!(found.bibliographies[0].literal);
    assert!(!found.bibliographies[1].literal);
    // Метка в `cite(<k>)` — употребление, а не определение.
    assert!(found.labels.iter().all(|label| label.name != "lamport1994"));
    // Места — в UTF-16 на кириллице.
    let place = found.cites[0].place;
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let written = String::from_utf16_lossy(&utf16[place.from as usize..place.to as usize]);
    assert_eq!(written, "#cite(<lamport1994>)");
}

pub fn mode_at_tells_markup_math_code_string_and_comment() {
    let text = "Текст $x + y$ #let a = \"строка\" // заметка\n";
    let at = |needle: &str, shift: usize| {
        let byte = text.find(needle).unwrap() + shift;
        text[..byte].encode_utf16().count()
    };
    assert_eq!(mode_at(text, at("кст", 0)), TextMode::Markup);
    assert_eq!(mode_at(text, at("+", 0)), TextMode::Math);
    assert_eq!(mode_at(text, at("= \"", 1)), TextMode::Code);
    assert_eq!(mode_at(text, at("трока", 0)), TextMode::String);
    assert_eq!(mode_at(text, at("заметка", 0)), TextMode::Comment);
}

// ── Чтение и библиотека (T012, T045) ───────────────────────────────────────

pub fn bib_and_yaml_give_the_same_source() {
    let from_bib = read_bibtex("a.bib", BIB);
    let from_yml = read_hayagriva("a.yml", YML);
    let bib = from_bib
        .sources
        .iter()
        .find(|s| s.key == "knuth1984")
        .unwrap();
    let yml = &from_yml.sources[0];
    assert_eq!((&bib.authors, bib.year), (&yml.authors, yml.year));
    assert_eq!(bib.title, "The TeXbook", "TeX-запись раскрыта");
    assert_eq!(bib.title, yml.title);
}

pub fn a_broken_bib_entry_keeps_the_rest_and_is_named() {
    let text = format!("{BIB}@misc{{broken,\n  title = {{Незакрыто\n");
    let reading = read_bibtex("a.bib", &text);
    assert_eq!(reading.sources.len(), 3);
    let problem = &reading.problems[0];
    assert_eq!(problem.key.as_deref(), Some("broken"));
    assert_eq!(problem.line, 19);
    assert!(matches!(problem.kind, SourceReadProblemKind::Syntax { .. }));
}

pub fn too_many_problems_are_folded() {
    let text: String = (0..230)
        .map(|n| format!("@misc{{k{n},\n  title = {{x\n"))
        .collect();
    let reading = read_bibtex("a.bib", &text);
    assert_eq!(reading.problems.len(), 201);
    assert!(matches!(
        reading.problems.last().unwrap().kind,
        SourceReadProblemKind::TooMany { hidden: 30 }
    ));
}

pub fn reading_never_panics_on_arbitrary_text() {
    for text in [
        "",
        "@",
        "@{}",
        "}}}{{{",
        "\u{0}\u{ffff}",
        "a: [",
        "- 1\n- 2",
        "k:\n  type: x",
    ] {
        let _ = read_bibtex("a.bib", text);
        let _ = read_hayagriva("a.yml", text);
    }
}

pub async fn the_library_recognizes_files_and_rereads_only_changes() {
    let vault = MemoryVault::new()
        .with_file("refs.bib", BIB)
        .with_file("lit/refs.yml", YML)
        .with_file("config.yml", "name: тест\n")
        .with_file(".hidden/x.bib", BIB);
    let mut engine = engine_with(&vault).await;
    let summary = engine.summary();
    let files: Vec<&str> = summary.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, vec!["lit/refs.yml", "refs.bib"]);
    assert_eq!(summary.yaml_candidates, vec!["config.yml"]);
    assert_eq!(
        summary.source_count, 4,
        "один ключ в двух файлах — два источника"
    );

    // Указанный пользователем YAML — источник, даже без записей.
    while engine
        .refresh(&vault, &["config.yml".to_string()])
        .await
        .pending
        > 0
    {}
    assert!(engine
        .summary()
        .files
        .iter()
        .any(|f| f.path == "config.yml"));

    // Повторное обновление без изменений файлов ничего не читает.
    let reads = vault.read_count("refs.bib");
    while engine
        .refresh(&vault, &["config.yml".to_string()])
        .await
        .pending
        > 0
    {}
    assert_eq!(vault.read_count("refs.bib"), reads);
}

pub async fn a_removed_file_leaves_the_summary() {
    let mut vault = MemoryVault::new().with_file("refs.bib", BIB);
    let mut engine = engine_with(&vault).await;
    vault.remove(Path::new("refs.bib"));
    while engine.refresh(&vault, &[]).await.pending > 0 {}
    assert!(engine.summary().files.is_empty());
}

// ── Поиск (T016) ───────────────────────────────────────────────────────────

pub async fn search_matches_every_word_and_orders_by_key_then_family() {
    let vault = MemoryVault::new().with_file("refs.bib", BIB);
    let engine = engine_with(&vault).await;
    let keys = |query: &str| {
        engine
            .search(query, 50, false)
            .hits
            .iter()
            .map(|hit| hit.source.key.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(keys("кнут"), vec!["кнут2001"]);
    assert_eq!(keys("knuth texbook"), vec!["knuth1984"]);
    assert_eq!(keys("1994"), vec!["lamport1994"]);
    assert_eq!(keys("LAMPORT"), vec!["lamport1994"]);
    assert!(keys("knuth 2001").is_empty());
    assert_eq!(keys("").len(), 3);
    let limited = engine.search("", 1, false);
    assert_eq!((limited.hits.len(), limited.total_matches), (1, 3));
    // До прохода индекса «не цитируется» не выводится.
    assert!(engine.search("", 50, true).hits.is_empty());
    assert!(engine
        .search("", 50, false)
        .hits
        .iter()
        .all(|hit| hit.cited.is_none()));
}

pub async fn search_over_ten_thousand_sources_keeps_its_order() {
    let text: String = (0..10_000)
        .map(|n| format!("@misc{{key{n},\n  author = {{Автор{n}, И.}},\n  title = {{Работа {n}}},\n  year = {},\n}}\n", 1900 + n % 100))
        .collect();
    let vault = MemoryVault::new().with_file("big.bib", text);
    let engine = engine_with(&vault).await;
    let hits = engine.search("key9999", 50, false);
    assert_eq!(hits.hits[0].source.key, "key9999");
    assert_eq!(engine.summary().source_count, 10_000);
}

// ── Запись цитаты (T017) ───────────────────────────────────────────────────

/// Применяет правку и проверяет, что результат разбирается без ошибок и даёт
/// ровно одну цитату с исходным ключом.
fn inserted_reads_back(text: &str, caret: usize, key: &str) -> String {
    let plan = plan_citation(text, caret, key, true).unwrap();
    let mut units: Vec<u16> = text.encode_utf16().collect();
    let edit = &plan.edits[0];
    units.splice(edit.from..edit.to, edit.insert.encode_utf16());
    let result = String::from_utf16(&units).unwrap();
    let root = typst::syntax::parse(&result);
    let (errors, _) = root.errors_and_warnings();
    assert!(errors.is_empty(), "{result:?}: {errors:?}");
    let found = parse(&result);
    let cited: Vec<String> = found
        .cites
        .iter()
        .map(|site| site.key.clone())
        .chain(found.refs.iter().map(|site| site.target.clone()))
        .collect();
    assert_eq!(cited, vec![key.to_string()], "{result:?}");
    result
}

pub fn every_citation_form_reads_back_as_one_citation() {
    assert!(inserted_reads_back("См. ", 4, "knuth1984").contains("@knuth1984"));
    // Следующий знак продолжил бы имя ключа — краткая запись слиплась бы.
    assert!(inserted_reads_back("См. и далее", 4, "knuth1984").contains("#cite(<knuth1984>)"));
    assert!(inserted_reads_back("См.  далее", 4, "knuth1984").contains("@knuth1984"));
    assert!(
        inserted_reads_back("Слово", 5, "k").contains("#cite(<k>)"),
        "приклеено к слову"
    );
    assert!(inserted_reads_back("x  y", 2, "key.").contains("#cite(<key.>)"));
    assert!(
        inserted_reads_back("x [y]", 2, "k").contains("#cite(<k>)"),
        "за кареткой «[» — дополнение, а не текст"
    );
    assert!(inserted_reads_back("x ", 2, "с пробелом").contains("label(\"с пробелом\")"));
    assert!(inserted_reads_back("x ", 2, "a\"b\\c").contains("label("));
    assert!(inserted_reads_back("$x$", 2, "k").contains("#cite(<k>)"));
    assert!(inserted_reads_back("#let a = ()", 10, "k").contains("cite(<k>)"));
}

pub fn strings_comments_and_unknown_keys_are_refused() {
    assert_eq!(
        plan_citation("#let a = \"текст\"", 12, "k", true),
        Err(CitationPlanError::NotInsertable {
            mode: TextMode::String
        })
    );
    assert_eq!(
        plan_citation("// заметка", 5, "k", true),
        Err(CitationPlanError::NotInsertable {
            mode: TextMode::Comment
        })
    );
    assert_eq!(
        plan_citation("x", 1, "k", false),
        Err(CitationPlanError::UnknownKey)
    );
}

// ── Список литературы (T018) ───────────────────────────────────────────────

pub fn the_bibliography_offer_adds_extends_or_stays_quiet() {
    let engine = SourceEngine::new();
    match engine.bibliography_offer("= Глава\nТекст\n", "Диссертация/Глава.typ", "refs.bib")
    {
        BibliographyOffer::Add { edits, .. } => {
            assert_eq!(edits[0].insert, "\n#bibliography(\"/refs.bib\")\n")
        }
        other => panic!("{other:?}"),
    }
    match engine.bibliography_offer("#bibliography(\"a.bib\")\n", "x.typ", "refs.bib") {
        BibliographyOffer::Extend { edits, .. } => {
            assert_eq!(edits[0].insert, "(\"a.bib\", \"/refs.bib\")")
        }
        other => panic!("{other:?}"),
    }
    for (text, note) in [
        ("#bibliography(\"../refs.bib\")\n", "a/x.typ"),
        ("#bibliography(\"/refs.bib\")\n", "a/b/x.typ"),
        ("#let f = \"q\"\n#bibliography(f)\n", "x.typ"),
    ] {
        assert_eq!(
            engine.bibliography_offer(text, note, "refs.bib"),
            BibliographyOffer::None,
            "{text}"
        );
    }
}

// ── Создание файла и настройка (T019) ──────────────────────────────────────

pub async fn creating_the_source_file_refuses_busy_names_and_read_only() {
    let mut vault = MemoryVault::new();
    assert_eq!(
        create_source_file(&mut vault).await,
        Ok("references.bib".into())
    );
    assert_eq!(vault.bytes_of("references.bib"), Some(Vec::new()));
    assert_eq!(
        create_source_file(&mut vault).await,
        Err(CreateSourceFileError::AlreadyExists)
    );
    let mut read_only = MemoryVault::new().read_only();
    assert_eq!(
        create_source_file(&mut read_only).await,
        Err(CreateSourceFileError::AccessDenied)
    );
}

pub async fn a_bad_source_file_setting_is_refused_before_writing() {
    use typsidian_core::vault::vault_settings::{
        read_vault_settings, write_vault_settings, VaultOverrides, VaultSettingsError,
    };
    let mut vault = MemoryVault::new();
    let good = VaultOverrides {
        source_files: vec![" /lit/refs.yml ".into()],
        ..VaultOverrides::default()
    };
    write_vault_settings(&mut vault, &good).await.unwrap();
    assert_eq!(
        read_vault_settings(&vault).await.unwrap().source_files,
        vec!["lit/refs.yml"]
    );
    let before = vault.bytes_of(".typsidian/vault.json");
    for bad in ["../x.bib", "refs.txt", ""] {
        let overrides = VaultOverrides {
            source_files: vec![bad.into()],
            ..VaultOverrides::default()
        };
        assert!(matches!(
            write_vault_settings(&mut vault, &overrides).await,
            Err(VaultSettingsError::InvalidSourceFile { .. })
        ));
    }
    assert_eq!(
        vault.bytes_of(".typsidian/vault.json"),
        before,
        "файл не тронут"
    );
}

// ── Индекс цитат (T033, T034) ──────────────────────────────────────────────

fn notes_vault() -> MemoryVault {
    MemoryVault::new()
        .with_file("refs.bib", BIB)
        .with_file(
            "a/Первая.typ",
            "= Первая <intro>\nСм. @knuth1984 и @intro.\nИ #cite(<нет2020>).\n",
        )
        .with_file(
            "b/Вторая.typ",
            "Опять @knuth1984.\n#bibliography(\"/refs.bib\")\n",
        )
        .with_file("c/Третья.typ", "Без цитат.\n")
}

pub async fn the_citation_index_scans_in_portions_and_answers() {
    let vault = notes_vault();
    let mut engine = engine_with(&vault).await;
    let notes = vault.list().await;
    let started = engine.scan_start(&notes);
    assert_eq!(
        (started.phase, started.total),
        (CitationScanPhase::Running, 3)
    );
    let step = engine.scan_step(&vault, 2).await;
    assert_eq!(
        (step.phase, step.processed),
        (CitationScanPhase::Running, 2)
    );
    let done = engine.scan_step(&vault, 2).await;
    assert_eq!(done.phase, CitationScanPhase::Done);
    // Вне `Running` шаг — не ошибка.
    assert_eq!(engine.scan_step(&vault, 2).await, done);

    let citing = engine.citing("knuth1984");
    let paths: Vec<&str> = citing.iter().map(|note| note.path.as_str()).collect();
    assert_eq!(paths, vec!["a/Первая.typ", "b/Вторая.typ"]);
    assert_eq!(citing[0].places[0].line, 2);
    let highlighted: Vec<&str> = citing[0].lines[0]
        .segments
        .iter()
        .filter(|segment| segment.highlighted)
        .map(|segment| segment.text.as_str())
        .collect();
    assert_eq!(highlighted, vec!["@knuth1984"]);
    // `@intro` — метка самой заметки, не цитата.
    assert!(engine.citing("intro").is_empty());
    let missing: Vec<String> = engine.missing_keys().into_iter().map(|m| m.key).collect();
    assert_eq!(missing, vec!["нет2020"]);
    // После прохода — счётчики и «не цитируется».
    let hit = engine.search("knuth", 50, false).hits.remove(0);
    assert_eq!(hit.cited, Some(2));
    let uncited: Vec<String> = engine
        .search("", 50, true)
        .hits
        .into_iter()
        .map(|hit| hit.source.key)
        .collect();
    assert_eq!(uncited, vec!["lamport1994", "кнут2001"]);
}

pub async fn cancel_keeps_what_was_scanned_and_a_rescan_skips_it() {
    let vault = notes_vault();
    let mut engine = engine_with(&vault).await;
    let notes = vault.list().await;
    engine.scan_start(&notes);
    engine.scan_step(&vault, 1).await;
    assert_eq!(engine.scan_cancel().phase, CitationScanPhase::Cancelled);
    assert_eq!(engine.citing("knuth1984").len(), 1, "собранное сохранено");
    let reads = vault.read_count("a/Первая.typ");
    let again = engine.scan_start(&notes);
    assert_eq!(
        again.total, 2,
        "прочитанная заметка с прежним отпечатком не идёт"
    );
    while engine.scan_step(&vault, 5).await.phase == CitationScanPhase::Running {}
    assert_eq!(vault.read_count("a/Первая.typ"), reads);
}

pub async fn single_notes_follow_vault_events() {
    let mut vault = notes_vault();
    let mut engine = engine_with(&vault).await;
    scanned(&vault, &mut engine).await;
    vault
        .write(Path::new("c/Третья.typ"), "Теперь @lamport1994.\n", None)
        .await
        .unwrap();
    engine.note_changed(&vault, "c/Третья.typ").await;
    assert_eq!(engine.citing("lamport1994").len(), 1);
    engine.note_renamed("c/Третья.typ", "c/Новая.typ");
    assert_eq!(engine.citing("lamport1994")[0].name, "Новая");
    engine.note_removed("c/Новая.typ");
    assert!(engine.citing("lamport1994").is_empty());
}

// ── Отметки (T046) ─────────────────────────────────────────────────────────

pub async fn citation_marks_follow_the_research_table() {
    let mut vault = MemoryVault::new().with_file("refs.bib", BIB);
    let mut engine = engine_with(&vault).await;
    let text = "Метка <м>. @knuth1984 @м @чужая #cite(<нет>) 😀 @тоже\n";
    let check = engine.check_note(text, &["тоже".to_string()]);
    let marked: Vec<(String, bool)> = check
        .marks
        .iter()
        .map(|mark| match &mark.kind {
            CitationMarkKind::MissingSource { key, ambiguous } => (key.clone(), *ambiguous),
        })
        .collect();
    assert_eq!(
        marked,
        vec![("чужая".to_string(), true), ("нет".to_string(), false)]
    );
    assert_eq!((check.citations, check.broken), (3, 2));
    assert!(!check.has_bibliography);
    // Место — UTF-16 на тексте с кириллицей и эмодзи.
    let mark = &check.marks[1];
    let utf16: Vec<u16> = text.encode_utf16().collect();
    assert_eq!(
        String::from_utf16_lossy(&utf16[mark.place.from as usize..mark.place.to as usize]),
        "#cite(<нет>)"
    );

    // Без файлов источников `@k` — не дело 017.
    let empty = SourceEngine::new();
    assert!(empty.check_note("@чужая", &[]).marks.is_empty());

    // После удаления записи та же заметка получает отметку.
    vault
        .write(Path::new("refs.bib"), "@misc{other, title = {x}}\n", None)
        .await
        .unwrap();
    while engine.refresh(&vault, &[]).await.pending > 0 {}
    assert!(engine
        .check_note("@knuth1984", &[])
        .marks
        .iter()
        .any(|mark| matches!(&mark.kind, CitationMarkKind::MissingSource { key, .. } if key == "knuth1984")));
}

// ── События (T047, общая часть) ────────────────────────────────────────────

pub async fn writing_a_source_file_is_announced() {
    // Запись байтов в путь-кандидат: событие кладёт настоящий backend; у
    // двойника его нет — здесь проверяется только, что сама библиотека видит
    // изменённый файл по отпечатку (событие проверяют раннеры таргетов).
    let mut vault = MemoryVault::new().with_file("refs.bib", BIB);
    let mut engine = engine_with(&vault).await;
    vault
        .write_bytes(Path::new("refs.bib"), b"@misc{new, title = {t}}\n")
        .await
        .unwrap();
    let step = engine.refresh(&vault, &[]).await;
    assert!(step.changed);
    assert_eq!(engine.search("", 50, false).hits[0].source.key, "new");
    let _ = VaultEvent::SourceFilesChanged;
}

// ── Офлайн (T058) ──────────────────────────────────────────────────────────

pub async fn the_whole_path_runs_without_a_network() {
    let vault = notes_vault();
    let mut engine = engine_with(&vault).await;
    let hit = engine.search("knuth", 1, false).hits.remove(0);
    let plan = engine
        .plan_citation("x ", 2, &hit.source.file, &hit.source.key)
        .unwrap();
    assert_eq!(plan.edits[0].insert, "@knuth1984");
    scanned(&vault, &mut engine).await;
    assert_eq!(engine.check_note("@knuth1984", &[]).citations, 1);
}
