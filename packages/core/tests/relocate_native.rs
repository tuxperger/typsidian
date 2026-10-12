//! cargo test — только native (принцип II). Парный файл — `relocate_wasm.rs`.
//!
//! Оба прогоняют **одни и те же** сценарии из `common::relocate_cases`
//! (фича 023): перенос папок, индекса, правок и ссылок обязан вести себя
//! одинаково на обоих таргетах.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::relocate_cases::{self as cases, Disk, Env, Files};
use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::events::VaultEvent;
use typsidian_core::vault::native::NativeBackend;
use typsidian_core::vault::AccessState;

struct NativeEnv {
    dir: TempDir,
    backend: NativeBackend,
}

fn seed(dir: &Path, files: Files<'_>) {
    for (path, bytes) in files {
        let full = dir.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, bytes).unwrap();
    }
}

fn collect(root: &Path, folder: &Path, disk: &mut Disk) {
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        let relative = path.strip_prefix(root).unwrap();
        if relative == Path::new(".typsidian") {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, disk);
        } else {
            disk.insert(
                relative.to_string_lossy().replace('\\', "/"),
                fs::read(&path).unwrap(),
            );
        }
    }
}

impl Env for NativeEnv {
    type Backend = NativeBackend;

    async fn open(files: Files<'_>) -> Self {
        let dir = TempDir::new().unwrap();
        seed(dir.path(), files);
        let backend = NativeBackend::open(dir.path()).await.unwrap();
        Self { dir, backend }
    }

    async fn open_read_only(files: Files<'_>) -> Option<Self> {
        use std::os::unix::fs::PermissionsExt;
        // Под root бит `w` процесс не ограничивает (как в соседних тестах).
        let probe = TempDir::new().unwrap();
        fs::set_permissions(probe.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let bypasses = fs::write(probe.path().join("x"), "").is_ok();
        fs::set_permissions(probe.path(), fs::Permissions::from_mode(0o755)).unwrap();
        if bypasses {
            return None;
        }

        let mut env = Self::open(files).await;
        fs::set_permissions(env.dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while env.backend.access_state() != AccessState::ReadOnly {
            assert!(
                tokio::time::Instant::now() < deadline,
                "отзыв прав должен стать ReadOnly"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
            env.backend.poll_events();
        }
        Some(env)
    }

    fn backend(&mut self) -> &mut NativeBackend {
        &mut self.backend
    }

    fn backend_ref(&self) -> &NativeBackend {
        &self.backend
    }

    async fn disk(&self) -> Disk {
        let mut disk = Disk::new();
        collect(self.dir.path(), self.dir.path(), &mut disk);
        disk
    }

    async fn write_outside(&mut self, path: &str, bytes: &[u8]) {
        fs::write(self.dir.path().join(path), bytes).unwrap();
    }

    /// Дебаунсер `notify` держит события 400 мс, поэтому окно шире.
    async fn settle(&mut self) -> Vec<VaultEvent> {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(1500);
        let mut events = Vec::new();
        while tokio::time::Instant::now() < deadline {
            events.extend(self.backend.poll_events());
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        events.extend(self.backend.poll_events());
        events
    }
}

impl Drop for NativeEnv {
    fn drop(&mut self) {
        // Права возвращаются, иначе временную папку не удалить.
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(self.dir.path(), fs::Permissions::from_mode(0o755));
    }
}

macro_rules! native_cases {
    ($($name:ident),* $(,)?) => {
        $(
            #[tokio::test]
            async fn $name() {
                cases::$name::<NativeEnv>().await;
            }
        )*
    };
}

native_cases!(
    a_note_moves_into_a_folder_and_to_the_root,
    a_board_moves_like_a_note,
    unsaved_edits_and_a_conflict_follow_the_note,
    own_snapshot_folders_travel_with_the_note,
    a_shared_snapshot_folder_stays_when_the_policy_is_vault_root,
    moving_never_rewrites_any_note_text,
    an_own_move_is_never_reported_as_an_external_change,
    a_single_note_move_survives_the_watcher,
    targets_start_with_the_root_and_skip_the_current_folder,
    a_folder_moves_with_everything_in_it,
    a_folder_cannot_enter_itself_or_any_descendant,
    a_cancelled_move_changes_nothing,
    incoming_relative_and_rooted_links_are_reported,
    own_relative_links_of_the_moved_note_are_reported,
    a_folder_gives_one_list_of_only_the_links_it_really_breaks,
    impact_refuses_what_the_move_would_refuse,
    moving_to_the_folder_it_is_in_is_a_silent_refusal,
    a_taken_name_is_refused_for_a_file_a_folder_and_a_snapshot_folder,
    names_that_differ_by_case_follow_the_file_system,
    a_missing_item_or_target_is_named,
    the_service_folder_is_neither_moved_nor_a_target,
    a_read_only_vault_refuses_every_move,
    the_filter_matches_name_with_a_span_and_path_without,
    undo_restores_a_note_and_its_snapshot_folder,
    undo_restores_a_folder_with_its_section_marks,
    undo_never_overwrites_what_appeared_in_the_old_place,
);
