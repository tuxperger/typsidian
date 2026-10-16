//! wasm-bindgen-test — только wasm32-unknown-unknown (принцип II). Парный файл
//! — `import_native.rs`.
//!
//! Одна и та же таблица `common::import_cases` над `MemoryVault`, плюс сквозная
//! проверка на настоящем `WebBackend`: источник и vault — папки в OPFS, а
//! вызовы идут через биндинги `importSurvey`, `importBegin`, `importStep`,
//! `importFinish`, теми же, что зовёт окно.
#![cfg(target_arch = "wasm32")]

mod common;

use common::import_cases::{self as cases, FIXTURES};
use js_sys::Function;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use typsidian_core::import::web::{
    import_begin_js, import_convert_js, import_finish_js, import_step_js, import_survey_js,
    ImportSource,
};
use typsidian_core::import::ConvertContext;
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::web::WebBackend;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

macro_rules! sync_cases {
    ($($name:ident),* $(,)?) => {
        $(#[wasm_bindgen_test] fn $name() { cases::$name(); })*
    };
}

macro_rules! async_cases {
    ($($name:ident),* $(,)?) => {
        $(#[wasm_bindgen_test] async fn $name() { cases::$name().await; })*
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

// ── OPFS ────────────────────────────────────────────────────────────────────

static NEXT_FOLDER: AtomicU32 = AtomicU32::new(0);

async fn await_js(value: JsValue) -> JsValue {
    JsFuture::from(js_sys::Promise::from(value))
        .await
        .expect("обещание JS должно выполниться")
}

fn eval_function(source: &str) -> Function {
    js_sys::eval(source)
        .expect("вспомогательный код JS должен собираться")
        .dyn_into()
        .expect("это функция")
}

async fn fresh_folder(prefix: &str) -> JsValue {
    let name = format!("{prefix}-{}", NEXT_FOLDER.fetch_add(1, Ordering::Relaxed));
    let make = eval_function(
        "(async (name) => { const root = await navigator.storage.getDirectory(); \
         try { await root.removeEntry(name, { recursive: true }); } catch (e) {} \
         return await root.getDirectoryHandle(name, { create: true }); })",
    );
    await_js(
        make.call1(&JsValue::NULL, &JsValue::from_str(&name))
            .unwrap(),
    )
    .await
}

/// Дерево папки: «путь:размер» для файлов и «путь/» для папок, по порядку.
async fn tree(handle: &JsValue) -> Vec<String> {
    let walk = eval_function(
        "(async (root) => { const out = []; \
           async function walk(dir, prefix) { \
             for await (const [name, h] of dir.entries()) { \
               const p = prefix ? prefix + '/' + name : name; \
               if (h.kind === 'directory') { out.push(p + '/'); await walk(h, p); } \
               else { const f = await h.getFile(); out.push(p + ':' + f.size); } } } \
           await walk(root, ''); out.sort(); return out.join('\\n'); })",
    );
    let text = await_js(walk.call1(&JsValue::NULL, handle).unwrap())
        .await
        .as_string()
        .unwrap_or_default();
    text.lines().map(str::to_owned).collect()
}

/// Сквозной импорт на `WebBackend`: источник — папка OPFS, открытая только для
/// чтения, vault — другая папка OPFS; источник после импорта прежний.
#[wasm_bindgen_test]
async fn an_opfs_source_is_untouched_and_the_import_matches_memory() {
    let source_handle = fresh_folder("import-source").await;
    let mut seeder = WebBackend::from_handle("source".into(), source_handle.clone())
        .await
        .unwrap();
    for (path, bytes) in FIXTURES {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                seeder.ensure_folder(parent).await.unwrap();
            }
        }
        seeder.write_bytes(Path::new(path), bytes).await.unwrap();
    }
    drop(seeder);
    let before = tree(&source_handle).await;

    let vault_handle = fresh_folder("import-vault").await;
    let mut vault = WebBackend::from_handle("vault".into(), vault_handle)
        .await
        .unwrap();
    let source = ImportSource::from_handle("source".into(), source_handle.clone());
    let job = cases::job();

    let found = import_survey_js(&source, &vault, job.clone()).await;
    assert_eq!(
        (found.notes, found.images, found.other, found.excluded),
        // Скрытых записей три: `.obsidian`, `.trash` и `.typsidian`, которую завёл
        // сам засеиватель (обычный `WebBackend`) до импорта.
        (22, 3, 3, 3)
    );
    let mut state = import_begin_js(&source, &vault, job).await;
    while state.next < state.total {
        state = import_step_js(&source, &mut vault, state).await.unwrap();
    }
    let report = import_finish_js(&mut vault, state).await.unwrap();

    assert_eq!(tree(&source_handle).await, before, "источник изменён");

    let (memory, _) = cases::import_all().await;
    for path in memory.paths_under("Импорт") {
        if path.ends_with(".typ") && !path.contains("Import report") {
            assert_eq!(
                cases::text_of(&vault, &path).await,
                cases::text_of(&memory, &path).await,
                "{path}"
            );
        }
    }
    assert!(vault
        .read_bytes(Path::new("Импорт/assets/10 Картинки/pic.png"))
        .await
        .is_ok());
    cases::assert_compiles(&vault, &report).await;
    cases::assert_compiles(&vault, "Импорт/Ссылки/08 Входная.typ").await;
}

/// `importConvert` — тот же перевод, что в пакете.
#[wasm_bindgen_test]
fn import_convert_translates_one_note() {
    let ctx = ConvertContext {
        source: "Заметка.md".into(),
        destination: String::new(),
        files: vec!["Цель.md".into()],
    };
    let result = import_convert_js("# Заголовок\n\n[[Цель|текст]]\n", ctx);
    assert_eq!(result.typst, "= Заголовок\n\n[[Цель|текст]]\n");
    assert!(result.losses.is_empty());
}
