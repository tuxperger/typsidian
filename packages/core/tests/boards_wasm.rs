//! wasm-bindgen-test — только wasm32-unknown-unknown (принцип II). Парный файл
//! — `boards_native.rs`: проверки совпадают дословно, включая эталонный хэш.
//! Это и есть проверка паритета: один и тот же вход обязан давать один и тот
//! же `BoardDigest` на обоих таргетах.
#![cfg(target_arch = "wasm32")]

use typsidian_core::boards::{digest, BoardContent, DigestError, EmbedState};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

const REFERENCE_BOARD: &[u8] =
    br#"{"type":"excalidraw","elements":[{"id":"a","type":"rectangle","x":1,"y":2}],"files":{}}"#;
const REFERENCE_DIGEST: &str = "ca0dc52f87f42b3b78ed05df121aeb01c0768bf4d966d75d13ea655be59d8153";

#[wasm_bindgen_test]
fn digest_of_the_reference_board_is_stable_across_targets() {
    assert_eq!(digest(REFERENCE_BOARD).unwrap().as_str(), REFERENCE_DIGEST);
}

#[wasm_bindgen_test]
fn revision_counters_do_not_change_the_digest() {
    let touched = br#"{"type":"excalidraw","elements":[{"id":"a","type":"rectangle","x":1,"y":2,"version":42,"versionNonce":777,"updated":1700000000000,"seed":9}],"files":{}}"#;
    assert_eq!(
        digest(touched).unwrap().as_str(),
        REFERENCE_DIGEST,
        "счётчики ревизий не являются содержимым"
    );
}

#[wasm_bindgen_test]
fn geometry_is_content() {
    let moved =
        br#"{"type":"excalidraw","elements":[{"id":"a","type":"rectangle","x":300,"y":2}],"files":{}}"#;
    assert_ne!(digest(moved).unwrap().as_str(), REFERENCE_DIGEST);
}

#[wasm_bindgen_test]
fn broken_board_is_reported_and_not_hashed() {
    assert_eq!(digest(b"{ not json"), Err(DigestError::Unreadable));
}

#[wasm_bindgen_test]
fn board_content_hashes_through_the_same_normalization() {
    let content = BoardContent::new(REFERENCE_BOARD.to_vec());
    assert_eq!(content.digest().unwrap().as_str(), REFERENCE_DIGEST);
}

#[wasm_bindgen_test]
fn embed_state_never_reports_up_to_date_without_a_comparison() {
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
// User Story 2 — вставка доски в текстовую заметку (T036-T039)
//
// Проверки совпадают с `boards_native.rs` дословно: разбор текста, имена файлов
// и метаданные снимка платформенно нейтральны, и расхождение здесь было бы
// дефектом. Проверки, требующие файловой системы (T040), живут только в
// native-файле — в headless-браузере vault не смонтировать.
// ---------------------------------------------------------------------------

use std::path::{Path, PathBuf};
use typsidian_core::boards::{
    embed_markup, embed_metadata_in_svg, embeds_of, plan_embed, read_metadata_from_svg,
    snapshot_metadata, EmbedError,
};

const NOTE: &str = "Идея.typ";
const BOARD_FILE: &str = "Схема.excalidraw";

/// Планирование вставки с политикой по умолчанию — «рядом с заметкой»
/// (006 FR-003), без помеченных разделов.
///
/// Тесты этой фичи проверяют поведение вставок как таковых, а не выбор
/// расположения: за него отвечает `placement_wasm.rs`.
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

#[wasm_bindgen_test]
fn every_embed_gets_its_own_snapshot_file() {
    let first = plan_next_to_note(Path::new(NOTE), "", Path::new(BOARD_FILE)).unwrap();
    let text = embed_markup(&first);
    let second = plan_next_to_note(Path::new(NOTE), &text, Path::new(BOARD_FILE)).unwrap();
    let other_note = plan_next_to_note(Path::new("Другая.typ"), "", Path::new(BOARD_FILE)).unwrap();

    assert_ne!(first.embed_id, second.embed_id, "две вставки в одну заметку");
    assert_ne!(first.snapshot_path, second.snapshot_path);
    assert_ne!(first.embed_id, other_note.embed_id, "вставки в разные заметки");
    assert_ne!(first.snapshot_path, other_note.snapshot_path);
}

#[wasm_bindgen_test]
fn an_embed_is_recognised_by_its_address_not_by_a_marker() {
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

#[wasm_bindgen_test]
fn markup_is_an_ordinary_typst_image_call() {
    let plan = plan_next_to_note(Path::new(NOTE), "", Path::new(BOARD_FILE)).unwrap();
    let markup = embed_markup(&plan);
    assert_eq!(markup, format!("#image(\"{}\")", plan.markup_path));
    assert!(!markup.to_lowercase().contains("typsidian"));
    assert!(!markup.contains("//") && !markup.contains("<!--"));
}

#[wasm_bindgen_test]
fn snapshot_metadata_survives_a_round_trip() {
    let board_digest = digest(REFERENCE_BOARD).unwrap();
    let metadata = snapshot_metadata(Path::new("Проект/Схема.excalidraw"), &board_digest);
    let svg = embed_metadata_in_svg("<svg></svg>", &metadata).unwrap();

    let read_back = read_metadata_from_svg(&svg).unwrap();
    assert_eq!(read_back.board_path, PathBuf::from("Проект/Схема.excalidraw"));
    assert_eq!(read_back.board_digest, board_digest);
}

#[wasm_bindgen_test]
fn a_board_is_not_a_valid_host_for_an_embed() {
    assert_eq!(
        plan_next_to_note(Path::new(BOARD_FILE), "", Path::new("Другая.excalidraw")),
        Err(EmbedError::NotANote)
    );
}

// ---------------------------------------------------------------------------
// User Story 3 — платформенно нейтральная часть актуальности (T049-T056)
//
// Пересчёт состояния читает файлы и потому проверяется в `boards_native.rs`.
// Здесь — правила, которые обязаны совпасть на обоих таргетах: сравнение хэшей,
// устойчивость к счётчикам ревизий движка и правило «когда пересчитывать».
// ---------------------------------------------------------------------------

use typsidian_core::boards::{embeds_changed, BoardEvent};

const NOTE_WITH_EMBED: &str = "#image(\"Идея.assets/board-aaaaaaaa.svg\")";

#[wasm_bindgen_test]
fn opening_a_board_without_drawing_does_not_change_its_digest() {
    // T050 — ключевое свойство нормализации на стороне WASM: тот же вход, тот
    // же хэш, что и на native (research.md §5).
    let original = br#"{"type":"excalidraw","elements":[{"id":"a","type":"rectangle","x":1,"y":2,"version":7,"versionNonce":111,"updated":1700000000000,"seed":42}],"appState":{"scrollX":0},"files":{}}"#;
    let touched = br#"{"type":"excalidraw","elements":[{"id":"a","type":"rectangle","x":1,"y":2,"version":9,"versionNonce":999,"updated":1700000009999,"seed":4242}],"appState":{"scrollX":512},"files":{}}"#;
    assert_eq!(digest(original).unwrap(), digest(touched).unwrap());
}

#[wasm_bindgen_test]
fn a_changed_board_notifies_the_note_that_embeds_it() {
    // T056.
    assert_eq!(
        embeds_changed(
            Path::new("Идея.typ"),
            NOTE_WITH_EMBED,
            Path::new("Схема.excalidraw")
        ),
        Some(BoardEvent::EmbedsChanged {
            note_path: PathBuf::from("Идея.typ")
        })
    );
    assert_eq!(
        embeds_changed(
            Path::new("Идея.typ"),
            NOTE_WITH_EMBED,
            Path::new("Другая.typ")
        ),
        None
    );
    assert_eq!(
        embeds_changed(
            Path::new("Идея.typ"),
            "текст без вставок",
            Path::new("Схема.excalidraw")
        ),
        None
    );
}

// ---------------------------------------------------------------------------
// Мобильный просмотр — гейт записи на стороне WASM (T067)
//
// Парная проверка к `a_view_only_profile_has_no_write_path_at_all` из
// `boards_native.rs`. Правило записи, существующее только на одном таргете,
// нарушает принцип II, а contracts/board-store-trait.md § Гейты записи требует
// все три гейта на обоих; профильный гейт — тот самый, которым мобильный
// просмотр отличается от desktop (research.md §8).
//
// Vault в headless-браузере не смонтировать, поэтому backend здесь — двойник, у
// которого падает **каждый** метод. Это и есть содержание проверки: в профиле
// просмотра запись обязана быть отклонена, ни разу не тронув backend. Если
// профиль однажды начнут проверять после обращения к файлу, тест упадёт на
// `unreachable!` вместо того, чтобы молча разойтись с native.
// ---------------------------------------------------------------------------

use typsidian_core::boards::{write_board, BoardWriteProfile, WriteBoardError};
use typsidian_core::stamp::FileStamp;
use typsidian_core::vault::backend::{
    CreateError, DeleteError, OpenError, ReadError, RenameError, ResolveError, VaultBackend,
    WriteError,
};
use typsidian_core::vault::conflict::{DiffLine, EditConflict};
use typsidian_core::vault::events::VaultEvent;
use typsidian_core::vault::index::Note;
use typsidian_core::vault::AccessState;

/// Backend, к которому обращаться нельзя: любой вызов — провал теста.
struct UntouchableVault;

impl VaultBackend for UntouchableVault {
    async fn open(_root_path: &Path) -> Result<Self, OpenError> {
        unreachable!("гейт профиля не открывает vault")
    }

    fn root_path(&self) -> &Path {
        unreachable!("гейт профиля не спрашивает корень vault")
    }

    fn access_state(&self) -> AccessState {
        unreachable!("отказ по профилю не зависит от доступности vault")
    }

    fn service_dir_existed_at_open(&self) -> bool {
        unreachable!("отказ по профилю не зависит от служебного места")
    }

    async fn write_bytes(&mut self, _path: &Path, _bytes: &[u8]) -> Result<(), WriteError> {
        unreachable!("гейт профиля отклоняет запись до обращения к диску")
    }

    async fn list(&self) -> Vec<Note> {
        unreachable!("гейт профиля не читает дерево")
    }

    async fn list_folder(&self, _folder: &Path) -> Vec<Note> {
        unreachable!("гейт профиля не читает дерево")
    }

    async fn folder_access(&self, _folder: &Path) -> typsidian_core::vault::FolderAccess {
        unreachable!("гейт профиля не спрашивает про папки")
    }

    async fn read(&self, _path: &Path) -> Result<String, ReadError> {
        unreachable!("гейт профиля не читает файлы")
    }

    async fn read_bytes(&self, _path: &Path) -> Result<Vec<u8>, ReadError> {
        unreachable!("гейт профиля не читает файлы")
    }

    async fn write(
        &mut self,
        _path: &Path,
        _content: &str,
        _expected: Option<FileStamp>,
    ) -> Result<(), WriteError> {
        unreachable!("FR-035: в профиле просмотра записи не происходит")
    }

    async fn create(&mut self, _path: &Path) -> Result<(), CreateError> {
        unreachable!("гейт профиля не создаёт файлов")
    }

    async fn ensure_folder(&mut self, _path: &Path) -> Result<(), WriteError> {
        unreachable!("гейт профиля не создаёт папок")
    }

    async fn rename(&mut self, _from: &Path, _to: &Path) -> Result<(), RenameError> {
        unreachable!("гейт профиля ничего не переименовывает")
    }

    async fn delete(&mut self, _path: &Path) -> Result<(), DeleteError> {
        unreachable!("гейт профиля ничего не удаляет")
    }

    async fn backlinks(&self, _path: &Path) -> Vec<Note> {
        unreachable!("гейт профиля не трогает индекс")
    }

    async fn broken_links(&self, _path: &Path) -> Vec<PathBuf> {
        unreachable!("гейт профиля не трогает индекс")
    }

    fn poll_events(&mut self) -> Vec<VaultEvent> {
        unreachable!("гейт профиля не читает события")
    }

    async fn begin_edit(&mut self, _path: &Path, _buffer: String) {
        unreachable!("просмотр не объявляет правок")
    }

    async fn discard_edit(&mut self, _path: &Path) {
        unreachable!("просмотр не объявляет правок")
    }

    fn has_unsaved_edits(&self, _path: &Path) -> bool {
        unreachable!("просмотр не объявляет правок")
    }

    fn pending_conflict(&self, _path: &Path) -> Option<EditConflict> {
        unreachable!("профиль проверяется раньше конфликта")
    }

    fn unresolved_edit_paths(&self) -> Vec<PathBuf> {
        unreachable!("просмотр не объявляет правок")
    }

    async fn conflict_diff(&self, _path: &Path) -> Vec<DiffLine> {
        unreachable!("в профиле просмотра конфликт не возникает")
    }

    async fn resolve_conflict_keep_local(&mut self, _path: &Path) -> Result<(), ResolveError> {
        unreachable!("в профиле просмотра конфликт не разрешается")
    }

    async fn resolve_conflict_accept_external(
        &mut self,
        _path: &Path,
    ) -> Result<Option<String>, ResolveError> {
        unreachable!("в профиле просмотра конфликт не разрешается")
    }
}

#[wasm_bindgen_test]
async fn a_view_only_profile_has_no_write_path_at_all() {
    // FR-035, research.md §8: гарантия лежит в ядре, а не во флаге интерфейса.
    let mut backend = UntouchableVault;

    let attempt = write_board(
        &mut backend,
        BoardWriteProfile::ViewOnly,
        Path::new("Схема.excalidraw"),
        &BoardContent::new(br#"{"elements":[{"id":"x"}]}"#.to_vec()),
        None,
    )
    .await;

    assert_eq!(attempt, Err(WriteBoardError::ReadOnlyProfile));
}

#[wasm_bindgen_test]
async fn a_view_only_profile_is_refused_even_with_a_matching_stamp() {
    // Совпадающий `expected` — тот случай, в котором запись прошла бы при любом
    // другом гейте. Профиль обязан отклонить и его: просмотр не пишет никогда,
    // а не «когда есть расхождение».
    let mut backend = UntouchableVault;
    let content = BoardContent::new(REFERENCE_BOARD.to_vec());

    let attempt = write_board(
        &mut backend,
        BoardWriteProfile::ViewOnly,
        Path::new("Схема.excalidraw"),
        &content,
        Some(FileStamp::from_content(0, REFERENCE_BOARD)),
    )
    .await;

    assert_eq!(attempt, Err(WriteBoardError::ReadOnlyProfile));
}
