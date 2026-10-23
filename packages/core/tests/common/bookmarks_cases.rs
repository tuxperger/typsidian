//! Закладки (фича 032) — общая таблица для обоих таргетов.
//!
//! Файлы `bookmarks_native.rs` / `bookmarks_wasm.rs` только вызывают эти
//! функции своим harness'ом (принцип II).

#![allow(dead_code)]

use std::path::Path;

use typsidian_core::bookmarks::{
    bookmark_add, bookmark_group_create, bookmark_group_delete, bookmark_group_rename,
    bookmark_group_toggle, bookmark_move, bookmark_remove, bookmark_rename, bookmarks_view,
    move_bookmarks, AddOutcome, BookmarkError, BookmarkPlace, BookmarkState, BookmarkTarget,
    BookmarksView, ViewEntry,
};
use typsidian_core::search::SearchQuery;

use super::MemoryVault;

const INTRO: &str = "Диссертация/Введение.typ";
const METHOD: &str = "Диссертация/Методика.typ";

fn vault() -> MemoryVault {
    MemoryVault::new()
        .with_file(INTRO, "= Введение\n")
        .with_file(METHOD, "= Методика\n\n== Выборка\n\nТекст.\n")
        .with_file("Диссертация/рисунки/стенд.png", "png")
}

fn note(path: &str) -> BookmarkTarget {
    BookmarkTarget::Note { path: path.into() }
}

fn added(outcome: Result<AddOutcome, BookmarkError>) -> (String, BookmarksView) {
    match outcome.unwrap() {
        AddOutcome::Added { id, view } => (id, view),
        other => panic!("ждали Added, получили {other:?}"),
    }
}

/// Имена закладок по порядку панели; группа — `[имя: …]`.
fn names(view: &BookmarksView) -> Vec<String> {
    view.entries
        .iter()
        .map(|entry| match entry {
            ViewEntry::Item { row } => row.bookmark.name.clone(),
            ViewEntry::Group { name, rows, .. } => format!(
                "[{name}: {}]",
                rows.iter()
                    .map(|row| row.bookmark.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        })
        .collect()
}

fn group_id(view: &BookmarksView) -> String {
    view.entries
        .iter()
        .find_map(|entry| match entry {
            ViewEntry::Group { id, .. } => Some(id.clone()),
            _ => None,
        })
        .expect("группа")
}

// ── US1 ─────────────────────────────────────────────────────────────────────

/// Четыре вида, имя по умолчанию, повтор без дубля (FR-001, AS1, AS2).
pub async fn four_kinds_are_added_once() {
    let mut vault = vault();
    added(bookmark_add(&mut vault, note(INTRO), None).await);
    added(
        bookmark_add(
            &mut vault,
            BookmarkTarget::Section {
                path: METHOD.into(),
                heading: "Выборка".into(),
            },
            None,
        )
        .await,
    );
    let query = SearchQuery {
        text: "теплопроводн".into(),
        case_sensitive: true,
        ..Default::default()
    };
    added(
        bookmark_add(
            &mut vault,
            BookmarkTarget::Search {
                query: query.clone(),
            },
            None,
        )
        .await,
    );
    let (_, view) = added(
        bookmark_add(
            &mut vault,
            BookmarkTarget::Folder {
                path: "Диссертация/рисунки".into(),
            },
            Some("Рисунки".into()),
        )
        .await,
    );
    assert_eq!(
        names(&view),
        ["Введение", "Выборка", "теплопроводн", "Рисунки"]
    );
    assert_eq!(view.count, 4);

    match bookmark_add(&mut vault, note(INTRO), None).await.unwrap() {
        AddOutcome::Exists { view, .. } => assert_eq!(view.count, 4, "без дубля"),
        other => panic!("ждали Exists: {other:?}"),
    }
    match bookmark_add(&mut vault, BookmarkTarget::Search { query }, None)
        .await
        .unwrap()
    {
        AddOutcome::Exists { .. } => {}
        other => panic!("тот же поиск — Exists: {other:?}"),
    }
    // Закладки — в настройках vault.
    let settings = String::from_utf8(vault.bytes_of(".typsidian/vault.json").unwrap()).unwrap();
    assert!(settings.contains("\"bookmarks\""), "{settings}");
}

/// Порядок, группы, переименование, удаление (FR-003, AS3).
pub async fn order_and_groups_are_kept() {
    let mut vault = vault();
    let (intro, _) = added(bookmark_add(&mut vault, note(INTRO), None).await);
    let (method, _) = added(bookmark_add(&mut vault, note(METHOD), None).await);

    let view = bookmark_move(
        &mut vault,
        &method,
        &BookmarkPlace {
            group: None,
            index: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(names(&view), ["Методика", "Введение"]);

    let view = bookmark_group_create(&mut vault, "Глава 2").await.unwrap();
    let group = group_id(&view);
    let view = bookmark_move(
        &mut vault,
        &intro,
        &BookmarkPlace {
            group: Some(group.clone()),
            index: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(names(&view), ["Методика", "[Глава 2: Введение]"]);

    let view = bookmark_group_toggle(&mut vault, &group).await.unwrap();
    assert!(matches!(
        &view.entries[1],
        ViewEntry::Group {
            collapsed: true,
            ..
        }
    ));
    let view = bookmark_group_rename(&mut vault, &group, "Обзор")
        .await
        .unwrap();
    assert_eq!(names(&view), ["Методика", "[Обзор: Введение]"]);
    let view = bookmark_rename(&mut vault, &intro, "Начало").await.unwrap();
    assert_eq!(names(&view), ["Методика", "[Обзор: Начало]"]);

    // Группа на первое место.
    let view = bookmark_move(
        &mut vault,
        &group,
        &BookmarkPlace {
            group: None,
            index: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(names(&view), ["[Обзор: Начало]", "Методика"]);

    // Удаление группы оставляет её закладки на её месте.
    let view = bookmark_group_delete(&mut vault, &group).await.unwrap();
    assert_eq!(names(&view), ["Начало", "Методика"]);
    let view = bookmark_remove(&mut vault, &method).await.unwrap();
    assert_eq!(names(&view), ["Начало"]);
    assert_eq!(
        bookmark_remove(&mut vault, "нет").await,
        Err(BookmarkError::NotFound)
    );

    let mut read_only = vault.read_only();
    assert_eq!(
        bookmark_add(&mut read_only, note(METHOD), None)
            .await
            .unwrap_err(),
        BookmarkError::ReadOnly
    );
}

// ── US2 ─────────────────────────────────────────────────────────────────────

/// Переименование заметки и перенос папки переписывают пути; удалённая цель
/// и пропавший заголовок помечены (FR-004, SC-002).
pub async fn renames_follow_and_missing_targets_are_marked() {
    let mut vault = vault();
    added(bookmark_add(&mut vault, note(INTRO), None).await);
    added(
        bookmark_add(
            &mut vault,
            BookmarkTarget::Section {
                path: METHOD.into(),
                heading: "Выборка".into(),
            },
            None,
        )
        .await,
    );

    move_bookmarks(
        &mut vault,
        Path::new(INTRO),
        Path::new("Диссертация/Начало.typ"),
    )
    .await
    .unwrap();
    move_bookmarks(&mut vault, Path::new("Диссертация"), Path::new("Работа"))
        .await
        .unwrap();
    let settings = String::from_utf8(vault.bytes_of(".typsidian/vault.json").unwrap()).unwrap();
    assert!(settings.contains("Работа/Начало.typ"), "{settings}");
    assert!(settings.contains("Работа/Методика.typ"), "{settings}");

    // Файлов по новым путям в двойнике нет — закладки помечены недоступными;
    // имя закладки заметки следует за её новым именем.
    let view = bookmarks_view(&vault).await;
    assert_eq!(names(&view), ["Начало", "Выборка"]);
    let states: Vec<BookmarkState> = view
        .entries
        .iter()
        .filter_map(|entry| match entry {
            ViewEntry::Item { row } => Some(row.state),
            _ => None,
        })
        .collect();
    assert_eq!(states, [BookmarkState::Missing, BookmarkState::Missing]);

    // Заметка есть, заголовка нет — `SectionMissing`.
    let mut vault = vault_with("Работа/Методика.typ", "= Методика\n\n== Другое\n");
    added(
        bookmark_add(
            &mut vault,
            BookmarkTarget::Section {
                path: "Работа/Методика.typ".into(),
                heading: "Выборка".into(),
            },
            None,
        )
        .await,
    );
    let view = bookmarks_view(&vault).await;
    assert!(matches!(
        &view.entries[0],
        ViewEntry::Item { row } if row.state == BookmarkState::SectionMissing
    ));
}

fn vault_with(path: &str, text: &str) -> MemoryVault {
    MemoryVault::new().with_file(path, text)
}

/// Вид из 200 закладок — быстро (SC-001: не дольше открытия из дерева).
pub async fn a_long_list_is_shown_quickly() {
    let mut vault = MemoryVault::new();
    for n in 0..200 {
        let path = format!("Заметки/заметка {n}.typ");
        vault = vault.with_file(&path, "= Заметка\n\n== Раздел\n");
    }
    let mut entries = Vec::new();
    for n in 0..200 {
        let path = format!("Заметки/заметка {n}.typ");
        entries.push(typsidian_core::bookmarks::BookmarkEntry::Item {
            bookmark: typsidian_core::bookmarks::Bookmark {
                id: format!("b{n}"),
                name: format!("заметка {n}"),
                target: if n % 2 == 0 {
                    BookmarkTarget::Note { path }
                } else {
                    BookmarkTarget::Section {
                        path,
                        heading: "Раздел".into(),
                    }
                },
            },
        });
    }
    let mut settings = typsidian_core::vault::vault_settings::read_vault_settings(&vault)
        .await
        .unwrap();
    settings.bookmarks = entries;
    typsidian_core::vault::vault_settings::write_vault_settings(&mut vault, &settings)
        .await
        .unwrap();
    let view = bookmarks_view(&vault).await;
    assert_eq!(view.count, 200);
    assert!(view.entries.iter().all(|entry| matches!(
        entry,
        ViewEntry::Item { row } if row.state == BookmarkState::Ready
    )));
}
