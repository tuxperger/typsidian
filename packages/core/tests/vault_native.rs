//! cargo test — только native (принцип II). Тесты организованы по user story
//! (см. tasks.md): US1 в начале файла, остальные добавляются по мере реализации
//! соответствующих фаз.
#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;
use typsidian_core::vault::backend::{
    CreateError, DeleteError, OpenError, ReadError, RenameError, VaultBackend, WriteError,
};
use typsidian_core::vault::conflict::{ConflictStatus, DiffLine};
use typsidian_core::vault::events::VaultEvent;
use typsidian_core::vault::index::{FileStamp, Index, Note};
use typsidian_core::vault::native::NativeBackend;
use typsidian_core::vault::{AccessState, NOTE_SIZE_LIMIT_BYTES, SUPPORTED_NOTE_COUNT};

fn write_note(dir: &Path, rel_path: &str, content: &str) {
    let full = dir.join(rel_path);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(full, content).unwrap();
}

/// Собирает `poll_events()` циклически в течение `duration` — общий помощник
/// для тестов Phase 5 (US3), где обнаружение внешних изменений асинхронно
/// (дебаунсер/фоновая сверка) и не имеет единого момента "готово".
async fn drain_events_for(backend: &mut NativeBackend, duration: Duration) -> Vec<VaultEvent> {
    let deadline = tokio::time::Instant::now() + duration;
    let mut collected = Vec::new();
    while tokio::time::Instant::now() < deadline {
        collected.extend(backend.poll_events());
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    collected.extend(backend.poll_events());
    collected
}

// ---------------------------------------------------------------------------
// US1 — Открыть папку как vault (T018, T020, T021)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn open_recursively_indexes_nested_notes_and_excludes_hidden_dirs() {
    // FR-002, FR-003, FR-004
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "root.typ", "корневая заметка");
    write_note(dir.path(), "sub/nested.typ", "вложенная заметка");
    write_note(dir.path(), "sub/deep/deeper.typ", "глубокая заметка");
    write_note(dir.path(), ".git/config.typ", "не должно попасть в vault");
    write_note(dir.path(), "attachments/image.png", "не заметка, не .typ");

    let backend = NativeBackend::open(dir.path())
        .await
        .expect("open should succeed");
    let notes = backend.list().await;

    let paths: Vec<String> = notes
        .iter()
        .map(|n| n.path.to_string_lossy().replace('\\', "/"))
        .collect();

    assert!(paths.contains(&"root.typ".to_string()));
    assert!(paths.contains(&"sub/nested.typ".to_string()));
    assert!(paths.contains(&"sub/deep/deeper.typ".to_string()));
    assert!(!paths.iter().any(|p| p.contains(".git")));
    assert!(!paths.iter().any(|p| p.ends_with(".png")));
    assert_eq!(notes.len(), 3);
}

#[tokio::test]
async fn open_on_empty_folder_succeeds_with_empty_tree() {
    // US1 AS3
    let dir = TempDir::new().unwrap();
    let backend = NativeBackend::open(dir.path())
        .await
        .expect("open should succeed on empty folder");
    assert!(backend.list().await.is_empty());
}

#[tokio::test]
async fn open_on_missing_folder_returns_clear_error() {
    // FR-008
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("does-not-exist");
    let result = NativeBackend::open(&missing).await;
    assert_eq!(result.err(), Some(OpenError::NotFound));
}

#[tokio::test]
async fn open_on_file_instead_of_directory_returns_clear_error() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("not-a-folder.typ");
    fs::write(&file_path, "").unwrap();
    let result = NativeBackend::open(&file_path).await;
    assert_eq!(result.err(), Some(OpenError::NotADirectory));
}

#[tokio::test]
async fn backend_remains_usable_after_open_error() {
    // FR-008: приложение остаётся работоспособным после отказа
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("missing");
    assert!(NativeBackend::open(&missing).await.is_err());

    // Повторная попытка на валидной папке должна работать как ни в чём не бывало.
    write_note(dir.path(), "note.typ", "содержимое");
    let backend = NativeBackend::open(dir.path())
        .await
        .expect("second open should succeed");
    assert_eq!(backend.list().await.len(), 1);
}

#[tokio::test]
async fn read_returns_note_content() {
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "note.typ", "привет, мир");
    let backend = NativeBackend::open(dir.path()).await.unwrap();
    let content = backend.read(Path::new("note.typ")).await.unwrap();
    assert_eq!(content, "привет, мир");
}

#[tokio::test]
async fn reopen_uses_cache_and_reconciles_in_background() {
    // SC-003 + FR-018: повторное открытие отдаёт кэш немедленно, а расхождения
    // (файл добавлен, пока vault был закрыт) обнаруживаются фоновой сверкой.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "first.typ", "первая заметка");
    {
        let backend = NativeBackend::open(dir.path()).await.unwrap();
        assert_eq!(backend.list().await.len(), 1);
        // Дать фоновой sqlx-записи кэша время завершиться перед закрытием.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    // Файл добавлен, пока backend был закрыт (аналог FR-018).
    write_note(dir.path(), "second.typ", "вторая заметка");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    // Кэш уже содержит первую заметку — она видна сразу.
    assert!(backend
        .list()
        .await
        .iter()
        .any(|n| n.path == Path::new("first.typ")));

    // Фоновая сверка обнаруживает новый файл и публикует событие.
    let mut found_new_note_event = false;
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        if backend.poll_events().iter().any(
            |e| matches!(e, typsidian_core::vault::events::VaultEvent::NoteCreated { path } if path == Path::new("second.typ")),
        ) {
            found_new_note_event = true;
            break;
        }
    }
    assert!(
        found_new_note_event,
        "реконсиляция должна обнаружить second.typ"
    );
}

// ---------------------------------------------------------------------------
// US2 — Управлять заметками из интерфейса (T033, T034, T036, T037, T039, T041)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_rejects_name_conflict_without_overwriting_existing_file() {
    // T033, FR-012
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "existing.typ", "исходное содержимое");
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();

    let result = backend.create(Path::new("existing.typ")).await;

    assert_eq!(result.err(), Some(CreateError::NameConflict));
    let content = fs::read_to_string(dir.path().join("existing.typ")).unwrap();
    assert_eq!(
        content, "исходное содержимое",
        "существующий файл не должен быть перезаписан"
    );
    // Индекс тоже не должен «забыть» исходную заметку из-за отклонённой попытки.
    assert!(backend
        .list()
        .await
        .iter()
        .any(|n| n.path == Path::new("existing.typ")));
}

#[tokio::test]
async fn create_rejects_invalid_filesystem_names_before_touching_disk() {
    // T034, FR-013: '/', '\\' и NUL-байт запрещены в конечном компоненте имени;
    // ни одна из попыток не должна создать файл на диске.
    let dir = TempDir::new().unwrap();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();

    for invalid in ["evil\0name.typ", "evil\\name.typ"] {
        let result = backend.create(Path::new(invalid)).await;
        assert_eq!(
            result.err(),
            Some(CreateError::InvalidName),
            "имя {invalid:?} должно быть отклонено"
        );
    }

    // `open()` сам заводит служебное место `.typsidian/` (фича 006, FR-020) —
    // исключаем его из проверки, важно лишь, что ни одна из отклонённых
    // операций create() ничего не добавила на диск.
    let entries: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name != ".typsidian")
        .collect();
    assert!(
        entries.is_empty(),
        "недопустимые имена не должны затрагивать диск, найдено: {entries:?}"
    );
}

#[tokio::test]
async fn create_rejects_blank_name_as_invalid_not_as_conflict() {
    // T034, FR-013: пустое имя должно сообщать «имя недопустимо», а не
    // случайно совпадать с уже существующим `root_path` и выглядеть как
    // «конфликт имён» — это реальный дефект, исправленный порядком проверок
    // в `NativeBackend::create` (валидация имени предшествует `exists()`).
    let dir = TempDir::new().unwrap();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();

    let result = backend.create(Path::new("")).await;

    assert_eq!(result.err(), Some(CreateError::InvalidName));
}

#[tokio::test]
async fn rename_rejects_invalid_filesystem_names_before_touching_disk() {
    // T034, FR-013
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "note.typ", "содержимое");
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();

    let result = backend
        .rename(Path::new("note.typ"), Path::new("evil\0name.typ"))
        .await;

    assert_eq!(result.err(), Some(RenameError::InvalidName));
    // Оригинал остаётся нетронутым — ни переименования, ни удаления не произошло.
    let content = fs::read_to_string(dir.path().join("note.typ")).unwrap();
    assert_eq!(content, "содержимое");
    assert!(backend
        .list()
        .await
        .iter()
        .any(|n| n.path == Path::new("note.typ")));
}

#[tokio::test]
async fn write_to_missing_parent_directory_reports_io_error_and_keeps_index_consistent() {
    // T036, FR-014: смоделированная неудавшаяся запись. Симулируем через
    // отсутствующую родительскую папку (запись гарантированно провалится
    // независимо от прав пользователя, включая root) — в отличие от
    // `chmod`-подхода, который root молча обходит и делает тест
    // недетерминированным в контейнерных/CI-окружениях, запускающих процесс
    // от имени root.
    let dir = TempDir::new().unwrap();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    let missing_path = Path::new("no-such-folder/note.typ");

    let result = backend.write(missing_path, "содержимое", None).await;

    assert_eq!(
        result.err(),
        Some(WriteError::Io),
        "должна быть сообщена причина отказа"
    );
    assert!(
        !dir.path().join(missing_path).exists(),
        "диск не должен получить файл, если запись провалилась"
    );
    assert!(
        backend.list().await.iter().all(|n| n.path != missing_path),
        "индекс не должен содержать фантомную запись о неудавшейся записи"
    );
}

#[tokio::test]
async fn write_failure_does_not_corrupt_index_of_preexisting_note() {
    // T036, FR-014, дополнение: неудавшаяся запись по уже известному индексу
    // пути не должна менять сохранённый стамп заметки — дерево остаётся
    // согласованным с фактическим (неизменившимся) состоянием диска.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "note.typ", "исходное содержимое");
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    let before = backend
        .list()
        .await
        .into_iter()
        .find(|n| n.path == Path::new("note.typ"))
        .unwrap();

    // Подменяем файл на диске директорией с тем же именем — запись
    // неизбежно провалится (EISDIR) независимо от прав пользователя.
    fs::remove_file(dir.path().join("note.typ")).unwrap();
    fs::create_dir(dir.path().join("note.typ")).unwrap();

    let result = backend
        .write(Path::new("note.typ"), "новое содержимое", None)
        .await;
    assert_eq!(result.err(), Some(WriteError::Io));

    let after = backend
        .list()
        .await
        .into_iter()
        .find(|n| n.path == Path::new("note.typ"))
        .unwrap();
    assert_eq!(
        before.stamp, after.stamp,
        "индекс не должен обновляться при неудачной записи"
    );
}

#[tokio::test]
async fn create_rename_delete_round_trip_persists_across_reopen() {
    // T037, T039, T041: create/rename/delete на native согласованно обновляют
    // и диск, и in-memory индекс, и персистентный SQLite-кэш — переоткрытие
    // backend (который читает состояние из кэша, T026) видит переименование,
    // а не исходное имя, что доказывает, что кэш обновлялся синхронно, а не
    // только in-memory индекс.
    let dir = TempDir::new().unwrap();
    {
        let mut backend = NativeBackend::open(dir.path()).await.unwrap();

        backend.create(Path::new("draft.typ")).await.unwrap();
        assert!(dir.path().join("draft.typ").exists());
        assert!(backend
            .list()
            .await
            .iter()
            .any(|n| n.path == Path::new("draft.typ")));

        backend
            .rename(Path::new("draft.typ"), Path::new("final.typ"))
            .await
            .unwrap();
        assert!(!dir.path().join("draft.typ").exists());
        assert!(dir.path().join("final.typ").exists());
        let notes = backend.list().await;
        assert!(!notes.iter().any(|n| n.path == Path::new("draft.typ")));
        assert!(notes.iter().any(|n| n.path == Path::new("final.typ")));

        // Дать фоновой sqlx-записи кэша время завершиться перед закрытием
        // (тот же паттерн, что и в reopen_uses_cache_and_reconciles_in_background).
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    // Кэш пережил закрытие backend: немедленный (без ожидания фоновой сверки)
    // список уже отражает переименованный файл.
    let notes = backend.list().await;
    assert!(notes.iter().any(|n| n.path == Path::new("final.typ")));
    assert!(!notes.iter().any(|n| n.path == Path::new("draft.typ")));

    backend.delete(Path::new("final.typ")).await.unwrap();
    assert!(!dir.path().join("final.typ").exists());
    assert!(backend.list().await.is_empty());
}

#[test]
fn index_rename_preserves_note_identity_for_editor_session_continuity() {
    // T040, FR-015: `Index` — код, общий для native и web (без cfg-разделения
    // внутри index.rs), поэтому этот тест доказывает поведение на обоих
    // таргетах одним прогоном. Переименование обязано сохранить состояние
    // заметки (в частности, `has_unsaved_edits` — признак открытой в редакторе
    // сессии с несохранёнными правками) и лишь обновить путь/имя, а не
    // создать «новую» заметку с сброшенным состоянием — иначе открытый
    // редактор потерял бы связь со своей заметкой при переименовании.
    let mut index = Index::new();
    let stamp = FileStamp::from_content(1000, "содержимое".as_bytes());
    let mut note = Note::new("draft.typ".into(), stamp.clone());
    note.has_unsaved_edits = true;
    index.insert(note);

    let renamed = index
        .rename(Path::new("draft.typ"), Path::new("final.typ"))
        .expect("rename должен найти существующую заметку");

    assert_eq!(renamed.path, Path::new("final.typ"));
    assert_eq!(renamed.name, "final.typ");
    assert!(
        renamed.has_unsaved_edits,
        "состояние несохранённых правок должно пережить переименование"
    );
    assert_eq!(
        renamed.stamp, stamp,
        "стамп заметки не должен сбрасываться при переименовании"
    );
    assert!(index.get(Path::new("draft.typ")).is_none());
    assert_eq!(
        index
            .get(Path::new("final.typ"))
            .map(|n| n.has_unsaved_edits),
        Some(true)
    );
}

// ---------------------------------------------------------------------------
// US3 — Подхватывать внешние изменения без перезапуска
// (T045, T046, T048, T049, T050, T051, T052)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn watch_emits_created_updated_renamed_deleted_for_external_changes() {
    // T045, FR-016: create/update/rename/delete выполнены "снаружи" —
    // напрямую через std::fs, а не через VaultBackend — и должны быть
    // обнаружены watch()-пайплайном без перезапуска.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "to_update.typ", "исходное содержимое");
    write_note(dir.path(), "to_rename.typ", "будет переименована");
    write_note(dir.path(), "to_delete.typ", "будет удалена");
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    // Дать стартовой фоновой сверке устояться, чтобы не путать её события
    // (все три файла уже видны при открытии, поэтому сверка не должна
    // породить ничего нового) с теми, что появятся ниже.
    tokio::time::sleep(Duration::from_millis(150)).await;
    backend.poll_events();

    write_note(dir.path(), "created.typ", "новая заметка снаружи");
    write_note(dir.path(), "to_update.typ", "изменённое содержимое");
    fs::rename(
        dir.path().join("to_rename.typ"),
        dir.path().join("renamed.typ"),
    )
    .unwrap();
    fs::remove_file(dir.path().join("to_delete.typ")).unwrap();

    let events = drain_events_for(&mut backend, Duration::from_secs(5)).await;

    assert!(
        events.iter().any(
            |e| matches!(e, VaultEvent::NoteCreated { path } if path == Path::new("created.typ"))
        ),
        "ожидался NoteCreated для created.typ: {events:?}"
    );
    assert!(
        events.iter().any(
            |e| matches!(e, VaultEvent::NoteUpdated { path } if path == Path::new("to_update.typ"))
        ),
        "ожидался NoteUpdated для to_update.typ: {events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            VaultEvent::NoteRenamed { from_path, to_path }
                if from_path == Path::new("to_rename.typ") && to_path == Path::new("renamed.typ")
        )),
        "ожидался NoteRenamed to_rename.typ -> renamed.typ: {events:?}"
    );
    assert!(
        events.iter().any(
            |e| matches!(e, VaultEvent::NoteDeleted { path } if path == Path::new("to_delete.typ"))
        ),
        "ожидался NoteDeleted для to_delete.typ: {events:?}"
    );

    // Индекс (не только поток событий) должен сойтись к состоянию диска.
    let paths: Vec<PathBuf> = backend.list().await.into_iter().map(|n| n.path).collect();
    assert!(paths.contains(&PathBuf::from("created.typ")));
    assert!(paths.contains(&PathBuf::from("renamed.typ")));
    assert!(!paths.contains(&PathBuf::from("to_rename.typ")));
    assert!(!paths.contains(&PathBuf::from("to_delete.typ")));
}

#[tokio::test]
async fn debounce_collapses_atomic_write_via_tmp_and_rename_into_one_event() {
    // T046, Edge Case: внешняя программа пишет через временный файл, затем
    // переименовывает его в целевое имя — не должно давать наблюдаемую
    // вспышку "заметка временно удалена/отсутствует".
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "atomic.typ", "исходное содержимое");
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    backend.poll_events();

    let tmp = dir.path().join("atomic.typ.tmp");
    fs::write(&tmp, "новое содержимое").unwrap();
    fs::rename(&tmp, dir.path().join("atomic.typ")).unwrap();

    let events = drain_events_for(&mut backend, Duration::from_secs(5)).await;

    let delete_count = events
        .iter()
        .filter(
            |e| matches!(e, VaultEvent::NoteDeleted { path } if path == Path::new("atomic.typ")),
        )
        .count();
    assert_eq!(
        delete_count, 0,
        "заметка не должна казаться временно удалённой: {events:?}"
    );

    let settle_count = events
        .iter()
        .filter(|e| {
            matches!(e, VaultEvent::NoteUpdated { path } if path == Path::new("atomic.typ"))
                || matches!(e, VaultEvent::NoteCreated { path } if path == Path::new("atomic.typ"))
        })
        .count();
    assert_eq!(
        settle_count, 1,
        "должно быть ровно одно итоговое событие на atomic.typ, без мигания: {events:?}"
    );

    let content = backend.read(Path::new("atomic.typ")).await.unwrap();
    assert_eq!(content, "новое содержимое");
}

#[tokio::test]
async fn bulk_external_change_emits_bulk_markers_and_converges_while_responsive() {
    // T048, FR-019, SC-005. 200 файлов вместо заявленных в SC-005 1000 —
    // тот же качественный сценарий (заметно выше BULK_CHANGE_THRESHOLD), с
    // разумным для юнит-теста временем выполнения; полный прогон на 1000
    // файлов — предмет отдельного бенчмарка (T093), не юнит-теста.
    let dir = TempDir::new().unwrap();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    backend.poll_events();

    // Каталог "bulk" создаётся заранее и ему даётся время осесть, прежде
    // чем в него льётся поток файлов: recursive-watch у inotify добавляет
    // вотч на новую поддиректорию асинхронно при обнаружении её создания
    // ("known problems" notify — TOCTOU-гонка между `mkdir` и записью
    // файлов в неё в первые миллисекунды). Реальные массовые внешние
    // изменения (переключение git-ветки и т.п.) обычно происходят в уже
    // существующих поддиректориях vault, поэтому это ограничение теста, не
    // продукта — сам конвейер обработки массовых изменений (T048/T055)
    // проверяется на файлах внутри уже отслеживаемого каталога.
    fs::create_dir_all(dir.path().join("bulk")).unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    backend.poll_events();

    const COUNT: usize = 200;
    for i in 0..COUNT {
        write_note(dir.path(), &format!("bulk/note-{i:04}.typ"), "содержимое");
    }

    // Отзывчивость (FR-019, SC-005: ≤100мс на отклик): вызов, не связанный
    // с этой пачкой, не должен зависать, даже пока пачка из COUNT файлов
    // ещё обрабатывается в фоне. Запас (300мс) выбран щедрее заявленного
    // порога, поскольку это интеграционный, не микро-бенчмарк тест.
    let responsiveness_start = std::time::Instant::now();
    let _ = backend.list_folder(Path::new("bulk")).await;
    assert!(
        responsiveness_start.elapsed() < Duration::from_millis(300),
        "backend не должен блокироваться во время фонового обновления после массового внешнего изменения"
    );

    let mut all_events = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let mut converged = false;
    while tokio::time::Instant::now() < deadline {
        all_events.extend(backend.poll_events());
        if backend.list().await.len() == COUNT {
            converged = true;
            // Дать шанс отставшему BulkChangeSettled (если он приходит
            // отдельным поздним тиком дебаунсера) попасть в очередь.
            tokio::time::sleep(Duration::from_millis(300)).await;
            all_events.extend(backend.poll_events());
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert!(
        converged,
        "индекс должен сойтись к {COUNT} заметкам на диске"
    );
    assert!(
        all_events
            .iter()
            .any(|e| matches!(e, VaultEvent::BulkChangeStarted)),
        "массовое изменение должно обрамляться BulkChangeStarted: длина потока {}",
        all_events.len()
    );
    assert!(
        all_events
            .iter()
            .any(|e| matches!(e, VaultEvent::BulkChangeSettled)),
        "массовое изменение должно обрамляться BulkChangeSettled: длина потока {}",
        all_events.len()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn losing_vault_access_emits_unavailable_and_does_not_wipe_index() {
    // T049, FR-020, FR-035. Смоделировано как потеря прав на проход через
    // родительский каталог (`chmod 000` у родителя vault) — файлы vault
    // физически остаются на диске, просто временно недостижимы через путь.
    // Намеренно НЕ `fs::remove_dir_all(vault)`: реальное посайловое
    // удаление — уже покрытый watch()-пайплайном сценарий (FR-016,
    // каждый файл честно получает свой NoteDeleted, это корректное
    // поведение, а не баг), а этот тест проверяет другой случай — доступ
    // пропал, но ничего в vault на самом деле не удалялось, поэтому индекс
    // не должен "сходиться к пустому" через серию NoteDeleted, а обязан
    // явно сообщить VaultAccessChanged(Unavailable), не трогая последний
    // известный список.
    use std::os::unix::fs::PermissionsExt;

    let parent = TempDir::new().unwrap();
    let vault_path = parent.path().join("vault");
    fs::create_dir(&vault_path).unwrap();
    write_note(&vault_path, "note.typ", "содержимое");

    let mut backend = NativeBackend::open(&vault_path).await.unwrap();
    assert_eq!(backend.list().await.len(), 1);
    assert_eq!(backend.access_state(), AccessState::ReadWrite);

    let mut perms = fs::metadata(parent.path()).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(parent.path(), perms).unwrap();

    let restore = || {
        let mut perms = fs::metadata(parent.path()).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(parent.path(), perms).unwrap();
    };

    // Тот же root-bypass guard, что и в
    // revoking_write_access_emits_read_only_and_write_is_denied_immediately:
    // под root ограничение прав доступа не действует, тест недетерминирован
    // в таком окружении — честно пропускаем вместо ложного результата.
    if fs::metadata(&vault_path).is_ok() {
        restore();
        eprintln!(
            "пропуск losing_vault_access_emits_unavailable_and_does_not_wipe_index: \
             процесс обходит права доступа (вероятно root) — тест недетерминирован в этом окружении"
        );
        return;
    }

    let mut saw_unavailable = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if backend.poll_events().iter().any(|e| {
            matches!(
                e,
                VaultEvent::VaultAccessChanged { access_state } if *access_state == AccessState::Unavailable
            )
        }) {
            saw_unavailable = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    if !saw_unavailable {
        restore();
        panic!("потеря доступа должна сообщаться через VaultAccessChanged(Unavailable)");
    }

    assert_eq!(backend.access_state(), AccessState::Unavailable);
    // FR-020: недоступность не трактуется как удаление всех заметок —
    // backend не обязан (и не должен) сам стирать последний известный
    // индекс, когда диск стал недоступен.
    assert_eq!(
        backend.list().await.len(),
        1,
        "backend не должен сам стирать индекс при потере доступа"
    );

    restore();
}

#[cfg(unix)]
#[tokio::test]
async fn revoking_write_access_emits_read_only_and_write_is_denied_immediately() {
    // T050, FR-036
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "note.typ", "содержимое");

    // Тест зависит от того, что ОС реально запрещает запись без бита `w` —
    // под root (типично для контейнерных CI-окружений) это не так (тот же
    // риск, что уже задокументирован в
    // write_to_missing_parent_directory_reports_io_error_and_keeps_index_consistent
    // выше). Пробуем заранее и, если ограничение не сработало, честно
    // пропускаем тест вместо недетерминированного падения/успеха.
    let probe = dir.path().join("probe-only");
    fs::create_dir(&probe).unwrap();
    let mut probe_perms = fs::metadata(&probe).unwrap().permissions();
    probe_perms.set_mode(0o555);
    fs::set_permissions(&probe, probe_perms).unwrap();
    let bypasses_permissions = fs::write(probe.join("x.typ"), "").is_ok();
    let mut restore_probe_perms = fs::metadata(&probe).unwrap().permissions();
    restore_probe_perms.set_mode(0o755);
    fs::set_permissions(&probe, restore_probe_perms).unwrap();
    fs::remove_dir_all(&probe).unwrap();
    if bypasses_permissions {
        eprintln!(
            "пропуск revoking_write_access_emits_read_only_and_write_is_denied_immediately: \
             процесс обходит права доступа (вероятно root) — тест недетерминирован в этом окружении"
        );
        return;
    }

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    assert_eq!(backend.access_state(), AccessState::ReadWrite);

    let mut perms = fs::metadata(dir.path()).unwrap().permissions();
    perms.set_mode(0o555); // r-x r-x r-x — читать можно, писать/создавать нельзя
    fs::set_permissions(dir.path(), perms).unwrap();

    let restore = || {
        let mut perms = fs::metadata(dir.path()).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(dir.path(), perms).unwrap();
    };

    let mut saw_read_only = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if backend.poll_events().iter().any(|e| {
            matches!(
                e,
                VaultEvent::VaultAccessChanged { access_state } if *access_state == AccessState::ReadOnly
            )
        }) {
            saw_read_only = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    if !saw_read_only {
        restore();
        panic!("отзыв прав на запись должен сообщаться через VaultAccessChanged(ReadOnly)");
    }

    assert_eq!(backend.access_state(), AccessState::ReadOnly);
    // Чтение продолжает работать как обычно (FR-036).
    assert_eq!(
        backend.read(Path::new("note.typ")).await.unwrap(),
        "содержимое"
    );
    // Запись отклоняется немедленно тем же механизмом, что и FR-014, без
    // попытки реальной записи на диск.
    let result = backend.write(Path::new("note.typ"), "новое", None).await;
    assert_eq!(result.err(), Some(WriteError::AccessDenied));

    restore();
}

#[tokio::test]
async fn reopening_after_close_detects_both_additions_and_deletions_made_while_closed() {
    // T051, FR-018: дополняет reopen_uses_cache_and_reconciles_in_background
    // (та проверяет только добавление файла, пока backend был закрыт) —
    // здесь дополнительно проверяется удаление, произошедшее, пока
    // приложение было закрыто, и то, что после сверки именно список (а не
    // только поток событий) отражает актуальное состояние диска.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "stays.typ", "остаётся");
    write_note(
        dir.path(),
        "removed_while_closed.typ",
        "будет удалена снаружи",
    );
    {
        let backend = NativeBackend::open(dir.path()).await.unwrap();
        assert_eq!(backend.list().await.len(), 2);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    fs::remove_file(dir.path().join("removed_while_closed.typ")).unwrap();
    write_note(dir.path(), "added_while_closed.typ", "добавлена снаружи");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut converged = false;
    while tokio::time::Instant::now() < deadline {
        backend.poll_events();
        let paths: Vec<PathBuf> = backend.list().await.into_iter().map(|n| n.path).collect();
        if paths.contains(&PathBuf::from("stays.typ"))
            && paths.contains(&PathBuf::from("added_while_closed.typ"))
            && !paths.contains(&PathBuf::from("removed_while_closed.typ"))
        {
            converged = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert!(
        converged,
        "сверка при открытии должна обнаружить и добавление, и удаление, \
         произошедшие, пока приложение было закрыто"
    );
}

#[tokio::test]
async fn changes_from_second_backend_instance_on_same_root_are_observed_as_external_by_the_first() {
    // T052, FR-034: второе окно приложения (второй VaultBackend на том же
    // root_path) — обычный внешний источник изменений для первого, без
    // отдельного канала синхронизации между окнами (Clarifications Q1).
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "shared.typ", "исходное содержимое");

    let mut window_a = NativeBackend::open(dir.path()).await.unwrap();
    let mut window_b = NativeBackend::open(dir.path()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    window_a.poll_events();
    window_b.poll_events();

    // Действие в окне B — обычный вызов VaultBackend::write, как от
    // пользователя, а не «сырая» файловая операция в обход API.
    window_b
        .write(Path::new("shared.typ"), "правки из другого окна", None)
        .await
        .unwrap();

    let events = drain_events_for(&mut window_a, Duration::from_secs(5)).await;
    assert!(
        events.iter().any(
            |e| matches!(e, VaultEvent::NoteUpdated { path } if path == Path::new("shared.typ"))
        ),
        "правка из окна B должна дойти до окна A как обычное внешнее VaultEvent: {events:?}"
    );
}

// ---------------------------------------------------------------------------
// US4 — Разрешить конфликт с внешним изменением (T064…T069)
//
// Во всех тестах ниже «несохранённые правки» объявляются ядру через
// `begin_edit()` — редактор не хранит буфер втайне от ядра, иначе решение
// «поднимать конфликт или тихо обновить» пришлось бы принимать в UI
// (нарушение принципа I). Буфер живёт только в памяти сессии (FR-035).
// ---------------------------------------------------------------------------

/// Дожидается конкретного события в потоке `poll_events()` или возвращает
/// собранный поток, если событие так и не пришло за `duration`. В отличие от
/// `drain_events_for`, не ждёт весь бюджет, когда событие уже получено, —
/// тесты US4 иначе стоили бы по 5 секунд каждый.
async fn wait_for_event<F>(
    backend: &mut NativeBackend,
    duration: Duration,
    predicate: F,
) -> Result<VaultEvent, Vec<VaultEvent>>
where
    F: Fn(&VaultEvent) -> bool,
{
    let deadline = tokio::time::Instant::now() + duration;
    let mut collected = Vec::new();
    while tokio::time::Instant::now() < deadline {
        for event in backend.poll_events() {
            if predicate(&event) {
                return Ok(event);
            }
            collected.push(event);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(collected)
}

#[tokio::test]
async fn external_change_to_note_with_unsaved_edits_raises_conflict_instead_of_silent_update() {
    // T064, FR-021: ни одна версия не применяется автоматически.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "note.typ", "исходное содержимое");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    backend
        .begin_edit(
            Path::new("note.typ"),
            "мои несохранённые правки".to_string(),
        )
        .await;

    write_note(dir.path(), "note.typ", "внешнее изменение");

    let event = wait_for_event(&mut backend, Duration::from_secs(5), |e| {
        matches!(e, VaultEvent::ConflictDetected { note_path, .. } if note_path == Path::new("note.typ"))
    })
    .await;
    let seen = match event {
        Ok(e) => e,
        Err(others) => panic!(
            "внешнее изменение заметки с несохранёнными правками должно давать ConflictDetected: {others:?}"
        ),
    };

    // Ни `NoteUpdated` вместо конфликта, ни автоматического применения
    // какой-либо из версий: диск остаётся с внешним содержимым, буфер — с
    // пользовательским.
    assert!(matches!(seen, VaultEvent::ConflictDetected { .. }));
    assert_eq!(
        fs::read_to_string(dir.path().join("note.typ")).unwrap(),
        "внешнее изменение",
        "внешняя версия не должна затираться локальным буфером без выбора пользователя"
    );
    let conflict = backend
        .pending_conflict(Path::new("note.typ"))
        .expect("конфликт должен остаться неразрешённым до действия пользователя");
    assert_eq!(conflict.local_buffer, "мои несохранённые правки");
    assert_eq!(conflict.status, ConflictStatus::Pending);
}

#[tokio::test]
async fn write_with_stale_expected_snapshot_is_rejected_instead_of_overwriting_disk() {
    // T065, contracts/vault-backend-trait.md: оптимистичная конкурентность.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "note.typ", "исходное содержимое");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    let stale = backend
        .list()
        .await
        .into_iter()
        .find(|n| n.path == Path::new("note.typ"))
        .expect("заметка должна быть в индексе")
        .stamp;

    // Диск ушёл вперёд относительно снимка, от которого отталкивался редактор.
    write_note(dir.path(), "note.typ", "содержимое, изменённое снаружи");

    let result = backend
        .write(
            Path::new("note.typ"),
            "запись поверх устаревшего снимка",
            Some(stale),
        )
        .await;

    assert_eq!(result.err(), Some(WriteError::Conflict));
    assert_eq!(
        fs::read_to_string(dir.path().join("note.typ")).unwrap(),
        "содержимое, изменённое снаружи",
        "отклонённая запись не должна касаться диска"
    );
}

#[tokio::test]
async fn resolving_keep_local_writes_buffer_to_disk_and_clears_conflict() {
    // T066, US4 AS2.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "note.typ", "исходное содержимое");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    backend
        .begin_edit(Path::new("note.typ"), "моя версия".to_string())
        .await;
    write_note(dir.path(), "note.typ", "внешняя версия");
    wait_for_event(&mut backend, Duration::from_secs(5), |e| {
        matches!(e, VaultEvent::ConflictDetected { .. })
    })
    .await
    .expect("конфликт должен быть обнаружен");

    backend
        .resolve_conflict_keep_local(Path::new("note.typ"))
        .await
        .expect("«оставить свою версию» должно записаться на диск");

    assert_eq!(
        fs::read_to_string(dir.path().join("note.typ")).unwrap(),
        "моя версия"
    );
    assert!(backend.pending_conflict(Path::new("note.typ")).is_none());
    assert!(!backend.has_unsaved_edits(Path::new("note.typ")));
}

#[tokio::test]
async fn accepting_external_discards_local_buffer_only_after_explicit_confirmation() {
    // T067, US4 AS3, FR-023.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "note.typ", "исходное содержимое");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    backend
        .begin_edit(Path::new("note.typ"), "моя версия".to_string())
        .await;
    write_note(dir.path(), "note.typ", "внешняя версия");
    wait_for_event(&mut backend, Duration::from_secs(5), |e| {
        matches!(e, VaultEvent::ConflictDetected { .. })
    })
    .await
    .expect("конфликт должен быть обнаружен");

    // До явного подтверждения буфер жив: сам факт показа диалога ничего не
    // отбрасывает (FR-023).
    assert!(backend.has_unsaved_edits(Path::new("note.typ")));
    assert_eq!(
        backend
            .pending_conflict(Path::new("note.typ"))
            .map(|c| c.local_buffer),
        Some("моя версия".to_string())
    );

    let external = backend
        .resolve_conflict_accept_external(Path::new("note.typ"))
        .await
        .expect("разрешение конфликта не должно отказывать")
        .expect("внешняя версия существует на диске, значит должна вернуться редактору");

    assert_eq!(external, "внешняя версия");
    assert!(!backend.has_unsaved_edits(Path::new("note.typ")));
    assert!(backend.pending_conflict(Path::new("note.typ")).is_none());
    assert_eq!(
        fs::read_to_string(dir.path().join("note.typ")).unwrap(),
        "внешняя версия",
        "принятие внешней версии не меняет диск — он уже её носитель"
    );
}

#[tokio::test]
async fn repeated_disk_change_before_resolution_refreshes_disk_snapshot() {
    // T068, US4 AS5: не оставлять устаревший снимок и не плодить второй
    // конфликт на ту же заметку.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "note.typ", "исходное содержимое");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    backend
        .begin_edit(Path::new("note.typ"), "моя версия".to_string())
        .await;

    write_note(dir.path(), "note.typ", "первое внешнее изменение");
    wait_for_event(&mut backend, Duration::from_secs(5), |e| {
        matches!(e, VaultEvent::ConflictDetected { .. })
    })
    .await
    .expect("первый конфликт должен быть обнаружен");
    let first = backend
        .pending_conflict(Path::new("note.typ"))
        .expect("конфликт должен быть активен")
        .disk_snapshot;

    write_note(dir.path(), "note.typ", "второе внешнее изменение");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut refreshed = None;
    while tokio::time::Instant::now() < deadline {
        backend.poll_events();
        let current = backend
            .pending_conflict(Path::new("note.typ"))
            .expect("конфликт не должен исчезать сам по себе")
            .disk_snapshot;
        if current != first {
            refreshed = Some(current);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let refreshed = refreshed.expect("повторное изменение диска должно обновить disk_snapshot");
    assert_eq!(
        refreshed.content_hash,
        FileStamp::from_content(0, "второе внешнее изменение".as_bytes()).content_hash,
        "снимок должен соответствовать актуальному содержимому диска, а не первому изменению"
    );
    assert_eq!(
        backend
            .pending_conflict(Path::new("note.typ"))
            .map(|c| c.local_buffer),
        Some("моя версия".to_string()),
        "локальный буфер не должен теряться при обновлении снимка"
    );
}

#[tokio::test]
async fn external_deletion_of_note_with_unsaved_edits_warns_and_allows_resave() {
    // T069, FR-025.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "note.typ", "исходное содержимое");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    backend
        .begin_edit(Path::new("note.typ"), "не потерять эти правки".to_string())
        .await;

    fs::remove_file(dir.path().join("note.typ")).unwrap();

    wait_for_event(&mut backend, Duration::from_secs(5), |e| {
        matches!(e, VaultEvent::NoteDeletedWithUnsavedEdits { note_path } if note_path == Path::new("note.typ"))
    })
    .await
    .expect("внешнее удаление заметки с правками должно давать NoteDeletedWithUnsavedEdits");

    // Обычный `NoteDeleted` вместо предупреждения означал бы молчаливую
    // потерю правок — правки обязаны остаться доступными для пересохранения.
    assert!(backend.has_unsaved_edits(Path::new("note.typ")));
    backend
        .resolve_conflict_keep_local(Path::new("note.typ"))
        .await
        .expect("пересохранение удалённой снаружи заметки должно быть возможно");
    assert_eq!(
        fs::read_to_string(dir.path().join("note.typ")).unwrap(),
        "не потерять эти правки"
    );
}

#[tokio::test]
async fn diff_between_local_buffer_and_disk_shows_both_sides() {
    // FR-024 (T073): просмотр отличий не разрешает конфликт сам по себе.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "note.typ", "общая строка\nстарая строка\n");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    backend
        .begin_edit(
            Path::new("note.typ"),
            "общая строка\nмоя строка\n".to_string(),
        )
        .await;
    write_note(dir.path(), "note.typ", "общая строка\nвнешняя строка\n");
    wait_for_event(&mut backend, Duration::from_secs(5), |e| {
        matches!(e, VaultEvent::ConflictDetected { .. })
    })
    .await
    .expect("конфликт должен быть обнаружен");

    let diff = backend.conflict_diff(Path::new("note.typ")).await;

    assert!(
        diff.contains(&DiffLine::Same {
            text: "общая строка".to_string()
        }),
        "общая часть должна показываться как общая: {diff:?}"
    );
    assert!(
        diff.contains(&DiffLine::Local {
            text: "моя строка".to_string()
        }),
        "локальная строка должна быть в отличиях: {diff:?}"
    );
    assert!(
        diff.contains(&DiffLine::External {
            text: "внешняя строка".to_string()
        }),
        "внешняя строка должна быть в отличиях: {diff:?}"
    );
    // FR-024: просмотр отличий возвращает пользователя к выбору — конфликт
    // остаётся неразрешённым.
    assert!(backend.pending_conflict(Path::new("note.typ")).is_some());
}

// ---------------------------------------------------------------------------
// US5 — Не терять ссылки при удалении (T080…T082)
// ---------------------------------------------------------------------------

/// Пути обратных ссылок в отсортированном виде — `backlinks()` не обязан
/// сохранять порядок (он приходит из SQL-запроса), а тестам нужен детерминизм.
async fn backlink_paths(backend: &NativeBackend, target: &str) -> Vec<String> {
    let mut paths: Vec<String> = backend
        .backlinks(Path::new(target))
        .await
        .into_iter()
        .map(|n| n.path.to_string_lossy().replace('\\', "/"))
        .collect();
    paths.sort();
    paths
}

/// Ждёт, пока обратные ссылки на `target` не совпадут с `expected` —
/// индексация внешних изменений асинхронна (дебаунсер/фоновая сверка).
async fn wait_for_backlinks(
    backend: &mut NativeBackend,
    target: &str,
    expected: &[&str],
) -> Vec<String> {
    let expected: Vec<String> = expected.iter().map(|s| (*s).to_string()).collect();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut last = Vec::new();
    while tokio::time::Instant::now() < deadline {
        backend.poll_events();
        last = backlink_paths(backend, target).await;
        if last == expected {
            return last;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    last
}

#[tokio::test]
async fn backlinks_returns_notes_that_link_to_the_target() {
    // T080, FR-026.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "target.typ", "цель ссылок");
    write_note(dir.path(), "b.typ", "смотри [[target]]");
    write_note(dir.path(), "c.typ", "и здесь [[target.typ]] тоже");
    write_note(dir.path(), "unrelated.typ", "ни на кого не ссылается");

    let backend = NativeBackend::open(dir.path()).await.unwrap();

    assert_eq!(
        backlink_paths(&backend, "target.typ").await,
        vec!["b.typ".to_string(), "c.typ".to_string()],
        "обе формы записи ссылки (с расширением и без) должны находиться"
    );
    assert!(
        backlink_paths(&backend, "unrelated.typ").await.is_empty(),
        "заметка без входящих ссылок не должна их получать из ниоткуда"
    );
}

#[tokio::test]
async fn backlinks_resolve_relative_to_the_linking_note() {
    // T080, FR-026: `[[имя]]` — соседняя заметка, а не заметка в корне.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "sub/target.typ", "цель во вложенной папке");
    write_note(dir.path(), "sub/sibling.typ", "рядом: [[target]]");
    write_note(dir.path(), "root_link.typ", "из корня: [[sub/target]]");
    write_note(dir.path(), "target.typ", "однофамилец в корне");

    let backend = NativeBackend::open(dir.path()).await.unwrap();

    assert_eq!(
        backlink_paths(&backend, "sub/target.typ").await,
        vec!["root_link.typ".to_string(), "sub/sibling.typ".to_string()]
    );
    assert!(
        backlink_paths(&backend, "target.typ").await.is_empty(),
        "ссылка из sub/ не должна попадать на однофамильца в корне"
    );
}

#[tokio::test]
async fn link_index_stays_current_across_external_create_rename_and_delete() {
    // T081, FR-031: включая изменения, сделанные вне приложения.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "target.typ", "цель ссылок");
    write_note(dir.path(), "b.typ", "смотри [[target]]");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    assert_eq!(
        backlink_paths(&backend, "target.typ").await,
        vec!["b.typ".to_string()]
    );

    // Создание снаружи.
    write_note(dir.path(), "c.typ", "тоже [[target]]");
    assert_eq!(
        wait_for_backlinks(&mut backend, "target.typ", &["b.typ", "c.typ"]).await,
        vec!["b.typ".to_string(), "c.typ".to_string()],
        "внешне созданная ссылающаяся заметка должна попасть в обратные ссылки"
    );

    // Изменение содержимого снаружи: ссылка убрана.
    write_note(dir.path(), "b.typ", "больше ни на кого не ссылаюсь");
    assert_eq!(
        wait_for_backlinks(&mut backend, "target.typ", &["c.typ"]).await,
        vec!["c.typ".to_string()],
        "снятая снаружи ссылка должна исчезнуть из обратных"
    );

    // Переименование снаружи: исходящие ссылки переезжают вместе с заметкой.
    fs::rename(dir.path().join("c.typ"), dir.path().join("c_renamed.typ")).unwrap();
    assert_eq!(
        wait_for_backlinks(&mut backend, "target.typ", &["c_renamed.typ"]).await,
        vec!["c_renamed.typ".to_string()],
        "переименованная снаружи заметка должна ссылаться под новым именем"
    );

    // Удаление снаружи.
    fs::remove_file(dir.path().join("c_renamed.typ")).unwrap();
    assert_eq!(
        wait_for_backlinks(&mut backend, "target.typ", &[]).await,
        Vec::<String>::new(),
        "удалённая снаружи заметка не должна оставаться в обратных ссылках"
    );
}

#[tokio::test]
async fn deleting_a_linked_note_reports_affected_notes_and_leaves_them_findable() {
    // T081 + FR-027, FR-030: удаление цели не трогает ссылающиеся заметки, но
    // сообщает, что их ссылки стали битыми.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "a.typ", "цель");
    write_note(dir.path(), "b.typ", "ссылка на [[a]]");
    write_note(dir.path(), "c.typ", "ещё ссылка на [[a]]");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    // Список, который UI обязан показать до удаления (FR-027).
    assert_eq!(
        backlink_paths(&backend, "a.typ").await,
        vec!["b.typ".to_string(), "c.typ".to_string()]
    );

    backend.delete(Path::new("a.typ")).await.unwrap();

    let event = wait_for_event(&mut backend, Duration::from_secs(5), |e| {
        matches!(e, VaultEvent::LinksInvalidated { .. })
    })
    .await
    .expect("удаление цели ссылок должно давать LinksInvalidated");
    match event {
        VaultEvent::LinksInvalidated { affected_paths } => {
            let mut paths: Vec<String> = affected_paths
                .iter()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .collect();
            paths.sort();
            assert_eq!(paths, vec!["b.typ".to_string(), "c.typ".to_string()]);
        }
        other => panic!("ожидалось LinksInvalidated, получено {other:?}"),
    }

    // FR-030: ссылающиеся заметки остаются на месте и остаются находимыми как
    // носители теперь уже битой ссылки.
    assert_eq!(
        backlink_paths(&backend, "a.typ").await,
        vec!["b.typ".to_string(), "c.typ".to_string()],
        "битая ссылка не перестаёт быть ссылкой — иначе её нельзя было бы найти"
    );
    assert!(dir.path().join("b.typ").exists());
    assert!(dir.path().join("c.typ").exists());
}

#[tokio::test]
async fn broken_links_are_findable_after_the_operation_completes() {
    // FR-030: после удаления цели ссылающаяся заметка находится как носитель
    // битой ссылки — это и есть «найти заметки с битыми ссылками».
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "a.typ", "цель");
    write_note(
        dir.path(),
        "b.typ",
        "ссылка на [[a]] и на [[никогда-не-было]]",
    );

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();

    let broken: Vec<String> = backend
        .broken_links(Path::new("b.typ"))
        .await
        .into_iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    assert_eq!(
        broken,
        vec!["никогда-не-было.typ".to_string()],
        "существующая цель битой не считается, несуществующая — считается"
    );

    backend.delete(Path::new("a.typ")).await.unwrap();

    let mut broken: Vec<String> = backend
        .broken_links(Path::new("b.typ"))
        .await
        .into_iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    broken.sort();
    assert_eq!(
        broken,
        vec!["a.typ".to_string(), "никогда-не-было.typ".to_string()],
        "после удаления цели ссылка на неё становится битой и остаётся находимой"
    );
}

#[tokio::test]
async fn rename_and_delete_of_note_without_backlinks_need_no_confirmation() {
    // T082, US5 AS2: пустой список обратных ссылок — UI не показывает диалог.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "lonely.typ", "на меня никто не ссылается");
    write_note(dir.path(), "other.typ", "и я ни на кого");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();

    assert!(backlink_paths(&backend, "lonely.typ").await.is_empty());
    backend
        .rename(Path::new("lonely.typ"), Path::new("renamed.typ"))
        .await
        .unwrap();
    assert!(dir.path().join("renamed.typ").exists());

    assert!(backlink_paths(&backend, "renamed.typ").await.is_empty());
    backend.delete(Path::new("renamed.typ")).await.unwrap();
    assert!(!dir.path().join("renamed.typ").exists());
}

#[tokio::test]
async fn renaming_a_linked_note_leaves_incoming_links_broken_and_findable() {
    // FR-028, FR-030: содержимое ссылающихся заметок приложение не переписывает,
    // поэтому после переименования их ссылки честно становятся битыми — и
    // именно это пользователь должен увидеть заранее.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "a.typ", "цель");
    write_note(dir.path(), "b.typ", "ссылка на [[a]]");

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    assert_eq!(
        backlink_paths(&backend, "a.typ").await,
        vec!["b.typ".to_string()]
    );

    backend
        .rename(Path::new("a.typ"), Path::new("a_renamed.typ"))
        .await
        .unwrap();

    assert_eq!(
        backlink_paths(&backend, "a.typ").await,
        vec!["b.typ".to_string()],
        "ссылка b.typ по-прежнему указывает на старое имя — она битая, а не исчезнувшая"
    );
    assert!(
        backlink_paths(&backend, "a_renamed.typ").await.is_empty(),
        "приложение не переписывает чужое содержимое, чтобы ссылка «починилась» сама"
    );
}

// ---------------------------------------------------------------------------
// Phase 9 (convergence): пределы масштаба и гейты записи
// ---------------------------------------------------------------------------

#[tokio::test]
async fn vault_over_supported_note_count_is_refused_with_named_reason() {
    // T103, FR-038. Предел проверяется с обеих сторон границы одним тестом:
    // отдельная проверка «ровно предел открывается» без «предел+1 не
    // открывается» (и наоборот) прошла бы и при сдвинутом на единицу условии.
    let dir = TempDir::new().unwrap();
    for i in 0..SUPPORTED_NOTE_COUNT {
        write_note(dir.path(), &format!("note{i}.typ"), "содержимое");
    }

    // Ровно предел — открывается: FR-038 запрещает «больше», а не «столько».
    let backend = NativeBackend::open(dir.path()).await.unwrap();
    assert_eq!(backend.list().await.len(), SUPPORTED_NOTE_COUNT);
    drop(backend);

    // Кэш, наполненный успешным открытием, не должен становиться лазейкой:
    // следующее открытие обязано отказать по тому же пределу, а не
    // проскочить по быстрому пути SC-003.
    write_note(dir.path(), "one_too_many.typ", "перебор");

    let result = NativeBackend::open(dir.path()).await;
    assert!(
        matches!(result, Err(OpenError::TooManyNotes)),
        "vault свыше предела MUST NOT открываться, и причина MUST быть названа именно этой"
    );
}

#[tokio::test]
async fn note_over_size_limit_stays_in_tree_but_is_not_opened() {
    // T108, spec Assumptions + Edge Cases.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "small.typ", "обычная заметка");
    fs::write(
        dir.path().join("huge.typ"),
        "a".repeat(NOTE_SIZE_LIMIT_BYTES as usize + 1),
    )
    .unwrap();

    let backend = NativeBackend::open(dir.path()).await.unwrap();

    // Заметка остаётся видимой в дереве — она существует на диске, и прятать
    // её значило бы соврать о содержимом папки.
    let paths: Vec<PathBuf> = backend.list().await.into_iter().map(|n| n.path).collect();
    assert!(
        paths.contains(&PathBuf::from("huge.typ")),
        "заметка выше порога MUST оставаться видимой в дереве"
    );

    // Отклоняется только открытие, и с названной причиной.
    assert_eq!(
        backend.read(Path::new("huge.typ")).await.err(),
        Some(ReadError::TooLarge)
    );
    // Соседняя заметка нормального размера при этом читается как обычно.
    assert_eq!(
        backend.read(Path::new("small.typ")).await.unwrap(),
        "обычная заметка"
    );
}

#[tokio::test]
async fn note_exactly_at_size_limit_still_opens() {
    // T108: порог — «до 10 МБ включительно», поэтому ровно предел обязан
    // открываться. Без этой проверки условие `>=` вместо `>` прошло бы мимо.
    let dir = TempDir::new().unwrap();
    let content = "a".repeat(NOTE_SIZE_LIMIT_BYTES as usize);
    fs::write(dir.path().join("exact.typ"), &content).unwrap();

    let backend = NativeBackend::open(dir.path()).await.unwrap();
    assert_eq!(
        backend.read(Path::new("exact.typ")).await.unwrap().len(),
        NOTE_SIZE_LIMIT_BYTES as usize
    );
}

#[cfg(unix)]
#[tokio::test]
async fn read_only_access_denies_create_rename_and_delete_not_only_write() {
    // T106, FR-036: перечень FR-036 — «сохранение, создание, переименование,
    // удаление». Существующий тест
    // revoking_write_access_emits_read_only_and_write_is_denied_immediately
    // покрывает только первое из четырёх.
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "note.typ", "содержимое");

    // Та же оговорка, что и в соседних тестах прав доступа: под root бит `w`
    // не ограничивает процесс, и тест был бы недетерминирован.
    let probe = dir.path().join("probe-only");
    fs::create_dir(&probe).unwrap();
    let mut probe_perms = fs::metadata(&probe).unwrap().permissions();
    probe_perms.set_mode(0o555);
    fs::set_permissions(&probe, probe_perms).unwrap();
    let bypasses_permissions = fs::write(probe.join("x.typ"), "").is_ok();
    let mut restore_probe_perms = fs::metadata(&probe).unwrap().permissions();
    restore_probe_perms.set_mode(0o755);
    fs::set_permissions(&probe, restore_probe_perms).unwrap();
    fs::remove_dir_all(&probe).unwrap();
    if bypasses_permissions {
        eprintln!(
            "пропуск read_only_access_denies_create_rename_and_delete_not_only_write: \
             процесс обходит права доступа (вероятно root) — тест недетерминирован в этом окружении"
        );
        return;
    }

    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    assert_eq!(backend.access_state(), AccessState::ReadWrite);

    let mut perms = fs::metadata(dir.path()).unwrap().permissions();
    perms.set_mode(0o555);
    fs::set_permissions(dir.path(), perms).unwrap();

    let restore = || {
        let mut perms = fs::metadata(dir.path()).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(dir.path(), perms).unwrap();
    };

    let mut saw_read_only = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if backend.poll_events().iter().any(|e| {
            matches!(
                e,
                VaultEvent::VaultAccessChanged { access_state } if *access_state == AccessState::ReadOnly
            )
        }) {
            saw_read_only = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    if !saw_read_only {
        restore();
        panic!("отзыв прав на запись должен сообщаться через VaultAccessChanged(ReadOnly)");
    }

    let create = backend.create(Path::new("new.typ")).await;
    let rename = backend
        .rename(Path::new("note.typ"), Path::new("renamed.typ"))
        .await;
    let delete = backend.delete(Path::new("note.typ")).await;

    restore();

    assert_eq!(create.err(), Some(CreateError::AccessDenied));
    assert_eq!(rename.err(), Some(RenameError::AccessDenied));
    assert_eq!(delete.err(), Some(DeleteError::AccessDenied));

    // Отказ обязан быть отказом, а не откатом уже сделанного: файл на месте,
    // подменного не появилось.
    assert!(dir.path().join("note.typ").exists());
    assert!(!dir.path().join("new.typ").exists());
    assert!(!dir.path().join("renamed.typ").exists());
}

// ---------------------------------------------------------------------------
// Классификация файлов и состав дерева (фича 002, T009, T010)
// ---------------------------------------------------------------------------

#[test]
fn classification_separates_notes_boards_snapshots_and_everything_else() {
    // FR-004 и FR-021 фичи 002. Правило одно на оба backend (research.md §3):
    // тот же набор проверяется в `vault_wasm.rs`, и расхождение результата
    // между таргетами — дефект, а не платформенная особенность.
    use typsidian_core::vault::{classify, FileKind};

    assert_eq!(classify(Path::new("Заметка.typ")), FileKind::Note);
    assert_eq!(classify(Path::new("папка/Заметка.typ")), FileKind::Note);
    assert_eq!(classify(Path::new("Схема.excalidraw")), FileKind::Board);
    assert_eq!(
        classify(Path::new("папка/Схема.excalidraw")),
        FileKind::Board
    );

    // Снимок опознаётся по расположению, а не по расширению.
    assert_eq!(
        classify(Path::new("Заметка.assets/board-a1b2c3.svg")),
        FileKind::Snapshot
    );
    assert_eq!(
        classify(Path::new("папка/Заметка.assets/board-a1b2c3.svg")),
        FileKind::Snapshot
    );

    // Картинка, положенная пользователем сам, снимком не считается.
    assert_eq!(classify(Path::new("схема.svg")), FileKind::Other);
    assert_eq!(classify(Path::new("вложения/фото.png")), FileKind::Other);

    assert!(FileKind::Note.is_listed());
    assert!(FileKind::Board.is_listed());
    assert!(!FileKind::Snapshot.is_listed());
    assert!(!FileKind::Other.is_listed());
}

#[tokio::test]
async fn tree_contains_boards_and_hides_snapshots() {
    // FR-004, FR-021 фичи 002. Без этого доска создаётся и пишется, но в дереве
    // не появляется, и US1 упирается в тупик.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "Заметка.typ", "текст");
    write_note(dir.path(), "Схема.excalidraw", "{\"type\":\"excalidraw\"}");
    write_note(dir.path(), "папка/Другая.excalidraw", "{}");
    write_note(dir.path(), "Заметка.assets/board-a1b2c3.svg", "<svg/>");
    write_note(dir.path(), "просто-картинка.svg", "<svg/>");

    let backend = NativeBackend::open(dir.path())
        .await
        .expect("открытие vault");
    let paths: Vec<String> = backend
        .list()
        .await
        .iter()
        .map(|note| note.path.to_string_lossy().replace('\\', "/"))
        .collect();

    assert!(paths.contains(&"Заметка.typ".to_string()));
    assert!(paths.contains(&"Схема.excalidraw".to_string()));
    assert!(paths.contains(&"папка/Другая.excalidraw".to_string()));
    assert!(
        !paths.iter().any(|p| p.contains(".assets/")),
        "снимок не должен попадать в дерево: {paths:?}"
    );
    assert!(
        !paths.contains(&"просто-картинка.svg".to_string()),
        "посторонний файл не должен попадать в дерево: {paths:?}"
    );
    assert_eq!(paths.len(), 3);
}

#[tokio::test]
async fn reading_a_missing_note_reports_not_found_not_a_generic_io_error() {
    // FR-008, FR-014 фичи 001 и принцип II. Обнаружено при реализации фичи 002:
    // native схлопывал все ошибки чтения в `Io`, поэтому формулировка «заметку
    // удалили или переименовали снаружи» была недостижима, а `WebBackend`
    // возвращал `NotFound` с самого начала — таргеты вели себя по-разному.
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "есть.typ", "текст");
    let backend = NativeBackend::open(dir.path()).await.unwrap();

    assert_eq!(
        backend.read(Path::new("нет.typ")).await,
        Err(ReadError::NotFound)
    );
    assert!(backend.read(Path::new("есть.typ")).await.is_ok());
}

#[tokio::test]
async fn external_change_signals_the_shell_without_polling() {
    // T114: оболочка рабочего стола ждёт сигнала ядра и пересылает события в
    // окно. Если сигнала нет, единственный способ узнать о внешнем изменении —
    // опрос по таймеру, то есть задержка, которой в самом ядре нет: обнаружение
    // здесь push-овое (`notify` + дебаунсер).
    let dir = TempDir::new().unwrap();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    let ready = backend.events_ready();

    fs::write(dir.path().join("снаружи.typ"), "текст").unwrap();

    // Бюджет тот же, что у SC-004: внешнее изменение видно ≤2 с; берём запас на
    // дебаунсер вотчера.
    let signalled = tokio::time::timeout(Duration::from_secs(5), ready.notified()).await;
    assert!(
        signalled.is_ok(),
        "ядро обязано просигналить о событиях, а не ждать опроса"
    );
    assert!(
        !backend.poll_events().is_empty(),
        "после сигнала в очереди обязано что-то быть"
    );
}

#[tokio::test]
async fn vault_settings_apply_at_once_and_an_outside_edit_raises_no_event() {
    // 006 T056. FR-033: значение, записанное через приложение, действует
    // сразу — следующее чтение его уже видит. FR-040: правка `vault.json`
    // мимо приложения никаким событием не сопровождается, потому что служебное
    // место исключено из наблюдения; подхватить её до следующего открытия
    // vault приложению нечем, и это закреплённое поведение, а не дефект.
    use typsidian_core::vault::vault_settings::{
        read_vault_settings, write_vault_settings, VaultOverrides,
    };

    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "заметка.typ", "текст");
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();

    let written = VaultOverrides {
        editor_font_size: Some(18),
        ..VaultOverrides::default()
    };
    write_vault_settings(&mut backend, &written).await.unwrap();
    assert_eq!(read_vault_settings(&backend).await.unwrap(), written);

    tokio::time::sleep(Duration::from_millis(150)).await;
    backend.poll_events();

    let settings_file = dir.path().join(".typsidian/vault.json");
    assert!(settings_file.is_file(), "настройки лежат в служебном месте");
    // Правка «руками»: то же содержимое с другим значением.
    let edited = fs::read_to_string(&settings_file)
        .unwrap()
        .replace("18", "22");
    fs::write(&settings_file, edited).unwrap();

    let events = drain_events_for(&mut backend, Duration::from_secs(2)).await;
    assert!(
        events.is_empty(),
        "правка служебного места не должна порождать событий: {events:?}"
    );
    // Сам файл при этом читается как есть — при следующем открытии vault
    // приложение увидит правку.
    assert_eq!(
        read_vault_settings(&backend)
            .await
            .unwrap()
            .editor_font_size,
        Some(22)
    );
}

// ---------------------------------------------------------------------------
// Фича 016 — изображения vault для сопоставления вложений (T008)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_images_walks_the_vault_or_one_folder_like_the_web_backend() {
    use typsidian_core::vault::ImageFile;
    let dir = TempDir::new().unwrap();
    write_note(dir.path(), "a.png", "png");
    write_note(dir.path(), "Папка/b.JPG", "jpeg");
    write_note(dir.path(), "Папка/вложено/c.svg", "<svg>");
    write_note(dir.path(), "Папка/note.typ", "= Заметка");
    write_note(dir.path(), "Папка/.attachments/f.gif", "GIF89a");
    write_note(dir.path(), ".hidden/d.png", "скрыто");
    write_note(dir.path(), ".typsidian/e.png", "служебное");
    let backend = NativeBackend::open(dir.path()).await.unwrap();
    let image = |path: &str, size: u64| ImageFile {
        path: PathBuf::from(path),
        size,
    };

    assert_eq!(
        backend.list_images(None).await,
        vec![
            image("a.png", 3),
            image("Папка/b.JPG", 4),
            image("Папка/вложено/c.svg", 5),
        ],
        "весь vault: скрытые папки и служебное место пропущены, регистр расширения не важен"
    );
    assert_eq!(
        backend.list_images(Some(Path::new("Папка"))).await,
        vec![image("Папка/b.JPG", 4)],
        "одна папка без вложенных"
    );
    assert_eq!(
        backend
            .list_images(Some(Path::new("Папка/.attachments")))
            .await,
        vec![image("Папка/.attachments/f.gif", 6)],
        "скрытая папка, названная явно, читается"
    );
    assert!(backend.list_images(Some(Path::new("нет"))).await.is_empty());
}
