//! Настоящий движок git (libgit2) под native (фича 011, T017, T044).
//!
//! Политику проверяет общая таблица `sync_cases` на двойнике движка; здесь —
//! то, что двойник подтвердить не может: снимки читаются стандартным git
//! (SC-008), обмен идёт по настоящему протоколу, а «нет сети» завершается
//! за ограниченное время (SC-004).
#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;
use std::time::{Duration, Instant};

use typsidian_core::sync::engine::native::NativeGitEngine;
use typsidian_core::sync::engine::CancelToken;
use typsidian_core::sync::secrets::MemorySecretStore;
use typsidian_core::sync::session::{self, Credentials};
use typsidian_core::sync::{
    Author, DeviceIdentity, DeviceVaultSync, HistoryState, ShellCapabilities, SyncContext,
    SyncError, SyncSession,
};
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::native::NativeBackend;

fn ctx(now: i64, id: char) -> SyncContext {
    SyncContext {
        device: Some(DeviceIdentity {
            id: id.to_string().repeat(32),
            name: "тест".into(),
        }),
        device_sync: DeviceVaultSync {
            author: Some(Author {
                name: "Тест".into(),
                email: "test@example.com".into(),
            }),
            ..DeviceVaultSync::default()
        },
        now,
        capabilities: ShellCapabilities::current(false),
    }
}

async fn open(dir: &Path) -> (NativeBackend, SyncSession<NativeGitEngine>) {
    let backend = NativeBackend::open(dir).await.expect("vault открывается");
    let session = SyncSession::new(NativeGitEngine::open(dir), None, "vault-1");
    (backend, session)
}

#[tokio::test]
async fn snapshots_are_plain_git_commits() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.typ"), "= A\n").unwrap();
    std::fs::write(dir.path().join("b.typ"), "= B\n").unwrap();
    let (mut backend, mut session) = open(dir.path()).await;
    let first = session
        .enable_history(&mut backend, &ctx(1_700_000_000_000, 'a'), "Первый")
        .await
        .unwrap();

    let repo = git2::Repository::open(dir.path()).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.id().to_string(), first.id);
    assert_eq!(head.message().ok(), Some("Первый"));
    let tree = head.tree().unwrap();
    assert!(tree.get_path(Path::new("a.typ")).is_ok());
    // Кэш индекса исключён правилами — его нет ни в снимке, ни в изменениях.
    assert!(tree.get_path(Path::new(".typsidian/cache.sqlite")).is_err());
    assert!(session.changes(&backend).await.unwrap().is_empty());

    std::fs::write(dir.path().join("a.typ"), "= A2\n").unwrap();
    std::fs::write(dir.path().join("b.typ"), "= B2\n").unwrap();
    session
        .snapshot(
            &backend,
            &ctx(1_700_000_001_000, 'a'),
            Some(vec!["a.typ".into()]),
            "только A",
            "",
            true,
            false,
        )
        .await
        .unwrap();
    let statuses = repo.statuses(None).unwrap();
    let dirty: Vec<_> = statuses
        .iter()
        .filter(|entry| {
            !matches!(
                entry.status(),
                git2::Status::CURRENT | git2::Status::IGNORED
            )
        })
        .filter_map(|entry| entry.path().ok().map(str::to_string))
        .collect();
    assert_eq!(dirty, ["b.typ"]);
}

#[tokio::test]
async fn exchange_through_a_bare_remote_with_merge() {
    let root = tempfile::tempdir().unwrap();
    let remote = root.path().join("remote.git");
    git2::Repository::init_bare(&remote).unwrap();
    let url = format!("file://{}", remote.display());
    let secrets = MemorySecretStore::new();

    let a_dir = root.path().join("a");
    std::fs::create_dir(&a_dir).unwrap();
    std::fs::write(
        a_dir.join("n.typ"),
        "= Заметка\n\nПервый.\n\nВторой.\n\nТретий.\n",
    )
    .unwrap();
    let (mut a, mut a_session) = open(&a_dir).await;
    a_session
        .enable_history(&mut a, &ctx(1_700_000_000_000, 'a'), "Первый")
        .await
        .unwrap();
    a_session
        .connect_remote(
            &mut a,
            &secrets,
            &ctx(1_700_000_000_500, 'a'),
            &url,
            Credentials::default(),
        )
        .await
        .unwrap();
    a_session
        .snapshot(
            &a,
            &ctx(1_700_000_001_000, 'a'),
            None,
            "адрес",
            "",
            true,
            false,
        )
        .await
        .unwrap();
    a_session
        .push(&a, &secrets, &ctx(1_700_000_001_500, 'a'), false)
        .await
        .unwrap();

    let b_dir = root.path().join("b");
    std::fs::create_dir(&b_dir).unwrap();
    session::clone_vault(
        &NativeGitEngine::open(&b_dir),
        &secrets,
        "vault-2",
        &ShellCapabilities::current(false),
        &url,
        Credentials::default(),
        &CancelToken::new(),
    )
    .await
    .unwrap();
    assert!(b_dir.join("n.typ").is_file());
    let (mut b, mut b_session) = open(&b_dir).await;

    std::fs::write(
        a_dir.join("n.typ"),
        "= Заметка\n\nПервый, A.\n\nВторой.\n\nТретий.\n",
    )
    .unwrap();
    a_session
        .snapshot(&a, &ctx(1_700_000_002_000, 'a'), None, "A", "", true, false)
        .await
        .unwrap();
    a_session
        .push(&a, &secrets, &ctx(1_700_000_002_500, 'a'), false)
        .await
        .unwrap();
    std::fs::write(
        b_dir.join("n.typ"),
        "= Заметка\n\nПервый.\n\nВторой.\n\nТретий, B.\n",
    )
    .unwrap();
    b_session
        .snapshot(&b, &ctx(1_700_000_003_000, 'b'), None, "B", "", true, false)
        .await
        .unwrap();

    let outcome = b_session
        .pull(&mut b, &secrets, &ctx(1_700_000_004_000, 'b'), false)
        .await
        .unwrap();
    assert_eq!(outcome.conflicts, 0);
    let merged = std::fs::read_to_string(b_dir.join("n.typ")).unwrap();
    assert!(merged.contains("Первый, A.") && merged.contains("Третий, B."));
    let repo = git2::Repository::open(&b_dir).unwrap();
    assert_eq!(
        repo.head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .parent_count(),
        2
    );
    assert!(b_session.changes(&b).await.unwrap().is_empty());
    b_session
        .push(&b, &secrets, &ctx(1_700_000_005_000, 'b'), false)
        .await
        .unwrap();

    // Не-хранитель читает историю из зеркала.
    let c_dir = root.path().join("c");
    std::fs::create_dir(&c_dir).unwrap();
    std::fs::create_dir(c_dir.join(".typsidian")).unwrap();
    std::fs::copy(
        b_dir.join(".typsidian/vault.json"),
        c_dir.join(".typsidian/vault.json"),
    )
    .unwrap();
    let mut c_backend = NativeBackend::open(&c_dir).await.unwrap();
    let mut settings = typsidian_core::sync::settings::read(&c_backend).await;
    settings.keeper = Some(typsidian_core::sync::Keeper {
        device_id: "a".repeat(32),
        device_name: "A".into(),
        since: 1,
    });
    typsidian_core::sync::settings::write(&mut c_backend, &settings)
        .await
        .unwrap();
    let mut c_session = SyncSession::new(
        NativeGitEngine::open(&c_dir),
        Some(NativeGitEngine::mirror(root.path().join("mirror.git"))),
        "vault-3",
    );
    let c_ctx = ctx(1_700_000_006_000, 'c');
    c_session
        .refresh_mirror(&c_backend, &secrets, &c_ctx)
        .await
        .unwrap();
    let overview = c_session.overview(&c_backend, &c_ctx).await;
    assert!(matches!(overview.history, HistoryState::Mirror { .. }));
    let history = c_session
        .history(&c_backend, Some("n.typ"), None, 10)
        .await
        .unwrap();
    assert!(history.snapshots.len() >= 3, "{history:#?}");
}

#[tokio::test]
async fn an_unreachable_server_fails_fast_and_keeps_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.typ"), "= A\n").unwrap();
    let (mut backend, mut session) = open(dir.path()).await;
    session
        .enable_history(&mut backend, &ctx(1_700_000_000_000, 'a'), "Первый")
        .await
        .unwrap();
    let secrets = MemorySecretStore::new();
    let started = Instant::now();
    // Порт 9 (discard) на loopback закрыт — соединение отклоняется сразу.
    let error = session
        .connect_remote(
            &mut backend,
            &secrets,
            &ctx(1_700_000_001_000, 'a'),
            "http://127.0.0.1:9/notes.git",
            Credentials::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error, SyncError::Offline);
    assert!(started.elapsed() < Duration::from_secs(30));
    let error = session
        .push(&backend, &secrets, &ctx(1_700_000_002_000, 'a'), false)
        .await
        .unwrap_err();
    assert_eq!(error, SyncError::Offline);
    assert_eq!(
        session
            .history(&backend, None, None, 10)
            .await
            .unwrap()
            .snapshots
            .len(),
        1
    );
}

#[tokio::test]
async fn conflict_copies_stay_out_of_the_tree_but_are_listed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("n.typ"), "наше\n").unwrap();
    std::fs::write(
        dir.path()
            .join("n.sync-conflict-20261003-120000-ABCDEFG.typ"),
        "их\n",
    )
    .unwrap();
    let backend = NativeBackend::open(dir.path()).await.unwrap();
    std::fs::write(
        dir.path()
            .join(".typsidian/vault.sync-conflict-20261003-120000-ABCDEFG.json"),
        "{}",
    )
    .unwrap();
    let listed: Vec<_> = backend
        .list()
        .await
        .into_iter()
        .map(|note| note.path)
        .collect();
    assert_eq!(listed, [Path::new("n.typ").to_path_buf()]);
    let copies = backend.list_conflict_copies().await;
    assert_eq!(copies.len(), 2);
}
