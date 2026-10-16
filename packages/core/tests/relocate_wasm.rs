//! wasm-bindgen-test — только wasm32-unknown-unknown (принцип II). Парный файл
//! — `relocate_native.rs`.
//!
//! Оба прогоняют **одни и те же** сценарии из `common::relocate_cases`, но
//! здесь vault — настоящая папка в OPFS браузера: она доступна в headless
//! Chrome без жеста пользователя, в отличие от `showDirectoryPicker`. Так
//! `WebBackend` проверяется по-настоящему — копирование байтами, удаление
//! источника, опрос изменений, — а не по двойнику.
#![cfg(target_arch = "wasm32")]

mod common;

use common::relocate_cases::{self as cases, Disk, Env, Files};
use js_sys::{Array, Function, Reflect, Uint8Array};
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::events::VaultEvent;
use typsidian_core::vault::web::WebBackend;
use typsidian_core::vault::AccessState;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

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

/// Пустая папка в OPFS под уникальным именем: хранилище общее на всё
/// происхождение, а тесты в одном браузере его разделяют.
async fn fresh_folder() -> JsValue {
    let name = format!("relocate-{}", NEXT_FOLDER.fetch_add(1, Ordering::Relaxed));
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

/// Кладёт файлы в папку через временный backend: родителей создаёт
/// `ensure_folder`, байты пишет `write_bytes`.
async fn seed(handle: &JsValue, files: Files<'_>) {
    let mut seeder = WebBackend::from_handle("vault".into(), handle.clone())
        .await
        .unwrap();
    for (path, bytes) in files {
        if let Some(parent) = Path::new(path).parent() {
            seeder.ensure_folder(parent).await.unwrap();
        }
        seeder.write_bytes(Path::new(path), bytes).await.unwrap();
    }
}

struct WebEnv {
    handle: JsValue,
    backend: WebBackend,
}

impl WebEnv {
    async fn open_over(files: Files<'_>, wrap: Option<&str>) -> Self {
        let handle = fresh_folder().await;
        seed(&handle, files).await;
        let exposed = match wrap {
            Some(source) => eval_function(source)
                .call1(&JsValue::NULL, &handle)
                .unwrap(),
            None => handle.clone(),
        };
        let backend = WebBackend::from_handle("vault".into(), exposed)
            .await
            .unwrap();
        Self { handle, backend }
    }
}

/// Посредник над хендлом: права на запись отозваны, чтение осталось.
const READ_ONLY_PROXY: &str = "(function (h) { return new Proxy(h, { get(t, p) { \
    if (p === 'queryPermission') return async (o) => ({ state: o && o.mode === 'read' ? 'granted' : 'denied' }); \
    const v = t[p]; return typeof v === 'function' ? v.bind(t) : v; } }); })";

impl Env for WebEnv {
    type Backend = WebBackend;

    async fn open(files: Files<'_>) -> Self {
        Self::open_over(files, None).await
    }

    async fn open_read_only(files: Files<'_>) -> Option<Self> {
        let mut env = Self::open_over(files, Some(READ_ONLY_PROXY)).await;
        env.backend.poll_for_changes().await;
        assert_eq!(env.backend.access_state(), AccessState::ReadOnly);
        Some(env)
    }

    /// Хендл, у которого `removeEntry(name)` отказывает на любой глубине:
    /// копирование проходит, а удаление источника — нет.
    async fn open_failing_removal(files: Files<'_>, name: &str) -> Option<Self> {
        let source = format!(
            "(function (h) {{ const wrap = (d) => new Proxy(d, {{ get(t, p) {{ \
             if (p === 'removeEntry') return async (n, o) => {{ \
                 if (n === {name:?}) throw new Error('отказ удаления'); return t.removeEntry(n, o); }}; \
             if (p === 'getDirectoryHandle') return async (...a) => wrap(await t.getDirectoryHandle(...a)); \
             const v = t[p]; return typeof v === 'function' ? v.bind(t) : v; }} }}); \
             return wrap(h); }})"
        );
        Some(Self::open_over(files, Some(&source)).await)
    }

    fn backend(&mut self) -> &mut WebBackend {
        &mut self.backend
    }

    fn backend_ref(&self) -> &WebBackend {
        &self.backend
    }

    async fn disk(&self) -> Disk {
        let walk = eval_function(
            "(async function walk(dir, prefix, out) { \
               for await (const [name, h] of dir.entries()) { \
                 if (prefix === '' && name === '.typsidian') continue; \
                 const p = prefix ? prefix + '/' + name : name; \
                 if (h.kind === 'directory') await walk(h, p, out); \
                 else { const f = await h.getFile(); \
                        out.push([p, new Uint8Array(await f.arrayBuffer())]); } } \
               return out; })",
        );
        let found = await_js(
            walk.call3(
                &JsValue::NULL,
                &self.handle,
                &JsValue::from_str(""),
                &Array::new(),
            )
            .unwrap(),
        )
        .await;
        let mut disk = Disk::new();
        for pair in Array::from(&found).iter() {
            let pair = Array::from(&pair);
            let path = pair.get(0).as_string().unwrap();
            let bytes = Uint8Array::new(&Reflect::get(&pair, &JsValue::from_f64(1.0)).unwrap());
            disk.insert(path, bytes.to_vec());
        }
        disk
    }

    /// Другой экземпляр backend над той же папкой — «сторонняя программа».
    async fn write_outside(&mut self, path: &str, bytes: &[u8]) {
        let mut other = WebBackend::from_handle("vault".into(), self.handle.clone())
            .await
            .unwrap();
        other.write_bytes(Path::new(path), bytes).await.unwrap();
    }

    /// На web «проход сверки» — явный `pollForChanges`.
    async fn settle(&mut self) -> Vec<VaultEvent> {
        self.backend.poll_for_changes().await;
        self.backend.poll_events()
    }
}

macro_rules! web_cases {
    ($($name:ident),* $(,)?) => {
        $(
            #[wasm_bindgen_test]
            async fn $name() {
                cases::$name::<WebEnv>().await;
            }
        )*
    };
}

web_cases!(
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
    a_link_with_a_label_is_judged_by_its_target,
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
    // Только web: перенос там — копия с удалением (research.md §6).
    cancelling_halfway_through_copying_restores_the_vault,
);

#[wasm_bindgen_test]
async fn a_failed_source_removal_leaves_a_full_copy_and_names_the_rest() {
    let files = cases::folder_files();
    let env = WebEnv::open_failing_removal(&files, "A").await.unwrap();
    cases::a_failed_source_removal_leaves_a_full_copy_and_names_the_rest(env).await;
}
