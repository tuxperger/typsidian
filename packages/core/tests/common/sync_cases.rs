//! Общие проверки политики синхронизации (фича 011, T017, T022, T031, T043,
//! T053, T061, T066, T069) — одна таблица для `cargo test` и
//! `wasm-bindgen-test` (принцип II).
//!
//! Движок — `FakeGitEngine` в памяти: он делит карту файлов с `MemoryVault`,
//! поэтому «рабочие файлы» движка и файлы, которые пишет сессия, — одно и то
//! же. Настоящий libgit2 проверяется отдельно, в `sync_engine_native.rs`.

#![allow(dead_code)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use typsidian_core::sync::autosnapshot::AutoDecision;
use typsidian_core::sync::engine::fake::{FakeGitEngine, FakeNetwork, FakeRepo};
use typsidian_core::sync::engine::CancelToken;
use typsidian_core::sync::ignore::{
    self, Editability, IgnoreDocument, IgnoreFile, IgnoreRefusal, IgnoreSaveOutcome,
    IgnoreSaveRequest, LineEnding,
};
use typsidian_core::sync::keeper::KeeperState;
use typsidian_core::sync::secrets::MemorySecretStore;
use typsidian_core::sync::session::{self, ConflictSource, Credentials, Resolution};
use typsidian_core::sync::settings::{self as sync_settings, Keeper, VaultSyncSettings};
use typsidian_core::sync::{
    Author, Change, ChangeKind, Content, DeviceIdentity, DeviceVaultSync, HistoryState, Revision,
    ShellCapabilities, SyncContext, SyncError, SyncEvent, SyncSession,
};
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::conflict::DiffLine;

use super::MemoryVault;

const REMOTE: &str = "https://example.com/me/notes.git";
const ME: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

pub struct Fixture {
    pub vault: MemoryVault,
    pub session: SyncSession<FakeGitEngine>,
    pub secrets: MemorySecretStore,
    pub device_sync: DeviceVaultSync,
    pub device_id: &'static str,
    pub now: i64,
}

fn engine_for(vault: &MemoryVault, remote: Option<Rc<RefCell<FakeRepo>>>) -> FakeGitEngine {
    let files = vault.files_handle();
    let read = files.clone();
    let engine = FakeGitEngine::new(
        Rc::new(move || {
            read.borrow()
                .iter()
                .map(|(path, bytes)| (path.to_string_lossy().replace('\\', "/"), bytes.clone()))
                .collect()
        }),
        Rc::new(move |path, bytes| match bytes {
            Some(bytes) => {
                files
                    .borrow_mut()
                    .insert(PathBuf::from(path), bytes.to_vec());
            }
            None => {
                files.borrow_mut().remove(Path::new(path));
            }
        }),
    );
    match remote {
        Some(remote) => engine.with_remote(remote),
        None => engine,
    }
}

pub fn fixture(files: &[(&str, &str)], remote: Option<Rc<RefCell<FakeRepo>>>) -> Fixture {
    let mut vault = MemoryVault::new();
    for (path, text) in files {
        vault = vault.with_file(path, text.as_bytes().to_vec());
    }
    let engine = engine_for(&vault, remote);
    Fixture {
        session: SyncSession::new(engine, None, "vault-1"),
        vault,
        secrets: MemorySecretStore::new(),
        device_sync: DeviceVaultSync {
            author: Some(Author {
                name: "Тест".into(),
                email: "test@example.com".into(),
            }),
            ..DeviceVaultSync::default()
        },
        device_id: ME,
        now: 1_700_000_000_000,
    }
}

impl Fixture {
    pub fn ctx(&self) -> SyncContext {
        SyncContext {
            device: Some(DeviceIdentity {
                id: self.device_id.to_string(),
                name: "это устройство".into(),
            }),
            device_sync: self.device_sync.clone(),
            now: self.now,
            capabilities: ShellCapabilities {
                ssh: true,
                direct_remote: true,
                syncthing_status: true,
                secure_secrets: true,
            },
        }
    }

    /// Применяет изменения значений устройства, как это делает оболочка.
    pub fn absorb_device_update(&mut self) {
        if let Some(update) = self.session.take_device_update() {
            self.device_sync = update;
        }
    }

    pub fn tick_clock(&mut self) {
        self.now += 1_000;
    }

    pub async fn put(&mut self, path: &str, text: &str) {
        self.vault
            .write_bytes(Path::new(path), text.as_bytes())
            .await
            .unwrap();
    }

    pub fn text(&self, path: &str) -> Option<String> {
        self.vault
            .bytes_of(path)
            .map(|bytes| String::from_utf8(bytes).unwrap())
    }

    pub async fn enable(&mut self) {
        let ctx = self.ctx();
        self.session
            .enable_history(&mut self.vault, &ctx, "Первый снимок")
            .await
            .unwrap();
        self.absorb_device_update();
        self.tick_clock();
    }

    pub async fn snap(&mut self, message: &str) -> String {
        let ctx = self.ctx();
        let snapshot = self
            .session
            .snapshot(&self.vault, &ctx, None, message, "Снимок", true, false)
            .await
            .unwrap();
        self.tick_clock();
        snapshot.id
    }

    pub async fn connect(&mut self) {
        let ctx = self.ctx();
        self.session
            .connect_remote(
                &mut self.vault,
                &self.secrets,
                &ctx,
                REMOTE,
                Credentials {
                    user: None,
                    token: Some("t0ken".into()),
                },
            )
            .await
            .unwrap();
    }

    pub async fn push(&mut self) -> Result<(), SyncError> {
        let ctx = self.ctx();
        self.session
            .push(&self.vault, &self.secrets, &ctx, false)
            .await
            .map(|_| ())
    }

    pub async fn pull(&mut self) -> Result<session::PullOutcome, SyncError> {
        let ctx = self.ctx();
        let result = self
            .session
            .pull(&mut self.vault, &self.secrets, &ctx, false)
            .await;
        self.tick_clock();
        result
    }

    pub async fn history_len(&self, path: Option<&str>) -> usize {
        self.session
            .history(&self.vault, path, None, 200)
            .await
            .unwrap()
            .snapshots
            .len()
    }
}

/// Второе устройство: клон того же «сервера».
async fn clone_of(remote: &Rc<RefCell<FakeRepo>>) -> Fixture {
    let mut fixture = fixture(&[], Some(remote.clone()));
    let engine = engine_for(&fixture.vault, Some(remote.clone()));
    session::clone_vault(
        &engine,
        &fixture.secrets,
        "vault-1",
        &fixture.ctx().capabilities,
        REMOTE,
        Credentials::default(),
        None,
        &CancelToken::new(),
    )
    .await
    .unwrap();
    fixture.session = SyncSession::new(engine, None, "vault-1");
    fixture.device_id = OTHER;
    fixture.now += 500;
    fixture.connect().await;
    fixture
}

// ── US1: история и снимки ─────────────────────────────────────────────────

pub async fn enabling_history_writes_rules_and_the_first_snapshot() {
    let mut f = fixture(
        &[
            ("a.typ", "= A\n"),
            ("b.typ", "= B\n"),
            (".typsidian/cache.sqlite", "кэш"),
            (".typsidian/vault.json", "{}"),
            (".gitignore", "*.pdf\n"),
        ],
        None,
    );
    f.enable().await;
    let gitignore = f.text(".gitignore").unwrap();
    assert!(gitignore.starts_with("*.pdf\n"));
    assert!(gitignore.contains("# typsidian:begin"));
    assert!(gitignore.contains("/.typsidian/cache.sqlite*"));
    assert_eq!(f.history_len(None).await, 1);
    // Кэш исключён, остальное — в снимке.
    assert!(f.session.changes(&f.vault).await.unwrap().is_empty());
    assert!(f.device_sync.last_seen_history);
}

pub async fn syncthing_rules_come_first_and_can_be_declined() {
    let mut f = fixture(&[("a.typ", "= A\n"), (".stfolder/.keep", "")], None);
    let ctx = f.ctx();
    assert_eq!(
        f.session
            .enable_history(&mut f.vault, &ctx, "x")
            .await
            .unwrap_err(),
        SyncError::StIgnoreNotConfirmed
    );
    f.session
        .apply_ignore(&mut f.vault, &ctx, IgnoreFile::StIgnore)
        .await
        .unwrap();
    assert!(f
        .text(".stignore")
        .unwrap()
        .starts_with("// typsidian:begin"));
    f.enable().await;
    assert_eq!(f.history_len(None).await, 1);

    // Отказ от правил тоже открывает путь — но только явный.
    let mut g = fixture(&[("a.typ", "= A\n"), (".stfolder/.keep", "")], None);
    let ctx = g.ctx();
    g.session.decline_stignore(&ctx);
    g.absorb_device_update();
    assert!(g.device_sync.stignore_declined);
    g.enable().await;
    assert!(g.text(".stignore").is_none());
}

pub async fn existing_history_is_not_enabled_twice() {
    let mut f = fixture(&[("a.typ", "= A\n")], None);
    f.enable().await;
    let ctx = f.ctx();
    assert_eq!(
        f.session
            .enable_history(&mut f.vault, &ctx, "x")
            .await
            .unwrap_err(),
        SyncError::HistoryExists
    );
}

pub async fn a_snapshot_of_selected_files_leaves_the_rest_pending() {
    let mut f = fixture(&[("a.typ", "= A\n"), ("b.typ", "= B\n")], None);
    f.enable().await;
    f.put("a.typ", "= A2\n").await;
    f.put("b.typ", "= B2\n").await;
    let ctx = f.ctx();
    f.session
        .snapshot(
            &f.vault,
            &ctx,
            Some(vec!["a.typ".into()]),
            "только A",
            "Снимок",
            true,
            false,
        )
        .await
        .unwrap();
    let changes = f.session.changes(&f.vault).await.unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, "b.typ");
    assert_eq!(changes[0].change, ChangeKind::Modified);
}

pub async fn an_empty_message_lists_the_changed_notes() {
    let mut f = fixture(&[("a.typ", "= A\n")], None);
    f.enable().await;
    f.put("a.typ", "= A2\n").await;
    f.put("Новая.typ", "= Н\n").await;
    let ctx = f.ctx();
    let snapshot = f
        .session
        .snapshot(&f.vault, &ctx, None, "  ", "Снимок", true, false)
        .await
        .unwrap();
    // Порядок — по пути: латиница раньше кириллицы.
    assert_eq!(snapshot.summary, "Снимок: a, Новая");
}

pub async fn a_snapshot_needs_saved_notes_and_an_author() {
    let mut f = fixture(&[("a.typ", "= A\n")], None);
    f.enable().await;
    f.put("a.typ", "= A2\n").await;
    let ctx = f.ctx();
    assert_eq!(
        f.session
            .snapshot(&f.vault, &ctx, None, "", "Снимок", false, false)
            .await
            .unwrap_err(),
        SyncError::NotFlushed
    );
    let mut anonymous = ctx.clone();
    anonymous.device_sync.author = None;
    assert_eq!(
        f.session
            .snapshot(&f.vault, &anonymous, None, "", "Снимок", true, false)
            .await
            .unwrap_err(),
        SyncError::AuthorRequired
    );
}

pub async fn a_rename_is_one_change_and_both_names_go_into_the_snapshot() {
    let mut f = fixture(&[("a.typ", "= A\n")], None);
    f.enable().await;
    f.vault.remove(Path::new("a.typ"));
    f.put("c.typ", "= A\n").await;
    let changes = f.session.changes(&f.vault).await.unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0].change,
        ChangeKind::Renamed {
            from: "a.typ".into()
        }
    );
    let ctx = f.ctx();
    f.session
        .snapshot(
            &f.vault,
            &ctx,
            Some(vec!["c.typ".into()]),
            "переименование",
            "",
            true,
            false,
        )
        .await
        .unwrap();
    assert!(f.session.changes(&f.vault).await.unwrap().is_empty());
}

pub async fn local_history_works_without_network() {
    let remote = FakeGitEngine::new_remote();
    let mut f = fixture(&[("a.typ", "= A\n")], Some(remote));
    f.session.engine().set_network(FakeNetwork::Offline);
    f.enable().await;
    f.put("a.typ", "= A2\n").await;
    f.snap("офлайн").await;
    assert_eq!(f.history_len(None).await, 2);
}

// ── US2: история файла и откат ─────────────────────────────────────────────

pub async fn file_history_follows_renames_and_restore_does_not_snapshot() {
    let mut f = fixture(&[("a.typ", "v1\n"), ("other.typ", "x\n")], None);
    f.enable().await;
    let first = f
        .session
        .history(&f.vault, None, None, 1)
        .await
        .unwrap()
        .snapshots[0]
        .id
        .clone();
    f.put("a.typ", "v2\n").await;
    f.snap("v2").await;
    f.vault.remove(Path::new("a.typ"));
    f.put("c.typ", "v2\n").await;
    f.snap("переименование").await;
    f.put("c.typ", "v3\n").await;
    f.snap("v3").await;
    f.put("other.typ", "y\n").await;
    f.snap("чужая заметка").await;

    let history = f
        .session
        .history(&f.vault, Some("c.typ"), None, 50)
        .await
        .unwrap();
    let names: Vec<_> = history
        .snapshots
        .iter()
        .map(|s| s.path.clone().unwrap())
        .collect();
    assert_eq!(names, ["c.typ", "c.typ", "a.typ", "a.typ"]);
    assert_eq!(history.cursor, None);

    let before = f.history_len(None).await;
    let restored = f.session.restore(&mut f.vault, "c.typ", &first).await;
    // В первом снимке файла `c.typ` не было — откат к нему вернул бы удаление.
    assert_eq!(restored.unwrap(), Content::Missing);
    f.put("c.typ", "v3\n").await;
    f.session
        .restore(&mut f.vault, "a.typ", &first)
        .await
        .unwrap();
    assert_eq!(f.text("a.typ").as_deref(), Some("v1\n"));
    assert_eq!(f.history_len(None).await, before);
}

pub async fn diff_shows_both_sides_and_binary_files_by_size() {
    let mut f = fixture(&[("a.typ", "один\nдва\n")], None);
    f.enable().await;
    f.put("a.typ", "один\nтри\n").await;
    let diff = f
        .session
        .diff(&f.vault, "a.typ", &Revision::Head, &Revision::Working)
        .await
        .unwrap();
    let typsidian_core::sync::FileDiff::Text { lines } = diff else {
        panic!("ожидался текст");
    };
    use typsidian_core::vault::conflict::DiffLine;
    assert!(lines.contains(&DiffLine::Local {
        text: "три".into()
    }));
    assert!(lines.contains(&DiffLine::External {
        text: "два".into()
    }));

    f.vault
        .write_bytes(Path::new("img.png"), &[0x89, b'P', 0, 1])
        .await
        .unwrap();
    let binary = f
        .session
        .diff(&f.vault, "img.png", &Revision::Head, &Revision::Working)
        .await
        .unwrap();
    assert_eq!(
        binary,
        typsidian_core::sync::FileDiff::Binary {
            old_size: None,
            new_size: Some(4)
        }
    );
}

pub async fn a_deleted_note_is_restored_in_place() {
    let mut f = fixture(&[("Папка/a.typ", "текст\n")], None);
    f.enable().await;
    let id = f
        .session
        .history(&f.vault, None, None, 1)
        .await
        .unwrap()
        .snapshots[0]
        .id
        .clone();
    f.vault.remove(Path::new("Папка/a.typ"));
    f.snap("удаление").await;

    // История vault называет удалённый файл и снимок, где он ещё был: по ним
    // интерфейс и предлагает восстановление (FR-011).
    let deletion = f
        .session
        .history(&f.vault, None, None, 1)
        .await
        .unwrap()
        .snapshots
        .remove(0);
    let changes = f
        .session
        .snapshot_changes(&f.vault, &deletion.id)
        .await
        .unwrap();
    assert_eq!(
        changes,
        vec![Change {
            path: "Папка/a.typ".to_string(),
            change: ChangeKind::Deleted,
        }]
    );
    assert_eq!(deletion.parents.first(), Some(&id));

    f.session
        .restore(&mut f.vault, "Папка/a.typ", &id)
        .await
        .unwrap();
    assert_eq!(f.text("Папка/a.typ").as_deref(), Some("текст\n"));
}

// ── US3: удалённый репозиторий ─────────────────────────────────────────────

pub async fn the_token_never_lands_in_the_vault() {
    let remote = FakeGitEngine::new_remote();
    let mut f = fixture(&[("a.typ", "= A\n")], Some(remote));
    f.enable().await;
    let ctx = f.ctx();
    f.session
        .connect_remote(
            &mut f.vault,
            &f.secrets,
            &ctx,
            "https://me:ghp_SECRET@example.com/me/notes.git",
            Credentials::default(),
        )
        .await
        .unwrap();
    let settings = sync_settings::read(&f.vault).await;
    assert_eq!(settings.remote_url.as_deref(), Some(REMOTE));
    for path in f.vault.paths_under("") {
        let bytes = f.vault.bytes_of(&path).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains("ghp_SECRET"),
            "{path}"
        );
    }
    use typsidian_core::sync::secrets::{SecretKind, SecretStore};
    assert_eq!(
        f.secrets
            .get("vault-1", SecretKind::RemoteToken)
            .await
            .unwrap()
            .as_deref(),
        Some("ghp_SECRET")
    );
}

pub async fn offline_exchange_fails_and_keeps_local_snapshots() {
    let remote = FakeGitEngine::new_remote();
    let mut f = fixture(&[("a.typ", "= A\n")], Some(remote));
    f.enable().await;
    f.session.engine().set_network(FakeNetwork::Offline);
    let ctx = f.ctx();
    let result = f
        .session
        .connect_remote(
            &mut f.vault,
            &f.secrets,
            &ctx,
            REMOTE,
            Credentials::default(),
        )
        .await;
    assert_eq!(result.unwrap_err(), SyncError::Offline);
    // Адрес сохранён, причина видна в сводке.
    let overview = f.session.overview(&f.vault, &ctx).await;
    let remote = overview.remote.unwrap();
    assert_eq!(remote.last_error, Some(SyncError::Offline));
    assert_eq!(f.push().await.unwrap_err(), SyncError::Offline);
    assert_eq!(f.history_len(None).await, 1);
}

pub async fn push_and_fast_forward_pull_between_devices() {
    let remote = FakeGitEngine::new_remote();
    let mut a = fixture(&[("a.typ", "= A\n")], Some(remote.clone()));
    a.enable().await;
    a.connect().await;
    a.push().await.unwrap();

    let mut b = clone_of(&remote).await;
    assert_eq!(b.text("a.typ").as_deref(), Some("= A\n"));

    a.put("a.typ", "= A2\n").await;
    a.snap("правка").await;
    a.push().await.unwrap();
    let outcome = b.pull().await.unwrap();
    assert_eq!(outcome.applied, 1);
    assert_eq!(outcome.conflicts, 0);
    assert_eq!(b.text("a.typ").as_deref(), Some("= A2\n"));
    assert!(b.pull().await.unwrap().up_to_date);
    assert!(b.session.changes(&b.vault).await.unwrap().is_empty());
}

const BASE: &str = "= Заметка\n\nПервый абзац.\n\nВторой абзац.\n\nТретий абзац.\n";

pub async fn disjoint_edits_merge_without_asking() {
    let remote = FakeGitEngine::new_remote();
    let mut a = fixture(&[("n.typ", BASE)], Some(remote.clone()));
    a.enable().await;
    a.connect().await;
    a.push().await.unwrap();
    let mut b = clone_of(&remote).await;

    a.put("n.typ", &BASE.replace("Первый абзац.", "Первый, правка A."))
        .await;
    a.snap("A").await;
    a.push().await.unwrap();
    b.put("n.typ", &BASE.replace("Третий абзац.", "Третий, правка B."))
        .await;
    b.snap("B").await;

    let outcome = b.pull().await.unwrap();
    assert_eq!(outcome.conflicts, 0);
    let merged = b.text("n.typ").unwrap();
    assert!(merged.contains("правка A") && merged.contains("правка B"));
    let head = b.session.history(&b.vault, None, None, 1).await.unwrap();
    assert_eq!(head.snapshots[0].parents.len(), 2);
    b.push().await.unwrap();
}

pub async fn a_conflict_blocks_snapshots_and_abort_restores_files() {
    let remote = FakeGitEngine::new_remote();
    let mut a = fixture(&[("n.typ", BASE)], Some(remote.clone()));
    a.enable().await;
    a.connect().await;
    a.push().await.unwrap();
    let mut b = clone_of(&remote).await;

    a.put("n.typ", &BASE.replace("Второй абзац.", "Второй — A."))
        .await;
    a.snap("A").await;
    a.push().await.unwrap();
    let ours = BASE.replace("Второй абзац.", "Второй — B.");
    b.put("n.typ", &ours).await;
    b.snap("B").await;

    let outcome = b.pull().await.unwrap();
    assert_eq!(outcome.conflicts, 1);
    let conflicts = b.session.conflicts(&b.vault).await;
    assert_eq!(conflicts.len(), 1);
    assert!(matches!(conflicts[0].source, ConflictSource::Pull { .. }));
    assert!(conflicts[0].base.is_some());

    b.put("n.typ", "ещё правка\n").await;
    let ctx = b.ctx();
    assert_eq!(
        b.session
            .snapshot(&b.vault, &ctx, None, "", "", true, false)
            .await
            .unwrap_err(),
        SyncError::PullConflictsPending
    );
    assert_eq!(b.push().await.unwrap_err(), SyncError::PullConflictsPending);
    b.put("n.typ", &ours).await;
    b.session.abort_pull(&mut b.vault).await.unwrap();
    assert_eq!(b.text("n.typ").as_deref(), Some(ours.as_str()));
    assert!(b.session.conflicts(&b.vault).await.is_empty());
}

pub async fn a_resolved_conflict_completes_the_pull() {
    let remote = FakeGitEngine::new_remote();
    let mut a = fixture(&[("n.typ", BASE)], Some(remote.clone()));
    a.enable().await;
    a.connect().await;
    a.push().await.unwrap();
    let mut b = clone_of(&remote).await;
    a.put("n.typ", &BASE.replace("Второй абзац.", "Второй — A."))
        .await;
    a.snap("A").await;
    a.push().await.unwrap();
    b.put("n.typ", &BASE.replace("Второй абзац.", "Второй — B."))
        .await;
    b.snap("B").await;
    b.pull().await.unwrap();

    let ctx = b.ctx();
    b.session
        .resolve(
            &mut b.vault,
            &ctx,
            "pull:n.typ",
            Resolution::Merged {
                text: "совмещено\n".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(b.text("n.typ").as_deref(), Some("совмещено\n"));
    assert!(b.session.conflicts(&b.vault).await.is_empty());
    b.push().await.unwrap();
    assert!(a.pull().await.is_ok());
    assert_eq!(a.text("n.typ").as_deref(), Some("совмещено\n"));
}

pub async fn a_push_behind_the_server_is_rejected_not_forced() {
    let remote = FakeGitEngine::new_remote();
    let mut a = fixture(&[("a.typ", "= A\n")], Some(remote.clone()));
    a.enable().await;
    a.connect().await;
    a.push().await.unwrap();
    let mut b = clone_of(&remote).await;
    a.put("a.typ", "= A2\n").await;
    a.snap("A").await;
    a.push().await.unwrap();
    b.put("b.typ", "= B\n").await;
    b.snap("B").await;
    assert_eq!(
        b.push().await.unwrap_err(),
        SyncError::RejectedNonFastForward
    );
}

pub async fn disconnecting_keeps_history_and_files() {
    let remote = FakeGitEngine::new_remote();
    let mut f = fixture(&[("a.typ", "= A\n")], Some(remote));
    f.enable().await;
    f.connect().await;
    f.session
        .disconnect_remote(&mut f.vault, &f.secrets)
        .await
        .unwrap();
    assert_eq!(f.history_len(None).await, 1);
    assert_eq!(f.text("a.typ").as_deref(), Some("= A\n"));
    assert_eq!(sync_settings::read(&f.vault).await.remote_url, None);
}

// ── US4: конфликтные копии Syncthing ───────────────────────────────────────

const COPY: &str = "n.sync-conflict-20261003-120000-ABCDEFG.typ";

pub async fn syncthing_copies_merge_against_the_last_snapshot() {
    let mut f = fixture(&[("n.typ", BASE)], None);
    f.enable().await;
    f.put("n.typ", &BASE.replace("Первый абзац.", "Первый — здесь."))
        .await;
    f.put(COPY, &BASE.replace("Третий абзац.", "Третий — там."))
        .await;
    let conflicts = f.session.conflicts(&f.vault).await;
    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(conflict.path, "n.typ");
    assert!(matches!(
        &conflict.source,
        ConflictSource::Syncthing { device, .. } if device == "ABCDEFG"
    ));
    let Some(typsidian_core::sync::merge::MergeResult::Clean { text }) = &conflict.auto_merge
    else {
        panic!("ожидалось чистое совмещение: {:?}", conflict.auto_merge);
    };
    assert!(text.contains("здесь") && text.contains("там"));

    let ctx = f.ctx();
    let merged = text.clone();
    f.session
        .resolve(
            &mut f.vault,
            &ctx,
            &conflict.id,
            Resolution::Merged { text: merged },
        )
        .await
        .unwrap();
    assert!(f.text(COPY).is_none());
    assert!(f.text("n.typ").unwrap().contains("там"));
}

pub async fn a_copy_can_be_taken_or_kept_as_a_separate_file() {
    let mut f = fixture(&[("n.typ", "наше\n"), (COPY, "их\n")], None);
    let ctx = f.ctx();
    let id = format!("st:{COPY}");
    f.session
        .resolve(
            &mut f.vault,
            &ctx,
            &id,
            Resolution::KeepBothAsFile {
                new_path: "n (копия).typ".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(f.text("n (копия).typ").as_deref(), Some("их\n"));
    assert!(f.text(COPY).is_none());

    f.put(COPY, "снова их\n").await;
    f.session
        .resolve(&mut f.vault, &ctx, &id, Resolution::TakeTheirs)
        .await
        .unwrap();
    assert_eq!(f.text("n.typ").as_deref(), Some("снова их\n"));
}

pub async fn conflict_copies_never_become_notes() {
    use typsidian_core::vault::{classify, FileKind};
    assert_eq!(classify(Path::new(COPY)), FileKind::Other);
    assert_eq!(classify(Path::new("n.typ")), FileKind::Note);
}

// ── US5: git и Syncthing вместе ────────────────────────────────────────────

async fn set_keeper(f: &mut Fixture, device_id: &str) {
    let mut settings = sync_settings::read(&f.vault).await;
    settings.keeper = Some(Keeper {
        device_id: device_id.to_string(),
        device_name: "ноутбук".into(),
        since: 1,
    });
    sync_settings::write(&mut f.vault, &settings).await.unwrap();
}

pub async fn a_non_keeper_needs_explicit_confirmation() {
    let mut f = fixture(&[("a.typ", "= A\n")], None);
    f.enable().await;
    set_keeper(&mut f, OTHER).await;
    f.put("a.typ", "= A2\n").await;
    let ctx = f.ctx();
    let error = f
        .session
        .snapshot(&f.vault, &ctx, None, "", "", true, false)
        .await
        .unwrap_err();
    assert!(matches!(error, SyncError::KeeperElsewhere { keeper } if keeper.device_id == OTHER));
    f.session
        .snapshot(&f.vault, &ctx, None, "", "", true, true)
        .await
        .unwrap();
}

pub async fn appointing_the_keeper_is_visible_and_a_settings_copy_is_a_dispute() {
    let mut f = fixture(&[("a.typ", "= A\n"), (".stfolder/.keep", "")], None);
    let ctx = f.ctx();
    f.session.decline_stignore(&ctx);
    f.absorb_device_update();
    f.enable().await;
    let ctx = f.ctx();
    let keeper = f.session.set_keeper(&mut f.vault, &ctx).await.unwrap();
    assert_eq!(keeper.device_id, ME);
    f.absorb_device_update();
    assert_eq!(f.device_sync.last_seen_keeper.as_deref(), Some(ME));
    let ctx = f.ctx();
    let overview = f.session.overview(&f.vault, &ctx).await;
    assert_eq!(overview.keeper_state, KeeperState::ThisDevice);
    assert!(overview.syncthing.detected);

    f.put(
        ".typsidian/vault.sync-conflict-20261003-120000-ABCDEFG.json",
        "{}",
    )
    .await;
    let overview = f.session.overview(&f.vault, &ctx).await;
    assert_eq!(overview.keeper_state, KeeperState::Disputed);
    f.put("a.typ", "= A2\n").await;
    assert_eq!(
        f.session
            .snapshot(&f.vault, &ctx, None, "", "", true, false)
            .await
            .unwrap_err(),
        SyncError::KeeperDisputed
    );
}

pub async fn missing_syncthing_rules_are_reported_with_history() {
    let mut f = fixture(&[("a.typ", "= A\n"), (".stfolder/.keep", "")], None);
    let ctx = f.ctx();
    f.session.decline_stignore(&ctx);
    f.absorb_device_update();
    f.enable().await;
    // Отказ снят — правил нет, история есть: предупреждение обязано быть.
    f.device_sync.stignore_declined = false;
    let ctx = f.ctx();
    let overview = f.session.overview(&f.vault, &ctx).await;
    assert_eq!(overview.ignore_warnings.len(), 1);
    assert!(overview.ignore_warnings[0]
        .missing
        .contains(&"/.git".to_string()));
    f.session
        .apply_ignore(&mut f.vault, &ctx, IgnoreFile::StIgnore)
        .await
        .unwrap();
    assert!(f
        .session
        .overview(&f.vault, &ctx)
        .await
        .ignore_warnings
        .is_empty());
}

pub async fn the_keeper_snapshots_after_the_pause() {
    let mut f = fixture(&[("a.typ", "= A\n")], None);
    f.enable().await;
    let ctx = f.ctx();
    f.session.set_keeper(&mut f.vault, &ctx).await.unwrap();
    f.session
        .update_settings(
            &mut f.vault,
            &sync_settings::VaultSyncSettingsPatch {
                auto_snapshot: Some(true),
                auto_snapshot_pause_secs: Some(30),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    f.put("a.typ", "= A2\n").await;
    f.session.note_change(f.now);
    let ctx = f.ctx();
    let early = f
        .session
        .tick(&mut f.vault, &f.secrets, &ctx, "Автоснимок")
        .await;
    assert_eq!(
        early.decision,
        AutoDecision::Wait {
            until: f.now + 30_000
        }
    );
    f.now += 31_000;
    let ctx = f.ctx();
    let due = f
        .session
        .tick(&mut f.vault, &f.secrets, &ctx, "Автоснимок")
        .await;
    assert_eq!(due.decision, AutoDecision::Snapshot);
    // В снимок попадает и `vault.json`, изменённый назначением хранителя.
    let summary = due.snapshot.unwrap().summary;
    assert!(summary.starts_with("Автоснимок: "), "{summary}");
    assert!(
        summary.split(", ").any(|name| name.ends_with('a')),
        "{summary}"
    );
}

pub async fn a_non_keeper_never_snapshots_automatically() {
    let mut f = fixture(&[("a.typ", "= A\n")], None);
    f.enable().await;
    set_keeper(&mut f, OTHER).await;
    f.session
        .update_settings(
            &mut f.vault,
            &sync_settings::VaultSyncSettingsPatch {
                auto_snapshot: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    f.put("a.typ", "= A2\n").await;
    f.now += 10_000_000;
    let ctx = f.ctx();
    let outcome = f.session.tick(&mut f.vault, &f.secrets, &ctx, "").await;
    assert!(outcome.snapshot.is_none());
    assert_eq!(f.history_len(None).await, 1);
}

pub async fn a_non_keeper_reads_history_from_the_mirror() {
    let remote = FakeGitEngine::new_remote();
    let mut keeper = fixture(&[("a.typ", "v1\n")], Some(remote.clone()));
    keeper.enable().await;
    keeper.connect().await;
    let ctx = keeper.ctx();
    keeper
        .session
        .set_keeper(&mut keeper.vault, &ctx)
        .await
        .unwrap();
    keeper.put("a.typ", "v2\n").await;
    keeper.snap("v2").await;
    keeper.push().await.unwrap();

    // Второе устройство: файлы пришли бы Syncthing, истории в папке нет.
    let mut other = fixture(
        &[
            ("a.typ", "v2\n"),
            (
                ".typsidian/vault.json",
                &keeper.text(".typsidian/vault.json").unwrap(),
            ),
        ],
        Some(remote.clone()),
    );
    other.device_id = OTHER;
    other.session = SyncSession::new(
        engine_for(&other.vault, Some(remote.clone())),
        Some(FakeGitEngine::bare().with_remote(remote.clone())),
        "vault-1",
    );
    // Адрес пришёл вместе с настройками vault, и на этом устройстве его ещё
    // никто не подтверждал — обмена нет, пока не подтвердят (T068).
    let ctx = other.ctx();
    assert_eq!(
        other
            .session
            .refresh_mirror(&other.vault, &other.secrets, &ctx)
            .await,
        Err(SyncError::RemoteChanged {
            url: REMOTE.to_string()
        })
    );
    other.connect().await;
    let ctx = other.ctx();
    other
        .session
        .refresh_mirror(&other.vault, &other.secrets, &ctx)
        .await
        .unwrap();
    let overview = other.session.overview(&other.vault, &ctx).await;
    assert!(matches!(overview.history, HistoryState::Mirror { .. }));
    assert_eq!(overview.keeper_state, KeeperState::OtherDevice);
    let history = other
        .session
        .history(&other.vault, Some("a.typ"), None, 10)
        .await
        .unwrap();
    assert_eq!(history.snapshots.len(), 2);
    let first = history.snapshots[1].id.clone();
    other
        .session
        .restore(&mut other.vault, "a.typ", &first)
        .await
        .unwrap();
    assert_eq!(other.text("a.typ").as_deref(), Some("v1\n"));
}

pub async fn a_lost_history_is_reported_as_lost() {
    let mut f = fixture(&[("a.typ", "= A\n")], None);
    f.device_sync.last_seen_history = true;
    let ctx = f.ctx();
    let overview = f.session.overview(&f.vault, &ctx).await;
    assert_eq!(overview.history, HistoryState::Lost);
}

// ── US6: возможности оболочек ──────────────────────────────────────────────

pub async fn shells_reject_what_they_cannot_do() {
    let remote = FakeGitEngine::new_remote();
    let mut f = fixture(&[("a.typ", "= A\n")], Some(remote));
    f.enable().await;
    let mut ctx = f.ctx();
    ctx.capabilities = ShellCapabilities::current(true);
    ctx.capabilities.ssh = false;
    assert_eq!(
        f.session
            .connect_remote(
                &mut f.vault,
                &f.secrets,
                &ctx,
                "git@github.com:me/notes.git",
                Credentials::default()
            )
            .await
            .unwrap_err(),
        SyncError::UnsupportedScheme {
            scheme: "ssh".into()
        }
    );
    ctx.capabilities.direct_remote = false;
    assert!(matches!(
        f.session
            .connect_remote(
                &mut f.vault,
                &f.secrets,
                &ctx,
                "/mnt/usb/notes.git",
                Credentials::default()
            )
            .await
            .unwrap_err(),
        SyncError::UnsupportedScheme { .. }
    ));
    assert!(f
        .session
        .connect_remote(
            &mut f.vault,
            &f.secrets,
            &ctx,
            "ftp://example.com/x",
            Credentials::default()
        )
        .await
        .is_err());
}

pub async fn settings_survive_in_vault_json_next_to_other_keys() {
    let mut f = fixture(
        &[(
            ".typsidian/vault.json",
            "{\"assets\": {\"placement\": \"vault-root\"}}",
        )],
        None,
    );
    let patched = f
        .session
        .update_settings(
            &mut f.vault,
            &sync_settings::VaultSyncSettingsPatch {
                auto_push: Some(true),
                auto_pull_interval_secs: Some(5),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(patched.auto_push);
    assert_eq!(patched.auto_pull_interval_secs, Some(60));
    let text = f.text(".typsidian/vault.json").unwrap();
    assert!(text.contains("vault-root"));
    assert_eq!(sync_settings::read(&f.vault).await, patched);
    let _ = VaultSyncSettings::default();
}

// ── T068: адрес из общих настроек не доверяется без подтверждения ──────────

pub async fn a_remote_address_changed_elsewhere_gets_neither_snapshots_nor_the_token() {
    let remote = FakeGitEngine::new_remote();
    let mut f = fixture(&[("a.typ", "= A\n")], Some(remote));
    f.enable().await;
    f.connect().await;
    f.snap("адрес").await;
    f.push().await.unwrap();

    // Настройки vault доставляются вместе с ним: адрес в них мог поменять
    // соавтор репозитория или другой участник папки Syncthing.
    let mut settings = sync_settings::read(&f.vault).await;
    settings.remote_url = Some("https://evil.example/steal.git".to_string());
    settings.auto_push = true;
    settings.auto_pull_on_open = true;
    sync_settings::write(&mut f.vault, &settings).await.unwrap();

    let changed = Err(SyncError::RemoteChanged {
        url: "https://evil.example/steal.git".to_string(),
    });
    assert_eq!(f.push().await, changed);
    assert_eq!(f.pull().await.map(|_| ()), changed);

    // Автоматика по такту тоже никуда не идёт — отказ уходит в индикатор.
    let ctx = f.ctx();
    let outcome = f
        .session
        .tick(&mut f.vault, &f.secrets, &ctx, "Снимок")
        .await;
    assert!(!outcome.pulled && !outcome.pushed);

    // Подтверждение на этом устройстве возвращает обмен — уже новому адресу и
    // с теми данными доступа, что ввели для него.
    f.session
        .connect_remote(
            &mut f.vault,
            &f.secrets,
            &ctx,
            REMOTE,
            Credentials {
                user: None,
                token: Some("t0ken".into()),
            },
        )
        .await
        .unwrap();
    f.snap("после подтверждения").await;
    f.push().await.unwrap();
}

pub async fn pulled_paths_that_alias_the_history_folder_are_refused() {
    let remote = FakeGitEngine::new_remote();
    let mut a = fixture(&[("a.typ", "= A\n")], Some(remote.clone()));
    a.enable().await;
    a.connect().await;
    a.snap("адрес").await;
    a.push().await.unwrap();
    let mut b = clone_of(&remote).await;

    // Файл, который Windows прочтёт как `.git/config`.
    a.put(".git./config", "[core]\n\tfsmonitor = calc.exe\n")
        .await;
    a.put("b.typ", "= B\n").await;
    a.snap("ловушка").await;
    a.push().await.unwrap();

    let error = b.pull().await.unwrap_err();
    assert!(matches!(error, SyncError::Io { .. }), "{error:?}");
    // Получение откатилось целиком: ни ловушки, ни соседних файлов.
    assert_eq!(b.text(".git./config"), None);
    assert_eq!(b.text("b.typ"), None);
}

// ── Редактор правил исключения (фича 012) ──────────────────────────────────

fn save_request(document: &IgnoreDocument, text: &str) -> IgnoreSaveRequest {
    IgnoreSaveRequest {
        file: document.file,
        text: text.to_string(),
        line_ending: document.line_ending,
        base: document.base.clone(),
        confirm_incomplete: false,
        overwrite: false,
    }
}

/// Файл правил со всеми обязательными правилами и строкой пользователя.
fn full_rules(file: IgnoreFile, user: &str) -> String {
    ignore::restore_block(file, user)
}

impl Fixture {
    async fn save_rules(&mut self, request: IgnoreSaveRequest) -> IgnoreSaveOutcome {
        let ctx = self.ctx();
        self.session
            .ignore_save(&mut self.vault, &ctx, request)
            .await
            .unwrap()
    }

    /// Запись в обход сессии — «другая программа» или Syncthing.
    fn write_outside(&self, path: &str, bytes: &[u8]) {
        self.vault
            .files_handle()
            .borrow_mut()
            .insert(PathBuf::from(path), bytes.to_vec());
    }
}

pub async fn ignore_editor_opens_an_existing_file_and_saves_the_edit() {
    let on_disk = full_rules(IgnoreFile::GitIgnore, "*.pdf\n");
    let mut f = fixture(&[("a.typ", "= A\n"), (".gitignore", &on_disk)], None);
    f.enable().await;
    f.session.poll_events();

    let document = f.session.ignore_open(&f.vault, IgnoreFile::GitIgnore).await;
    assert!(document.exists);
    assert_eq!(document.editable, Editability::Editable);
    assert!(document.in_effect);
    assert_eq!(document.text, on_disk);
    assert!(document.check.missing.is_empty());
    assert!(document.check.block.is_some());

    let edited = format!("черновики/\n{on_disk}");
    let outcome = f.save_rules(save_request(&document, &edited)).await;
    let IgnoreSaveOutcome::Saved { document: saved } = outcome else {
        panic!("ожидалось сохранение: {outcome:?}");
    };
    assert_eq!(f.text(".gitignore").unwrap(), edited);
    assert_eq!(saved.text, edited);
    assert_ne!(saved.base, document.base);
    // Сводка перечитывается, а сам файл правил — обычное изменение vault.
    assert!(f
        .session
        .poll_events()
        .contains(&SyncEvent::OverviewChanged));
    let changes = f.session.changes(&f.vault).await.unwrap();
    assert!(changes.iter().any(|change| change.path == ".gitignore"));
}

pub async fn ignore_editor_offers_the_block_for_an_absent_file_without_creating_it() {
    let mut f = fixture(&[("a.typ", "= A\n")], None);
    let document = f.session.ignore_open(&f.vault, IgnoreFile::StIgnore).await;
    assert!(!document.exists);
    assert_eq!(document.base, None);
    assert_eq!(document.editable, Editability::Editable);
    // Ни истории, ни Syncthing: правила пока не действуют, но правятся.
    assert!(!document.in_effect);
    assert!(document.text.starts_with("// typsidian:begin"));
    assert!(document.check.missing.is_empty());
    assert_eq!(f.text(".stignore"), None);

    let text = document.text.clone();
    let outcome = f.save_rules(save_request(&document, &text)).await;
    assert!(matches!(outcome, IgnoreSaveOutcome::Saved { .. }));
    assert_eq!(f.text(".stignore").unwrap(), text);

    let git = f.session.ignore_open(&f.vault, IgnoreFile::GitIgnore).await;
    assert!(!git.in_effect);
}

pub async fn ignore_editor_keeps_crlf_line_endings() {
    let unix = full_rules(IgnoreFile::GitIgnore, "*.pdf\n");
    let dos = unix.replace('\n', "\r\n");
    let mut f = fixture(&[(".gitignore", &dos)], None);
    let document = f.session.ignore_open(&f.vault, IgnoreFile::GitIgnore).await;
    assert_eq!(document.line_ending, LineEnding::CrLf);
    assert_eq!(document.text, unix);

    let edited = format!("*.log\n{unix}");
    let outcome = f.save_rules(save_request(&document, &edited)).await;
    assert!(matches!(outcome, IgnoreSaveOutcome::Saved { .. }));
    assert_eq!(f.text(".gitignore").unwrap(), edited.replace('\n', "\r\n"));
}

pub async fn ignore_editor_does_not_touch_what_it_cannot_edit() {
    let mut f = fixture(&[("a.typ", "= A\n")], None);
    f.write_outside(".gitignore", &[0xff, 0xfe, 0x00]);
    let document = f.session.ignore_open(&f.vault, IgnoreFile::GitIgnore).await;
    assert_eq!(document.editable, Editability::NotText);
    assert_eq!(document.text, "");
    let outcome = f.save_rules(save_request(&document, "*.pdf\n")).await;
    assert_eq!(
        outcome,
        IgnoreSaveOutcome::Refused {
            reason: IgnoreRefusal::NotText
        }
    );
    assert_eq!(f.vault.bytes_of(".gitignore").unwrap(), [0xff, 0xfe, 0x00]);

    // Vault без права записи: файл виден, сохранение названо причиной.
    let rules = full_rules(IgnoreFile::GitIgnore, "");
    let mut g = fixture(&[(".gitignore", &rules)], None);
    let vault = std::mem::replace(&mut g.vault, MemoryVault::new());
    g.vault = vault.read_only();
    let document = g.session.ignore_open(&g.vault, IgnoreFile::GitIgnore).await;
    assert_eq!(document.editable, Editability::ReadOnly);
    assert_eq!(document.text, rules);
    let outcome = g.save_rules(save_request(&document, "*.pdf\n")).await;
    assert_eq!(
        outcome,
        IgnoreSaveOutcome::Refused {
            reason: IgnoreRefusal::ReadOnly
        }
    );
    assert_eq!(g.text(".gitignore").unwrap(), rules);
}

pub async fn ignore_editor_saves_on_a_device_that_is_not_the_keeper() {
    let rules = full_rules(IgnoreFile::GitIgnore, "");
    let mut f = fixture(&[("a.typ", "= A\n"), (".gitignore", &rules)], None);
    f.enable().await;
    set_keeper(&mut f, OTHER).await;
    let document = f.session.ignore_open(&f.vault, IgnoreFile::GitIgnore).await;
    let edited = format!("*.log\n{rules}");
    let outcome = f.save_rules(save_request(&document, &edited)).await;
    assert!(matches!(outcome, IgnoreSaveOutcome::Saved { .. }));
    assert_eq!(f.text(".gitignore").unwrap(), edited);
}

pub async fn ignore_editor_asks_before_saving_without_required_rules() {
    let rules = full_rules(IgnoreFile::StIgnore, "*.tmp\n");
    let mut f = fixture(
        &[
            ("a.typ", "= A\n"),
            (".stfolder/.keep", ""),
            (".stignore", &rules),
        ],
        None,
    );
    f.enable().await;
    let document = f.session.ignore_open(&f.vault, IgnoreFile::StIgnore).await;
    assert!(document.in_effect);

    let broken = rules.replace("/.git\n", "");
    let outcome = f.save_rules(save_request(&document, &broken)).await;
    assert_eq!(
        outcome,
        IgnoreSaveOutcome::NeedsConfirmation {
            missing: vec!["/.git".to_string()]
        }
    );
    assert_eq!(f.text(".stignore").unwrap(), rules);

    // Дословная отмена выше блока — то же нарушение (в `.stignore` выигрывает
    // первая строка).
    let cancelled = format!("!/.git\n{rules}");
    let outcome = f.save_rules(save_request(&document, &cancelled)).await;
    assert!(matches!(
        outcome,
        IgnoreSaveOutcome::NeedsConfirmation { .. }
    ));

    let outcome = f
        .save_rules(IgnoreSaveRequest {
            confirm_incomplete: true,
            ..save_request(&document, &broken)
        })
        .await;
    assert!(matches!(outcome, IgnoreSaveOutcome::Saved { .. }));
    assert_eq!(f.text(".stignore").unwrap(), broken);
    // Предупреждение панели никуда не делось (011 §FR-026).
    let ctx = f.ctx();
    let overview = f.session.overview(&f.vault, &ctx).await;
    assert!(overview
        .ignore_warnings
        .iter()
        .any(|report| report.file == IgnoreFile::StIgnore));
}

pub async fn ignore_editor_complete_rules_lift_the_declined_flag() {
    let mut f = fixture(&[("a.typ", "= A\n"), (".stfolder/.keep", "")], None);
    let ctx = f.ctx();
    f.session.decline_stignore(&ctx);
    f.absorb_device_update();
    assert!(f.device_sync.stignore_declined);

    let document = f.session.ignore_open(&f.vault, IgnoreFile::StIgnore).await;
    let text = document.text.clone();
    let outcome = f.save_rules(save_request(&document, &text)).await;
    assert!(matches!(outcome, IgnoreSaveOutcome::Saved { .. }));
    f.absorb_device_update();
    assert!(!f.device_sync.stignore_declined);
}

pub async fn ignore_editor_does_not_overwrite_an_outside_change() {
    let rules = full_rules(IgnoreFile::GitIgnore, "*.pdf\n");
    let mut f = fixture(&[("a.typ", "= A\n"), (".gitignore", &rules)], None);
    let document = f.session.ignore_open(&f.vault, IgnoreFile::GitIgnore).await;
    let mine = format!("моё/\n{rules}");
    let theirs = format!("чужое/\n{rules}");
    f.write_outside(".gitignore", theirs.as_bytes());

    let outcome = f.save_rules(save_request(&document, &mine)).await;
    let IgnoreSaveOutcome::Diverged { disk, diff } = outcome else {
        panic!("ожидалось расхождение: {outcome:?}");
    };
    assert_eq!(disk.text, theirs);
    assert!(diff.contains(&DiffLine::Local {
        text: "моё/".into()
    }));
    assert!(diff.contains(&DiffLine::External {
        text: "чужое/".into()
    }));
    assert_eq!(f.text(".gitignore").unwrap(), theirs);

    // Тот же текст, что уже на диске, расхождением не считается.
    let outcome = f.save_rules(save_request(&document, &theirs)).await;
    assert!(matches!(outcome, IgnoreSaveOutcome::Saved { .. }));

    // «Записать свою» — явный выбор пользователя.
    let outcome = f
        .save_rules(IgnoreSaveRequest {
            overwrite: true,
            ..save_request(&document, &mine)
        })
        .await;
    assert!(matches!(outcome, IgnoreSaveOutcome::Saved { .. }));
    assert_eq!(f.text(".gitignore").unwrap(), mine);
}

pub async fn ignore_editor_recreates_a_deleted_file_only_on_request() {
    let rules = full_rules(IgnoreFile::GitIgnore, "*.pdf\n");
    let mut f = fixture(&[(".gitignore", &rules)], None);
    let document = f.session.ignore_open(&f.vault, IgnoreFile::GitIgnore).await;
    f.vault
        .files_handle()
        .borrow_mut()
        .remove(Path::new(".gitignore"));

    let edited = format!("*.log\n{rules}");
    let outcome = f.save_rules(save_request(&document, &edited)).await;
    let IgnoreSaveOutcome::Diverged { disk, .. } = outcome else {
        panic!("ожидалось расхождение: {outcome:?}");
    };
    assert!(!disk.exists);
    assert_eq!(f.text(".gitignore"), None);

    let outcome = f
        .save_rules(IgnoreSaveRequest {
            overwrite: true,
            ..save_request(&document, &edited)
        })
        .await;
    assert!(matches!(outcome, IgnoreSaveOutcome::Saved { .. }));
    assert_eq!(f.text(".gitignore").unwrap(), edited);
}

pub async fn ignore_editor_waits_for_an_unfinished_pull() {
    let remote = FakeGitEngine::new_remote();
    let mut a = fixture(&[("n.typ", BASE)], Some(remote.clone()));
    a.enable().await;
    a.connect().await;
    a.push().await.unwrap();
    let mut b = clone_of(&remote).await;
    a.put("n.typ", &BASE.replace("Второй абзац.", "Второй — A."))
        .await;
    a.snap("A").await;
    a.push().await.unwrap();
    b.put("n.typ", &BASE.replace("Второй абзац.", "Второй — B."))
        .await;
    b.snap("B").await;
    assert_eq!(b.pull().await.unwrap().conflicts, 1);

    let document = b.session.ignore_open(&b.vault, IgnoreFile::GitIgnore).await;
    let before = b.text(".gitignore");
    let edited = format!("*.log\n{}", document.text);
    let outcome = b.save_rules(save_request(&document, &edited)).await;
    assert_eq!(
        outcome,
        IgnoreSaveOutcome::Refused {
            reason: IgnoreRefusal::HistoryBusy
        }
    );
    assert_eq!(b.text(".gitignore"), before);

    // Правила Syncthing история не держит.
    let stignore = b.session.ignore_open(&b.vault, IgnoreFile::StIgnore).await;
    let text = stignore.text.clone();
    let outcome = b.save_rules(save_request(&stignore, &text)).await;
    assert!(matches!(outcome, IgnoreSaveOutcome::Saved { .. }));
}
