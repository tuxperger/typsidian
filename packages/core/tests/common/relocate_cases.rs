//! Сценарии переноса элементов vault (фича 023) — общие для обоих таргетов.
//!
//! Работают по **настоящим** backend, а не по `MemoryVault`: перенос папки,
//! индекс, правки и таблица ссылок живут в `NativeBackend` и `WebBackend`, и
//! двойник проверил бы не то. Раннеры `relocate_native.rs` (временная папка) и
//! `relocate_wasm.rs` (папка в OPFS браузера) дают [`Env`], а сценарии одни.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;
use typsidian_core::boards::placement::AssetsPlacement;
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::events::VaultEvent;
use typsidian_core::vault::relocate::{
    MoveControl, MoveError, MoveOutcome, MoveRefusal, MoveRequest, NoControl, PathChange,
};
use typsidian_core::vault::vault_settings::{
    read_vault_settings, set_section, write_vault_settings, VaultOverrides,
};

/// Содержимое диска: файлы по пути через `/` → байты. Папки отдельно не
/// учитываются, служебная папка `.typsidian` не входит.
pub type Disk = BTreeMap<String, Vec<u8>>;

pub type Files<'a> = &'a [(&'a str, &'a [u8])];

/// Среда сценария: vault на диске таргета и открытый над ним backend.
pub trait Env: Sized {
    type Backend: VaultBackend;

    /// Свежий vault, в котором лежат ровно эти файлы (папки создаются сами).
    async fn open(files: Files<'_>) -> Self;

    /// То же, но vault открыт только для чтения; `None` — среда не умеет это
    /// изобразить (процесс обходит права файловой системы).
    async fn open_read_only(_files: Files<'_>) -> Option<Self> {
        None
    }

    /// Vault, в котором удаление записи с этим именем отказывает: то, чем
    /// проверяется `Partial` (только web, где перенос — копия с удалением).
    async fn open_failing_removal(_files: Files<'_>, _name: &str) -> Option<Self> {
        None
    }

    fn backend(&mut self) -> &mut Self::Backend;
    fn backend_ref(&self) -> &Self::Backend;

    /// Весь диск как есть — независимо от backend.
    async fn disk(&self) -> Disk;

    /// Меняет файл «снаружи»: мимо backend, как сторонняя программа.
    async fn write_outside(&mut self, path: &str, bytes: &[u8]);

    /// Даёт обнаружению внешних изменений отработать и возвращает все события,
    /// накопившиеся к этому моменту, — собственные и обнаруженные.
    async fn settle(&mut self) -> Vec<VaultEvent>;
}

fn mv(from: &str, to_folder: &str) -> MoveRequest {
    MoveRequest {
        from: from.into(),
        to_folder: to_folder.into(),
    }
}

/// Ожидает `Done` и возвращает его части.
fn done(result: Result<MoveOutcome, MoveError>) -> (Vec<PathChange>, Vec<PathChange>, MoveRequest) {
    match result {
        Ok(MoveOutcome::Done {
            moved,
            folders,
            undo,
        }) => (moved, folders, undo),
        other => panic!("ожидалось Done, получено {other:?}"),
    }
}

async fn refusal<E: Env>(env: &mut E, from: &str, to_folder: &str) -> MoveRefusal {
    match env
        .backend()
        .move_entry(&mv(from, to_folder), &NoControl)
        .await
    {
        Err(MoveError::Refused { reason }) => reason,
        other => panic!("ожидался отказ, получено {other:?}"),
    }
}

async fn indexed<B: VaultBackend>(backend: &B) -> Vec<String> {
    backend
        .list()
        .await
        .into_iter()
        .map(|note| note.path.to_string_lossy().replace('\\', "/"))
        .collect()
}

/// Диск после переноса: каждый путь под `from` переехал под `to`, остальное
/// на месте, байты те же.
fn relocated(before: &Disk, moves: &[(&str, &str)]) -> Disk {
    before
        .iter()
        .map(|(path, bytes)| {
            let new = moves
                .iter()
                .find_map(|(from, to)| {
                    if path == from || path == &format!("{from}/") {
                        Some(format!("{to}{}", &path[from.len()..]))
                    } else {
                        path.strip_prefix(&format!("{from}/"))
                            .map(|rest| format!("{to}/{rest}"))
                    }
                })
                .unwrap_or_else(|| path.clone());
            (new, bytes.clone())
        })
        .collect()
}

fn assert_not_external(events: &[VaultEvent]) {
    for event in events {
        assert!(
            !matches!(
                event,
                VaultEvent::NoteDeleted { .. }
                    | VaultEvent::NoteDeletedWithUnsavedEdits { .. }
                    | VaultEvent::BulkChangeStarted
                    | VaultEvent::BulkChangeSettled
                    | VaultEvent::ConflictDetected { .. }
            ),
            "собственный перенос показан как внешнее изменение (FR-008): {event:?}"
        );
    }
}

const NOTE: &[u8] = "текст заметки".as_bytes();

// ---------------------------------------------------------------------------
// US1 — перенос заметки и доски (T018)
// ---------------------------------------------------------------------------

/// FR-001: заметка уезжает в папку и в корень под тем же именем; байты те же,
/// источника нет, индекс под новым путём.
pub async fn a_note_moves_into_a_folder_and_to_the_root<E: Env>() {
    let mut env = E::open(&[
        ("A/план.typ", "план: [[/B/другая]]".as_bytes()),
        ("B/другая.typ", NOTE),
        ("A/рядом.typ", NOTE),
    ])
    .await;
    let before = env.disk().await;

    let (moved, folders, undo) = done(
        env.backend()
            .move_entry(&mv("A/план.typ", "B"), &NoControl)
            .await,
    );
    assert_eq!(
        moved,
        vec![PathChange {
            from: "A/план.typ".into(),
            to: "B/план.typ".into()
        }]
    );
    assert!(folders.is_empty(), "файл папок не переносит");
    assert_eq!(undo, mv("B/план.typ", "A"));
    assert_eq!(
        env.disk().await,
        relocated(&before, &[("A/план.typ", "B/план.typ")])
    );
    assert_eq!(
        indexed(env.backend_ref()).await,
        ["A/рядом.typ", "B/другая.typ", "B/план.typ"]
    );
    let note = env
        .backend_ref()
        .list()
        .await
        .into_iter()
        .find(|note| note.path == Path::new("B/план.typ"))
        .unwrap();
    assert_eq!(note.name, "план.typ", "имя при переносе не меняется");

    // Второй перенос — на свободное место дерева, то есть в корень.
    done(
        env.backend()
            .move_entry(&mv("B/план.typ", ""), &NoControl)
            .await,
    );
    assert!(env.disk().await.contains_key("план.typ"));
    assert!(!env.disk().await.contains_key("B/план.typ"));
}

/// Доска переносится так же, как заметка (US1 AS8).
pub async fn a_board_moves_like_a_note<E: Env>() {
    let mut env = E::open(&[
        ("A/эскиз.excalidraw", br#"{"elements":[[[0,0],[1,1]]]}"#),
        ("B/x.typ", NOTE),
    ])
    .await;
    let before = env.disk().await;

    done(
        env.backend()
            .move_entry(&mv("A/эскиз.excalidraw", "B"), &NoControl)
            .await,
    );

    assert_eq!(
        env.disk().await,
        relocated(&before, &[("A/эскиз.excalidraw", "B/эскиз.excalidraw")])
    );
    assert!(indexed(env.backend_ref())
        .await
        .contains(&"B/эскиз.excalidraw".to_string()));
}

/// FR-020: несохранённые правки и неразрешённый конфликт остаются за новым
/// путём, и сохранение после переноса пишет в новое место.
pub async fn unsaved_edits_and_a_conflict_follow_the_note<E: Env>() {
    let mut env = E::open(&[("A/x.typ", "было".as_bytes()), ("B/y.typ", NOTE)]).await;
    env.backend()
        .begin_edit(Path::new("A/x.typ"), "мои правки".into())
        .await;
    env.write_outside("A/x.typ", "внешнее".as_bytes()).await;
    env.settle().await;
    assert!(
        env.backend_ref()
            .pending_conflict(Path::new("A/x.typ"))
            .is_some(),
        "предпосылка: внешнее изменение подняло конфликт"
    );

    done(
        env.backend()
            .move_entry(&mv("A/x.typ", "B"), &NoControl)
            .await,
    );

    let backend = env.backend_ref();
    assert!(!backend.has_unsaved_edits(Path::new("A/x.typ")));
    assert!(backend.has_unsaved_edits(Path::new("B/x.typ")));
    assert_eq!(
        backend
            .pending_conflict(Path::new("B/x.typ"))
            .map(|conflict| (conflict.note_path, conflict.local_buffer)),
        Some(("B/x.typ".into(), "мои правки".to_string()))
    );
    assert_eq!(backend.unresolved_edit_paths(), [Path::new("B/x.typ")]);
    assert!(
        backend
            .list()
            .await
            .iter()
            .any(|note| note.path == Path::new("B/x.typ") && note.has_unsaved_edits),
        "признак правок едет вместе с записью индекса"
    );

    // Конфликт разрешается как обычно — и пишет в новое место.
    env.backend()
        .resolve_conflict_keep_local(Path::new("B/x.typ"))
        .await
        .unwrap();
    let disk = env.disk().await;
    assert_eq!(disk["B/x.typ"], "мои правки".as_bytes());
    assert!(!disk.contains_key("A/x.typ"));
}

/// FR-018a: собственная папка снимков едет с заметкой в обеих раскладках, а
/// общая папка корня vault — нет.
pub async fn own_snapshot_folders_travel_with_the_note<E: Env>() {
    let mut env = E::open(&[
        ("A/план.typ", NOTE),
        ("A/план.assets/board-1.svg", b"<svg>1</svg>"),
        ("A/assets/план/board-2.svg", b"<svg>2</svg>"),
        ("assets/план/board-3.svg", b"<svg>3</svg>"),
        ("B/x.typ", NOTE),
    ])
    .await;
    let before = env.disk().await;

    done(
        env.backend()
            .move_entry(&mv("A/план.typ", "B"), &NoControl)
            .await,
    );

    assert_eq!(
        env.disk().await,
        relocated(
            &before,
            &[
                ("A/план.typ", "B/план.typ"),
                ("A/план.assets", "B/план.assets"),
                ("A/assets/план", "B/assets/план"),
            ]
        )
    );
    let disk = env.disk().await;
    assert!(disk.contains_key("assets/план/board-3.svg"));
    assert!(!disk.contains_key("A/план.assets/board-1.svg"));
}

/// FR-018a, Edge Cases: при политике «в корне vault» `assets/<заметка>/` рядом
/// с заметкой — не её собственная папка, и она остаётся.
pub async fn a_shared_snapshot_folder_stays_when_the_policy_is_vault_root<E: Env>() {
    let mut env = E::open(&[
        ("A/план.typ", NOTE),
        ("A/план.assets/board-1.svg", b"<svg>1</svg>"),
        ("A/assets/план/board-2.svg", b"<svg>2</svg>"),
        ("B/x.typ", NOTE),
    ])
    .await;
    write_vault_settings(
        env.backend(),
        &VaultOverrides {
            assets_placement: Some(AssetsPlacement::VaultRoot),
            ..VaultOverrides::default()
        },
    )
    .await
    .unwrap();

    done(
        env.backend()
            .move_entry(&mv("A/план.typ", "B"), &NoControl)
            .await,
    );

    let disk = env.disk().await;
    assert!(
        disk.contains_key("B/план.assets/board-1.svg"),
        "прежняя раскладка распознаётся при любой политике"
    );
    assert!(disk.contains_key("A/assets/план/board-2.svg"));
    assert!(!disk.contains_key("B/assets/план/board-2.svg"));
}

/// FR-019: текст заметок перенос не трогает — ни переносимой, ни ссылающихся.
pub async fn moving_never_rewrites_any_note_text<E: Env>() {
    let mut env = E::open(&[
        ("a.typ", "см. [[b]] и [[/b]]".as_bytes()),
        ("b.typ", "ссылка из b: [[a]]".as_bytes()),
        ("B/keep.typ", NOTE),
    ])
    .await;
    let before = env.disk().await;

    done(
        env.backend()
            .move_entry(&mv("b.typ", "B"), &NoControl)
            .await,
    );

    assert_eq!(
        env.disk().await,
        relocated(&before, &[("b.typ", "B/b.typ")])
    );
}

/// FR-008: собственный перенос не порождает событий удаления и массовых
/// изменений, даже когда файлов больше порога пачки; есть `NoteRenamed` на
/// каждую заметку и один `LinksInvalidated`.
pub async fn an_own_move_is_never_reported_as_an_external_change<E: Env>() {
    let owned: Vec<(String, Vec<u8>)> = (0..25)
        .map(|i| {
            (
                format!("A/n{i:02}.typ"),
                format!("заметка {i}").into_bytes(),
            )
        })
        .collect();
    let mut files: Vec<(&str, &[u8])> = owned
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
        .collect();
    files.push(("B/keep.typ", NOTE));
    files.push(("out.typ", "[[A/n03]]".as_bytes()));
    let mut env = E::open(&files).await;
    env.backend()
        .begin_edit(Path::new("A/n01.typ"), "правки".into())
        .await;
    env.settle().await;

    done(env.backend().move_entry(&mv("A", "B"), &NoControl).await);
    let events = env.settle().await;

    assert_not_external(&events);
    let renamed = events
        .iter()
        .filter(|event| matches!(event, VaultEvent::NoteRenamed { .. }))
        .count();
    assert!(
        renamed >= 25,
        "NoteRenamed на каждую заметку, а их {renamed}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            VaultEvent::LinksInvalidated { affected_paths }
                if affected_paths.iter().any(|path| path == Path::new("out.typ"))
        )),
        "ссылающаяся заметка названа в LinksInvalidated"
    );
    assert!(env
        .backend_ref()
        .has_unsaved_edits(Path::new("B/A/n01.typ")));
}

/// Одиночная заметка: после прохода обнаружения событий удаления нет, а
/// индекс по-прежнему на новом пути (на native — после доставки событий
/// наблюдателя).
pub async fn a_single_note_move_survives_the_watcher<E: Env>() {
    let mut env = E::open(&[("A/x.typ", NOTE), ("B/y.typ", NOTE)]).await;
    env.backend()
        .begin_edit(Path::new("A/x.typ"), "правки".into())
        .await;

    done(
        env.backend()
            .move_entry(&mv("A/x.typ", "B"), &NoControl)
            .await,
    );
    let events = env.settle().await;

    assert_not_external(&events);
    assert_eq!(indexed(env.backend_ref()).await, ["B/x.typ", "B/y.typ"]);
    assert!(env.backend_ref().has_unsaved_edits(Path::new("B/x.typ")));
    assert!(
        env.backend_ref()
            .pending_conflict(Path::new("B/x.typ"))
            .is_none(),
        "перенос не создаёт конфликта"
    );
}

// ---------------------------------------------------------------------------
// US1 — множество допустимых целей (T019)
// ---------------------------------------------------------------------------

pub async fn targets_start_with_the_root_and_skip_the_current_folder<E: Env>() {
    let mut env = E::open(&[
        ("w.typ", NOTE),
        ("A/x.typ", NOTE),
        ("A/C/y.typ", NOTE),
        ("B/z.typ", NOTE),
    ])
    .await;
    set_section(env.backend(), Path::new("B"), true)
        .await
        .unwrap();

    let targets = env
        .backend_ref()
        .move_targets(Path::new("A/x.typ"), "")
        .await;
    let paths: Vec<String> = targets
        .iter()
        .map(|target| target.path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(paths, ["", "A/C", "B"], "корень первым, далее по пути");
    assert!(
        !paths.iter().any(|path| path.starts_with(".typsidian")),
        "служебной папки среди целей нет"
    );
    assert!(!targets[0].is_section);
    assert!(targets[2].is_section, "признак раздела");
    assert_eq!(targets[2].name, "B");
    assert_eq!(targets[1].parent, "A");

    let from_root = env.backend_ref().move_targets(Path::new("w.typ"), "").await;
    assert!(
        !from_root
            .iter()
            .any(|target| target.path.as_os_str().is_empty()),
        "корень не предлагается заметке, которая в нём уже лежит"
    );
}

// ---------------------------------------------------------------------------
// US2 — перенос папки (T026)
// ---------------------------------------------------------------------------

const PICTURE: &[u8] = &[0xff, 0xfe, 0x00, 0x01, 0x80, 0x7f, 0x00, 0xc3];

pub fn folder_files() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("A/n1.typ", "первая [[n2]]".as_bytes()),
        ("A/C/n2.typ", "вторая".as_bytes()),
        ("A/C/D/доска.excalidraw", br#"{"elements":[]}"#),
        ("A/картинка.png", PICTURE),
        ("A/n1.assets/board-1.svg", b"<svg/>"),
        ("A/.скрытый/файл.bin", PICTURE),
        ("B/z.typ", NOTE),
        ("вне.typ", NOTE),
    ]
}

/// FR-002, FR-018: папка едет со всем, что в ней — видимым и нет; пометки
/// разделов и буферы едут следом.
pub async fn a_folder_moves_with_everything_in_it<E: Env>() {
    let files = folder_files();
    let mut env = E::open(&files).await;
    set_section(env.backend(), Path::new("A"), true)
        .await
        .unwrap();
    set_section(env.backend(), Path::new("A/C"), true)
        .await
        .unwrap();
    env.backend()
        .begin_edit(Path::new("A/n1.typ"), "правки".into())
        .await;
    let before = env.disk().await;

    let (moved, folders, undo) = done(env.backend().move_entry(&mv("A", "B"), &NoControl).await);

    assert_eq!(
        env.disk().await,
        relocated(&before, &[("A", "B/A")]),
        "содержимое и структура побайтно те же, прежней A нет"
    );
    let moved_to: Vec<String> = moved
        .iter()
        .map(|change| change.to.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        moved_to,
        ["B/A/C/D/доска.excalidraw", "B/A/C/n2.typ", "B/A/n1.typ"]
    );
    let folder_pairs: Vec<(String, String)> = folders
        .iter()
        .map(|change| {
            (
                change.from.to_string_lossy().into_owned(),
                change.to.to_string_lossy().into_owned(),
            )
        })
        .collect();
    assert_eq!(
        folder_pairs,
        [
            ("A".into(), "B/A".into()),
            ("A/C".into(), "B/A/C".into()),
            ("A/C/D".into(), "B/A/C/D".into()),
        ]
    );
    assert_eq!(undo, mv("B/A", ""));
    assert_eq!(
        indexed(env.backend_ref()).await,
        [
            "B/A/C/D/доска.excalidraw",
            "B/A/C/n2.typ",
            "B/A/n1.typ",
            "B/z.typ",
            "вне.typ"
        ]
    );

    let marks: Vec<String> = read_vault_settings(env.backend_ref())
        .await
        .unwrap()
        .sections
        .into_iter()
        .map(|mark| mark.path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        marks,
        ["B/A", "B/A/C"],
        "самая папка и вложенная остаются разделами"
    );
    assert!(env.backend_ref().has_unsaved_edits(Path::new("B/A/n1.typ")));
    assert!(!env.backend_ref().has_unsaved_edits(Path::new("A/n1.typ")));
}

/// FR-003: папка не входит в себя и в потомков любой глубины; диск не тронут.
pub async fn a_folder_cannot_enter_itself_or_any_descendant<E: Env>() {
    let files = folder_files();
    let mut env = E::open(&files).await;
    let before = env.disk().await;

    for target in ["A", "A/C", "A/C/D"] {
        assert_eq!(
            refusal(&mut env, "A", target).await,
            MoveRefusal::IntoItself,
            "{target}"
        );
    }
    assert_eq!(env.disk().await, before);

    let targets = env.backend_ref().move_targets(Path::new("A"), "").await;
    let paths: Vec<String> = targets
        .iter()
        .map(|target| target.path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        paths,
        ["B"],
        "в списке нет ни самой A, ни потомков, ни корня"
    );
}

/// Принцип X, отмена: просьба отменить до начала оставляет vault как был.
pub async fn a_cancelled_move_changes_nothing<E: Env>() {
    struct Cancelled;
    impl MoveControl for Cancelled {
        fn progress(&self, _done: u32, _total: u32) {}
        fn cancelled(&self) -> bool {
            true
        }
    }
    let files = folder_files();
    let mut env = E::open(&files).await;
    let before = env.disk().await;

    let outcome = env
        .backend()
        .move_entry(&mv("A", "B"), &Cancelled)
        .await
        .unwrap();

    assert_eq!(outcome, MoveOutcome::Cancelled);
    assert_eq!(env.disk().await, before);
    assert_eq!(indexed(env.backend_ref()).await.len(), 5);
}

/// Web: отмена на середине копирования удаляет скопированное — vault прежний
/// (research.md §6).
pub async fn cancelling_halfway_through_copying_restores_the_vault<E: Env>() {
    use std::sync::atomic::{AtomicU32, Ordering};
    struct After {
        files: u32,
        seen: AtomicU32,
    }
    impl MoveControl for After {
        fn progress(&self, done: u32, _total: u32) {
            self.seen.store(done, Ordering::Relaxed);
        }
        fn cancelled(&self) -> bool {
            self.seen.load(Ordering::Relaxed) >= self.files
        }
    }
    let files = folder_files();
    let mut env = E::open(&files).await;
    let before = env.disk().await;
    let control = After {
        files: 3,
        seen: AtomicU32::new(0),
    };

    let outcome = env
        .backend()
        .move_entry(&mv("A", "B"), &control)
        .await
        .unwrap();

    assert_eq!(outcome, MoveOutcome::Cancelled);
    assert!(
        control.seen.load(Ordering::Relaxed) >= 3,
        "копирование шло до отмены"
    );
    assert_eq!(env.disk().await, before, "ни одного лишнего файла в цели");
    assert!(!indexed(env.backend_ref())
        .await
        .iter()
        .any(|path| path.starts_with("B/A")));
}

/// Web, FR-007: сбой удаления источника — полная копия в цели, остаток назван,
/// ни один файл не потерян, индекс показывает то, что на диске.
pub async fn a_failed_source_removal_leaves_a_full_copy_and_names_the_rest<E: Env>(mut env: E) {
    let before = env.disk().await;
    env.backend()
        .begin_edit(Path::new("A/n1.typ"), "правки".into())
        .await;

    let outcome = env
        .backend()
        .move_entry(&mv("A", "B"), &NoControl)
        .await
        .unwrap();

    let MoveOutcome::Partial { moved, left, .. } = outcome else {
        panic!("ожидалось Partial, получено {outcome:?}");
    };
    assert_eq!(moved.len(), 3);
    assert!(left.iter().any(|path| path == Path::new("A/n1.typ")));
    assert!(left.iter().any(|path| path == Path::new("A/картинка.png")));

    let after = env.disk().await;
    for (path, bytes) in &before {
        assert_eq!(after.get(path), Some(bytes), "{path} не потерян");
        if let Some(rest) = path.strip_prefix("A/") {
            assert_eq!(
                after.get(&format!("B/A/{rest}")),
                Some(bytes),
                "копия {path} полная и побайтно та же"
            );
        }
    }
    let paths = indexed(env.backend_ref()).await;
    assert!(
        paths.contains(&"A/n1.typ".to_string()),
        "остаток виден в дереве"
    );
    assert!(paths.contains(&"B/A/n1.typ".to_string()));
    assert!(env.backend_ref().has_unsaved_edits(Path::new("B/A/n1.typ")));
}

// ---------------------------------------------------------------------------
// US3 — ссылки (T029)
// ---------------------------------------------------------------------------

async fn broken_after<E: Env>(env: &mut E, from: &str, to: &str) -> Vec<String> {
    let before = env.disk().await;
    let impact = env
        .backend_ref()
        .move_impact(&mv(from, to))
        .await
        .expect("перенос допустим");
    assert_eq!(env.disk().await, before, "оценка ничего не меняет на диске");
    impact
        .broken
        .into_iter()
        .map(|note| note.path.to_string_lossy().into_owned())
        .collect()
}

/// FR-016: входящие относительная и корневая ссылки рвутся; список по пути.
pub async fn incoming_relative_and_rooted_links_are_reported<E: Env>() {
    let mut env = E::open(&[
        ("a.typ", "[[b]]".as_bytes()),
        ("c.typ", "[[/b]] [[b]]".as_bytes()),
        ("b.typ", NOTE),
        ("d.typ", "без ссылок".as_bytes()),
        ("B/keep.typ", NOTE),
    ])
    .await;
    assert_eq!(
        broken_after(&mut env, "b.typ", "B").await,
        ["a.typ", "c.typ"]
    );
    assert!(broken_after(&mut env, "d.typ", "B").await.is_empty());
}

/// FR-016: собственные относительные ссылки переносимой заметки на соседей.
pub async fn own_relative_links_of_the_moved_note_are_reported<E: Env>() {
    let mut env = E::open(&[
        ("x/m.typ", "[[n]] и [[/top]]".as_bytes()),
        ("x/n.typ", NOTE),
        ("top.typ", NOTE),
    ])
    .await;
    assert_eq!(broken_after(&mut env, "x/m.typ", "").await, ["x/m.typ"]);
}

/// FR-017: ссылки внутри переносимой папки не рвутся, внешние — один общий
/// список без повторов; уже битая ссылка в список не попадает.
pub async fn a_folder_gives_one_list_of_only_the_links_it_really_breaks<E: Env>() {
    let mut env = E::open(&[
        ("F/p.typ", "[[q]] [[нет-такой]]".as_bytes()),
        ("F/q.typ", "[[p]]".as_bytes()),
        ("out.typ", "[[F/p]] и [[F/q]]".as_bytes()),
        ("G/keep.typ", NOTE),
    ])
    .await;
    assert_eq!(broken_after(&mut env, "F", "G").await, ["out.typ"]);

    // Без внешних ссылок на папку предупреждать не о чем.
    let mut quiet = E::open(&[
        ("F/p.typ", "[[q]]".as_bytes()),
        ("F/q.typ", "[[p]]".as_bytes()),
        ("G/keep.typ", NOTE),
    ])
    .await;
    assert!(broken_after(&mut quiet, "F", "G").await.is_empty());
}

/// Оценка называет те же отказы, что и перенос: подтверждать перенос, который
/// всё равно отклонят, незачем.
pub async fn impact_refuses_what_the_move_would_refuse<E: Env>() {
    let env = E::open(&[("A/x.typ", NOTE), ("B/x.typ", NOTE)]).await;
    assert_eq!(
        env.backend_ref()
            .move_impact(&mv("A/x.typ", "B"))
            .await
            .err(),
        Some(MoveRefusal::NameConflict {
            name: "x.typ".into()
        })
    );
    assert_eq!(
        env.backend_ref()
            .move_impact(&mv("A/x.typ", "A"))
            .await
            .err(),
        Some(MoveRefusal::SamePlace)
    );
}

// ---------------------------------------------------------------------------
// US3 — отказы (T030)
// ---------------------------------------------------------------------------

pub async fn moving_to_the_folder_it_is_in_is_a_silent_refusal<E: Env>() {
    let mut env = E::open(&[("A/x.typ", NOTE), ("y.typ", NOTE)]).await;
    let before = env.disk().await;
    assert_eq!(
        refusal(&mut env, "A/x.typ", "A").await,
        MoveRefusal::SamePlace
    );
    assert_eq!(refusal(&mut env, "y.typ", "").await, MoveRefusal::SamePlace);
    assert_eq!(env.disk().await, before);
}

/// FR-004: занятое имя — отказ; ни один из двух файлов не затёрт.
pub async fn a_taken_name_is_refused_for_a_file_a_folder_and_a_snapshot_folder<E: Env>() {
    let mut env = E::open(&[
        ("A/план.typ", "из A".as_bytes()),
        ("B/план.typ", "из B".as_bytes()),
        ("A/D/в-папке.typ", NOTE),
        ("B/D/тут-своё.typ", NOTE),
        ("A/n.typ", NOTE),
        ("A/n.assets/board-1.svg", b"<svg/>"),
        ("B/n.assets/чужое.svg", b"<svg/>"),
        ("B/невидимое.dat", PICTURE),
        ("A/невидимое.dat", PICTURE),
    ])
    .await;
    let before = env.disk().await;

    assert_eq!(
        refusal(&mut env, "A/план.typ", "B").await,
        MoveRefusal::NameConflict {
            name: "план.typ".into()
        }
    );
    assert_eq!(
        refusal(&mut env, "A/D", "B").await,
        MoveRefusal::NameConflict { name: "D".into() }
    );
    assert_eq!(
        refusal(&mut env, "A/n.typ", "B").await,
        MoveRefusal::NameConflict {
            name: "n.assets".into()
        },
        "занятая папка снимков — тот же конфликт имён (FR-018a)"
    );
    assert_eq!(
        refusal(&mut env, "A/невидимое.dat", "B").await,
        MoveRefusal::NameConflict {
            name: "невидимое.dat".into()
        },
        "имя занято файлом, которого дерево не показывает"
    );
    assert_eq!(env.disk().await, before, "ни один файл не тронут");
}

/// 001 §FR-012: имена, различающиеся регистром, — конфликт ровно там, где
/// файловая система регистр не различает.
pub async fn names_that_differ_by_case_follow_the_file_system<E: Env>() {
    let mut env = E::open(&[
        ("A/План.typ", "один".as_bytes()),
        ("B/план.typ", "два".as_bytes()),
    ])
    .await;
    let before = env.disk().await;
    let case_insensitive = env
        .backend_ref()
        .read(Path::new("B/План.typ"))
        .await
        .is_ok();

    if case_insensitive {
        assert!(matches!(
            refusal(&mut env, "A/План.typ", "B").await,
            MoveRefusal::NameConflict { .. }
        ));
        assert_eq!(env.disk().await, before);
    } else {
        done(
            env.backend()
                .move_entry(&mv("A/План.typ", "B"), &NoControl)
                .await,
        );
        let disk = env.disk().await;
        assert_eq!(disk["B/План.typ"], "один".as_bytes());
        assert_eq!(
            disk["B/план.typ"],
            "два".as_bytes(),
            "второй файл не затёрт"
        );
    }
}

pub async fn a_missing_item_or_target_is_named<E: Env>() {
    let mut env = E::open(&[("A/x.typ", NOTE)]).await;
    let before = env.disk().await;
    assert_eq!(
        refusal(&mut env, "A/нет.typ", "").await,
        MoveRefusal::NotFound
    );
    assert_eq!(
        refusal(&mut env, "A/x.typ", "Нет").await,
        MoveRefusal::TargetNotFound
    );
    assert_eq!(env.disk().await, before);
}

pub async fn the_service_folder_is_neither_moved_nor_a_target<E: Env>() {
    let mut env = E::open(&[("A/x.typ", NOTE)]).await;
    set_section(env.backend(), Path::new("A"), true)
        .await
        .unwrap();
    let before = env.disk().await;
    assert_eq!(
        refusal(&mut env, "A/x.typ", ".typsidian").await,
        MoveRefusal::ServiceFolder
    );
    assert_eq!(
        refusal(&mut env, ".typsidian", "A").await,
        MoveRefusal::ServiceFolder
    );
    assert_eq!(env.disk().await, before);
}

/// FR-006: vault только для чтения — отказ с названной причиной, диск прежний.
pub async fn a_read_only_vault_refuses_every_move<E: Env>() {
    let files: Files = &[("A/x.typ", NOTE), ("B/y.typ", NOTE)];
    let Some(mut env) = E::open_read_only(files).await else {
        eprintln!("пропуск: среда не умеет изображать vault только для чтения");
        return;
    };
    let before = env.disk().await;

    assert_eq!(
        refusal(&mut env, "A/x.typ", "B").await,
        MoveRefusal::AccessDenied
    );
    assert_eq!(
        env.backend_ref()
            .move_impact(&mv("A/x.typ", "B"))
            .await
            .err(),
        Some(MoveRefusal::AccessDenied)
    );
    assert_eq!(env.disk().await, before);
}

// ---------------------------------------------------------------------------
// US4 — фильтр целей (T034)
// ---------------------------------------------------------------------------

pub async fn the_filter_matches_name_with_a_span_and_path_without<E: Env>() {
    let env = E::open(&[
        ("Работа/Проекты/x.typ", NOTE),
        ("Личное/y.typ", NOTE),
        ("z.typ", NOTE),
    ])
    .await;
    let by_name = env
        .backend_ref()
        .move_targets(Path::new("z.typ"), "ПРОЕ")
        .await;
    assert_eq!(by_name.len(), 1, "подстрока без учёта регистра, кириллица");
    assert_eq!(by_name[0].path, Path::new("Работа/Проекты"));
    let span = by_name[0].name_match.expect("совпало в имени");
    assert_eq!((span.start, span.end), (0, 4));

    let by_path = env
        .backend_ref()
        .move_targets(Path::new("z.typ"), "работа/пр")
        .await;
    assert_eq!(by_path.len(), 1);
    assert!(
        by_path[0].name_match.is_none(),
        "совпало в пути, не в имени"
    );

    assert!(env
        .backend_ref()
        .move_targets(Path::new("z.typ"), "нет такой")
        .await
        .is_empty());

    // Корень находится по имени vault; для заметки из подпапки он допустим.
    let root_name = env
        .backend_ref()
        .root_path()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    if let Some(root_name) = root_name {
        let probe: String = root_name.chars().take(3).collect();
        let found = env
            .backend_ref()
            .move_targets(Path::new("Личное/y.typ"), &probe)
            .await;
        assert!(
            found
                .iter()
                .any(|target| target.path.as_os_str().is_empty()),
            "корень находится по имени vault"
        );
    }
}

// ---------------------------------------------------------------------------
// US6 — отмена (T041)
// ---------------------------------------------------------------------------

/// FR-024: `undo` возвращает файлы, индекс, раздел и буфер — для заметки с
/// папкой снимков и для папки.
pub async fn undo_restores_a_note_and_its_snapshot_folder<E: Env>() {
    let mut env = E::open(&[
        ("A/план.typ", NOTE),
        ("A/план.assets/board-1.svg", b"<svg/>"),
        ("B/x.typ", NOTE),
    ])
    .await;
    env.backend()
        .begin_edit(Path::new("A/план.typ"), "правки".into())
        .await;
    let before = env.disk().await;
    let indexed_before = indexed(env.backend_ref()).await;

    let (_, _, undo) = done(
        env.backend()
            .move_entry(&mv("A/план.typ", "B"), &NoControl)
            .await,
    );
    done(env.backend().move_entry(&undo, &NoControl).await);

    assert_eq!(env.disk().await, before);
    assert_eq!(indexed(env.backend_ref()).await, indexed_before);
    assert!(env.backend_ref().has_unsaved_edits(Path::new("A/план.typ")));
    assert!(!env.backend_ref().has_unsaved_edits(Path::new("B/план.typ")));
}

pub async fn undo_restores_a_folder_with_its_section_marks<E: Env>() {
    let files = folder_files();
    let mut env = E::open(&files).await;
    set_section(env.backend(), Path::new("A/C"), true)
        .await
        .unwrap();
    let before = env.disk().await;
    let indexed_before = indexed(env.backend_ref()).await;

    let (_, _, undo) = done(env.backend().move_entry(&mv("A", "B"), &NoControl).await);
    done(env.backend().move_entry(&undo, &NoControl).await);

    assert_eq!(env.disk().await, before);
    assert_eq!(indexed(env.backend_ref()).await, indexed_before);
    let marks: Vec<String> = read_vault_settings(env.backend_ref())
        .await
        .unwrap()
        .sections
        .into_iter()
        .map(|mark| mark.path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(marks, ["A/C"]);
}

/// FR-025: прежнее место занято — отмена не затирает, называет причину.
pub async fn undo_never_overwrites_what_appeared_in_the_old_place<E: Env>() {
    let mut env = E::open(&[("A/x.typ", "прежнее".as_bytes()), ("B/y.typ", NOTE)]).await;
    let (_, _, undo) = done(
        env.backend()
            .move_entry(&mv("A/x.typ", "B"), &NoControl)
            .await,
    );
    env.backend()
        .write(Path::new("A/x.typ"), "новое на старом месте", None)
        .await
        .unwrap();
    let before = env.disk().await;

    let result = env.backend().move_entry(&undo, &NoControl).await;

    assert_eq!(
        result,
        Err(MoveError::Refused {
            reason: MoveRefusal::NameConflict {
                name: "x.typ".into()
            }
        })
    );
    assert_eq!(env.disk().await, before);
    assert_eq!(before["B/x.typ"], "прежнее".as_bytes());
}

/// Фича 021, FR-005: ссылка `[[путь|текст]]` рвётся ровно по своей цели, текст
/// на это не влияет; сам перенос текст ссылок не трогает.
pub async fn a_link_with_a_label_is_judged_by_its_target<E: Env>() {
    let mut env = E::open(&[
        ("a.typ", "[[b|подпись]]".as_bytes()),
        ("c.typ", "[[/b|ещё подпись]]".as_bytes()),
        ("d.typ", "[[нет-такой|текст]]".as_bytes()),
        ("b.typ", NOTE),
        ("B/keep.typ", NOTE),
    ])
    .await;
    assert_eq!(
        broken_after(&mut env, "b.typ", "B").await,
        ["a.typ", "c.typ"]
    );
    let outcome = env
        .backend()
        .move_entry(&mv("b.typ", "B"), &NoControl)
        .await
        .expect("перенос выполнен");
    assert!(matches!(outcome, MoveOutcome::Done { .. }));
    let disk = env.disk().await;
    assert_eq!(disk["a.typ"], "[[b|подпись]]".as_bytes());
    assert_eq!(disk["c.typ"], "[[/b|ещё подпись]]".as_bytes());
}
