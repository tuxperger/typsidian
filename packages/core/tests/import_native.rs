//! cargo test — только native (принцип II). Парный файл — `import_wasm.rs`.
//!
//! Оба прогоняют одну таблицу из `common::import_cases` над `MemoryVault`;
//! файловую систему native проверяют тесты ниже.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::import_cases as cases;

macro_rules! sync_cases {
    ($($name:ident),* $(,)?) => {
        $(#[test] fn $name() { cases::$name(); })*
    };
}

macro_rules! async_cases {
    ($($name:ident),* $(,)?) => {
        $(#[tokio::test] async fn $name() { cases::$name().await; })*
    };
}

sync_cases!(
    the_translation_table_row_by_row,
    inline_markers_are_safe_next_to_words_and_punctuation,
);

async_cases!(
    special_characters_are_escaped_and_the_note_compiles,
    every_sample_becomes_a_note_that_compiles,
    the_source_is_never_written,
    the_read_only_wrapper_refuses_every_write,
    wiki_links_of_every_form,
    link_problems_reach_the_report,
    images_land_in_assets_and_a_shared_one_is_copied_once,
    yaml_header_becomes_properties_or_a_comment,
    formulas_translate_or_stay_visible,
    forbidden_characters_are_replaced_and_links_follow,
    survey_counts_files_by_kind,
    existing_files_are_skipped_or_kept_beside,
    a_cancelled_import_leaves_whole_files_and_a_report,
    a_resumed_import_does_not_duplicate,
    the_report_note_compiles_and_lists_losses,
    other_files_are_skipped_unless_copying_is_chosen,
);

// ── Настоящая файловая система ──────────────────────────────────────────────

mod disk {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    use super::common::import_cases::{self as cases, FIXTURES};
    use tempfile::TempDir;
    use typsidian_core::import::{import_finish, survey};
    use typsidian_core::vault::backend::{ReadOnly, VaultBackend};
    use typsidian_core::vault::native::NativeBackend;

    fn write_fixtures(root: &Path) {
        for (path, bytes) in FIXTURES {
            let full = root.join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, bytes).unwrap();
        }
    }

    /// Все файлы и папки под корнем → байты (папки — `None`).
    fn tree(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
        fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Option<Vec<u8>>>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                if path.is_dir() {
                    out.insert(relative, None);
                    walk(root, &path, out);
                } else {
                    out.insert(relative, Some(fs::read(&path).unwrap()));
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(root, root, &mut out);
        out
    }

    /// SC-005 на диске: источник после обзора и импорта побайтно прежний, без
    /// `.typsidian/`, кэша и временных файлов; результат совпадает с тем,
    /// что даёт `MemoryVault`.
    #[tokio::test]
    async fn a_disk_source_is_untouched_and_the_result_matches_memory() {
        let source_dir = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();
        write_fixtures(source_dir.path());
        let before = tree(source_dir.path());

        let source = ReadOnly::new(NativeBackend::open_read_only(source_dir.path()).unwrap());
        let mut vault = NativeBackend::open(dest_dir.path()).await.unwrap();
        let job = cases::job();
        let found = survey(&source, &vault, &job).await;
        assert_eq!(
            (found.notes, found.images, found.other, found.excluded),
            (22, 3, 3, 2)
        );
        let state = cases::drive(&source, &mut vault, &job, None).await;
        assert_eq!(state.next, state.total);
        let report = import_finish(&mut vault, &state).await.unwrap();

        assert_eq!(tree(source_dir.path()), before, "источник изменён");
        assert!(!source_dir.path().join(".typsidian").exists());

        // Записанное — целые файлы; служебных остатков записи нет.
        let after = tree(dest_dir.path());
        assert!(after
            .keys()
            .all(|path| !path.starts_with(".typsidian/tmp/") || path == ".typsidian/tmp"));
        assert!(after.contains_key("Импорт/assets/10 Картинки/pic.png"));

        // Тот же импорт в память даёт те же заметки.
        let (memory, _) = cases::import_all().await;
        for path in memory.paths_under("Импорт") {
            if path.ends_with(".typ") && !path.contains("Import report") {
                let on_disk = fs::read_to_string(dest_dir.path().join(&path)).unwrap();
                assert_eq!(on_disk, cases::text_of(&memory, &path).await, "{path}");
            }
        }
        cases::assert_compiles(&vault, &report).await;
        cases::assert_compiles(&vault, "Импорт/Ссылки/08 Входная.typ").await;
    }

    /// `open_read_only` не создаёт ничего даже в пустой папке.
    #[test]
    fn opening_for_reading_creates_nothing() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), "# a").unwrap();
        let before = tree(dir.path());
        let source = ReadOnly::new(NativeBackend::open_read_only(dir.path()).unwrap());
        assert_eq!(
            source.access_state(),
            typsidian_core::vault::AccessState::ReadOnly
        );
        drop(source);
        assert_eq!(tree(dir.path()), before);
    }
}
