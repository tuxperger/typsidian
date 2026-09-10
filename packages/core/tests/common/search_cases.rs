//! Расширенный поиск по vault — общая таблица для обоих таргетов
//! (макет «Typsidian Утилиты» § Поиск).
//!
//! Проверки живут здесь, а не по копии в каждом файле: правила совпадения
//! платформенно нейтральны, и расхождение между native и wasm было бы дефектом
//! (принцип II). Файлы `search_native.rs` / `search_wasm.rs` только вызывают
//! эти функции своим harness'ом.

#![allow(dead_code)]

use typsidian_core::search::{search, FilterMode, SearchError, SearchQuery};

use super::MemoryVault;

/// Vault из трёх заметок в двух папках плюс доска — она в выдачу попадать не
/// должна ни при каком запросе.
fn vault() -> MemoryVault {
    MemoryVault::new()
        .with_file(
            "Диссертация/Результаты.typ",
            "где $delta$ — глубина проникновения\n\
             тепловой волны в образце при частоте 3 Гц.\n\
             Измерения выполнены термопарами типа K.\n",
        )
        .with_file(
            "Диссертация/Методология.typ",
            "Для оценки теплопроводности применяется метод\n\
             затухающей тепловой волны (Ångström, 1861).\n\
             Частота модуляции подбирается по толщине.\n",
        )
        .with_file(
            "Заметки/Лекция_12.typ",
            "Уравнение ТЕПЛОВОЙ волны выводится из закона Фурье\n",
        )
        .with_file("Заметки/схема.excalidraw", "{\"elements\":[\"тепловой\"]}")
}

fn query(text: &str) -> SearchQuery {
    SearchQuery {
        text: text.to_owned(),
        ..SearchQuery::default()
    }
}

/// Совпадения находятся во всех заметках и сгруппированы по папкам.
pub async fn matches_are_grouped_by_folder() {
    let found = search(&vault(), &query("тепловой волны"))
        .await
        .expect("поиск");

    // Три: две в «Диссертации» и «ТЕПЛОВОЙ волны» в «Заметках» — регистр по
    // умолчанию не различается.
    assert_eq!(found.total_notes, 3);
    assert_eq!(found.groups.len(), 2, "две папки — две группы");
    assert_eq!(found.groups[0].folder, "Диссертация");
    assert_eq!(found.groups[0].matches, 2);
    assert_eq!(found.groups[1].folder, "Заметки");
    assert_eq!(found.total_matches, 3);
}

/// Регистр по умолчанию не различается, а с чипом «Aa» — различается.
pub async fn case_sensitivity_is_switchable() {
    let insensitive = search(&vault(), &query("тепловой")).await.expect("поиск");
    assert_eq!(
        insensitive.total_notes, 3,
        "без учёта регистра находится и «ТЕПЛОВОЙ»"
    );

    let sensitive = search(
        &vault(),
        &SearchQuery {
            case_sensitive: true,
            ..query("тепловой")
        },
    )
    .await
    .expect("поиск");
    assert_eq!(sensitive.total_notes, 2, "с учётом регистра — только две");
}

/// Строка отдаётся уже разрезанной: подсвечивать нечего вычислять.
pub async fn the_matched_line_arrives_split_into_segments() {
    let found = search(&vault(), &query("тепловой волны"))
        .await
        .expect("поиск");
    let hit = &found.groups[0].hits[0];

    let line = hit
        .lines
        .iter()
        .find(|line| line.is_match)
        .expect("совпавшая строка");
    let highlighted: Vec<&str> = line
        .segments
        .iter()
        .filter(|part| part.highlighted)
        .map(|part| part.text.as_str())
        .collect();
    assert_eq!(highlighted, vec!["тепловой волны"]);

    // Заметки идут в порядке пути, поэтому первой в группе стоит Методология.
    let whole: String = line.segments.iter().map(|part| part.text.as_str()).collect();
    assert_eq!(
        whole, "затухающей тепловой волны (Ångström, 1861).",
        "куски обязаны складываться обратно в исходную строку"
    );
}

/// Соседние строки приходят для контекста и помечены как несовпавшие.
pub async fn context_lines_come_with_the_match() {
    let found = search(&vault(), &query("тепловой волны"))
        .await
        .expect("поиск");
    let hit = &found.groups[0].hits[0];

    assert!(
        hit.lines.len() > 1,
        "одна строка без контекста читается хуже: {:?}",
        hit.lines
    );
    assert!(hit.lines.iter().any(|line| !line.is_match));
    assert_eq!(hit.first_line, 2, "совпадение во второй строке файла");
}

/// Доска — не текст: её содержимое в выдачу не попадает.
pub async fn boards_are_not_searched() {
    let found = search(&vault(), &query("тепловой")).await.expect("поиск");
    assert!(
        !found
            .groups
            .iter()
            .flat_map(|group| &group.hits)
            .any(|hit| hit.path.ends_with(".excalidraw")),
        "содержимое доски — непрозрачные байты, совпадение в нём бессмысленно"
    );
}

/// Пустой запрос — пустая выдача, а не весь vault.
pub async fn an_empty_query_finds_nothing() {
    let found = search(&vault(), &query("   ")).await.expect("поиск");
    assert_eq!(found.total_notes, 0);
    assert!(found.groups.is_empty());
}

/// Regex включается чипом и ищет по выражению.
pub async fn regex_mode_matches_by_pattern() {
    let found = search(
        &vault(),
        &SearchQuery {
            regex: true,
            ..query(r"частот\w+")
        },
    )
    .await
    .expect("поиск");

    assert_eq!(found.total_notes, 2, "«частоте» и «Частота»");
}

/// Без чипа regex выражение ищется как обычный текст.
pub async fn without_the_regex_chip_the_pattern_is_literal() {
    let found = search(&vault(), &query(r"частот\w+")).await.expect("поиск");
    assert_eq!(found.total_notes, 0, "такой подстроки в vault нет");
}

/// Неверное выражение обязано быть названо, а не молча ничего не найти.
pub async fn a_broken_pattern_is_reported_with_its_reason() {
    let error = search(
        &vault(),
        &SearchQuery {
            regex: true,
            ..query("(незакрытая")
        },
    )
    .await
    .expect_err("выражение неверно");

    let SearchError::BadPattern { message } = error;
    assert!(!message.is_empty(), "причина обязана дойти до пользователя");
}

/// Фильтр «ссылается на …» и строка складываются по «И».
pub async fn the_links_to_filter_narrows_the_text_query() {
    // Ссылка от корня: `[[Методология]]` из «Заметок» указывала бы на соседа
    // по папке — так её разрешает `graph::resolve_link_target`.
    let vault = vault().with_file(
        "Заметки/Идеи.typ",
        "Проверить тепловой поток, см. [[/Диссертация/Методология]]\n",
    );

    let both = search(
        &vault,
        &SearchQuery {
            links_to: Some("Диссертация/Методология.typ".to_owned()),
            ..query("тепловой")
        },
    )
    .await
    .expect("поиск");

    assert_eq!(
        both.total_notes, 1,
        "обоим условиям отвечает только Идеи.typ: {:?}",
        both.groups
    );
    assert!(both.groups[0].hits[0].path.ends_with("Идеи.typ"));
}

/// В режиме «ИЛИ» достаточно одного условия.
pub async fn the_or_mode_widens_the_result() {
    let vault = vault().with_file(
        "Заметки/Идеи.typ",
        "Совсем про другое, см. [[/Диссертация/Методология]]\n",
    );

    let found = search(
        &vault,
        &SearchQuery {
            links_to: Some("Диссертация/Методология.typ".to_owned()),
            mode: FilterMode::Or,
            ..query("тепловой волны")
        },
    )
    .await
    .expect("поиск");

    assert_eq!(
        found.total_notes, 4,
        "три по тексту плюс одна по ссылке: {:?}",
        found.groups
    );
}
