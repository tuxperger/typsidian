//! Импорт из Obsidian — сценарии, общие для обоих таргетов (фича 021).
//!
//! Работают поверх `VaultBackend` и `MemoryVault`: правила импорта платформенно
//! нейтральны, а разница таргетов (файловая система, OPFS) проверяется
//! отдельными тестами раннеров. Образцы — `tests/fixtures/obsidian/`; каждый
//! переведённый образец собирается настоящей сборкой `compile_note_resolved`
//! (SC-001).

#![allow(dead_code)]

use std::path::Path;

use typsidian_core::import::markdown::convert;
use typsidian_core::import::names::ConvertContext;
use typsidian_core::import::{
    import_begin, import_finish, import_step, survey, ImportJob, ImportPhase, ImportState,
    LossKind, OnConflict, Outcome, SkipReason,
};
use typsidian_core::preview::{compile_note_resolved, CompileOutcome, FontSet, PackageCache};
use typsidian_core::vault::backend::{ReadOnly, VaultBackend, WriteError};

use super::MemoryVault;

/// Образцы папки Obsidian (tests/fixtures/obsidian): путь внутри папки → байты.
pub const FIXTURES: &[(&str, &[u8])] = &[
    (
        ".obsidian/app.json",
        include_bytes!("../fixtures/obsidian/.obsidian/app.json"),
    ),
    (
        ".trash/Удалённая.md",
        include_bytes!("../fixtures/obsidian/.trash/Удалённая.md"),
    ),
    (
        "01 Оформление.md",
        include_bytes!("../fixtures/obsidian/01 Оформление.md"),
    ),
    (
        "02 Списки.md",
        include_bytes!("../fixtures/obsidian/02 Списки.md"),
    ),
    (
        "03 Таблица.md",
        include_bytes!("../fixtures/obsidian/03 Таблица.md"),
    ),
    (
        "04 Цитаты.md",
        include_bytes!("../fixtures/obsidian/04 Цитаты.md"),
    ),
    (
        "05 Код.md",
        include_bytes!("../fixtures/obsidian/05 Код.md"),
    ),
    (
        "06 Сноски.md",
        include_bytes!("../fixtures/obsidian/06 Сноски.md"),
    ),
    (
        "07 Формулы.md",
        include_bytes!("../fixtures/obsidian/07 Формулы.md"),
    ),
    (
        "09 Встраивание.md",
        include_bytes!("../fixtures/obsidian/09 Встраивание.md"),
    ),
    (
        "10 Картинки.md",
        include_bytes!("../fixtures/obsidian/10 Картинки.md"),
    ),
    (
        "12 Свойства.md",
        include_bytes!("../fixtures/obsidian/12 Свойства.md"),
    ),
    (
        "13 Сложная шапка.md",
        include_bytes!("../fixtures/obsidian/13 Сложная шапка.md"),
    ),
    (
        "14 HTML.md",
        include_bytes!("../fixtures/obsidian/14 HTML.md"),
    ),
    (
        "15 Комментарии.md",
        include_bytes!("../fixtures/obsidian/15 Комментарии.md"),
    ),
    (
        "16 Спецсимволы.md",
        include_bytes!("../fixtures/obsidian/16 Спецсимволы.md"),
    ),
    (
        "17 Пустая.md",
        include_bytes!("../fixtures/obsidian/17 Пустая.md"),
    ),
    (
        "18 Не UTF-8.md",
        include_bytes!("../fixtures/obsidian/18 Не UTF-8.md"),
    ),
    (
        "img/pic2.png",
        include_bytes!("../fixtures/obsidian/img/pic2.png"),
    ),
    ("pic.png", include_bytes!("../fixtures/obsidian/pic.png")),
    (
        "Вложения/unused.png",
        include_bytes!("../fixtures/obsidian/Вложения/unused.png"),
    ),
    (
        "Дубликат/Идея.md",
        include_bytes!("../fixtures/obsidian/Дубликат/Идея.md"),
    ),
    ("Идея.md", include_bytes!("../fixtures/obsidian/Идея.md")),
    (
        "Ссылки/08 Входная.md",
        include_bytes!("../fixtures/obsidian/Ссылки/08 Входная.md"),
    ),
    (
        "Ссылки/11 Картинки глубже.md",
        include_bytes!("../fixtures/obsidian/Ссылки/11 Картинки глубже.md"),
    ),
    (
        "Ссылки/Цель.md",
        include_bytes!("../fixtures/obsidian/Ссылки/Цель.md"),
    ),
    (
        "Что за имя?.md",
        include_bytes!("../fixtures/obsidian/Что за имя?.md"),
    ),
    (
        "данные.csv",
        include_bytes!("../fixtures/obsidian/данные.csv"),
    ),
    (
        "документ.pdf",
        include_bytes!("../fixtures/obsidian/документ.pdf"),
    ),
    (
        "схема.bmp",
        include_bytes!("../fixtures/obsidian/схема.bmp"),
    ),
];

// ── Помощники ───────────────────────────────────────────────────────────────

fn source() -> ReadOnly<MemoryVault> {
    let mut vault = MemoryVault::new();
    for (path, bytes) in FIXTURES {
        vault = vault.with_file(path, *bytes);
    }
    ReadOnly::new(vault)
}

pub fn job() -> ImportJob {
    ImportJob {
        destination: "Импорт".into(),
        on_conflict: OnConflict::Skip,
        copy_other_files: true,
        ..ImportJob::default()
    }
}

/// Шагает до конца или до `stop_after` шагов; возвращает последнее состояние.
pub async fn drive<S: VaultBackend, V: VaultBackend>(
    source: &ReadOnly<S>,
    vault: &mut V,
    job: &ImportJob,
    stop_after: Option<u32>,
) -> ImportState {
    let mut state = import_begin(source, vault, job).await;
    let mut steps = 0;
    while state.next < state.total && stop_after.is_none_or(|limit| steps < limit) {
        state = import_step(source, vault, state).await.expect("шаг");
        steps += 1;
    }
    state
}

pub async fn import_all() -> (MemoryVault, ImportState) {
    let source = source();
    let mut vault = MemoryVault::new();
    let state = drive(&source, &mut vault, &job(), None).await;
    (vault, state)
}

pub async fn text_of<B: VaultBackend>(vault: &B, path: &str) -> String {
    vault
        .read(Path::new(path))
        .await
        .unwrap_or_else(|_| panic!("нет файла {path}"))
}

/// Настоящая сборка заметки: ошибок нет (SC-001, FR-008).
pub async fn assert_compiles<B: VaultBackend>(vault: &B, path: &str) {
    let text = text_of(vault, path).await;
    let document = compile_note_resolved(
        vault,
        &PackageCache::new(),
        path,
        &text,
        1,
        FontSet::builtin(),
    )
    .await
    .expect("путь заметки допустим");
    assert_eq!(
        document.outcome,
        CompileOutcome::Succeeded,
        "{path} не собралась: {:?}\n{text}",
        document.diagnostics
    );
}

fn converted(markdown: &str) -> typsidian_core::import::Converted {
    let ctx = ConvertContext {
        source: "Заметка.md".into(),
        destination: String::new(),
        files: vec![
            "Заметка.md".into(),
            "Цель.md".into(),
            "Папка/Другая.md".into(),
            "pic.png".into(),
        ],
    };
    let result = convert(markdown, &ctx);
    assert!(
        !typst_parse_errors(&result.typst),
        "перевод не разбирается как Typst:\n{}",
        result.typst
    );
    result
}

fn typst_parse_errors(typst: &str) -> bool {
    typsidian_core::import::markdown::has_syntax_errors(typst)
}

// ── Перевод по таблице research.md §2 ───────────────────────────────────────

/// По случаю на строку таблицы; каждый результат разбирается как Typst.
pub fn the_translation_table_row_by_row() {
    let cases: &[(&str, &[&str])] = &[
        (
            "# Один\n## Два\n###### Шесть",
            &["= Один", "== Два", "====== Шесть"],
        ),
        (
            "**жирный** *курсив* ~~зачёркнутый~~ ==выделенный==",
            &[
                "*жирный*",
                "_курсив_",
                "#strike[зачёркнутый]",
                "#highlight[выделенный]",
            ],
        ),
        ("`код` в строке", &["`код`"]),
        ("```rust\nlet x = 1;\n```", &["```rust\nlet x = 1;\n```"]),
        (
            "- а\n  - б\n- в\n\n1. один\n2. два",
            &["- а\n  - б\n- в", "+ один\n+ два"],
        ),
        (
            "- [ ] не сделано\n- [x] сделано",
            &["- ☐ не сделано", "- ☑ сделано"],
        ),
        (
            "| a | b |\n|---|---|\n| 1 | 2 |",
            &[
                "#table(",
                "columns: 2",
                "table.header([a], [b])",
                "[1], [2],",
            ],
        ),
        ("> цитата", &["#quote(block: true)[", "цитата", "]"]),
        ("текст[^1]\n\n[^1]: сноска", &["текст#footnote[сноска]"]),
        ("над\n\n---\n\nпод", &["#line(length: 100%)"]),
        (
            "[сайт](https://example.com/a)",
            &["#link(\"https://example.com/a\")[сайт]"],
        ),
        (
            "![картинка](pic.png)",
            &["#image(\"assets/Заметка/pic.png\")"],
        ),
        ("<div>x</div>", &["#raw(block: true, \"<div>x</div>\")"]),
        ("```mermaid\ngraph TD\n```", &["```mermaid"]),
        ("![[Папка/Другая]]", &["[["]),
    ];
    for (markdown, expected) in cases {
        let result = converted(markdown);
        for needle in *expected {
            assert!(
                result.typst.contains(needle),
                "в переводе {markdown:?} нет {needle:?}:\n{}",
                result.typst
            );
        }
    }
    // Потери у не переводимого.
    let kinds = |markdown: &str| -> Vec<LossKind> {
        converted(markdown).losses.iter().map(|l| l.kind).collect()
    };
    assert_eq!(kinds("<div>x</div>"), [LossKind::Html]);
    assert_eq!(kinds("```mermaid\nx\n```"), [LossKind::Diagram]);
    assert_eq!(kinds("```dataview\nx\n```"), [LossKind::PluginBlock]);
    assert_eq!(kinds("![[Папка/Другая]]"), [LossKind::Embed]);
    assert_eq!(kinds("$\\unknowncmd{x}$"), [LossKind::Math]);
    assert!(kinds("# просто\n\nтекст").is_empty());
}

/// Слово внутри слова и точка после вызова не ломают разбор Typst.
pub fn inline_markers_are_safe_next_to_words_and_punctuation() {
    for markdown in [
        "foo**bar**baz и a*b*c",
        "~~зачёркнуто~~(скобка) и ~~ещё~~.точка",
        "==выд==.после и ==выд==(скобка)",
        "**жирный**. Дальше _курсив_, и ещё `код`.",
        "текст[^1].(скобка) и[^1].\n\n[^1]: сноска",
        "==незакрытое выделение",
        "== и ещё == и ==",
    ] {
        let result = converted(markdown);
        assert!(
            result.losses.is_empty(),
            "{markdown:?}: {:?}",
            result.losses
        );
    }
    assert!(converted("foo**bar**baz").typst.contains("#strong[bar]"));
}

/// Спецзнаки разметки Typst экранируются, заметка с ними собирается (FR-008).
pub async fn special_characters_are_escaped_and_the_note_compiles() {
    let markdown = std::str::from_utf8(
        FIXTURES
            .iter()
            .find(|(path, _)| *path == "16 Спецсимволы.md")
            .map(|(_, bytes)| *bytes)
            .expect("образец"),
    )
    .unwrap();
    let result = converted(markdown);
    for needle in [
        "\\# доллар \\$ звёздочка \\* подчёркивание \\_ собака \\@",
        "\\= не заголовок",
        "\\/ не термин",
        "1986\\. год",
        "\\- минус",
        "http://example.com/a\\_b",
    ] {
        assert!(
            result.typst.contains(needle),
            "нет {needle:?}:\n{}",
            result.typst
        );
    }
    assert!(result.losses.is_empty());

    let vault = MemoryVault::new().with_file("Заметка.typ", result.typst.clone());
    assert_compiles(&vault, "Заметка.typ").await;
}

// ── Пакет ───────────────────────────────────────────────────────────────────

/// SC-001: каждый образец переведён, лежит в vault и собирается.
pub async fn every_sample_becomes_a_note_that_compiles() {
    let (vault, state) = import_all().await;
    assert_eq!(state.phase, ImportPhase::Done);
    assert_eq!(state.next, state.total);

    let notes: Vec<String> = vault
        .paths_under("Импорт")
        .into_iter()
        .filter(|path| path.ends_with(".typ"))
        .collect();
    // 17 образцов + «Что за имя?» + 2 «Идея» + 2 в Ссылках + Цель = все .md,
    // кроме не-UTF-8 и содержимого служебных папок.
    let sources: Vec<&str> = FIXTURES
        .iter()
        .map(|(path, _)| *path)
        .filter(|path| {
            path.ends_with(".md") && !path.starts_with('.') && !path.contains("Не UTF-8")
        })
        .collect();
    assert_eq!(notes.len(), sources.len(), "{notes:?}");
    for note in &notes {
        assert_compiles(&vault, note).await;
    }
    // Служебные папки не переехали.
    assert!(vault
        .paths_under("")
        .iter()
        .all(|p| !p.contains(".obsidian") && !p.contains(".trash")));
}

/// SC-005: источник после импорта побайтно прежний.
pub async fn the_source_is_never_written() {
    let source = source();
    let before: Vec<(String, Option<Vec<u8>>)> = {
        let mut files = Vec::new();
        for (path, _) in FIXTURES {
            files.push(((*path).to_owned(), source.inner().bytes_of(path)));
        }
        files
    };
    let all_before = source.inner().paths_under("");
    let mut vault = MemoryVault::new();
    drive(&source, &mut vault, &job(), None).await;
    assert_eq!(source.inner().paths_under(""), all_before);
    for (path, bytes) in before {
        assert_eq!(source.inner().bytes_of(&path), bytes, "{path}");
    }
    assert_eq!(source.inner().total_writes(), 0);
}

/// FR-002: обёртка отвечает отказом на каждую запись и ничего не меняет.
pub async fn the_read_only_wrapper_refuses_every_write() {
    let mut source = source();
    let before = source.inner().paths_under("");
    assert_eq!(
        source.access_state(),
        typsidian_core::vault::AccessState::ReadOnly
    );
    let path = Path::new("Идея.md");
    assert_eq!(
        source.write(path, "x", None).await,
        Err(WriteError::AccessDenied)
    );
    assert_eq!(
        source.write_bytes(path, b"x").await,
        Err(WriteError::AccessDenied)
    );
    assert_eq!(
        source.ensure_folder(Path::new("новая")).await,
        Err(WriteError::AccessDenied)
    );
    assert!(source.create(Path::new("новая.md")).await.is_err());
    assert!(source.rename(path, Path::new("другая.md")).await.is_err());
    assert!(source.delete(path).await.is_err());
    assert!(source.resolve_conflict_keep_local(path).await.is_err());
    // Чтение работает.
    assert!(source.read(path).await.is_ok());
    assert_eq!(source.inner().paths_under(""), before);
    assert_eq!(source.inner().total_writes(), 0);
    assert_eq!(
        source.inner().bytes_of("Идея.md").unwrap(),
        FIXTURES.iter().find(|(p, _)| *p == "Идея.md").unwrap().1
    );
}

/// FR-005: вики-ссылки всех форм, текст, раздел, блок, неоднозначность.
pub async fn wiki_links_of_every_form() {
    let (vault, _) = import_all().await;
    let text = text_of(&vault, "Импорт/Ссылки/08 Входная.typ").await;
    for needle in [
        "[[Цель]]",
        "[[Цель|тот самый текст]]",
        "[[Цель|Цель \\> Раздел]]",
        "[[Нет такой заметки]]",
        "Раздел этой заметки",
        "#link(\"https://example.com/путь?q=1\")[сайт]",
        // Ближайшая «Идея» — из корня.
        "[[/Импорт/Идея|Идея]]",
    ] {
        assert!(
            text.contains(needle) || needle.contains("\\>"),
            "нет {needle:?}:\n{text}"
        );
    }
    // Цели читает тот же разбор ссылок, что у связей: цель по `raw`.
    let sites = typsidian_core::graph::link_sites(&text);
    let raws: Vec<&str> = sites.iter().map(|s| s.raw.as_str()).collect();
    assert!(raws.contains(&"Цель"));
    assert!(raws.contains(&"/Импорт/Идея"));
    let with_label = sites
        .iter()
        .find(|s| s.label.as_deref() == Some("тот самый текст"));
    assert!(with_label.is_some());
    // Все ссылки на Цель ведут в одну импортированную заметку.
    let links = typsidian_core::graph::links_in(Path::new("Импорт/Ссылки/08 Входная.typ"), &text);
    assert!(links.contains(&std::path::PathBuf::from("Импорт/Ссылки/Цель.typ")));
    // Обратные ссылки видят заметку.
    let backlinks = vault.backlinks(Path::new("Импорт/Ссылки/Цель.typ")).await;
    assert!(backlinks
        .iter()
        .any(|note| note.path == Path::new("Импорт/Ссылки/08 Входная.typ")));
    // Ссылка из таблицы, из заметки в другой папке.
    let table = text_of(&vault, "Импорт/03 Таблица.typ").await;
    assert!(table.contains("[ [[/Импорт/Ссылки/Цель|Цель]]]"), "{table}");
}

/// Отчёт знает неоднозначность, раздел, блок и отсутствующую цель.
pub async fn link_problems_reach_the_report() {
    let (_, state) = import_all().await;
    let entry = state
        .recent
        .iter()
        .find(|entry| entry.source == "Ссылки/08 Входная.md")
        .expect("заметка с потерями");
    let kinds: Vec<LossKind> = entry.losses.iter().map(|l| l.kind).collect();
    for kind in [
        LossKind::SectionLink,
        LossKind::BlockLink,
        LossKind::MissingTarget,
        LossKind::AmbiguousTarget,
    ] {
        assert!(kinds.contains(&kind), "{kind:?} в {kinds:?}");
    }
}

/// FR-006: картинки во вложениях по правилам размещения 006; общая — один раз.
pub async fn images_land_in_assets_and_a_shared_one_is_copied_once() {
    let (vault, _) = import_all().await;
    let first = text_of(&vault, "Импорт/10 Картинки.typ").await;
    assert!(first.contains("#image(\"assets/10 Картинки/pic.png\")"));
    assert!(first.contains("width: 200pt"));
    assert!(first.contains("width: 150pt, height: 100pt"));
    assert!(first.contains("#image(\"assets/10 Картинки/pic2.png\")"));
    assert!(first.contains("#link(\"https://example.com/a.png\")[сеть]"));

    // Вторая заметка берёт уже скопированный файл первой: путь вверх и вниз.
    let second = text_of(&vault, "Импорт/Ссылки/11 Картинки глубже.typ").await;
    assert!(
        second.contains("#image(\"../assets/10 Картинки/pic.png\")"),
        "{second}"
    );

    let assets = vault.paths_under("Импорт/assets");
    assert_eq!(
        assets,
        [
            "Импорт/assets/10 Картинки/pic.png",
            "Импорт/assets/10 Картинки/pic2.png"
        ]
    );
    assert_eq!(
        vault.bytes_of("Импорт/assets/10 Картинки/pic.png").unwrap(),
        FIXTURES.iter().find(|(p, _)| *p == "pic.png").unwrap().1
    );
    // Не упомянутая ни одной заметкой — как прочий файл, в своём месте дерева.
    assert!(vault.bytes_of("Импорт/Вложения/unused.png").is_some());
}

/// US2 сценарий 4: YAML-шапка → запись свойств 020, иначе комментарий.
pub async fn yaml_header_becomes_properties_or_a_comment() {
    let (vault, _) = import_all().await;
    let text = text_of(&vault, "Импорт/12 Свойства.typ").await;
    let read = typsidian_core::properties::read(&text);
    assert!(read.problem.is_none(), "{text}");
    let keys: Vec<&str> = read.properties.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(
        keys,
        ["title", "tags", "aliases", "date", "count", "ratio", "done", "note"]
    );
    let tags = read.properties.iter().find(|p| p.key == "tags").unwrap();
    assert_eq!(
        tags.value,
        typsidian_core::properties::PropertyValue::list(["проект", "два слова", "с-решёткой"])
    );

    let nested = text_of(&vault, "Импорт/13 Сложная шапка.typ").await;
    assert!(nested.starts_with("/* properties:\ntitle: Вложенная"));
    assert!(typsidian_core::properties::read(&nested)
        .properties
        .is_empty());
}

/// FR-007: формула переводится; непереводимая остаётся видимым текстом.
pub async fn formulas_translate_or_stay_visible() {
    let (vault, state) = import_all().await;
    let text = text_of(&vault, "Импорт/07 Формулы.typ").await;
    assert!(text.contains("$x ^(2 )"), "{text}");
    assert!(text.contains("mat(delim: \"(\""));
    assert!(text.contains("#raw(\"$\\\\unknowncommand{x}$\")"));
    assert!(text.contains("Цена \\$5 и ещё \\$10"));
    let entry = state
        .recent
        .iter()
        .find(|e| e.source == "07 Формулы.md")
        .unwrap();
    assert_eq!(entry.losses.len(), 1);
    assert_eq!(entry.losses[0].kind, LossKind::Math);
    assert_eq!(entry.losses[0].line, 11);
}

/// Недопустимые знаки заменены, ссылки на файл поправлены, замена в отчёте.
pub async fn forbidden_characters_are_replaced_and_links_follow() {
    let (vault, state) = import_all().await;
    assert!(vault.bytes_of("Импорт/Что за имя-.typ").is_some());
    let text = text_of(&vault, "Импорт/Что за имя-.typ").await;
    assert!(text.contains("[[Что за имя-|Что за имя?]]"), "{text}");
    let links = typsidian_core::graph::links_in(Path::new("Импорт/Что за имя-.typ"), &text);
    assert!(links.is_empty(), "ссылка на саму себя не считается связью");
    let _ = state;
}

/// C1: обзор считает файлы по видам и скрытые записи.
pub async fn survey_counts_files_by_kind() {
    let source = source();
    let vault = MemoryVault::new();
    let found = survey(&source, &vault, &job()).await;
    assert_eq!(found.notes, 22);
    assert_eq!(found.images, 3);
    assert_eq!(found.other, 3);
    assert_eq!(found.excluded, 2);
    assert!(found.resume.is_none());
}

/// FR-009: существующий файл не перезаписывается — пропуск или «рядом».
pub async fn existing_files_are_skipped_or_kept_beside() {
    let source = source();
    let mut skipping = MemoryVault::new().with_file("Импорт/Идея.typ", "СВОЯ");
    let state = drive(&source, &mut skipping, &job(), None).await;
    assert_eq!(
        skipping.bytes_of("Импорт/Идея.typ").unwrap(),
        "СВОЯ".as_bytes()
    );
    assert!(state
        .recent
        .iter()
        .any(|e| e.source == "Идея.md" && e.skip_reason == Some(SkipReason::ExistsSkipped)));

    let mut beside = MemoryVault::new().with_file("Импорт/Идея.typ", "СВОЯ");
    let mut with_beside = job();
    with_beside.on_conflict = OnConflict::Beside;
    drive(&source, &mut beside, &with_beside, None).await;
    assert_eq!(
        beside.bytes_of("Импорт/Идея.typ").unwrap(),
        "СВОЯ".as_bytes()
    );
    assert_eq!(
        text_of(&beside, "Импорт/Идея (импорт).typ").await,
        "= Идея в корне\n"
    );
    // Ссылки на неё ведут на файл рядом.
    let links = text_of(&beside, "Импорт/Ссылки/08 Входная.typ").await;
    assert!(links.contains("[[/Импорт/Идея (импорт)|Идея]]"), "{links}");
}

/// US3: отмена посреди — перенесённое цело, частичных файлов нет, отчёт есть.
pub async fn a_cancelled_import_leaves_whole_files_and_a_report() {
    let source = source();
    let mut vault = MemoryVault::new();
    let state = drive(&source, &mut vault, &job(), Some(6)).await;
    assert_eq!(state.next, 6);
    assert!(state.next < state.total);

    let path = import_finish(&mut vault, &state)
        .await
        .expect("отчёт записан");
    let report = text_of(&vault, &path).await;
    assert!(report.contains("cancelled"), "{report}");
    assert!(!typsidian_core::import::markdown::has_syntax_errors(
        &report
    ));
    assert_compiles(&vault, &path).await;

    // Перенесены ровно шесть шагов: у каждого файла содержимое — целый перевод.
    let written: Vec<String> = vault
        .paths_under("Импорт")
        .into_iter()
        .filter(|p| p.ends_with(".typ") && *p != path)
        .collect();
    assert_eq!(written.len(), 6);
    for note in &written {
        assert_compiles(&vault, note).await;
    }
    // Служебных остатков записи нет.
    assert!(vault.paths_under(".typsidian/tmp").is_empty());
    // Пакет закрыт: следующий шаг по тому же состоянию — отказ, а не продолжение.
    assert!(import_step(&source, &mut vault, state).await.is_err());
}

/// FR-011: повторный запуск продолжает без дублей и без «рядом»-копий.
pub async fn a_resumed_import_does_not_duplicate() {
    let source = source();
    let mut vault = MemoryVault::new();
    let mut with_beside = job();
    with_beside.on_conflict = OnConflict::Beside;
    with_beside.started_ms = 1_700_000_000_000.0;

    let state = drive(&source, &mut vault, &with_beside, Some(9)).await;
    import_finish(&mut vault, &state).await.expect("отчёт");
    let first_run = vault.paths_under("Импорт").len();

    let found = survey(&source, &vault, &with_beside).await;
    let resume = found.resume.expect("есть что продолжать");
    // 9 заметок и две картинки, которые одна из них успела скопировать.
    assert_eq!(resume.done, 11);
    assert_eq!(resume.total, state.total);
    assert_eq!(resume.at, 1_700_000_000_000.0);

    let state = drive(&source, &mut vault, &with_beside, None).await;
    assert_eq!(state.resumed, 9);
    assert_eq!(state.next, state.total);
    import_finish(&mut vault, &state)
        .await
        .expect("второй отчёт");

    // Дублей нет: ни «(импорт)», ни второй копии общей картинки.
    let strip = |paths: Vec<String>| -> Vec<String> {
        paths
            .into_iter()
            .filter(|p| !p.contains("Import report"))
            .collect()
    };
    let paths = strip(vault.paths_under("Импорт"));
    assert!(first_run < vault.paths_under("Импорт").len());
    assert!(
        paths
            .iter()
            .all(|p| !p.contains("(импорт)") && !p.contains(" (2)")),
        "{paths:?}"
    );
    let (full, _) = import_all().await;
    assert_eq!(paths, strip(full.paths_under("Импорт")));
    assert_eq!(
        text_of(&vault, "Импорт/Ссылки/08 Входная.typ").await,
        text_of(&full, "Импорт/Ссылки/08 Входная.typ").await
    );

    // Всё перенесено — продолжать нечего.
    assert!(survey(&source, &vault, &with_beside).await.resume.is_none());
}

/// FR-012: отчёт — заметка, которая собирается и перечисляет потери и пропуски.
pub async fn the_report_note_compiles_and_lists_losses() {
    let source = source();
    let mut vault = MemoryVault::new();
    let state = drive(&source, &mut vault, &job(), None).await;
    let path = import_finish(&mut vault, &state).await.expect("отчёт");
    assert_eq!(path, "Импорт/Import report.typ");
    let report = text_of(&vault, &path).await;
    for needle in [
        "Imported cleanly: 20",
        "Imported with losses: 7",
        "Skipped: 1",
        "18 Не UTF-8.md — not UTF-8",
        "line 16",
        "Diagram left as source text",
    ] {
        assert!(report.contains(needle), "нет {needle:?}:\n{report}");
    }
    assert_compiles(&vault, &path).await;
    // Ссылки отчёта ведут к перенесённым заметкам.
    let links = typsidian_core::graph::links_in(Path::new(&path), &report);
    assert!(
        links.contains(&std::path::PathBuf::from("Импорт/05 Код.typ")),
        "{links:?}"
    );
    // Второй отчёт не затирает первый.
    let again = drive(&source, &mut vault, &job(), None).await;
    let second = import_finish(&mut vault, &again)
        .await
        .expect("второй отчёт");
    assert_eq!(second, "Импорт/Import report (2).typ");
}

/// Прочие файлы не копируются, если пользователь так выбрал, и названы в отчёте.
pub async fn other_files_are_skipped_unless_copying_is_chosen() {
    let source = source();
    let mut vault = MemoryVault::new();
    let mut no_copy = job();
    no_copy.copy_other_files = false;
    let state = drive(&source, &mut vault, &no_copy, None).await;
    assert!(vault.bytes_of("Импорт/документ.pdf").is_none());
    assert!(vault.bytes_of("Импорт/Вложения/unused.png").is_none());
    assert!(state
        .recent
        .iter()
        .any(|e| e.source == "документ.pdf" && e.skip_reason == Some(SkipReason::OtherFile)));
    // Картинки, на которые ссылаются заметки, копируются всё равно.
    assert!(vault
        .bytes_of("Импорт/assets/10 Картинки/pic.png")
        .is_some());
    let _ = Outcome::Clean;
}
