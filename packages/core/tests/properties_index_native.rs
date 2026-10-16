//! cargo test — только native: индекс свойств в `NativeBackend` и его SQLite-кэш
//! (фича 020, T006, T016). Общие правила отбора проверяет `properties_native.rs`.
#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::path::Path;

use tempfile::TempDir;
use typsidian_core::properties::{self, select_notes, Condition, PropertyValue, Selection};
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::native::NativeBackend;

const TAGGED: &str = "#metadata((tags: (\"проект\",), статус: \"черновик\")) <properties>\nТекст\n";

fn by_tag(tag: &str) -> Selection {
    Selection {
        conditions: vec![Condition::TagHas { tag: tag.into() }],
        ..Selection::default()
    }
}

fn paths(result: &properties::SelectionResult) -> Vec<String> {
    result.rows.iter().map(|row| row.path.clone()).collect()
}

fn vault_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("Проекты")).unwrap();
    fs::write(dir.path().join("Проекты/а.typ"), TAGGED).unwrap();
    fs::write(dir.path().join("б.typ"), "Без свойств").unwrap();
    dir
}

/// Холодное открытие наполняет индекс; запись заметки обновляет его в том же
/// шаге; удаление и переименование уносят свойства (FR-009).
#[tokio::test]
async fn the_index_is_filled_on_open_and_follows_writes() {
    let dir = vault_dir();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();

    assert_eq!(
        paths(&select_notes(&backend, &by_tag("проект")).await),
        ["Проекты/а.typ"]
    );

    backend
        .write(Path::new("б.typ"), TAGGED, None)
        .await
        .unwrap();
    assert_eq!(
        paths(&select_notes(&backend, &by_tag("проект")).await),
        ["Проекты/а.typ", "б.typ"]
    );

    backend
        .write(Path::new("б.typ"), "Тег убрали", None)
        .await
        .unwrap();
    assert_eq!(
        paths(&select_notes(&backend, &by_tag("проект")).await),
        ["Проекты/а.typ"]
    );

    backend
        .rename(Path::new("Проекты/а.typ"), Path::new("Проекты/новое.typ"))
        .await
        .unwrap();
    assert_eq!(
        paths(&select_notes(&backend, &by_tag("проект")).await),
        ["Проекты/новое.typ"]
    );

    backend
        .delete(Path::new("Проекты/новое.typ"))
        .await
        .unwrap();
    assert!(select_notes(&backend, &by_tag("проект"))
        .await
        .rows
        .is_empty());

    let keys = properties::property_keys(&backend).await;
    assert!(keys.is_empty(), "{keys:?}");
}

/// Индекс переживает закрытие: второе открытие берёт его из кэша.
#[tokio::test]
async fn the_index_survives_reopening() {
    let dir = vault_dir();
    drop(NativeBackend::open(dir.path()).await.unwrap());

    let backend = NativeBackend::open(dir.path()).await.unwrap();
    assert!(backend.properties_current().await);
    let result = select_notes(&backend, &by_tag("проект")).await;
    assert_eq!(paths(&result), ["Проекты/а.typ"]);

    let notes = backend.indexed_notes().await;
    let tagged = notes.iter().find(|n| n.path == "Проекты/а.typ").unwrap();
    assert_eq!(tagged.properties[1].value, PropertyValue::text("черновик"));
    assert!(
        tagged.modified_ms.is_some(),
        "на native время изменения известно"
    );
}

/// Кэш прежней версии (без таблицы свойств и отметки формата) не годится для
/// быстрого пути: открытие строит свойства заново (research.md §5).
#[tokio::test]
async fn an_old_cache_is_rebuilt_with_properties() {
    let dir = vault_dir();
    // Кэш времён до фичи 020: только заметки и ссылки.
    let service = dir.path().join(".typsidian");
    fs::create_dir_all(&service).unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        service.join("cache.sqlite").display()
    );
    {
        use sqlx::Connection;
        let mut connection = sqlx::SqliteConnection::connect(&url).await.unwrap();
        for statement in include_str!("../migrations/0001_vault_cache.sql").split(';') {
            if !statement.trim().is_empty() {
                sqlx::query(statement.trim())
                    .execute(&mut connection)
                    .await
                    .unwrap();
            }
        }
        // Заметка в кэше есть и совпадает с диском — быстрый путь не нашёл бы
        // в ней свойств.
        sqlx::query(
            "INSERT INTO notes (path, name, mtime_millis, size, content_hash) VALUES (?, ?, 0, 0, ?)",
        )
        .bind("Проекты/а.typ")
        .bind("а.typ")
        .bind(vec![0u8; 32])
        .execute(&mut connection)
        .await
        .unwrap();
    }

    let backend = NativeBackend::open(dir.path()).await.unwrap();
    assert_eq!(
        paths(&select_notes(&backend, &by_tag("проект")).await),
        ["Проекты/а.typ"]
    );
    assert!(backend.properties_current().await);
}

/// Пакетный пересчёт восстанавливает свойства, потерянные в кэше, и ставит
/// отметку актуальности.
#[tokio::test]
async fn reindex_restores_properties_in_the_cache() {
    let dir = vault_dir();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    backend
        .set_indexed_properties(Path::new("Проекты/а.typ"), Vec::new())
        .await;
    assert!(select_notes(&backend, &by_tag("проект"))
        .await
        .rows
        .is_empty());

    struct Quiet;
    impl typsidian_core::vault::relocate::MoveControl for Quiet {
        fn progress(&self, _done: u32, _total: u32) {}
        fn cancelled(&self) -> bool {
            false
        }
    }
    let outcome = properties::reindex_properties(&mut backend, &Quiet).await;
    assert!(!outcome.cancelled);
    assert_eq!(outcome.total, 2);
    assert_eq!(
        paths(&select_notes(&backend, &by_tag("проект")).await),
        ["Проекты/а.typ"]
    );
}

/// Блок выборки в заметке собирается по индексу native: заметка с блоком видит
/// другую заметку с тегом.
#[tokio::test]
async fn a_block_builds_over_the_native_index() {
    use typsidian_core::preview::{compile_note_resolved, CompileOutcome, FontSet, PackageCache};
    let dir = vault_dir();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    properties::ensure_select_library(&mut backend)
        .await
        .unwrap();

    let source =
        "#import \"/.typsidian/lib/select.typ\": select\n#select(where: ((tag: \"проект\"),))\n";
    let document = compile_note_resolved(
        &backend,
        &PackageCache::new(),
        "обзор.typ",
        source,
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap();
    assert_eq!(
        document.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        document.diagnostics
    );
    let shown: String = document
        .pages
        .iter()
        .flat_map(|page| page.text_runs.iter().map(|run| run.text.clone()))
        .collect();
    assert!(shown.contains('а'), "{shown}");
}
