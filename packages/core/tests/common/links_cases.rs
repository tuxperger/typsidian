//! Связи заметки — общая таблица для обоих таргетов (фича 013).
//!
//! Правила чистые и работают поверх `VaultBackend`, поэтому одни и те же
//! проверки гоняются на native и в браузере (принцип II): расхождение между
//! таргетами было бы дефектом. Файлы `links_native.rs` и `links_wasm.rs` только
//! вызывают эти функции своим harness'ом.

#![allow(dead_code)]

use std::path::Path;

use typsidian_core::graph::{link_sites, link_text, resolve_link_target};
use typsidian_core::links::mentions::{self, MentionScan, ScanPhase};
use typsidian_core::links::snippet::{place_of, window};
use typsidian_core::links::{
    availability, backlinks_page, in_notes, link_mention, mentions_in_text, plan_link, LinkOutcome,
    LinkRequest, MentionsAvailability, OpenText, SiteRow, SourceProblem, BACKLINKS_PAGE_MAX,
    MENTION_STEP_NOTES,
};
use typsidian_core::vault::backend::{ReadError, VaultBackend};

use super::MemoryVault;

const TARGET: &str = "Проекты/Альфа.typ";

/// Три источника на разном расстоянии от цели и сама цель со ссылкой на себя.
fn vault() -> MemoryVault {
    MemoryVault::new()
        .with_file(TARGET, "Проект Альфа. Сама на себя: [[Альфа]]\n")
        .with_file(
            "Проекты/Б.typ",
            "Начало\nсм. [[Альфа]] тут и ещё [[Альфа]]\n[[/Проекты/Альфа]]\n",
        )
        .with_file("Заметки/В.typ", "ссылка на [[/Проекты/Альфа]] здесь")
        .with_file("Корень.typ", "[[/Проекты/Альфа]]")
        .with_file("Заметки/Не ссылка.typ", "про Альфа без скобок")
}

fn highlighted(row: &SiteRow) -> Vec<&str> {
    row.snippet
        .iter()
        .filter(|segment| segment.highlighted)
        .map(|segment| segment.text.as_str())
        .collect()
}

fn paths(page: &typsidian_core::links::BacklinksPage) -> Vec<&str> {
    page.groups
        .iter()
        .map(|group| group.path.as_str())
        .collect()
}

// ── US1: обратные ссылки ────────────────────────────────────────────────────

/// Две и более заметки-источника, ссылка выделена во фрагменте (FR-001, FR-002,
/// AS1); порядок групп — по близости папки.
pub async fn sources_come_with_highlighted_fragments_nearest_first() {
    let page = backlinks_page(&vault(), TARGET, &[], 0, 50).await;

    assert_eq!(page.target, TARGET);
    assert_eq!(page.total_sources, 3);
    assert_eq!(page.next_offset, None);
    assert_eq!(
        paths(&page),
        ["Проекты/Б.typ", "Корень.typ", "Заметки/В.typ"]
    );

    let near = &page.groups[0];
    assert_eq!(
        (near.title.as_str(), near.folder.as_str()),
        ("Б", "Проекты")
    );
    assert!(near.same_folder);
    assert_eq!(near.problem, None);

    let far = &page.groups[2];
    assert_eq!(far.folder, "Заметки");
    assert!(!far.same_folder);
    assert_eq!(far.rows.len(), 1);
    assert_eq!(highlighted(&far.rows[0]), ["[[/Проекты/Альфа]]"]);
    assert_eq!(far.rows[0].text, "[[/Проекты/Альфа]]");

    // Цель источником не бывает (FR-006), хотя ссылается на себя.
    assert!(!paths(&page).contains(&TARGET));
    // Упоминание без скобок ссылкой не считается.
    assert!(!paths(&page).contains(&"Заметки/Не ссылка.typ"));
}

/// Три ссылки из одной заметки — одна группа, три строки в порядке текста
/// (AS2); место строки указывает на `[[`.
pub async fn three_links_from_one_note_are_one_group() {
    let page = backlinks_page(&vault(), TARGET, &[], 0, 50).await;
    let rows = &page.groups[0].rows;
    assert_eq!(rows.len(), 3);

    let places: Vec<(u32, u32)> = rows
        .iter()
        .map(|row| (row.place.line, row.place.column))
        .collect();
    // «см. » — четыре символа; вторая ссылка — после «тут и ещё ».
    assert_eq!(places, [(2, 5), (2, 25), (3, 1)]);

    let text = "Начало\nсм. [[Альфа]] тут и ещё [[Альфа]]\n[[/Проекты/Альфа]]\n";
    for row in rows {
        let range = row.place.byte_start as usize..row.place.byte_end as usize;
        assert_eq!(&text[range], row.text);
        assert!(row.text.starts_with("[["));
    }
}

/// Нет ссылок — пустая страница (AS4).
pub async fn no_backlinks_is_an_empty_page() {
    let page = backlinks_page(&vault(), "Заметки/Не ссылка.typ", &[], 0, 50).await;
    assert_eq!(page.total_sources, 0);
    assert!(page.groups.is_empty());
    assert_eq!(page.next_offset, None);
}

/// Порции по `limit` и `next_offset`; `total_sources` из индекса (FR-005);
/// первая порция читает не больше `limit` файлов.
pub async fn pages_follow_limit_and_next_offset() {
    let mut vault = MemoryVault::new().with_file("цель.typ", "x");
    for i in 0..120 {
        vault = vault.with_file(&format!("s/{i:03}.typ"), "см. [[/цель]]");
    }

    let first = backlinks_page(&vault, "цель.typ", &[], 0, 50).await;
    assert_eq!(first.total_sources, 120);
    assert_eq!(first.groups.len(), 50);
    assert_eq!(first.next_offset, Some(50));
    let read: u32 = vault.reads.borrow().values().sum();
    assert_eq!(read, 50, "читаются только заметки порции");

    let second = backlinks_page(&vault, "цель.typ", &[], 50, 50).await;
    assert_eq!(second.groups.len(), 50);
    assert_eq!(second.next_offset, Some(100));
    assert_ne!(first.groups[0].path, second.groups[0].path);

    let last = backlinks_page(&vault, "цель.typ", &[], 100, 50).await;
    assert_eq!(last.groups.len(), 20);
    assert_eq!(last.next_offset, None);

    // Предел прижимается к 1..=BACKLINKS_PAGE_MAX.
    assert_eq!(
        backlinks_page(&vault, "цель.typ", &[], 0, 0)
            .await
            .groups
            .len(),
        1
    );
    assert_eq!(
        backlinks_page(&vault, "цель.typ", &[], 0, 10_000)
            .await
            .groups
            .len(),
        BACKLINKS_PAGE_MAX as usize
    );
    // Смещение за концом — пустая страница.
    let beyond = backlinks_page(&vault, "цель.typ", &[], 500, 50).await;
    assert!(beyond.groups.is_empty());
    assert_eq!(beyond.next_offset, None);
}

/// Источник сверх предела и нечитаемый — группа с `problem`.
pub async fn unreadable_sources_become_problem_groups() {
    let vault = vault()
        .with_failure("Проекты/Б.typ", ReadError::TooLarge)
        .with_failure("Корень.typ", ReadError::Io);
    let page = backlinks_page(&vault, TARGET, &[], 0, 50).await;

    assert_eq!(page.total_sources, 3);
    assert_eq!(page.groups[0].problem, Some(SourceProblem::TooLarge));
    assert!(page.groups[0].rows.is_empty());
    assert_eq!(page.groups[1].problem, Some(SourceProblem::Unreadable));
    assert_eq!(page.groups[2].problem, None);
    assert_eq!(page.groups[2].rows.len(), 1);
}

/// Ссылка на доску из заметки показывается в панели доски (Edge Cases).
pub async fn a_link_to_a_board_is_listed_for_the_board() {
    let vault = MemoryVault::new()
        .with_file("Доски/схема.excalidraw", "{}")
        .with_file("Доски/Г.typ", "рисунок [[схема.excalidraw]] рядом")
        .with_file("Доски/Д.typ", "ложная [[схема]]");
    let page = backlinks_page(&vault, "Доски/схема.excalidraw", &[], 0, 50).await;
    assert_eq!(paths(&page), ["Доски/Г.typ"]);
    assert_eq!(page.groups[0].rows[0].text, "[[схема.excalidraw]]");
}

/// Диапазоны `link_sites` на кириллице, колонка на строке с символом вне BMP,
/// обрезка окна с «…», подсветка одного из двух вхождений в строке.
pub fn sites_places_and_windows_are_the_same_everywhere() {
    // Диапазоны — байтовые и лежат на границах символов.
    let text = "см. [[Проект]] и [[б]]";
    let sites = link_sites(text);
    assert_eq!(sites.len(), 2);
    assert_eq!(&text[sites[0].byte_start..sites[0].byte_end], "[[Проект]]");
    assert_eq!(&text[sites[1].byte_start..sites[1].byte_end], "[[б]]");

    // 😀 — четыре байта, две единицы UTF-16 и один символ: колонка считается
    // в символах, как у диагностик предпросмотра.
    let text = "первая\nа😀б [[цель]]";
    let start = text.find("[[").unwrap();
    let place = place_of(text, start, start + "[[цель]]".len());
    assert_eq!((place.line, place.column), (2, 5));

    // Обрезка окна: многоточия по краям, слова целые, вхождение подсвечено.
    let text = format!("{}[[цель]]{}", "лево ".repeat(30), " право".repeat(30));
    let start = text.find("[[").unwrap();
    let segments = window(&text, start, start + "[[цель]]".len());
    let shown: String = segments.iter().map(|s| s.text.as_str()).collect();
    assert!(
        shown.starts_with("…лево") && shown.ends_with("право…"),
        "{shown}"
    );
    assert_eq!(segments.iter().filter(|s| s.highlighted).count(), 1);

    // Два вхождения в строке: подсвечено ровно то, чья это строка.
    let text = "[[A]] и снова [[A]] конец";
    let second = text.rfind("[[A]]").unwrap();
    let segments = window(text, second, second + 5);
    assert_eq!(segments[0].text, "[[A]] и снова ");
    assert!(!segments[0].highlighted);
    assert_eq!(segments[1].text, "[[A]]");
    assert!(segments[1].highlighted);
}

// ── US2: актуальность ───────────────────────────────────────────────────────

/// Запись новой ссылки видна при повторном запросе (FR-007); удаление ссылки
/// убирает группу.
pub async fn the_page_follows_writes() {
    let vault = MemoryVault::new()
        .with_file("цель.typ", "x")
        .with_file("б.typ", "пока без ссылки");
    assert!(backlinks_page(&vault, "цель.typ", &[], 0, 50)
        .await
        .groups
        .is_empty());

    vault.replace_externally("б.typ", "теперь [[/цель]]");
    let page = backlinks_page(&vault, "цель.typ", &[], 0, 50).await;
    assert_eq!(paths(&page), ["б.typ"]);

    vault.replace_externally("б.typ", "ссылку убрали");
    assert!(backlinks_page(&vault, "цель.typ", &[], 0, 50)
        .await
        .groups
        .is_empty());
}

/// Текст из `open_texts` приоритетнее диска; источник, чей текст уже без
/// ссылки при отставшем индексе, пустой группы не даёт.
pub async fn open_text_wins_over_the_disk() {
    let vault = MemoryVault::new()
        .with_file("цель.typ", "x")
        .with_file("б.typ", "[[/цель]]")
        .with_file("в.typ", "[[/цель]]");
    let open = [
        // Правка в редакторе сдвинула ссылку на вторую строку.
        OpenText {
            path: "б.typ".to_owned(),
            text: "вставка\n[[/цель]]".to_owned(),
        },
        // В редакторе ссылку уже убрали, на диске она ещё есть.
        OpenText {
            path: "в.typ".to_owned(),
            text: "без ссылки".to_owned(),
        },
    ];
    let page = backlinks_page(&vault, "цель.typ", &open, 0, 50).await;
    assert_eq!(paths(&page), ["б.typ"]);
    assert_eq!(page.groups[0].rows[0].place.line, 2);
}

/// Удаление и переименование источника — страница соответствует индексу
/// (FR-008).
pub async fn deleted_and_renamed_sources_follow_the_index() {
    let mut vault = MemoryVault::new()
        .with_file("цель.typ", "x")
        .with_file("б.typ", "[[/цель]]")
        .with_file("в.typ", "[[/цель]]");
    assert_eq!(
        backlinks_page(&vault, "цель.typ", &[], 0, 50)
            .await
            .total_sources,
        2
    );

    vault.delete(Path::new("б.typ")).await.unwrap();
    let page = backlinks_page(&vault, "цель.typ", &[], 0, 50).await;
    assert_eq!(paths(&page), ["в.typ"]);

    // Переименование для индекса — исчезновение старого пути и появление нового.
    vault.delete(Path::new("в.typ")).await.unwrap();
    vault.replace_externally("г.typ", "[[/цель]]");
    let page = backlinks_page(&vault, "цель.typ", &[], 0, 50).await;
    assert_eq!(paths(&page), ["г.typ"]);
    assert_eq!(page.total_sources, 1);
}

/// Страница для другой цели не содержит групп прежней (FR-009).
pub async fn a_page_belongs_to_its_target() {
    let vault = MemoryVault::new()
        .with_file("а.typ", "x")
        .with_file("б.typ", "y")
        .with_file("в.typ", "[[/а]]")
        .with_file("г.typ", "[[/б]]");
    let first = backlinks_page(&vault, "а.typ", &[], 0, 50).await;
    let second = backlinks_page(&vault, "б.typ", &[], 0, 50).await;
    assert_eq!(
        (first.target.as_str(), second.target.as_str()),
        ("а.typ", "б.typ")
    );
    assert_eq!(paths(&first), ["в.typ"]);
    assert_eq!(paths(&second), ["г.typ"]);
}

/// Ссылки на ещё не существующую заметку появляются в её странице и остаются,
/// когда заметку создают.
pub async fn links_to_a_missing_note_wait_for_it() {
    let vault = MemoryVault::new().with_file("б.typ", "см. [[Будущая]]");
    let before = backlinks_page(&vault, "Будущая.typ", &[], 0, 50).await;
    assert_eq!(paths(&before), ["б.typ"]);

    vault.replace_externally("Будущая.typ", "создана");
    let after = backlinks_page(&vault, "Будущая.typ", &[], 0, 50).await;
    assert_eq!(paths(&after), ["б.typ"]);
    assert_eq!(after.total_sources, 1);
}

// ── US3: упоминания ─────────────────────────────────────────────────────────

fn mention_vault() -> MemoryVault {
    MemoryVault::new()
        .with_file(
            "Проекты/Проект Альфа.typ",
            "Проект Альфа: заметка о проекте Альфа",
        )
        .with_file("Проекты/Б.typ", "см. проект альфа и ПРОЕКТ АЛЬФА")
        .with_file(
            "Заметки/В.typ",
            "ссылка [[/Проекты/Проект Альфа]] и Проект Альфа",
        )
        .with_file("Заметки/Г.typ", "ничего общего")
        .with_file("Доски/схема.excalidraw", "{\"t\":\"Проект Альфа\"}")
}

const MENTION_TARGET: &str = "Проекты/Проект Альфа.typ";

async fn scan_all(
    vault: &MemoryVault,
    target: &str,
    open: &[OpenText],
) -> (MentionScan, Vec<typsidian_core::links::SourceGroup>, usize) {
    let mut state = mentions::start(vault, target).await.expect("доступно");
    let mut found = Vec::new();
    let mut steps = 0;
    while state.phase == ScanPhase::Running {
        let step = mentions::step(vault, state, open).await;
        state = step.state;
        found.extend(step.found);
        steps += 1;
        assert!(steps < 100, "проход не кончается");
    }
    (state, found, steps)
}

/// Регистр не учитывается (FR-010, AS1); текст — как написан.
pub fn case_is_ignored() {
    let rows = mentions_in_text("см. проект альфа и ПРОЕКТ АЛЬФА", "Проект Альфа");
    let words: Vec<&str> = rows.iter().map(|row| row.text.as_str()).collect();
    assert_eq!(words, ["проект альфа", "ПРОЕКТ АЛЬФА"]);
    assert_eq!(rows[0].place.column, 5);
    assert_eq!(highlighted(&rows[1]), ["ПРОЕКТ АЛЬФА"]);
}

/// «Кот» внутри «который» не упоминание; название, кончающееся не буквой,
/// находится; название внутри `[[…]]` не упоминание (AS5); код и формулы не
/// исключаются.
pub fn word_boundaries_links_code_and_formulas() {
    assert!(mentions_in_text("который, котик, скот_1", "Кот").is_empty());
    assert_eq!(mentions_in_text("(кот), «Кот»; кот.", "Кот").len(), 3);

    assert_eq!(mentions_in_text("пишу на C++ и C++x", "C++").len(), 1);

    let text = "[[Проект Альфа]] и Проект Альфа, [[/a/Проект Альфа]]";
    let rows = mentions_in_text(text, "Проект Альфа");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].place.byte_start as usize,
        text.find(" и ").unwrap() + " и ".len()
    );

    let code = "`Проект Альфа` и $Проект Альфа$ и #let x = \"Проект Альфа\"";
    assert_eq!(mentions_in_text(code, "Проект Альфа").len(), 3);
}

/// Название короче трёх знаков — `TitleTooShort`, чтений нет (FR-011).
pub async fn a_short_title_reads_nothing() {
    let vault = MemoryVault::new()
        .with_file("a/ab.typ", "ab ab")
        .with_file("a/б.typ", "ab");
    assert!(matches!(
        availability("a/ab.typ"),
        MentionsAvailability::TitleTooShort { .. }
    ));
    let refused = mentions::start(&vault, "a/ab.typ").await;
    assert!(matches!(
        refused,
        Err(MentionsAvailability::TitleTooShort { ref title }) if title == "ab"
    ));
    assert!(in_notes(&vault, "a/ab.typ", &["a/б.typ".to_owned()], &[])
        .await
        .is_empty());
    assert_eq!(vault.total_reads(), 0);

    // Считаются символы, а не байты.
    assert!(matches!(
        availability("Кот.typ"),
        MentionsAvailability::Available { .. }
    ));
}

/// Проход обходит заметки, но не цель и не доски (FR-006).
pub async fn the_scan_skips_the_target_and_boards() {
    let vault = mention_vault();
    let (state, found, _) = scan_all(&vault, MENTION_TARGET, &[]).await;

    let mut paths: Vec<&str> = found.iter().map(|group| group.path.as_str()).collect();
    paths.sort();
    assert_eq!(paths, ["Заметки/В.typ", "Проекты/Б.typ"]);
    // Ближайшие по папке — первыми.
    assert_eq!(found[0].path, "Проекты/Б.typ");

    assert_eq!(vault.read_count(MENTION_TARGET), 0);
    assert_eq!(vault.read_count("Доски/схема.excalidraw"), 0);
    // Всего просмотрено три заметки: Б, В, Г.
    assert_eq!((state.scanned, state.total, state.skipped), (3, 3, 0));
    assert_eq!(state.phase, ScanPhase::Finished);
}

/// Проход больше одной порции доходит до `Finished` с верными счётчиками;
/// порция ограничена по заметкам и по байтам.
pub async fn a_long_scan_finishes_with_exact_counters() {
    let mut vault = MemoryVault::new().with_file("цель.typ", "x");
    for i in 0..150 {
        let body = if i % 10 == 0 {
            "тут цель рядом"
        } else {
            "пусто"
        };
        vault = vault.with_file(&format!("n/{i:03}.typ"), body);
    }

    let (state, found, steps) = scan_all(&vault, "цель.typ", &[]).await;
    assert!(steps >= 150_usize.div_ceil(MENTION_STEP_NOTES));
    assert_eq!((state.scanned, state.total, state.skipped), (150, 150, 0));
    assert_eq!(state.phase, ScanPhase::Finished);
    assert_eq!(found.len(), 15);

    // Бюджет по байтам: три заметки по 400 КиБ за шаг не влезают — только две.
    let big = "я".repeat(200 * 1024);
    let mut vault = MemoryVault::new().with_file("цель.typ", "x");
    for i in 0..5 {
        vault = vault.with_file(&format!("n/{i}.typ"), big.as_str());
    }
    let state = mentions::start(&vault, "цель.typ").await.unwrap();
    let step = mentions::step(&vault, state, &[]).await;
    assert_eq!(step.state.scanned, 2);
    assert_eq!(step.state.phase, ScanPhase::Running);
}

/// После `cancel` `step` ничего не читает.
pub async fn cancel_stops_reading() {
    let mut vault = MemoryVault::new().with_file("цель.typ", "x");
    for i in 0..100 {
        vault = vault.with_file(&format!("n/{i:03}.typ"), "цель");
    }
    let state = mentions::start(&vault, "цель.typ").await.unwrap();
    let step = mentions::step(&vault, state, &[]).await;
    assert_eq!(step.state.phase, ScanPhase::Running);
    assert_eq!(step.state.scanned as usize, MENTION_STEP_NOTES);

    let cancelled = mentions::cancel(step.state);
    assert_eq!(cancelled.phase, ScanPhase::Cancelled);
    let reads = vault.total_reads();

    let after = mentions::step(&vault, cancelled.clone(), &[]).await;
    assert!(after.found.is_empty());
    assert_eq!(after.state, cancelled);
    assert_eq!(vault.total_reads(), reads, "после отмены чтений нет");
}

/// Заметка, появившаяся между шагами, курсор не ломает.
pub async fn a_note_added_between_steps_does_not_break_the_cursor() {
    let mut vault = MemoryVault::new().with_file("цель.typ", "x");
    for i in 0..100 {
        vault = vault.with_file(&format!("n/{i:03}.typ"), "пусто");
    }
    let state = mentions::start(&vault, "цель.typ").await.unwrap();
    let first = mentions::step(&vault, state, &[]).await;
    assert_eq!(first.state.after.as_deref(), Some("n/063.typ"));

    // Одна заметка легла до курсора (её проход уже не увидит), другая — после.
    vault.replace_externally("n/000a.typ", "цель");
    vault.replace_externally("n/200.typ", "цель");
    let mut state = first.state;
    let mut found = Vec::new();
    while state.phase == ScanPhase::Running {
        let step = mentions::step(&vault, state, &[]).await;
        state = step.state;
        found.extend(step.found);
    }
    assert_eq!(state.phase, ScanPhase::Finished);
    assert!(state.scanned <= state.total);
    let paths: Vec<&str> = found.iter().map(|group| group.path.as_str()).collect();
    assert_eq!(paths, ["n/200.typ"]);
}

/// Нечитаемая заметка учитывается в `skipped`; текст открытого редактора
/// приоритетнее диска.
pub async fn unreadable_notes_are_skipped_and_open_texts_win() {
    let vault = MemoryVault::new()
        .with_file("цель.typ", "x")
        .with_file("a.typ", "цель")
        .with_file("б.typ", "ничего")
        .with_file("в.typ", "ничего")
        .with_failure("a.typ", ReadError::Io)
        .with_failure("в.typ", ReadError::TooLarge);
    let open = [OpenText {
        path: "б.typ".to_owned(),
        text: "в редакторе: цель".to_owned(),
    }];
    let (state, found, _) = scan_all(&vault, "цель.typ", &open).await;
    assert_eq!(state.skipped, 2);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, "б.typ");
    assert_eq!(found[0].rows[0].place.column, 14);
}

/// `in_notes` возвращает группы только названных заметок.
pub async fn in_notes_answers_only_for_the_named_notes() {
    let vault = mention_vault();
    let named = ["Проекты/Б.typ".to_owned(), "Заметки/Г.typ".to_owned()];
    let groups = in_notes(&vault, MENTION_TARGET, &named, &[]).await;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].path, "Проекты/Б.typ");
    assert_eq!(groups[0].rows.len(), 2);

    // Цель и доски среди источников не бывает.
    let refused = [
        MENTION_TARGET.to_owned(),
        "Доски/схема.excalidraw".to_owned(),
    ];
    assert!(in_notes(&vault, MENTION_TARGET, &refused, &[])
        .await
        .is_empty());
}

// ── US3: «Связать» ──────────────────────────────────────────────────────────

const SOURCE: &str = "Проекты/Б.typ";
const SOURCE_TEXT: &str = "до проект альфа после\nвторая 😀 строка Проект Альфа\n";

fn link_vault() -> MemoryVault {
    MemoryVault::new()
        .with_file(MENTION_TARGET, "Проект Альфа")
        .with_file(SOURCE, SOURCE_TEXT)
        .with_file("Заметки/В.typ", "тут проект альфа")
        .checking_expected()
}

fn request_for(source: &str, target: &str, text: &str, index: usize) -> LinkRequest {
    let rows = mentions_in_text(text, &typsidian_core::links::title_of(target));
    LinkRequest {
        target: target.to_owned(),
        source: source.to_owned(),
        place: rows[index].place,
        text: rows[index].text.clone(),
    }
}

/// Применяет замену в единицах UTF-16 — так её применит редактор.
fn apply_utf16(text: &str, edit: &typsidian_core::elements::edit::TextEdit) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut out: Vec<u16> = units[..edit.from].to_vec();
    out.extend(edit.insert.encode_utf16());
    out.extend_from_slice(&units[edit.to..]);
    String::from_utf16(&out).unwrap()
}

/// `Written`: файл отличается от прежнего только заменённым диапазоном
/// (FR-013, FR-014, SC-005); регистр заменяется названием в настоящем
/// написании; соседу — `[[название]]`.
pub async fn linking_changes_only_the_mention() {
    let mut vault = link_vault();
    let request = request_for(SOURCE, MENTION_TARGET, SOURCE_TEXT, 0);
    assert_eq!(request.text, "проект альфа");

    let outcome = link_mention(&mut vault, &request, None).await;
    assert_eq!(outcome, LinkOutcome::Written);
    assert_eq!(
        String::from_utf8(vault.bytes_of(SOURCE).unwrap()).unwrap(),
        "до [[Проект Альфа]] после\nвторая 😀 строка Проект Альфа\n"
    );
    assert_eq!(vault.write_count(SOURCE), 1);
    assert_eq!(vault.total_writes(), 1);
    assert_eq!(
        vault.bytes_of("Заметки/В.typ").unwrap(),
        "тут проект альфа".as_bytes()
    );

    // Теперь заметка действительно ссылается на цель.
    let page = backlinks_page(&vault, MENTION_TARGET, &[], 0, 50).await;
    assert_eq!(paths(&page), [SOURCE]);
}

/// Заметка из другой папки получает `[[/путь/название]]`; при двух одноимённых
/// заметках ссылка разрешается в ту, что в запросе (FR-017).
pub async fn the_link_resolves_to_the_requested_note() {
    let mut vault = link_vault();
    let text = "тут проект альфа";
    let request = request_for("Заметки/В.typ", MENTION_TARGET, text, 0);
    assert_eq!(
        link_mention(&mut vault, &request, None).await,
        LinkOutcome::Written
    );
    assert_eq!(
        vault.bytes_of("Заметки/В.typ").unwrap(),
        "тут [[/Проекты/Проект Альфа]]".as_bytes()
    );

    // Две «Альфа»: в Проектах и в Архиве. Источник — в Архиве, цель — Проекты.
    let mut twins = MemoryVault::new()
        .with_file("Проекты/Альфа.typ", "x")
        .with_file("Архив/Альфа.typ", "y")
        .with_file("Архив/Заметка.typ", "про Альфа и ещё Альфа")
        .checking_expected();
    let request = request_for(
        "Архив/Заметка.typ",
        "Проекты/Альфа.typ",
        "про Альфа и ещё Альфа",
        0,
    );
    assert_eq!(
        link_mention(&mut twins, &request, None).await,
        LinkOutcome::Written
    );
    let linked = backlinks_page(&twins, "Проекты/Альфа.typ", &[], 0, 50).await;
    assert_eq!(paths(&linked), ["Архив/Заметка.typ"]);
    let other = backlinks_page(&twins, "Архив/Альфа.typ", &[], 0, 50).await;
    assert!(
        other.groups.is_empty(),
        "ссылка не ушла в одноимённую заметку"
    );
}

/// Сдвиг текста перед местом — `Stale`, записей нет (FR-015).
pub async fn shifted_text_is_stale() {
    let mut vault = link_vault();
    let request = request_for(SOURCE, MENTION_TARGET, SOURCE_TEXT, 0);
    let shifted = format!("вставка\n{SOURCE_TEXT}");
    vault.replace_externally(SOURCE, shifted.as_bytes());

    assert_eq!(
        link_mention(&mut vault, &request, None).await,
        LinkOutcome::Stale
    );
    assert_eq!(vault.total_writes(), 0);
    assert_eq!(vault.bytes_of(SOURCE).unwrap(), shifted.as_bytes());
}

/// Файл изменён между индексом и записью — `Stale` по `Conflict`, записей нет.
pub async fn a_stale_stamp_conflicts_into_stale() {
    let mut vault = link_vault().with_lagging_index(SOURCE);
    let request = request_for(SOURCE, MENTION_TARGET, SOURCE_TEXT, 0);
    // Правка в другом месте файла: место прежнее, но отпечаток индекса отстал.
    let edited = format!("{SOURCE_TEXT}дописано снаружи\n");
    vault.replace_externally(SOURCE, edited.as_bytes());

    assert_eq!(
        link_mention(&mut vault, &request, None).await,
        LinkOutcome::Stale
    );
    assert_eq!(vault.total_writes(), 0);
    assert_eq!(vault.bytes_of(SOURCE).unwrap(), edited.as_bytes());
}

/// Vault только на чтение — `ReadOnly`, файл не тронут (FR-018).
pub async fn a_read_only_vault_refuses() {
    let mut vault = link_vault().read_only();
    let request = request_for(SOURCE, MENTION_TARGET, SOURCE_TEXT, 0);
    assert_eq!(
        link_mention(&mut vault, &request, None).await,
        LinkOutcome::ReadOnly
    );
    assert_eq!(vault.total_writes(), 0);
    assert_eq!(vault.bytes_of(SOURCE).unwrap(), SOURCE_TEXT.as_bytes());
}

/// `editor_text` → `EditorEdit` с правкой в UTF-16, записей на диск нет;
/// применение правки сохраняет остальные правки редактора (FR-016, SC-006).
pub async fn the_editor_gets_an_edit_and_the_disk_nothing() {
    let mut vault = link_vault();
    // В редакторе уже есть несохранённые правки, которых на диске нет.
    let editor = "😀 набрано, ещё не сохранено\nдо проект альфа после\n";
    let request = request_for(SOURCE, MENTION_TARGET, editor, 0);

    let outcome = link_mention(&mut vault, &request, Some(editor)).await;
    let LinkOutcome::EditorEdit { edit } = outcome else {
        panic!("ждали EditorEdit, получили {outcome:?}");
    };
    assert_eq!(vault.total_writes(), 0);
    assert_eq!(vault.bytes_of(SOURCE).unwrap(), SOURCE_TEXT.as_bytes());

    // 😀 занимает две единицы UTF-16: правка считается в них, а не в байтах.
    assert_eq!(edit.insert, "[[Проект Альфа]]");
    assert_eq!(
        apply_utf16(editor, &edit),
        "😀 набрано, ещё не сохранено\nдо [[Проект Альфа]] после\n"
    );

    // Текст редактора устарел — тот же вопрос, но `Stale`.
    let moved = "вставка\n😀 набрано, ещё не сохранено\nдо проект альфа после\n";
    assert_eq!(
        link_mention(&mut vault, &request, Some(moved)).await,
        LinkOutcome::Stale
    );
    assert_eq!(vault.total_writes(), 0);
}

/// Несохранённые правки вне редактора — `UnsavedElsewhere`, записей нет.
pub async fn unsaved_edits_without_an_editor_refuse() {
    let mut vault = link_vault().with_unsaved_edits(SOURCE);
    let request = request_for(SOURCE, MENTION_TARGET, SOURCE_TEXT, 0);
    assert_eq!(
        link_mention(&mut vault, &request, None).await,
        LinkOutcome::UnsavedElsewhere
    );
    assert_eq!(vault.total_writes(), 0);
}

/// Цель с `]]` в пути — `Unexpressible`.
pub async fn an_unwritable_target_is_unexpressible() {
    let target = "Папка/Хитрая]]Заметка.typ";
    let mut vault = MemoryVault::new()
        .with_file(target, "x")
        .with_file("б.typ", "про Хитрая]]Заметка тут")
        .checking_expected();
    let request = LinkRequest {
        target: target.to_owned(),
        source: "б.typ".to_owned(),
        place: place_of("про Хитрая]]Заметка тут", 7, 7 + "Хитрая]]Заметка".len()),
        text: "Хитрая]]Заметка".to_owned(),
    };
    assert_eq!(
        link_mention(&mut vault, &request, None).await,
        LinkOutcome::Unexpressible
    );
    assert_eq!(vault.total_writes(), 0);
}

/// Чистое планирование отказывает, когда на месте уже не упоминание.
pub fn planning_is_pure_and_strict() {
    let text = "до проект альфа после";
    let request = request_for(SOURCE, MENTION_TARGET, text, 0);
    let plan = plan_link(text, &request, "[[Проект Альфа]]").unwrap();
    assert_eq!(plan.new_text, "до [[Проект Альфа]] после");
    // Теперь это уже ссылка, а не упоминание.
    assert!(plan_link(&plan.new_text, &request, "[[Проект Альфа]]").is_err());
}

/// Таблица `link_text` с обратной проверкой `resolve_link_target`.
pub fn link_text_round_trips_through_resolve() {
    let table = [
        ("a/src.typ", "a/Проект.typ", "[[Проект]]"),
        ("a/src.typ", "b/Проект.typ", "[[/b/Проект]]"),
        ("src.typ", "docs/x.typ", "[[/docs/x]]"),
        ("src.typ", "x.typ", "[[x]]"),
        ("a/src.typ", "a/схема.excalidraw", "[[схема.excalidraw]]"),
        ("a/src.typ", "a/глава.1.typ", "[[глава.1]]"),
    ];
    for (source, target, expected) in table {
        let text = link_text(Path::new(source), Path::new(target)).expect(target);
        assert_eq!(text, expected);
        let inner = &text[2..text.len() - 2];
        assert_eq!(
            resolve_link_target(Path::new(source), inner).as_deref(),
            Some(Path::new(target))
        );
    }
    assert_eq!(link_text(Path::new("s.typ"), Path::new("a]]b.typ")), None);
}
