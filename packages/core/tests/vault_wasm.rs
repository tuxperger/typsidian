//! wasm-bindgen-test — только wasm32-unknown-unknown (принцип II). Тесты
//! требуют браузерного окружения (File System Access API недоступен в Node),
//! запускаются через `wasm-pack test --headless --chrome` (или другой браузер).
#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// ---------------------------------------------------------------------------
// US1 — паритет с NativeBackend (T019): полноценный прогон требует реального
// FileSystemDirectoryHandle, который может быть получен только по жесту
// пользователя (`showDirectoryPicker`) в браузере — недоступен в headless
// тестовом рантайме без синтетического мока FS Access API. Здесь фиксируется
// то, что можно проверить без интерактивного жеста: модуль линкуется и
// экспортируется в WASM с ожидаемым публичным API.
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn web_backend_type_is_exported() {
    // Компиляционная проверка паритета: если сигнатура WebBackend разойдётся с
    // VaultBackend, этот файл не соберётся под wasm32-unknown-unknown — что и
    // является гейтом принципа II для функций, не требующих живого браузерного
    // FS-хендла.
    fn _assert_implements_vault_backend<T: typsidian_core::vault::backend::VaultBackend>() {}
    // Сам вызов и есть проверка: если `WebBackend` перестанет удовлетворять
    // `VaultBackend`, файл не соберётся под wasm32.
    _assert_implements_vault_backend::<typsidian_core::vault::web::WebBackend>();
}

// ---------------------------------------------------------------------------
// US2 — валидация имён (T035, FR-013): `WebBackend::create`/`rename` вызывают
// ровно ту же функцию `vault::backend::valid_note_name`, что и
// `NativeBackend` (см. packages/core/src/vault/web.rs, native.rs) — по той же
// причине, что и выше (showDirectoryPicker требует пользовательского жеста и
// недоступен в headless wasm-pack test рантайме), полный прогон create()/
// rename() на живом FileSystemDirectoryHandle здесь не воспроизводим. Вместо
// синтетического (и потенциально нестабильного) мока FS Access API этот тест
// честно фиксирует то, что реально проверяемо под wasm32: сама валидация,
// которую WebBackend вызывает первым делом до обращения к диску, даёт
// идентичный NativeBackend результат.
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn note_name_validation_matches_native_before_touching_disk() {
    use typsidian_core::vault::backend::is_valid_note_name;

    assert!(is_valid_note_name("note.typ"));
    assert!(!is_valid_note_name(""), "пустое имя недопустимо");
    assert!(
        !is_valid_note_name("evil/name.typ"),
        "разделитель пути недопустим в имени"
    );
    assert!(
        !is_valid_note_name("evil\\name.typ"),
        "обратный слеш недопустим в имени"
    );
    assert!(
        !is_valid_note_name("evil\0name.typ"),
        "NUL-байт недопустим в имени (парность с NativeBackend)"
    );
}

// ---------------------------------------------------------------------------
// US3 — паритет обнаружения внешних изменений (T047): `WebBackend` не имеет
// нативных FS-событий (research.md §3), поэтому вместо `watch()` у него
// `pollForChanges()` — один цикл сверки, вызываемый JS-стороной по таймеру.
// Как и у T019/T035, живой `FileSystemDirectoryHandle` недоступен в headless
// wasm-pack test (нужен пользовательский жест `showDirectoryPicker`), так
// что здесь честно фиксируется только то, что реально проверяемо без него —
// метод существует, компилируется и типизируется под wasm32 (гейт принципа
// II). Полноценный прогон обнаружения изменений на живом хендле — предмет
// ручного/browser E2E прогона quickstart.md (T063).
// ---------------------------------------------------------------------------

/// Никогда не вызывается — само наличие этой функции в скомпилированном
/// тестовом бинаре доказывает, что `pollForChanges` существует с ожидаемой
/// сигнатурой (`&mut WebBackend -> impl Future<Output = ()>`) под wasm32.
#[allow(dead_code)]
async fn _poll_for_changes_signature_check(backend: &mut typsidian_core::vault::web::WebBackend) {
    backend.poll_for_changes().await;
}

#[wasm_bindgen_test]
fn poll_for_changes_compiles_under_wasm32() {
    // Взятие адреса функции-проверки заставляет компилятор её инстанцировать:
    // без существующего `poll_for_changes` с ожидаемой сигнатурой этот файл не
    // соберётся под wasm32.
    let check = _poll_for_changes_signature_check;
    std::hint::black_box(&check);
}

// ---------------------------------------------------------------------------
// US4 — паритет обнаружения и разрешения конфликта (T070). В отличие от
// тестов выше, здесь живой `FileSystemDirectoryHandle` не нужен: правила
// «когда поднимать конфликт», «что делает повторное изменение диска» и «что
// именно отбрасывает буфер» живут в `ConflictRegistry` — общем коде обоих
// таргетов (принцип II: реализации не расходятся по платформе, платформенным
// остаётся только источник событий). Поэтому под wasm32 проверяется та же
// логика и теми же утверждениями, что и в vault_native.rs, а не одна лишь
// компилируемость.
// ---------------------------------------------------------------------------

use std::path::Path;
use typsidian_core::vault::conflict::{
    is_meaningful_change, line_diff, looks_changed, ConflictRegistry, ConflictStatus, DiffLine,
};
use typsidian_core::vault::index::FileStamp;

fn stamp(content: &str, mtime: i64) -> FileStamp {
    FileStamp::from_content(mtime, content.as_bytes())
}

#[wasm_bindgen_test]
fn conflict_is_raised_only_for_notes_with_unsaved_edits() {
    // FR-021, паритет с
    // external_change_to_note_with_unsaved_edits_raises_conflict_instead_of_silent_update.
    let mut registry = ConflictRegistry::new();
    let path = Path::new("note.typ");
    let known = stamp("исходное", 100);
    let external = stamp("внешнее", 200);

    assert!(
        !registry.observe_disk_change(path, &known, &external),
        "без несохранённых правок внешнее изменение — обычный NoteUpdated"
    );

    registry.set_buffer(path, "мои правки".to_string());
    assert!(registry.observe_disk_change(path, &known, &external));
    let conflict = registry
        .conflict(path)
        .expect("конфликт должен быть поднят");
    assert_eq!(conflict.local_buffer, "мои правки");
    assert_eq!(conflict.status, ConflictStatus::Pending);
}

#[wasm_bindgen_test]
fn touched_file_with_identical_content_is_not_a_conflict_on_web_either() {
    // research.md §5: предфильтр срабатывает, подтверждение — нет.
    let known = stamp("одно и то же", 100);
    let current = stamp("одно и то же", 900);
    assert!(looks_changed(&known, &current));
    assert!(!is_meaningful_change(&known, &current));
}

#[wasm_bindgen_test]
fn repeated_disk_change_refreshes_snapshot_without_second_conflict() {
    // US4 AS5, паритет с repeated_disk_change_before_resolution_refreshes_disk_snapshot.
    let mut registry = ConflictRegistry::new();
    let path = Path::new("note.typ");
    registry.set_buffer(path, "моя версия".to_string());

    let first = stamp("первое внешнее", 200);
    registry.observe_disk_change(path, &stamp("исходное", 100), &first);
    let second = stamp("второе внешнее", 300);
    registry.observe_disk_change(path, &first, &second);

    let conflict = registry
        .conflict(path)
        .expect("конфликт должен остаться один");
    assert_eq!(conflict.disk_snapshot, second);
    assert_eq!(conflict.local_buffer, "моя версия");
}

#[wasm_bindgen_test]
fn buffer_is_discarded_only_by_explicit_resolution() {
    // FR-023, паритет с
    // accepting_external_discards_local_buffer_only_after_explicit_confirmation.
    let mut registry = ConflictRegistry::new();
    let path = Path::new("note.typ");
    registry.set_buffer(path, "моя версия".to_string());
    registry.observe_disk_change(path, &stamp("a", 1), &stamp("b", 2));

    assert!(
        registry.has_unsaved_edits(path),
        "показ диалога ничего не отбрасывает"
    );

    assert!(registry.resolve(path, ConflictStatus::ResolvedAcceptExternal));
    assert!(!registry.has_unsaved_edits(path));
    assert!(registry.conflict(path).is_none());
    assert!(registry.unresolved_paths().is_empty());
}

#[wasm_bindgen_test]
fn external_deletion_with_unsaved_edits_keeps_buffer_for_resave() {
    // FR-025, паритет с external_deletion_of_note_with_unsaved_edits_warns_and_allows_resave.
    let mut registry = ConflictRegistry::new();
    let path = Path::new("note.typ");

    assert!(!registry.observe_disk_deletion(path));

    registry.set_buffer(path, "не потерять эти правки".to_string());
    assert!(registry.observe_disk_deletion(path));
    assert!(registry.was_deleted_with_edits(path));
    assert_eq!(registry.buffer(path), Some("не потерять эти правки"));
}

#[wasm_bindgen_test]
fn diff_shows_both_sides_under_wasm32() {
    // FR-024, паритет с diff_between_local_buffer_and_disk_shows_both_sides.
    let diff = line_diff(
        "общая строка\nмоя строка\n",
        "общая строка\nвнешняя строка\n",
    );
    assert!(diff.contains(&DiffLine::Same {
        text: "общая строка".to_string()
    }));
    assert!(diff.contains(&DiffLine::Local {
        text: "моя строка".to_string()
    }));
    assert!(diff.contains(&DiffLine::External {
        text: "внешняя строка".to_string()
    }));
}

// ---------------------------------------------------------------------------
// US5 — паритет ссылочной целостности (T083). Разрешение цели ссылки —
// общий код обоих таргетов (`graph`), поэтому под wasm32 проверяется та же
// логика и теми же случаями, что и в vault_native.rs: именно она решает, какие
// заметки попадут в `backlinks()` и в список «что сломается при удалении».
// Хранилище таблицы `links` у web своё (`MemoryCache`), но правила — общие.
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn link_targets_resolve_identically_to_native() {
    use std::path::PathBuf;
    use typsidian_core::graph::{links_in, resolve_link_target};

    // Соседняя заметка, а не заметка в корне — тот же случай, что и
    // backlinks_resolve_relative_to_the_linking_note.
    assert_eq!(
        resolve_link_target(Path::new("sub/sibling.typ"), "target"),
        Some(PathBuf::from("sub/target.typ"))
    );
    // Обе формы записи ведут к одной цели.
    assert_eq!(
        resolve_link_target(Path::new("b.typ"), "target"),
        resolve_link_target(Path::new("c.typ"), "target.typ")
    );
    // От корня vault.
    assert_eq!(
        resolve_link_target(Path::new("deep/nested/source.typ"), "/top"),
        Some(PathBuf::from("top.typ"))
    );
    // Выход за пределы vault неотличим от битой ссылки.
    assert_eq!(
        resolve_link_target(Path::new("source.typ"), "../outside"),
        None
    );

    assert_eq!(
        links_in(
            Path::new("notes/source.typ"),
            "[[target]] снова [[target]] и [[source]]"
        ),
        vec![PathBuf::from("notes/target.typ")]
    );
}

/// Никогда не вызывается — доказывает, что `WebBackend` несёт те же операции
/// разрешения конфликта с теми же сигнатурами, что и `NativeBackend`
/// (инвариант паритета из contracts/vault-backend-trait.md). Живой хендл для
/// сквозного прогона по-прежнему требует пользовательского жеста, см. T079.
#[allow(dead_code)]
async fn _conflict_api_signature_check(backend: &mut typsidian_core::vault::web::WebBackend) {
    use typsidian_core::vault::backend::VaultBackend;
    backend
        .begin_edit(Path::new("note.typ"), String::new())
        .await;
    let _: bool = backend.has_unsaved_edits(Path::new("note.typ"));
    let _ = backend.pending_conflict(Path::new("note.typ"));
    let _ = backend.unresolved_edit_paths();
    let _ = backend.conflict_diff(Path::new("note.typ")).await;
    let _ = backend
        .resolve_conflict_keep_local(Path::new("note.typ"))
        .await;
    let _ = backend
        .resolve_conflict_accept_external(Path::new("note.typ"))
        .await;
    backend.discard_edit(Path::new("note.typ")).await;
}

// ---------------------------------------------------------------------------
// Phase 9 (convergence): пределы масштаба и гейты записи под wasm32
//
// В отличие от T019/T035/T047 выше, эти правила НЕ живут в общем коде: предел
// FR-038 применяется внутри самого обхода `WebBackend`, а гейты FR-036 — в его
// `create`/`rename`/`delete`. Проверить их «через общий модуль», как сделано
// для конфликтов, нельзя, а без проверки принцип II не выполняется: именно
// расхождение между таргетами в этой области и обнаружил `/speckit-converge`
// (T100).
//
// Поэтому здесь используется синтетический двойник `FileSystemDirectoryHandle`
// — ровно та поверхность, которую трогает `vault::web` (`entries`,
// `getFileHandle`, `removeEntry`, `getFile`, `queryPermission`), и ничего
// сверх неё. Отказ от мока в комментариях выше относится к воспроизведению
// *пользовательского жеста* `showDirectoryPicker`, которого здесь и не
// требуется: `from_handle` принимает уже готовый хендл.
// ---------------------------------------------------------------------------

use typsidian_core::vault::backend::{CreateError, DeleteError, OpenError, RenameError};
use typsidian_core::vault::web::WebBackend;
use typsidian_core::vault::{AccessState, SUPPORTED_NOTE_COUNT};
use wasm_bindgen::JsValue;

/// Строит двойник каталога. `note_count` файлов вида `note{i}.typ`, плюс
/// именованные файлы из `extra` (пары `[имя, содержимое]`).
/// `readwrite`/`read` — то, что вернёт `queryPermission` для каждого режима.
fn mock_dir(note_count: usize, extra: &[(&str, &str)], readwrite: &str, read: &str) -> JsValue {
    let extra_json: String = extra
        .iter()
        .map(|(name, content)| format!("[{:?},{:?}]", name, content))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        r#"(function() {{
            const children = [];
            for (let i = 0; i < {note_count}; i++) {{
                children.push([`note${{i}}.typ`, `содержимое ${{i}}`]);
            }}
            for (const pair of [{extra_json}]) children.push(pair);

            // Содержимое живёт в общей таблице, а не в замыкании файла: тесту
            // задержки обнаружения (T109) нужно изменить файл «снаружи» между
            // двумя циклами сверки, как это делает сторонняя программа.
            const contents = new Map(children);
            const versions = new Map(children.map(([n]) => [n, 0]));

            function makeFile(name) {{
                return {{
                    kind: "file",
                    name,
                    async getFile() {{
                        const content = contents.get(name);
                        return {{
                            size: new TextEncoder().encode(content).length,
                            lastModified: versions.get(name),
                            async text() {{ return content; }},
                        }};
                    }},
                    async createWritable() {{
                        let pending = "";
                        return {{
                            async write(text) {{ pending = text; }},
                            async close() {{
                                contents.set(name, pending);
                                versions.set(name, (versions.get(name) ?? 0) + 1);
                            }},
                        }};
                    }},
                }};
            }}

            const entries = children.map(([n]) => [n, makeFile(n)]);
            return {{
                __setContent(name, text) {{
                    contents.set(name, text);
                    versions.set(name, (versions.get(name) ?? 0) + 1);
                }},
                __addFile(name, text) {{
                    contents.set(name, text);
                    versions.set(name, 0);
                    entries.push([name, makeFile(name)]);
                }},
                kind: "directory",
                name: "vault",
                async queryPermission(opts) {{
                    // Реальный API разрешается в PermissionStatus, а не в строку.
                    return {{ state: opts && opts.mode === "read" ? {read:?} : {readwrite:?} }};
                }},
                entries() {{
                    // Снимок на момент вызова: файл, созданный после начала
                    // обхода, этому обходу не виден (001 T136).
                    const list = entries.slice();
                    let i = 0;
                    return {{
                        async next() {{
                            if (i >= list.length) return {{ done: true }};
                            return {{ done: false, value: list[i++] }};
                        }},
                    }};
                }},
                async getFileHandle(n, opts) {{
                    const hit = entries.find(([k]) => k === n);
                    if (hit) return hit[1];
                    if (!(opts && opts.create)) throw new Error("not found");
                    contents.set(n, "");
                    versions.set(n, 0);
                    const made = [n, makeFile(n)];
                    entries.push(made);
                    return made[1];
                }},
                async removeEntry(n) {{
                    const i = entries.findIndex(([k]) => k === n);
                    if (i < 0) throw new Error("not found");
                    entries.splice(i, 1);
                }},
            }};
        }})()"#
    );
    js_sys::eval(&source).expect("двойник каталога должен собираться")
}

#[wasm_bindgen_test]
async fn vault_over_supported_note_count_is_refused_on_web_too() {
    // T103, FR-038, паритет с
    // vault_over_supported_note_count_is_refused_with_named_reason.
    let over = mock_dir(SUPPORTED_NOTE_COUNT + 1, &[], "granted", "granted");
    let result = WebBackend::from_handle("vault".into(), over).await;
    assert!(
        matches!(result, Err(OpenError::TooManyNotes)),
        "vault свыше предела MUST NOT открываться и на web"
    );
}

#[wasm_bindgen_test]
async fn note_over_size_limit_stays_in_tree_but_is_not_opened_on_web() {
    // T108, паритет с note_over_size_limit_stays_in_tree_but_is_not_opened.
    use std::path::PathBuf;
    use typsidian_core::vault::backend::{ReadError, VaultBackend};
    use typsidian_core::vault::NOTE_SIZE_LIMIT_BYTES;

    // `size` двойника считается по UTF-8 длине, поэтому байты и символы здесь
    // совпадают — превышение задаётся честной длиной, а не подделкой поля.
    let huge = "a".repeat(NOTE_SIZE_LIMIT_BYTES as usize + 1);
    let dir = mock_dir(
        0,
        &[("small.typ", "обычная заметка"), ("huge.typ", &huge)],
        "granted",
        "granted",
    );
    let backend = WebBackend::from_handle("vault".into(), dir).await.unwrap();

    let paths: Vec<PathBuf> = backend.list().await.into_iter().map(|n| n.path).collect();
    assert!(
        paths.contains(&PathBuf::from("huge.typ")),
        "заметка выше порога MUST оставаться видимой в дереве и на web"
    );
    assert_eq!(
        backend.read(Path::new("huge.typ")).await.err(),
        Some(ReadError::TooLarge)
    );
    assert_eq!(
        backend.read(Path::new("small.typ")).await.unwrap(),
        "обычная заметка"
    );
}

#[wasm_bindgen_test]
async fn read_only_access_denies_create_rename_and_delete_on_web_too() {
    // T106, FR-036, паритет с
    // read_only_access_denies_create_rename_and_delete_not_only_write.
    use typsidian_core::vault::backend::{VaultBackend, WriteError};

    // Права на запись отозваны, чтение осталось: readwrite=denied, read=granted.
    let dir = mock_dir(0, &[("note.typ", "содержимое")], "denied", "granted");
    let mut backend = WebBackend::from_handle("vault".into(), dir).await.unwrap();

    // Состояние доступа на web обновляется циклом сверки, а не подпиской.
    backend.poll_for_changes().await;
    assert_eq!(backend.access_state(), AccessState::ReadOnly);

    // Чтение продолжает работать как обычно (FR-036).
    assert_eq!(
        backend.read(Path::new("note.typ")).await.unwrap(),
        "содержимое"
    );

    // Все четыре попытки записи из перечня FR-036 отклонены, а не только write.
    assert_eq!(
        backend
            .write(Path::new("note.typ"), "новое", None)
            .await
            .err(),
        Some(WriteError::AccessDenied)
    );
    assert_eq!(
        backend.create(Path::new("new.typ")).await.err(),
        Some(CreateError::AccessDenied)
    );
    assert_eq!(
        backend
            .rename(Path::new("note.typ"), Path::new("renamed.typ"))
            .await
            .err(),
        Some(RenameError::AccessDenied)
    );
    assert_eq!(
        backend.delete(Path::new("note.typ")).await.err(),
        Some(DeleteError::AccessDenied)
    );
}

#[wasm_bindgen_test]
async fn unavailable_is_terminal_until_explicit_reopen_on_web() {
    // T100, FR-020: паритет с `NativeBackend`, где `spawn_access_watch`
    // останавливается на первом же `Unavailable`. Регрессионный тест ровно на
    // то расхождение таргетов, которое нашёл `/speckit-converge`.
    use typsidian_core::vault::backend::VaultBackend;

    use wasm_bindgen::JsCast;

    let dir = mock_dir(0, &[("note.typ", "содержимое")], "denied", "denied");
    let mut backend = WebBackend::from_handle("vault".into(), dir.clone())
        .await
        .unwrap();

    backend.poll_for_changes().await;
    assert_eq!(backend.access_state(), AccessState::Unavailable);

    // Доступ «вернулся»: подменяем ответ queryPermission на granted. Даже так
    // ни один следующий цикл опроса не имеет права поднять состояние обратно —
    // выход из недоступности только через явное переоткрытие vault.
    let restore = js_sys::eval(
        r#"(function(h) { h.queryPermission = async () => ({ state: "granted" }); })"#,
    )
    .unwrap()
    .dyn_into::<js_sys::Function>()
    .unwrap();
    restore.call1(&JsValue::NULL, &dir).unwrap();

    backend.poll_for_changes().await;
    backend.poll_for_changes().await;
    assert_eq!(
        backend.access_state(),
        AccessState::Unavailable,
        "Unavailable MUST оставаться терминальным до явного переоткрытия"
    );
}

#[wasm_bindgen_test]
async fn external_change_is_detected_within_sc004_budget_at_full_scale() {
    // T109, SC-004. Кларификация 2026-08-09 распространила бюджет «внешнее
    // изменение отражается ≤2 секунд» на весь поддерживаемый диапазон и на все
    // клиенты, а существующие бенчмарки (`benches/vault_scale.rs`,
    // `benches/vault_bulk_change.rs`) — только нативные. Здесь измеряется
    // именно web-путь: один цикл `pollForChanges` на vault предельного размера.
    //
    // ВАЖНО про интерпретацию числа: двойник каталога держит содержимое в
    // памяти JS, поэтому измеряется стоимость самого алгоритма сверки (обход,
    // предфильтр {size, lastModified}, диффинг индекса), а НЕ задержка
    // реального диска и не работа File System Access API. Это нижняя граница:
    // на реальном хранилище цикл не может стать дешевле. Проверка на реальном
    // хендле остаётся за ручным прогоном quickstart.md (T063).
    use std::path::Path;
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::events::VaultEvent;
    use wasm_bindgen::JsCast;

    let dir = mock_dir(SUPPORTED_NOTE_COUNT, &[], "granted", "granted");
    let mut backend = WebBackend::from_handle("vault".into(), dir.clone())
        .await
        .unwrap();
    assert_eq!(backend.list().await.len(), SUPPORTED_NOTE_COUNT);

    // Первый цикл после открытия — снимок уже наполнен в `from_handle`, так
    // что здесь измеряется установившийся режим, а не холодный старт.
    backend.poll_for_changes().await;
    let _ = backend.poll_events();

    // Внешняя программа меняет одну заметку из десяти тысяч.
    let set_content = js_sys::Reflect::get(&dir, &JsValue::from_str("__setContent"))
        .unwrap()
        .dyn_into::<js_sys::Function>()
        .unwrap();
    set_content
        .call2(
            &dir,
            &JsValue::from_str("note4242.typ"),
            &JsValue::from_str("изменено снаружи"),
        )
        .unwrap();

    let started = js_sys::Date::now();
    backend.poll_for_changes().await;
    let elapsed_ms = js_sys::Date::now() - started;

    // Изменение действительно замечено, а не «уложились в бюджет, ничего не
    // сделав».
    assert_eq!(
        backend.read(Path::new("note4242.typ")).await.unwrap(),
        "изменено снаружи"
    );
    assert!(
        backend.poll_events().iter().any(
            |e| matches!(e, VaultEvent::NoteUpdated { path } if path == Path::new("note4242.typ"))
        ),
        "внешнее изменение обязано дойти до UI событием, а не только до индекса"
    );

    // Один цикл сверки должен укладываться в бюджет с запасом: JS-сторона
    // вызывает его по таймеру, и сам цикл — лишь часть задержки SC-004.
    assert!(
        elapsed_ms < 2000.0,
        "цикл сверки на vault предельного размера занял {elapsed_ms} мс — \
         бюджет SC-004 (2000 мс) не выдержан даже без учёта реального диска"
    );
    // Число печатается, чтобы прогон был измерением, а не только проверкой.
    console_log(&format!(
        "T109: pollForChanges на {SUPPORTED_NOTE_COUNT} заметках — {elapsed_ms} мс (бюджет SC-004: 2000 мс)"
    ));
}

/// Меняет файл двойника «снаружи», как сторонняя программа.
fn set_content_outside(dir: &JsValue, name: &str, text: &str) {
    use wasm_bindgen::JsCast;
    js_sys::Reflect::get(dir, &JsValue::from_str("__setContent"))
        .unwrap()
        .dyn_into::<js_sys::Function>()
        .unwrap()
        .call2(dir, &JsValue::from_str(name), &JsValue::from_str(text))
        .unwrap();
}

#[wasm_bindgen_test]
async fn stepped_poll_reports_changes_only_after_the_full_walk() {
    // 001 T136: шаги с нулевым бюджетом продвигают цикл понемногу; события
    // появляются только по завершении полного обхода, и внешняя правка,
    // сделанная до цикла, в нём замечена.
    use std::path::Path;
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::events::VaultEvent;

    let dir = mock_dir(40, &[], "granted", "granted");
    let mut backend = WebBackend::from_handle("vault".into(), dir.clone())
        .await
        .unwrap();
    backend.poll_for_changes().await;
    let _ = backend.poll_events();

    set_content_outside(&dir, "note7.typ", "изменено снаружи");
    let mut steps = 0;
    loop {
        steps += 1;
        let finished = backend.poll_step(0.0).await;
        if finished {
            break;
        }
        assert!(
            backend.poll_events().is_empty(),
            "по незавершённому обходу событий быть не должно (шаг {steps})"
        );
        assert!(steps < 1000, "цикл обязан завершиться");
    }
    assert!(steps > 1, "нулевой бюджет делит цикл на шаги");
    assert!(
        backend.poll_events().iter().any(
            |e| matches!(e, VaultEvent::NoteUpdated { path } if path == Path::new("note7.typ"))
        ),
        "внешняя правка замечена по завершении цикла"
    );
}

#[wasm_bindgen_test]
async fn note_created_by_the_app_between_poll_steps_is_not_reported_deleted() {
    // 001 T136: обход начался до создания заметки и её не увидит. Без учёта
    // правок самого приложения сверка сочла бы новую заметку пропавшей с диска
    // и убрала бы её из дерева.
    use std::path::{Path, PathBuf};
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::events::VaultEvent;

    let dir = mock_dir(20, &[], "granted", "granted");
    let mut backend = WebBackend::from_handle("vault".into(), dir.clone())
        .await
        .unwrap();
    backend.poll_for_changes().await;
    let _ = backend.poll_events();

    assert!(!backend.poll_step(0.0).await, "цикл начат, но не закончен");
    backend.create(Path::new("new.typ")).await.unwrap();
    while !backend.poll_step(0.0).await {}

    let deleted = |events: &[VaultEvent]| {
        events
            .iter()
            .any(|e| matches!(e, VaultEvent::NoteDeleted { path } if path == Path::new("new.typ")))
    };
    assert!(!deleted(&backend.poll_events()), "своя заметка не удалена");
    let listed = |notes: Vec<typsidian_core::vault::index::Note>| {
        notes
            .into_iter()
            .any(|n| n.path == PathBuf::from("new.typ"))
    };
    assert!(listed(backend.list().await), "заметка осталась в дереве");

    // Следующий цикл видит файл на диске и тоже ничего не удаляет.
    backend.poll_for_changes().await;
    assert!(!deleted(&backend.poll_events()));
    assert!(listed(backend.list().await));
}

#[wasm_bindgen_test]
async fn external_deletion_between_poll_steps_is_seen_by_the_next_cycle() {
    // 001 T136: файл, удалённый снаружи после того, как обход его прошёл,
    // текущий цикл ещё считает живым; следующий — объявляет удалённым.
    use std::path::Path;
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::events::VaultEvent;
    use wasm_bindgen::JsCast;

    let dir = mock_dir(20, &[], "granted", "granted");
    let mut backend = WebBackend::from_handle("vault".into(), dir.clone())
        .await
        .unwrap();
    backend.poll_for_changes().await;
    let _ = backend.poll_events();

    assert!(!backend.poll_step(0.0).await);
    let remove = js_sys::Reflect::get(&dir, &JsValue::from_str("removeEntry"))
        .unwrap()
        .dyn_into::<js_sys::Function>()
        .unwrap();
    let promise = remove.call1(&dir, &JsValue::from_str("note0.typ")).unwrap();
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
        .await
        .unwrap();
    while !backend.poll_step(0.0).await {}
    let _ = backend.poll_events();

    backend.poll_for_changes().await;
    assert!(
        backend.poll_events().iter().any(
            |e| matches!(e, VaultEvent::NoteDeleted { path } if path == Path::new("note0.typ"))
        ),
        "удаление снаружи замечено следующим циклом"
    );
}

#[wasm_bindgen_test]
async fn conflict_copies_come_from_the_last_full_walk_without_a_walk_of_their_own() {
    // 011 T081: копии известны с открытия, разобранная копия пропадает из
    // списка сразу, новая извне появляется после ближайшего цикла опроса.
    use std::path::PathBuf;
    use typsidian_core::vault::backend::VaultBackend;
    use wasm_bindgen::JsCast;

    const OLD: &str = "note1.sync-conflict-20261003-120000-ABCDEFG.typ";
    const NEW: &str = "note2.sync-conflict-20261019-090000-HIJKLMN.typ";
    let dir = mock_dir(5, &[(OLD, "их версия")], "granted", "granted");
    let mut backend = WebBackend::from_handle("vault".into(), dir.clone())
        .await
        .unwrap();
    assert_eq!(
        backend.list_conflict_copies().await,
        vec![PathBuf::from(OLD)],
        "копия известна с открытия"
    );

    let call = |method: &str, args: &[&str]| {
        let f = js_sys::Reflect::get(&dir, &JsValue::from_str(method))
            .unwrap()
            .dyn_into::<js_sys::Function>()
            .unwrap();
        let args: js_sys::Array = args.iter().map(|a| JsValue::from_str(a)).collect();
        f.apply(&dir, &args).unwrap()
    };
    let removed = call("removeEntry", &[OLD]);
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(removed))
        .await
        .unwrap();
    assert!(
        backend.list_conflict_copies().await.is_empty(),
        "разобранная копия не ждёт цикла опроса"
    );

    call("__addFile", &[NEW, "ещё одна версия"]);
    backend.poll_for_changes().await;
    assert_eq!(
        backend.list_conflict_copies().await,
        vec![PathBuf::from(NEW)],
        "копия извне видна после цикла опроса"
    );
}

// Прямая привязка вместо web-sys-фичи "console": включать фичу зависимости
// ради одной строки в тесте — плата не по месту.
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(message: &str);
}

// ---------------------------------------------------------------------------
// Классификация файлов (фича 002, T009) — тот же набор, что в vault_native.rs
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn classification_matches_native_on_the_same_paths() {
    // Принцип II: правило одно, и одинаковый путь обязан давать одинаковый
    // результат на обоих таргетах (research.md §3 фичи 002). Раньше признак
    // «это заметка» жил в двух местах и по-разному — здесь это исключено.
    use std::path::Path;
    use typsidian_core::vault::{classify, FileKind};

    assert_eq!(classify(Path::new("Заметка.typ")), FileKind::Note);
    assert_eq!(classify(Path::new("папка/Заметка.typ")), FileKind::Note);
    assert_eq!(classify(Path::new("Схема.excalidraw")), FileKind::Board);
    assert_eq!(
        classify(Path::new("папка/Схема.excalidraw")),
        FileKind::Board
    );
    assert_eq!(
        classify(Path::new("Заметка.assets/board-a1b2c3.svg")),
        FileKind::Snapshot
    );
    assert_eq!(
        classify(Path::new("папка/Заметка.assets/board-a1b2c3.svg")),
        FileKind::Snapshot
    );
    assert_eq!(classify(Path::new("схема.svg")), FileKind::Other);
    assert_eq!(classify(Path::new("вложения/фото.png")), FileKind::Other);

    assert!(FileKind::Note.is_listed());
    assert!(FileKind::Board.is_listed());
    assert!(!FileKind::Snapshot.is_listed());
    assert!(!FileKind::Other.is_listed());
}

// ---------------------------------------------------------------------------
// Фича 016 — изображения vault для сопоставления вложений (T009)
// ---------------------------------------------------------------------------

/// Двойник каталога с вложенными папками: пары «путь через `/` — содержимое».
fn mock_tree(files: &[(&str, &str)]) -> JsValue {
    let pairs: String = files
        .iter()
        .map(|(path, content)| format!("[{:?},{:?}]", path, content))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        r#"(function() {{
            function makeDir(name) {{
                const children = new Map();
                return {{
                    kind: "directory",
                    name,
                    children,
                    async queryPermission() {{ return {{ state: "granted" }}; }},
                    entries() {{
                        const list = [...children.entries()];
                        let i = 0;
                        return {{ async next() {{
                            if (i >= list.length) return {{ done: true }};
                            return {{ done: false, value: list[i++] }};
                        }} }};
                    }},
                    async getDirectoryHandle(n, opts) {{
                        const hit = children.get(n);
                        if (hit && hit.kind === "directory") return hit;
                        if (hit || !(opts && opts.create)) throw new Error("not found");
                        const made = makeDir(n);
                        children.set(n, made);
                        return made;
                    }},
                    async getFileHandle(n) {{
                        const hit = children.get(n);
                        if (!hit || hit.kind !== "file") throw new Error("not found");
                        return hit;
                    }},
                }};
            }}
            function makeFile(name, content) {{
                return {{
                    kind: "file",
                    name,
                    async getFile() {{
                        return {{
                            size: new TextEncoder().encode(content).length,
                            lastModified: 0,
                            async text() {{ return content; }},
                        }};
                    }},
                }};
            }}
            const root = makeDir("vault");
            for (const [path, content] of [{pairs}]) {{
                const parts = path.split("/");
                let dir = root;
                for (const part of parts.slice(0, -1)) {{
                    if (!dir.children.has(part)) dir.children.set(part, makeDir(part));
                    dir = dir.children.get(part);
                }}
                const name = parts[parts.length - 1];
                dir.children.set(name, makeFile(name, content));
            }}
            return root;
        }})()"#
    );
    js_sys::eval(&source).expect("двойник дерева должен собираться")
}

#[wasm_bindgen_test]
async fn list_images_walks_the_vault_or_one_folder_like_the_native_backend() {
    use std::path::{Path, PathBuf};
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::ImageFile;

    let dir = mock_tree(&[
        ("a.png", "png"),
        ("Папка/b.JPG", "jpeg"),
        ("Папка/вложено/c.svg", "<svg>"),
        ("Папка/note.typ", "= Заметка"),
        ("Папка/.attachments/f.gif", "GIF89a"),
        (".hidden/d.png", "скрыто"),
        (".typsidian/e.png", "служебное"),
    ]);
    let backend = WebBackend::from_handle("vault".into(), dir).await.unwrap();
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
        ]
    );
    assert_eq!(
        backend.list_images(Some(Path::new("Папка"))).await,
        vec![image("Папка/b.JPG", 4)]
    );
    assert_eq!(
        backend
            .list_images(Some(Path::new("Папка/.attachments")))
            .await,
        vec![image("Папка/.attachments/f.gif", 6)]
    );
    assert!(backend.list_images(Some(Path::new("нет"))).await.is_empty());
}

// ---------------------------------------------------------------------------
// Фича 017 — изменения файлов источников на web (T047)
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
async fn source_file_changes_are_announced_by_the_poll_and_by_own_writes() {
    use std::path::Path;
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::events::VaultEvent;
    use wasm_bindgen::JsCast;

    let announced = |events: &[VaultEvent]| {
        events
            .iter()
            .any(|event| matches!(event, VaultEvent::SourceFilesChanged))
    };
    let dir = mock_dir(3, &[], "granted", "granted");
    let mut backend = WebBackend::from_handle("vault".into(), dir.clone())
        .await
        .unwrap();
    // Первый цикл только запоминает снимок кандидатов.
    backend.poll_for_changes().await;
    assert!(!announced(&backend.poll_events()));

    let add = js_sys::Reflect::get(&dir, &JsValue::from_str("__addFile"))
        .unwrap()
        .dyn_into::<js_sys::Function>()
        .unwrap();
    add.call2(
        &dir,
        &JsValue::from_str("notes.txt"),
        &JsValue::from_str("x"),
    )
    .unwrap();
    backend.poll_for_changes().await;
    assert!(
        !announced(&backend.poll_events()),
        "посторонний файл — не событие"
    );

    add.call2(
        &dir,
        &JsValue::from_str("refs.bib"),
        &JsValue::from_str("@misc{a, title={x}}"),
    )
    .unwrap();
    backend.poll_for_changes().await;
    assert!(
        announced(&backend.poll_events()),
        "новый .bib замечен опросом"
    );

    set_content_outside(&dir, "refs.bib", "@misc{b, title={y}}");
    backend.poll_for_changes().await;
    assert!(
        announced(&backend.poll_events()),
        "правка .bib замечена опросом"
    );

    backend
        .write_bytes(Path::new("own.bib"), b"")
        .await
        .unwrap();
    assert!(announced(&backend.poll_events()), "своя запись — событие");
}
