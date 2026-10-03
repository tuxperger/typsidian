//! cargo test — только native (принцип II). Парный файл — `boards_wasm.rs`:
//! проверки в них совпадают дословно, потому что логика досок платформенно
//! нейтральна и расхождение результата между таргетами является дефектом, а не
//! особенностью платформы.
#![cfg(not(target_arch = "wasm32"))]

use typsidian_core::boards::{digest, BoardContent, DigestError, EmbedState};

/// Эталонная доска и её хэш. Литерал, а не вычисление на месте: он и есть
/// проверка паритета — тот же байт-в-байт хэш ожидается в `boards_wasm.rs`.
/// Если реализация на одном из таргетов разойдётся, упадёт ровно один из двух
/// тестов, и станет видно, какой именно.
const REFERENCE_BOARD: &[u8] =
    br#"{"type":"excalidraw","elements":[{"id":"a","type":"rectangle","x":1,"y":2}],"files":{}}"#;
const REFERENCE_DIGEST: &str = "ca0dc52f87f42b3b78ed05df121aeb01c0768bf4d966d75d13ea655be59d8153";

#[test]
fn digest_of_the_reference_board_is_stable_across_targets() {
    assert_eq!(digest(REFERENCE_BOARD).unwrap().as_str(), REFERENCE_DIGEST);
}

#[test]
fn revision_counters_do_not_change_the_digest() {
    // research.md §5: движок трогает `version`/`versionNonce`/`updated`/`seed`
    // при любом касании. Если они попадут в хэш, индикатор устаревания
    // загорится от простого открытия доски, и SC-008 станет бесполезным.
    let touched = br#"{"type":"excalidraw","elements":[{"id":"a","type":"rectangle","x":1,"y":2,"version":42,"versionNonce":777,"updated":1700000000000,"seed":9}],"files":{}}"#;
    assert_eq!(
        digest(touched).unwrap().as_str(),
        REFERENCE_DIGEST,
        "счётчики ревизий не являются содержимым"
    );
}

#[test]
fn geometry_is_content() {
    let moved =
        br#"{"type":"excalidraw","elements":[{"id":"a","type":"rectangle","x":300,"y":2}],"files":{}}"#;
    assert_ne!(digest(moved).unwrap().as_str(), REFERENCE_DIGEST);
}

#[test]
fn broken_board_is_reported_and_not_hashed() {
    // FR-017: повреждённый файл не читается и не перезаписывается.
    assert_eq!(digest(b"{ not json"), Err(DigestError::Unreadable));
}

#[test]
fn board_content_hashes_through_the_same_normalization() {
    let content = BoardContent::new(REFERENCE_BOARD.to_vec());
    assert_eq!(content.digest().unwrap().as_str(), REFERENCE_DIGEST);
}

#[test]
fn embed_state_never_reports_up_to_date_without_a_comparison() {
    // SC-008: молчаливое расхождение запрещено. Отсутствие метаданных в снимке
    // даёт отдельное состояние, а не «актуально».
    let board = digest(REFERENCE_BOARD).unwrap();
    assert!(matches!(
        EmbedState::compare(Some(&board), None),
        EmbedState::Undeterminable { .. }
    ));
    assert_eq!(EmbedState::compare(None, None), EmbedState::BoardMissing);
    assert_eq!(
        EmbedState::compare(Some(&board), Some(&board.clone())),
        EmbedState::UpToDate
    );
}

// ---------------------------------------------------------------------------
// User Story 1 — доска как файл vault (T016-T024)
// ---------------------------------------------------------------------------

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use typsidian_core::boards::{
    create_board, read_board, write_board, BoardWriteProfile, CreateBoardError, ReadBoardError,
    WriteBoardError,
};
use typsidian_core::stamp::FileStamp;
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::native::NativeBackend;
use typsidian_core::vault::{classify, FileKind};

async fn open_vault(dir: &TempDir) -> NativeBackend {
    NativeBackend::open(dir.path())
        .await
        .expect("открытие vault")
}

#[tokio::test]
async fn created_board_is_a_board_and_appears_in_the_tree() {
    // FR-001, FR-004. Без включения досок в `list()` (T008) этот тест ловит
    // ровно ту дыру, которую нашёл /speckit-analyze.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;

    let board = create_board(&mut backend, Path::new("Схема.excalidraw"))
        .await
        .expect("создание доски");
    assert_eq!(board.name, "Схема.excalidraw");
    assert_eq!(classify(&board.path), FileKind::Board);

    let paths: Vec<String> = backend
        .list()
        .await
        .iter()
        .map(|note| note.path.to_string_lossy().to_string())
        .collect();
    assert!(paths.contains(&"Схема.excalidraw".to_string()), "{paths:?}");
    assert!(dir.path().join("Схема.excalidraw").exists());
}

#[tokio::test]
async fn a_note_name_is_not_accepted_as_a_board_name() {
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    assert_eq!(
        create_board(&mut backend, Path::new("Заметка.typ")).await,
        Err(CreateBoardError::NotABoardName)
    );
    assert!(!dir.path().join("Заметка.typ").exists());
}

#[tokio::test]
async fn boards_are_renamed_and_deleted_by_the_same_calls_as_notes() {
    // FR-003: отдельных операций для доски не заводится — работают те же
    // `VaultBackend::rename` и `delete`, что и для заметок.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new("Схема.excalidraw"))
        .await
        .unwrap();

    backend
        .rename(
            Path::new("Схема.excalidraw"),
            Path::new("Другая.excalidraw"),
        )
        .await
        .expect("переименование доски");
    assert!(dir.path().join("Другая.excalidraw").exists());
    assert!(!dir.path().join("Схема.excalidraw").exists());

    backend
        .delete(Path::new("Другая.excalidraw"))
        .await
        .expect("удаление доски");
    assert!(!dir.path().join("Другая.excalidraw").exists());
    assert!(backend.list().await.is_empty());
}

#[tokio::test]
async fn a_board_created_outside_the_app_shows_up_without_a_restart() {
    // FR-005: внешние изменения доски идут тем же потоком событий, что и для
    // заметок — своего вотчера фича не заводит.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    assert!(backend.list().await.is_empty());

    fs::write(dir.path().join("Снаружи.excalidraw"), "{\"elements\":[]}").unwrap();

    // Обнаружение асинхронно (вотчер + дебаунсер), единого момента «готово» у
    // него нет — опрашиваем до появления или до истечения бюджета SC-004.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut paths: Vec<String> = Vec::new();
    while Instant::now() < deadline {
        let _ = backend.poll_events();
        paths = backend
            .list()
            .await
            .iter()
            .map(|note| note.path.to_string_lossy().to_string())
            .collect();
        if paths.contains(&"Снаружи.excalidraw".to_string()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert!(
        paths.contains(&"Снаружи.excalidraw".to_string()),
        "доска, созданная снаружи, должна появиться в дереве: {paths:?}"
    );
}

#[tokio::test]
async fn write_with_a_stale_stamp_is_refused_and_the_file_is_untouched() {
    // FR-014: расхождение снимка — конфликт, а не молчаливая перезапись.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new("Схема.excalidraw"))
        .await
        .unwrap();
    let on_disk = fs::read_to_string(dir.path().join("Схема.excalidraw")).unwrap();

    let stale = FileStamp::from_content(1, "совсем другое содержимое".as_bytes());
    let attempt = write_board(
        &mut backend,
        BoardWriteProfile::ReadWrite,
        Path::new("Схема.excalidraw"),
        &typsidian_core::boards::BoardContent::new(br#"{"elements":[{"id":"x"}]}"#.to_vec()),
        Some(stale),
    )
    .await;

    assert_eq!(attempt, Err(WriteBoardError::Conflicted));
    assert_eq!(
        fs::read_to_string(dir.path().join("Схема.excalidraw")).unwrap(),
        on_disk,
        "файл не должен быть тронут"
    );
}

#[tokio::test]
async fn a_view_only_profile_has_no_write_path_at_all() {
    // FR-035, research.md §8: гарантия лежит в ядре, а не во флаге интерфейса.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new("Схема.excalidraw"))
        .await
        .unwrap();
    let on_disk = fs::read_to_string(dir.path().join("Схема.excalidraw")).unwrap();

    let attempt = write_board(
        &mut backend,
        BoardWriteProfile::ViewOnly,
        Path::new("Схема.excalidraw"),
        &typsidian_core::boards::BoardContent::new(br#"{"elements":[{"id":"x"}]}"#.to_vec()),
        None,
    )
    .await;

    assert_eq!(attempt, Err(WriteBoardError::ReadOnlyProfile));
    assert_eq!(
        fs::read_to_string(dir.path().join("Схема.excalidraw")).unwrap(),
        on_disk
    );
}

#[tokio::test]
async fn a_broken_board_is_reported_and_never_overwritten() {
    // FR-017.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("Битая.excalidraw"), "{ это не json").unwrap();
    let backend = open_vault(&dir).await;

    assert_eq!(
        read_board(&backend, Path::new("Битая.excalidraw")).await,
        Err(ReadBoardError::Unreadable)
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("Битая.excalidraw")).unwrap(),
        "{ это не json",
        "нечитаемый файл не перезаписывается"
    );
}

#[tokio::test]
async fn a_missing_board_is_reported_separately_from_a_broken_one() {
    let dir = TempDir::new().unwrap();
    let backend = open_vault(&dir).await;
    assert_eq!(
        read_board(&backend, Path::new("Нет.excalidraw")).await,
        Err(ReadBoardError::NotFound)
    );
}

#[tokio::test]
async fn an_unresolved_conflict_blocks_every_write_including_a_forced_flush() {
    // FR-014, инвариант 1 data-model.md. Дебаунс автосохранения принудительно
    // сбрасывается перед переключением заметки и закрытием окна — и даже этот
    // путь не должен пробить гейт конфликта.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new("Схема.excalidraw"))
        .await
        .unwrap();

    backend
        .begin_edit(
            Path::new("Схема.excalidraw"),
            r#"{"elements":[{"id":"moy"}]}"#.to_string(),
        )
        .await;
    fs::write(
        dir.path().join("Схема.excalidraw"),
        r#"{"elements":[{"id":"vneshniy"}]}"#,
    )
    .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let _ = backend.poll_events();
        if backend
            .pending_conflict(Path::new("Схема.excalidraw"))
            .is_some()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        backend
            .pending_conflict(Path::new("Схема.excalidraw"))
            .is_some(),
        "конфликт должен быть поднят"
    );

    let external = fs::read_to_string(dir.path().join("Схема.excalidraw")).unwrap();
    let attempt = write_board(
        &mut backend,
        BoardWriteProfile::ReadWrite,
        Path::new("Схема.excalidraw"),
        &typsidian_core::boards::BoardContent::new(br#"{"elements":[{"id":"moy"}]}"#.to_vec()),
        None,
    )
    .await;

    assert_eq!(attempt, Err(WriteBoardError::Conflicted));
    assert_eq!(
        fs::read_to_string(dir.path().join("Схема.excalidraw")).unwrap(),
        external,
        "до разрешения конфликта в файл не пишется ничего"
    );
}

#[tokio::test]
async fn unsaved_board_edits_survive_the_file_disappearing() {
    // spec.md Edge Cases: несохранённые изменения доски не теряются, если vault
    // стал недоступен во время рисования. Правило то же, что для заметок в 001
    // (FR-035): буфер живёт в памяти сессии и ядром не выбрасывается.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new("Схема.excalidraw"))
        .await
        .unwrap();

    backend
        .begin_edit(
            Path::new("Схема.excalidraw"),
            r#"{"elements":[{"id":"narisovannoe"}]}"#.to_string(),
        )
        .await;
    assert!(backend.has_unsaved_edits(Path::new("Схема.excalidraw")));

    fs::remove_file(dir.path().join("Схема.excalidraw")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let _ = backend.poll_events();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert!(
        backend.has_unsaved_edits(Path::new("Схема.excalidraw")),
        "буфер не должен выбрасываться вместе с файлом"
    );
}

// ---------------------------------------------------------------------------
// User Story 2 — вставка доски в текстовую заметку (T036-T040)
// ---------------------------------------------------------------------------

use std::path::PathBuf;
use typsidian_core::boards::{
    embed_markup, embeds_of, plan_embed, read_metadata_from_svg, read_snapshot_metadata,
    snapshot_metadata, write_snapshot, EmbedError,
};

const NOTE: &str = "Идея.typ";
const BOARD_FILE: &str = "Схема.excalidraw";

/// Планирование вставки с политикой по умолчанию — «рядом с заметкой»
/// (006 FR-003), без помеченных разделов.
///
/// Тесты этой фичи проверяют поведение вставок как таковых, а не выбор
/// расположения: за него отвечает `placement_native.rs`.
fn plan_next_to_note(
    note: &Path,
    text: &str,
    board: &Path,
) -> Result<typsidian_core::boards::EmbedPlan, EmbedError> {
    plan_embed(
        note,
        text,
        board,
        typsidian_core::boards::AssetsPlacement::NextToNote,
        &[],
    )
}

#[test]
fn every_embed_gets_its_own_snapshot_file() {
    // T036, FR-020, FR-023: идентификатор уникален для вставки, а не для доски.
    let first = plan_next_to_note(Path::new(NOTE), "", Path::new(BOARD_FILE)).unwrap();
    let text = embed_markup(&first);
    let second = plan_next_to_note(Path::new(NOTE), &text, Path::new(BOARD_FILE)).unwrap();
    let other_note = plan_next_to_note(Path::new("Другая.typ"), "", Path::new(BOARD_FILE)).unwrap();

    assert_ne!(
        first.embed_id, second.embed_id,
        "две вставки в одну заметку"
    );
    assert_ne!(first.snapshot_path, second.snapshot_path);
    assert_ne!(
        first.embed_id, other_note.embed_id,
        "вставки в разные заметки"
    );
    assert_ne!(first.snapshot_path, other_note.snapshot_path);
}

#[test]
fn an_embed_is_recognised_by_its_address_not_by_a_marker() {
    // T037, FR-026, research.md §11.
    let text = concat!(
        "Начало\n",
        "#image(\"Идея.assets/board-aaaaaaaa.svg\")\n",
        "#image(\"Фото.png\")\n",
        "#image(\"Идея.assets/board-bbbbbbbb.svg\", width: 60%)\n",
    );
    let found = embeds_of(Path::new(NOTE), text);

    assert_eq!(found.len(), 2, "обычная картинка вставкой не считается");
    assert_eq!(found[0].id.as_str(), "board-aaaaaaaa");
    assert_eq!(found[1].id.as_str(), "board-bbbbbbbb");
    for one in &found {
        let call = &text[one.range.start..one.range.end];
        assert!(call.starts_with("#image("), "{call}");
        assert!(call.ends_with(')'), "{call}");
        assert!(call.contains(one.id.as_str()), "{call}");
    }
}

#[test]
fn markup_is_an_ordinary_typst_image_call() {
    // T038, FR-022: ни одного маркера приложения в тексте заметки.
    let plan = plan_next_to_note(Path::new(NOTE), "", Path::new(BOARD_FILE)).unwrap();
    let markup = embed_markup(&plan);
    assert_eq!(markup, format!("#image(\"{}\")", plan.markup_path));
    assert!(!markup.to_lowercase().contains("typsidian"));
    assert!(!markup.contains("//") && !markup.contains("<!--"));
}

#[test]
fn snapshot_metadata_survives_a_round_trip() {
    // T039, data-model.md «Snapshot».
    let board_digest = digest(REFERENCE_BOARD).unwrap();
    let metadata = snapshot_metadata(Path::new("Проект/Схема.excalidraw"), &board_digest);
    let svg = typsidian_core::boards::embed_metadata_in_svg("<svg></svg>", &metadata).unwrap();

    let read_back = read_metadata_from_svg(&svg).unwrap();
    assert_eq!(
        read_back.board_path,
        PathBuf::from("Проект/Схема.excalidraw")
    );
    assert_eq!(read_back.board_digest, board_digest);
}

#[test]
fn a_board_is_not_a_valid_host_for_an_embed() {
    assert_eq!(
        plan_next_to_note(Path::new(BOARD_FILE), "", Path::new("Другая.excalidraw")),
        Err(EmbedError::NotANote)
    );
}

#[tokio::test]
async fn a_snapshot_is_written_next_to_its_note_and_read_back() {
    // T039 на файлах: путь из research.md §4 плюс метаданные внутри файла.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new(BOARD_FILE))
        .await
        .unwrap();
    backend.create(Path::new(NOTE)).await.unwrap();

    let plan = plan_next_to_note(Path::new(NOTE), "", Path::new(BOARD_FILE)).unwrap();
    let metadata = snapshot_metadata(Path::new(BOARD_FILE), &digest(REFERENCE_BOARD).unwrap());
    write_snapshot(&mut backend, &plan.snapshot_path, "<svg></svg>", &metadata)
        .await
        .expect("запись снимка");

    // Папка снимков создаётся первой же вставкой — без этого путь из §4
    // недостижим ни на одном таргете.
    assert!(dir.path().join(&plan.snapshot_path).exists());
    assert_eq!(
        read_snapshot_metadata(&backend, &plan.snapshot_path)
            .await
            .unwrap(),
        metadata
    );

    // FR-021: снимок остаётся обычным файлом vault, но в дереве его нет.
    let paths: Vec<PathBuf> = backend.list().await.into_iter().map(|n| n.path).collect();
    assert!(!paths.contains(&plan.snapshot_path), "{paths:?}");
    assert_eq!(classify(&plan.snapshot_path), FileKind::Snapshot);
}

#[tokio::test]
async fn removing_the_call_from_the_note_touches_neither_the_board_nor_the_snapshot() {
    // T040, FR-024, инвариант 4 data-model.md.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new(BOARD_FILE))
        .await
        .unwrap();
    backend.create(Path::new(NOTE)).await.unwrap();

    let plan = plan_next_to_note(Path::new(NOTE), "", Path::new(BOARD_FILE)).unwrap();
    let metadata = snapshot_metadata(Path::new(BOARD_FILE), &digest(REFERENCE_BOARD).unwrap());
    write_snapshot(&mut backend, &plan.snapshot_path, "<svg></svg>", &metadata)
        .await
        .unwrap();
    let with_embed = format!("Текст\n{}\n", embed_markup(&plan));
    backend
        .write(Path::new(NOTE), &with_embed, None)
        .await
        .unwrap();

    let board_before = fs::read_to_string(dir.path().join(BOARD_FILE)).unwrap();
    let snapshot_before = fs::read_to_string(dir.path().join(&plan.snapshot_path)).unwrap();

    // Пользователь убрал вызов из текста — руками или сторонним редактором.
    backend
        .write(Path::new(NOTE), "Текст без вставки\n", None)
        .await
        .unwrap();

    assert!(embeds_of(Path::new(NOTE), "Текст без вставки\n").is_empty());
    assert_eq!(
        fs::read_to_string(dir.path().join(BOARD_FILE)).unwrap(),
        board_before,
        "доска не тронута"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(&plan.snapshot_path)).unwrap(),
        snapshot_before,
        "файл снимка не удаляется автоматически"
    );
}

// ---------------------------------------------------------------------------
// User Story 3 — актуальность вставки (T049-T054)
//
// Проверки состояния вставки читают файлы, поэтому живут только здесь: vault в
// headless-браузере не смонтировать (то же ограничение, что у T016-T024).
// Платформенно нейтральная часть — сравнение хэшей, разбор метаданных и правило
// пересчёта — покрыта в `boards_wasm.rs` и в модульных тестах ядра на обоих
// таргетах.
// ---------------------------------------------------------------------------

use typsidian_core::boards::{embed_state, embeds_changed, embeds_with_state, BoardEvent};

/// Вставка доски в заметку целиком: доска, снимок, текст заметки.
/// Возвращает текст заметки и путь снимка.
async fn embed_board(
    backend: &mut NativeBackend,
    note: &str,
    board: &str,
    board_content: &str,
) -> (String, PathBuf) {
    let plan = plan_next_to_note(Path::new(note), "", Path::new(board)).unwrap();
    let metadata = snapshot_metadata(Path::new(board), &digest(board_content.as_bytes()).unwrap());
    write_snapshot(backend, &plan.snapshot_path, "<svg></svg>", &metadata)
        .await
        .unwrap();
    let text = format!("Текст\n{}\n", embed_markup(&plan));
    backend.write(Path::new(note), &text, None).await.unwrap();
    (text, plan.snapshot_path)
}

#[tokio::test]
async fn a_changed_board_makes_the_embed_stale() {
    // T049, FR-026, SC-008.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new(BOARD_FILE))
        .await
        .unwrap();
    backend.create(Path::new(NOTE)).await.unwrap();
    let board_at_embed_time = String::from_utf8(
        read_board(&backend, Path::new(BOARD_FILE))
            .await
            .unwrap()
            .as_bytes()
            .to_vec(),
    )
    .unwrap();

    let (_, snapshot) = embed_board(&mut backend, NOTE, BOARD_FILE, &board_at_embed_time).await;
    assert_eq!(
        embed_state(&backend, &snapshot, None).await,
        EmbedState::UpToDate
    );

    // Пользователь дорисовал на доске.
    fs::write(
        dir.path().join(BOARD_FILE),
        r#"{"type":"excalidraw","elements":[{"id":"новый","type":"ellipse"}]}"#,
    )
    .unwrap();
    assert_eq!(
        embed_state(&backend, &snapshot, None).await,
        EmbedState::Stale
    );
}

#[tokio::test]
async fn opening_a_board_without_drawing_keeps_the_embed_up_to_date() {
    // T050 — ключевой тест нормализации (research.md §5). Если счётчики ревизий
    // движка попадут в хэш, индикатор устаревания загорится от простого
    // открытия доски, и SC-008 станет бесполезным.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    let original = r#"{"type":"excalidraw","elements":[{"id":"a","type":"rectangle","x":1,"y":2,"version":7,"versionNonce":111,"updated":1700000000000,"seed":42}],"appState":{"scrollX":0},"files":{}}"#;
    fs::write(dir.path().join(BOARD_FILE), original).unwrap();
    backend.create(Path::new(NOTE)).await.unwrap();

    let (_, snapshot) = embed_board(&mut backend, NOTE, BOARD_FILE, original).await;
    assert_eq!(
        embed_state(&backend, &snapshot, None).await,
        EmbedState::UpToDate
    );

    // Открыли доску, ничего не нарисовали, закрыли: движок переписал счётчики
    // ревизий и подвинул камеру.
    let touched = original
        .replace("\"version\":7", "\"version\":9")
        .replace("\"versionNonce\":111", "\"versionNonce\":999")
        .replace("\"updated\":1700000000000", "\"updated\":1700000009999")
        .replace("\"seed\":42", "\"seed\":4242")
        .replace("\"scrollX\":0", "\"scrollX\":512");
    fs::write(dir.path().join(BOARD_FILE), &touched).unwrap();

    assert_eq!(
        embed_state(&backend, &snapshot, None).await,
        EmbedState::UpToDate,
        "открытие доски без правок не устаревает вставку"
    );
}

#[tokio::test]
async fn a_snapshot_without_metadata_is_undeterminable_not_up_to_date() {
    // T051, SC-008, spec.md Edge Cases: снимок заменён сторонним инструментом.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new(BOARD_FILE))
        .await
        .unwrap();
    backend.create(Path::new(NOTE)).await.unwrap();
    let plan = plan_next_to_note(Path::new(NOTE), "", Path::new(BOARD_FILE)).unwrap();

    fs::create_dir_all(dir.path().join(plan.snapshot_path.parent().unwrap())).unwrap();
    fs::write(
        dir.path().join(&plan.snapshot_path),
        "<svg><desc>чужая подпись</desc></svg>",
    )
    .unwrap();

    assert!(
        matches!(
            embed_state(&backend, &plan.snapshot_path, None).await,
            EmbedState::Undeterminable { .. }
        ),
        "молчаливое расхождение запрещено"
    );

    // Испорченный хэш — тоже «сравнить не удалось», а не «расходится»:
    // предлагать обновить снимок из-за мусора в атрибуте было бы неправдой.
    fs::write(
        dir.path().join(&plan.snapshot_path),
        "<svg><desc data-typsidian-board=\"Схема.excalidraw\" data-typsidian-board-hash=\"не хэш\"></desc></svg>",
    )
    .unwrap();
    assert!(matches!(
        embed_state(&backend, &plan.snapshot_path, None).await,
        EmbedState::Undeterminable { .. }
    ));
}

#[tokio::test]
async fn a_missing_board_and_a_missing_snapshot_stay_different_states() {
    // T052, FR-031, FR-032: у них разные действия восстановления, и схлопывать
    // их в одно состояние нельзя.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new(BOARD_FILE))
        .await
        .unwrap();
    backend.create(Path::new(NOTE)).await.unwrap();
    let board_content = String::from_utf8(
        read_board(&backend, Path::new(BOARD_FILE))
            .await
            .unwrap()
            .as_bytes()
            .to_vec(),
    )
    .unwrap();
    let (_, snapshot) = embed_board(&mut backend, NOTE, BOARD_FILE, &board_content).await;

    // Снимок на месте, доску удалили снаружи.
    fs::remove_file(dir.path().join(BOARD_FILE)).unwrap();
    assert_eq!(
        embed_state(&backend, &snapshot, None).await,
        EmbedState::BoardMissing
    );

    // Доска на месте, снимок удалили.
    fs::write(dir.path().join(BOARD_FILE), &board_content).unwrap();
    fs::remove_file(dir.path().join(&snapshot)).unwrap();
    assert_eq!(
        embed_state(&backend, &snapshot, Some(Path::new(BOARD_FILE))).await,
        EmbedState::SnapshotMissing
    );

    // Нет ни снимка, ни доски — отсутствует именно источник.
    fs::remove_file(dir.path().join(BOARD_FILE)).unwrap();
    assert_eq!(
        embed_state(&backend, &snapshot, Some(Path::new(BOARD_FILE))).await,
        EmbedState::BoardMissing
    );
}

#[tokio::test]
async fn updating_one_embed_leaves_the_other_snapshot_alone() {
    // T053, FR-029, инвариант 3 data-model.md.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new(BOARD_FILE))
        .await
        .unwrap();
    backend.create(Path::new(NOTE)).await.unwrap();
    let board_content = String::from_utf8(
        read_board(&backend, Path::new(BOARD_FILE))
            .await
            .unwrap()
            .as_bytes()
            .to_vec(),
    )
    .unwrap();

    // Две вставки одной доски в одну заметку.
    let first = plan_next_to_note(Path::new(NOTE), "", Path::new(BOARD_FILE)).unwrap();
    let text_with_first = embed_markup(&first);
    let second =
        plan_next_to_note(Path::new(NOTE), &text_with_first, Path::new(BOARD_FILE)).unwrap();
    let metadata = snapshot_metadata(
        Path::new(BOARD_FILE),
        &digest(board_content.as_bytes()).unwrap(),
    );
    write_snapshot(
        &mut backend,
        &first.snapshot_path,
        "<svg>первая</svg>",
        &metadata,
    )
    .await
    .unwrap();
    write_snapshot(
        &mut backend,
        &second.snapshot_path,
        "<svg>вторая</svg>",
        &metadata,
    )
    .await
    .unwrap();
    let second_before = fs::read_to_string(dir.path().join(&second.snapshot_path)).unwrap();

    // Обновляется только первая вставка.
    let changed = r#"{"type":"excalidraw","elements":[{"id":"новый"}]}"#;
    let updated_metadata =
        snapshot_metadata(Path::new(BOARD_FILE), &digest(changed.as_bytes()).unwrap());
    write_snapshot(
        &mut backend,
        &first.snapshot_path,
        "<svg>первая обновлённая</svg>",
        &updated_metadata,
    )
    .await
    .unwrap();

    assert!(fs::read_to_string(dir.path().join(&first.snapshot_path))
        .unwrap()
        .contains("первая обновлённая"));
    assert_eq!(
        fs::read_to_string(dir.path().join(&second.snapshot_path)).unwrap(),
        second_before,
        "снимок второй вставки не тронут"
    );
}

#[tokio::test]
async fn no_state_transition_rewrites_a_snapshot_by_itself() {
    // T054, FR-030, инвариант 2 data-model.md: снимок обновляется только явным
    // действием пользователя.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new(BOARD_FILE))
        .await
        .unwrap();
    backend.create(Path::new(NOTE)).await.unwrap();
    let board_content = String::from_utf8(
        read_board(&backend, Path::new(BOARD_FILE))
            .await
            .unwrap()
            .as_bytes()
            .to_vec(),
    )
    .unwrap();
    let (text, snapshot) = embed_board(&mut backend, NOTE, BOARD_FILE, &board_content).await;
    let snapshot_before = fs::read_to_string(dir.path().join(&snapshot)).unwrap();

    // Проводим вставку через все переходы состояния, какие достижимы снаружи.
    fs::write(
        dir.path().join(BOARD_FILE),
        r#"{"elements":[{"id":"новый"}]}"#,
    )
    .unwrap();
    let stale = embed_state(&backend, &snapshot, None).await;
    fs::remove_file(dir.path().join(BOARD_FILE)).unwrap();
    let missing = embed_state(&backend, &snapshot, None).await;
    let listed = embeds_with_state(&backend, Path::new(NOTE), &text).await;

    assert_eq!(stale, EmbedState::Stale);
    assert_eq!(missing, EmbedState::BoardMissing);
    assert_eq!(listed.len(), 1);
    assert_eq!(
        fs::read_to_string(dir.path().join(&snapshot)).unwrap(),
        snapshot_before,
        "ни один пересчёт состояния не пишет в файл снимка"
    );
}

#[tokio::test]
async fn a_changed_board_announces_the_notes_that_embed_it() {
    // T056: событие возникает и при изменении доски, а не только текста заметки.
    let dir = TempDir::new().unwrap();
    let mut backend = open_vault(&dir).await;
    create_board(&mut backend, Path::new(BOARD_FILE))
        .await
        .unwrap();
    backend.create(Path::new(NOTE)).await.unwrap();
    let board_content = String::from_utf8(
        read_board(&backend, Path::new(BOARD_FILE))
            .await
            .unwrap()
            .as_bytes()
            .to_vec(),
    )
    .unwrap();
    let (text, _) = embed_board(&mut backend, NOTE, BOARD_FILE, &board_content).await;

    assert_eq!(
        embeds_changed(Path::new(NOTE), &text, Path::new(BOARD_FILE)),
        Some(BoardEvent::EmbedsChanged {
            note_path: PathBuf::from(NOTE)
        })
    );
    assert_eq!(
        embeds_changed(Path::new(NOTE), &text, Path::new("Постороннее.typ")),
        None
    );
}
