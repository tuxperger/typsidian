use crate::vault::backend::{
    valid_note_name, CreateError, DeleteError, OpenError, ReadError, RenameError, ResolveError,
    VaultBackend, WriteError,
};
use crate::vault::cache::{record_links, CacheMove, IndexCache, MemoryCache};
use crate::vault::conflict::{line_diff, ConflictRegistry, ConflictStatus, DiffLine, EditConflict};
use crate::vault::events::{links_invalidated, wrap_bulk_if_needed, VaultEvent};
use crate::vault::index::{FileStamp, Index, Note};
use crate::vault::relocate::{
    check_move, conflict_name, normalize_request, placement_of, plan_steps, snapshot_folders_of,
    MoveControl, MoveError, MoveImpact, MoveOutcome, MovePlan, MoveRefusal, MoveRequest,
    MoveTarget,
};
use crate::vault::{AccessState, NOTE_SIZE_LIMIT_BYTES, SUPPORTED_NOTE_COUNT};
use js_sys::{Object, Reflect};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

fn is_excluded(entry_name: &str) -> bool {
    entry_name.starts_with('.')
}

/// Попадает ли файл в дерево. Классификация идёт по **пути**, а не по имени:
/// снимок опознаётся по папке `*.assets`, которую в одном имени файла не
/// видно. Правило общее с native — `super::classify` (research.md §3 фичи 002).
fn is_listed(path: &Path) -> bool {
    super::classify(path).is_listed()
}

async fn js_call0(obj: &JsValue, method: &str) -> Result<JsValue, JsValue> {
    let f = Reflect::get(obj, &JsValue::from_str(method))?;
    let f: js_sys::Function = f.dyn_into()?;
    let result = f.call0(obj)?;
    if result.has_type::<js_sys::Promise>() {
        JsFuture::from(js_sys::Promise::from(result)).await
    } else {
        Ok(result)
    }
}

/// Рекурсивно обходит `FileSystemDirectoryHandle` через JS async-итератор
/// `.entries()` — web-sys не даёт типизированного асинхронного итератора для
/// директорий на всех версиях, поэтому протокол управляется вручную через
/// `js_sys::Reflect` (research.md §4: обход выполняется без `SharedArrayBuffer`,
/// однопоточно).
/// `limit` — предел числа заметок (FR-038): обход прерывается
/// [`WalkAbort::TooManyNotes`], как только найденных заметок стало **больше**
/// предела. `usize::MAX` отключает проверку (сверка уже открытого vault, где
/// предел применён при открытии).
async fn walk_directory(
    dir_handle: &JsValue,
    prefix: PathBuf,
    out: &mut Vec<(PathBuf, JsValue)>,
    limit: usize,
) -> Result<(), WalkAbort> {
    walk_directory_matching(dir_handle, prefix, out, limit, is_listed).await
}

/// Тот же обход с произвольным отбором файлов — шрифтам (FR-027) нужны файлы,
/// которые в дерево как раз не попадают.
async fn walk_directory_matching(
    dir_handle: &JsValue,
    prefix: PathBuf,
    out: &mut Vec<(PathBuf, JsValue)>,
    limit: usize,
    keep: fn(&Path) -> bool,
) -> Result<(), WalkAbort> {
    let entries_iter = js_call0(dir_handle, "entries").await?;
    loop {
        let next_result = js_call0(&entries_iter, "next").await?;
        let done = Reflect::get(&next_result, &JsValue::from_str("done"))?
            .as_bool()
            .unwrap_or(true);
        if done {
            break;
        }
        let value = Reflect::get(&next_result, &JsValue::from_str("value"))?;
        let name = Reflect::get(&value, &JsValue::from_str("0"))?
            .as_string()
            .unwrap_or_default();
        let handle = Reflect::get(&value, &JsValue::from_str("1"))?;
        if is_excluded(&name) {
            continue;
        }
        let kind = Reflect::get(&handle, &JsValue::from_str("kind"))?
            .as_string()
            .unwrap_or_default();
        let child_path = prefix.join(&name);
        if kind == "directory" {
            Box::pin(walk_directory_matching(
                &handle, child_path, out, limit, keep,
            ))
            .await?;
        } else if kind == "file" && keep(&child_path) {
            out.push((child_path, handle));
            if out.len() > limit {
                return Err(WalkAbort::TooManyNotes);
            }
        }
    }
    Ok(())
}

/// Изображения одной папки без вложенных (фича 016, `list_images`): файлы
/// `dir`, у которых путь `prefix/имя` — изображение.
async fn collect_folder_files(
    dir: &JsValue,
    prefix: &Path,
    out: &mut Vec<(PathBuf, JsValue)>,
) -> Result<(), WalkAbort> {
    let entries_iter = js_call0(dir, "entries").await?;
    loop {
        let next_result = js_call0(&entries_iter, "next").await?;
        let done = Reflect::get(&next_result, &JsValue::from_str("done"))?
            .as_bool()
            .unwrap_or(true);
        if done {
            return Ok(());
        }
        let value = Reflect::get(&next_result, &JsValue::from_str("value"))?;
        let name = Reflect::get(&value, &JsValue::from_str("0"))?
            .as_string()
            .unwrap_or_default();
        let handle = Reflect::get(&value, &JsValue::from_str("1"))?;
        let kind = Reflect::get(&handle, &JsValue::from_str("kind"))?
            .as_string()
            .unwrap_or_default();
        let path = prefix.join(&name);
        if kind == "file" && super::is_image_file(&path) {
            out.push((path, handle));
        }
    }
}

/// Обход vault по частям (001 T136): то же, что `walk_directory` без предела,
/// плюс попутный сбор конфликтных копий по правилу `walk_conflict_copies`
/// (011 T081) — тогда синхронизации не нужен свой обход всего vault.
struct StepWalk {
    /// Незаконченные каталоги: путь, итератор `entries()` и попадают ли их
    /// файлы в дерево (`false` — служебное место, из него нужны только копии).
    stack: Vec<(PathBuf, JsValue, bool)>,
    found: Vec<(PathBuf, JsValue)>,
    copies: Vec<PathBuf>,
    /// Кандидаты в файлы источников (017) — для снимка в цикле опроса.
    sources: Vec<(PathBuf, JsValue)>,
}

impl StepWalk {
    async fn start(root: &JsValue) -> Result<Self, WalkAbort> {
        let entries_iter = js_call0(root, "entries").await?;
        Ok(StepWalk {
            stack: vec![(PathBuf::new(), entries_iter, true)],
            found: Vec::new(),
            copies: Vec::new(),
            sources: Vec::new(),
        })
    }

    /// Продвигает обход до `deadline` и возвращает, закончен ли он. Хотя бы
    /// одна запись обрабатывается за вызов при любом бюджете, иначе нулевой
    /// бюджет не сдвинул бы обход никогда.
    async fn step(&mut self, deadline: f64) -> Result<bool, WalkAbort> {
        loop {
            let Some((prefix, entries_iter, listed)) = self.stack.last() else {
                return Ok(true);
            };
            let listed = *listed;
            let next_result = js_call0(entries_iter, "next").await?;
            let done = Reflect::get(&next_result, &JsValue::from_str("done"))?
                .as_bool()
                .unwrap_or(true);
            if done {
                self.stack.pop();
            } else {
                let value = Reflect::get(&next_result, &JsValue::from_str("value"))?;
                let name = Reflect::get(&value, &JsValue::from_str("0"))?
                    .as_string()
                    .unwrap_or_default();
                let handle = Reflect::get(&value, &JsValue::from_str("1"))?;
                let kind = Reflect::get(&handle, &JsValue::from_str("kind"))?
                    .as_string()
                    .unwrap_or_default();
                let child_path = prefix.join(&name);
                if kind == "directory" {
                    let service = name == super::service_dir::SERVICE_DIR_NAME;
                    if !is_excluded(&name) || service {
                        let child_iter = js_call0(&handle, "entries").await?;
                        self.stack
                            .push((child_path, child_iter, listed && !is_excluded(&name)));
                    }
                } else if kind == "file" {
                    if crate::sync::conflict_copy::is_conflict_copy(&name) {
                        self.copies.push(child_path.clone());
                    }
                    if listed
                        && !is_excluded(&name)
                        && crate::sources::discover::is_source_candidate(&child_path)
                    {
                        self.sources.push((child_path.clone(), handle.clone()));
                    }
                    if listed && !is_excluded(&name) && is_listed(&child_path) {
                        self.found.push((child_path, handle));
                    }
                }
            }
            if self.stack.is_empty() {
                return Ok(true);
            }
            if js_sys::Date::now() >= deadline {
                return Ok(false);
            }
        }
    }
}

/// Незавершённый цикл сверки web (001 T136). Цикл идёт шагами, чтобы вызовы
/// пользователя вставали между ними в очереди вызовов wasm, а не ждали обхода
/// всего vault (025 research.md §15).
struct PollCycle {
    /// Оценка доступа на начало цикла — как у прежнего цельного опроса.
    permission_guess: AccessState,
    walk: StepWalk,
    /// Предфильтр `{size, lastModified}` для первых `metas.len()` из `walk.found`.
    metas: Vec<Option<(u64, f64)>>,
    /// Отпечатки индекса на начало цикла. Путь, который само приложение
    /// изменило между шагами (создало, сохранило, удалило, перенесло), в этом
    /// цикле не сверяется: обход мог пройти его каталог до правки и счёл бы,
    /// например, только что созданную заметку удалённой. Сверит следующий цикл.
    start: HashMap<PathBuf, FileStamp>,
}

/// Конфликтные копии Syncthing (фича 011, FR-024) — тот же обход, но со
/// служебным местом: копия `vault.json` означает спор о хранителе. Правило
/// каталогов то же, что у `native::walk_conflict_copies` (принцип II).
async fn walk_conflict_copies(
    dir_handle: &JsValue,
    prefix: PathBuf,
    out: &mut Vec<PathBuf>,
) -> Result<(), WalkAbort> {
    let entries_iter = js_call0(dir_handle, "entries").await?;
    loop {
        let next_result = js_call0(&entries_iter, "next").await?;
        let done = Reflect::get(&next_result, &JsValue::from_str("done"))?
            .as_bool()
            .unwrap_or(true);
        if done {
            break;
        }
        let value = Reflect::get(&next_result, &JsValue::from_str("value"))?;
        let name = Reflect::get(&value, &JsValue::from_str("0"))?
            .as_string()
            .unwrap_or_default();
        let handle = Reflect::get(&value, &JsValue::from_str("1"))?;
        let kind = Reflect::get(&handle, &JsValue::from_str("kind"))?
            .as_string()
            .unwrap_or_default();
        let child_path = prefix.join(&name);
        if kind == "directory" {
            if !is_excluded(&name) || name == super::service_dir::SERVICE_DIR_NAME {
                Box::pin(walk_conflict_copies(&handle, child_path, out)).await?;
            }
        } else if kind == "file" && crate::sync::conflict_copy::is_conflict_copy(&name) {
            out.push(child_path);
        }
    }
    Ok(())
}

/// Почему обход не дошёл до конца. `Js` — реальный отказ доступа (папка
/// пропала, разрешение отозвано), `TooManyNotes` — предел FR-038; смешивать их
/// нельзя: первое означает `Unavailable`, второе — отказ открыть исправную
/// папку.
enum WalkAbort {
    /// Сам `JsValue` не сохраняется: единственное, что вызывающая сторона с
    /// ним делает, — переводит в `AccessDenied`/`Unavailable`, а исходное
    /// JS-исключение причиной для пользователя всё равно не является (FR-008).
    Js,
    TooManyNotes,
}

impl From<JsValue> for WalkAbort {
    fn from(_: JsValue) -> Self {
        WalkAbort::Js
    }
}

async fn read_file_handle_text(file_handle: &JsValue) -> Result<String, JsValue> {
    let file = js_call0(file_handle, "getFile").await?;
    let text_promise = js_call0(&file, "text").await?;
    Ok(text_promise.as_string().unwrap_or_default())
}

/// Содержимое файла байтами — `File.arrayBuffer()` вместо `File.text()`.
///
/// Нужно предпросмотру для изображений: `text()` испортил бы всё, что не
/// UTF-8, заменой недопустимых последовательностей.
async fn read_file_handle_bytes(file_handle: &JsValue) -> Result<Vec<u8>, JsValue> {
    let file = js_call0(file_handle, "getFile").await?;
    let buffer = js_call0(&file, "arrayBuffer").await?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

fn stamp_from_content(content: &str) -> FileStamp {
    FileStamp::from_content(0, content.as_bytes())
}

/// Дешёвые метаданные файла — `{size, lastModified}` из JS `File` — без
/// чтения содержимого (research.md §3: снимок `{path, size, lastModified}`
/// как предфильтр перед полным чтением файла на каждом цикле опроса).
async fn file_meta(file_handle: &JsValue) -> Option<(u64, f64)> {
    let file = js_call0(file_handle, "getFile").await.ok()?;
    let size = Reflect::get(&file, &JsValue::from_str("size"))
        .ok()?
        .as_f64()? as u64;
    let last_modified = Reflect::get(&file, &JsValue::from_str("lastModified"))
        .ok()?
        .as_f64()?;
    Some((size, last_modified))
}

/// Запрашивает `PermissionState` ('granted'/'denied'/'prompt') у File System
/// Access API `FileSystemHandle.queryPermission({mode})` — в отличие от
/// `requestPermission`, не требует пользовательского жеста и не показывает
/// диалог, поэтому пригоден для периодического пассивного опроса (T056).
/// `None`, если сама функция недоступна в этом браузере (не все реализации
/// File System Access API её экспортируют).
async fn permission_state(handle: &JsValue, mode: &str) -> Option<String> {
    let query_fn = Reflect::get(handle, &JsValue::from_str("queryPermission")).ok()?;
    let query_fn: js_sys::Function = query_fn.dyn_into().ok()?;
    let opts = Object::new();
    Reflect::set(&opts, &JsValue::from_str("mode"), &JsValue::from_str(mode)).ok()?;
    let promise = query_fn.call1(handle, &opts).ok()?;
    let result = JsFuture::from(js_sys::Promise::from(promise)).await.ok()?;
    // `queryPermission` разрешается в `PermissionStatus`, а не в строку —
    // состояние лежит в поле `state`. Читать сам объект как строку означало бы
    // всегда получать `None` и, через `_ => ReadWrite` в
    // `probe_permission_access_state`, никогда не замечать отзыв прав на web
    // (FR-036) и потерю доступа (FR-020). Голая строка тоже принимается: так
    // возвращают некоторые реализации и наши тестовые двойники.
    if let Some(state) = result.as_string() {
        return Some(state);
    }
    Reflect::get(&result, &JsValue::from_str("state"))
        .ok()?
        .as_string()
}

/// Оценка `AccessState` на основе `queryPermission` (T056, FR-020, FR-036).
/// Не единственный сигнал: `poll_for_changes` уточняет её до `Unavailable`,
/// если сам обход директории реально проваливается (более сильное,
/// конкретное свидетельство, чем догадка по разрешениям). Если API
/// недоступен в этом браузере или её результат не определён однозначно
/// ('prompt', сама функция отсутствует) — не понижаем состояние по одному
/// этому признаку.
async fn probe_permission_access_state(handle: &JsValue) -> AccessState {
    match permission_state(handle, "readwrite").await.as_deref() {
        Some("granted") => AccessState::ReadWrite,
        Some("denied") => match permission_state(handle, "read").await.as_deref() {
            Some("denied") => AccessState::Unavailable,
            _ => AccessState::ReadOnly,
        },
        _ => AccessState::ReadWrite,
    }
}

/// Все файлы и папки под папкой, **включая скрытое** (фича 023, FR-002).
///
/// Отдельный обход, а не `walk_directory`: тот отбирает то, что видит дерево, а
/// перенос папки обязан увезти всё — вложения, снимки, скрытые файлы. Пути
/// относительны `prefix`; папка всегда раньше своего содержимого, так что
/// создавать их можно в порядке списка.
async fn walk_everything(
    dir_handle: &JsValue,
    prefix: PathBuf,
    files: &mut Vec<PathBuf>,
    dirs: &mut Vec<PathBuf>,
) -> Result<(), JsValue> {
    let entries_iter = js_call0(dir_handle, "entries").await?;
    loop {
        let next_result = js_call0(&entries_iter, "next").await?;
        if Reflect::get(&next_result, &JsValue::from_str("done"))?
            .as_bool()
            .unwrap_or(true)
        {
            return Ok(());
        }
        let value = Reflect::get(&next_result, &JsValue::from_str("value"))?;
        let name = Reflect::get(&value, &JsValue::from_str("0"))?
            .as_string()
            .unwrap_or_default();
        let handle = Reflect::get(&value, &JsValue::from_str("1"))?;
        let kind = Reflect::get(&handle, &JsValue::from_str("kind"))?
            .as_string()
            .unwrap_or_default();
        let child = prefix.join(&name);
        if kind == "directory" {
            dirs.push(child.clone());
            Box::pin(walk_everything(&handle, child, files, dirs)).await?;
        } else {
            files.push(child);
        }
    }
}

/// Все файлы под папкой с размерами, кроме скрытых записей (фича 021): их
/// число копится в `excluded`. Правило то же, что у `native::walk_source_files`
/// (принцип II).
async fn walk_source_files(
    dir_handle: &JsValue,
    prefix: PathBuf,
    listing: &mut super::SourceListing,
) -> Result<(), JsValue> {
    let entries_iter = js_call0(dir_handle, "entries").await?;
    loop {
        let next_result = js_call0(&entries_iter, "next").await?;
        if Reflect::get(&next_result, &JsValue::from_str("done"))?
            .as_bool()
            .unwrap_or(true)
        {
            return Ok(());
        }
        let value = Reflect::get(&next_result, &JsValue::from_str("value"))?;
        let name = Reflect::get(&value, &JsValue::from_str("0"))?
            .as_string()
            .unwrap_or_default();
        let handle = Reflect::get(&value, &JsValue::from_str("1"))?;
        if is_excluded(&name) {
            listing.excluded += 1;
            continue;
        }
        let kind = Reflect::get(&handle, &JsValue::from_str("kind"))?
            .as_string()
            .unwrap_or_default();
        let child = prefix.join(&name);
        if kind == "directory" {
            Box::pin(walk_source_files(&handle, child, listing)).await?;
        } else if let Some((size, _)) = file_meta(&handle).await {
            listing.files.push(super::SourceFile { path: child, size });
        }
    }
}

/// `removeEntry(name, { recursive: true })` на хендле папки.
async fn remove_entry_recursive(parent: &JsValue, name: &str) -> Result<(), JsValue> {
    let remove_fn: js_sys::Function =
        Reflect::get(parent, &JsValue::from_str("removeEntry"))?.dyn_into()?;
    let opts = Object::new();
    Reflect::set(
        &opts,
        &JsValue::from_str("recursive"),
        &JsValue::from_bool(true),
    )?;
    let promise = remove_fn.call2(parent, &JsValue::from_str(name), &opts)?;
    JsFuture::from(js_sys::Promise::from(promise)).await?;
    Ok(())
}

/// Путь внутри шага переноса: пустой `rel` — сам шаг (файл).
fn step_path(base: &Path, rel: &Path) -> PathBuf {
    if rel.as_os_str().is_empty() {
        base.to_path_buf()
    } else {
        base.join(rel)
    }
}

/// Что известно о шаге после обхода источника.
struct StepInventory {
    source: PathBuf,
    dest: PathBuf,
    /// Папки внутри шага (относительно него), родитель раньше потомка.
    dirs: Vec<PathBuf>,
    /// Файлы шага; `""` — сам шаг является файлом.
    files: Vec<PathBuf>,
}

/// Почему копирование остановилось.
enum CopyStop {
    Cancelled,
    Failed,
}

/// Результат копирования, нужный для обновления состояния.
#[derive(Default)]
struct CopyReport {
    /// Свежие метаданные `{size, lastModified}` скопированных файлов — чтобы
    /// ближайший `pollForChanges` не принял собственный перенос за правку.
    metas: Vec<(PathBuf, (u64, f64))>,
    /// Тексты скопированных заметок для пересчёта ссылок.
    texts: HashMap<PathBuf, String>,
}

/// Ход и отмена с JS-стороны (фича 023, research.md §6).
///
/// `on_progress(done, total)` и объект-флаг `{ cancelled: boolean }`: вызов
/// ядра занят `&mut self`, второй вызов «отмени» в него не войдёт, поэтому
/// интерфейс держит флаг у себя, а ядро читает его между файлами.
pub(crate) struct JsMoveControl {
    pub(crate) on_progress: Option<js_sys::Function>,
    pub(crate) cancel: JsValue,
}

impl MoveControl for JsMoveControl {
    fn progress(&self, done: u32, total: u32) {
        if let Some(callback) = &self.on_progress {
            let _ = callback.call2(&JsValue::NULL, &JsValue::from(done), &JsValue::from(total));
        }
    }

    fn cancelled(&self) -> bool {
        if self.cancel.is_undefined() || self.cancel.is_null() {
            return false;
        }
        Reflect::get(&self.cancel, &JsValue::from_str("cancelled"))
            .ok()
            .and_then(|flag| flag.as_bool())
            .unwrap_or(false)
    }
}

/// Web-реализация `VaultBackend`: File System Access API + периодическая сверка
/// вместо нативных FS-событий (research.md §3 — у File System Access API нет
/// API подписки на изменения).
#[wasm_bindgen]
pub struct WebBackend {
    root_path: PathBuf,
    root_handle: JsValue,
    access_state: AccessState,
    index: Index,
    cache: MemoryCache,
    /// Несохранённые буферы и активные конфликты (US4) — тот же общий
    /// `ConflictRegistry`, что и у `NativeBackend`: правила поднятия и
    /// разрешения конфликта не имеют права расходиться по таргету (принцип II).
    conflicts: ConflictRegistry,
    events: VecDeque<VaultEvent>,
    /// Право этой оболочки писать доски (фича 002, FR-035). Поле, а не
    /// константа в месте записи: тот же экземпляр отвечает и на вопрос
    /// интерфейса «можно ли рисовать», и на запрос записи, поэтому объявленное
    /// пользователю ограничение и фактический гейт не могут разойтись.
    board_write_profile: crate::boards::BoardWriteProfile,
    /// Кэш превью заготовок (фича 003, research.md §4).
    ///
    /// Производное восстановимое состояние: авторитетной копией ничему не
    /// является, живёт ровно столько, сколько открыт этот vault. Ключ — хэш
    /// содержимого, поэтому правка заготовки снаружи пересчитывает превью сама.
    template_previews: crate::templates::PreviewCache,
    /// Последний известный дешёвый снимок `{size, lastModified}` на путь
    /// (research.md §3) — предфильтр `poll_for_changes`, отдельный от
    /// `Note.stamp` (который хранит content_hash, а не lastModified: web
    /// намеренно не использует `lastModified` как часть идентичности
    /// заметки, см. `stamp_from_content`, только как сигнал "возможно,
    /// изменилось, стоит перечитать").
    poll_snapshot: HashMap<PathBuf, (u64, f64)>,
    /// Незавершённый цикл сверки `pollStep` (001 T136); `None` — следующий шаг
    /// начинает новый цикл.
    poll_cycle: Option<PollCycle>,
    /// Конфликтные копии по последнему полному обходу (открытие или цикл
    /// опроса, 011 T081); `None` — полного обхода ещё не было.
    conflict_copies: Option<Vec<PathBuf>>,
    /// Снимок `{size, lastModified}` кандидатов в источники (017, research.md
    /// §7): расхождение кладёт `SourceFilesChanged`. `None` — опроса ещё не было.
    source_snapshot: Option<HashMap<PathBuf, (u64, f64)>>,
    /// Существовало ли `.typsidian/` до открытия (фича 006, FR-024).
    ///
    /// Снимок момента открытия, а не текущее состояние: открытие само эту
    /// папку создаёт, поэтому спросить позже — значит всегда получить `true`.
    service_dir_existed: bool,
}

// `JsValue` не Send/Sync, но wasm32 однопоточен — обёртка безопасна в этом контексте.
unsafe impl Send for WebBackend {}

impl WebBackend {
    /// Папка-источник импорта (фича 021, FR-002): хендл читается, но служебное
    /// место не заводится, индекс не строится, доступ — `ReadOnly`. Обычный
    /// [`WebBackend::from_handle`] записал бы в папку `.typsidian/`.
    pub(crate) fn for_reading(name: String, handle: JsValue) -> Self {
        WebBackend {
            root_path: PathBuf::from(name),
            root_handle: handle,
            access_state: AccessState::ReadOnly,
            index: Index::new(),
            cache: MemoryCache::new(),
            conflicts: ConflictRegistry::new(),
            events: VecDeque::new(),
            board_write_profile: crate::boards::BoardWriteProfile::ReadWrite,
            template_previews: crate::templates::PreviewCache::new(),
            poll_snapshot: HashMap::new(),
            poll_cycle: None,
            conflict_copies: None,
            source_snapshot: None,
            service_dir_existed: true,
        }
    }
}

impl WebBackend {
    /// Единственный гейт записи для всех четырёх операций, названных в FR-036
    /// (сохранение, создание, переименование, удаление) — паритет с
    /// `NativeBackend::write_denied` (принцип II).
    fn write_denied(&self) -> bool {
        self.access_state != AccessState::ReadWrite
    }

    /// Есть ли по этому пути файл или папка. Занятость имени в цели
    /// проверяется по обоим видам: на диске имя одно на файл и папку.
    async fn entry_exists(&self, path: &Path) -> bool {
        self.find_folder_handle(path).await.is_some() || self.find_handle(path).await.is_some()
    }

    /// Проверки переноса и план — паритет с `NativeBackend::plan_move`
    /// (фича 023, принцип II). Занятость имён смотрится по хендлам, а не по
    /// индексу: в цели может лежать файл, которого дерево не показывает.
    async fn plan_move(&self, request: &MoveRequest) -> Result<MovePlan, MoveRefusal> {
        if self.write_denied() {
            return Err(MoveRefusal::AccessDenied);
        }
        let (from, to_folder) = normalize_request(request)?;
        let new_path = check_move(&from, &to_folder)?;

        let is_folder = if self.find_folder_handle(&from).await.is_some() {
            true
        } else if self.find_handle(&from).await.is_some() {
            false
        } else {
            return Err(MoveRefusal::NotFound);
        };
        if !to_folder.as_os_str().is_empty() && self.find_folder_handle(&to_folder).await.is_none()
        {
            return Err(MoveRefusal::TargetNotFound);
        }

        let placement = placement_of(self).await;
        let mut existing_snapshots = Vec::new();
        for folder in snapshot_folders_of(&from, is_folder, placement) {
            if self.find_folder_handle(&folder).await.is_some() {
                existing_snapshots.push(folder);
            }
        }
        let steps = plan_steps(&from, &new_path, &existing_snapshots);
        for (_, dest) in &steps {
            if self.entry_exists(dest).await {
                return Err(MoveRefusal::NameConflict {
                    name: conflict_name(&to_folder, dest),
                });
            }
        }
        let indexed = self.index.all();
        Ok(MovePlan::new(&indexed, from, new_path, is_folder, steps))
    }

    /// Копирует шаги плана байтами в цель (research.md §6). Источник не
    /// трогает: отмена и сбой откатываются удалением копии.
    async fn copy_steps(
        &mut self,
        inventory: &[StepInventory],
        control: &dyn MoveControl,
    ) -> Result<CopyReport, CopyStop> {
        let total: u32 = inventory.iter().map(|step| step.files.len() as u32).sum();
        let mut done = 0u32;
        let mut report = CopyReport::default();
        control.progress(0, total);
        for step in inventory {
            if let Some(parent) = step.dest.parent() {
                self.ensure_folder(parent)
                    .await
                    .map_err(|_| CopyStop::Failed)?;
            }
            if !step.dirs.is_empty() || !step.files.iter().any(|rel| rel.as_os_str().is_empty()) {
                self.ensure_folder(&step.dest)
                    .await
                    .map_err(|_| CopyStop::Failed)?;
            }
            for dir in &step.dirs {
                self.ensure_folder(&step.dest.join(dir))
                    .await
                    .map_err(|_| CopyStop::Failed)?;
            }
            for rel in &step.files {
                // Между файлами управление возвращается циклу событий: флаг
                // отмены ставит обработчик интерфейса.
                if control.cancelled() {
                    return Err(CopyStop::Cancelled);
                }
                let source = step_path(&step.source, rel);
                let dest = step_path(&step.dest, rel);
                let bytes = self
                    .read_bytes_any_size(&source)
                    .await
                    .map_err(|_| CopyStop::Failed)?;
                self.write_bytes(&dest, &bytes)
                    .await
                    .map_err(|_| CopyStop::Failed)?;
                // «Убедиться» до удаления источника: размер копии совпал.
                let handle = self.find_handle(&dest).await.ok_or(CopyStop::Failed)?;
                let meta = file_meta(&handle).await.ok_or(CopyStop::Failed)?;
                if meta.0 != bytes.len() as u64 {
                    return Err(CopyStop::Failed);
                }
                if is_listed(&dest) {
                    report.metas.push((dest.clone(), meta));
                }
                if crate::vault::classify(&dest) == crate::vault::FileKind::Note {
                    report
                        .texts
                        .insert(dest, String::from_utf8_lossy(&bytes).into_owned());
                }
                done += 1;
                control.progress(done, total);
            }
        }
        Ok(report)
    }

    /// Удаляет скопированное в цели (откат до удаления источника).
    async fn discard_copies(&self, roots: &[PathBuf]) {
        for root in roots {
            let Some(name) = root.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            let parent = match root.parent() {
                Some(parent) if !parent.as_os_str().is_empty() => {
                    self.find_folder_handle(parent).await
                }
                _ => Some(self.root_handle.clone()),
            };
            if let Some(parent) = parent {
                let _ = remove_entry_recursive(&parent, &name).await;
            }
        }
    }

    /// Файлы, оставшиеся от шага на прежнем месте (FR-007).
    async fn leftovers(&self, source: &Path) -> Vec<PathBuf> {
        if let Some(handle) = self.find_folder_handle(source).await {
            let (mut files, mut dirs) = (Vec::new(), Vec::new());
            let _ = walk_everything(&handle, PathBuf::new(), &mut files, &mut dirs).await;
            files.into_iter().map(|rel| source.join(rel)).collect()
        } else if self.find_handle(source).await.is_some() {
            vec![source.to_path_buf()]
        } else {
            Vec::new()
        }
    }

    /// Перечитывает индекс, таблицу ссылок и снимок опроса с диска — после
    /// частичного переноса дерево обязано показывать то, что есть (FR-007).
    /// Несохранённые правки определяются реестром, а не диском.
    async fn reindex_from_disk(&mut self) {
        let mut found = Vec::new();
        if walk_directory(&self.root_handle, PathBuf::new(), &mut found, usize::MAX)
            .await
            .is_err()
        {
            return;
        }
        let mut index = Index::new();
        let mut cache = MemoryCache::new();
        let mut snapshot = HashMap::new();
        for (path, handle) in found {
            let Ok(content) = read_file_handle_text(&handle).await else {
                continue;
            };
            let mut note = Note::new(path.clone(), stamp_from_content(&content));
            note.has_unsaved_edits = self.conflicts.has_unsaved_edits(&path);
            index.insert(note);
            record_links(&mut cache, &path, &content).await;
            if let Some(meta) = file_meta(&handle).await {
                snapshot.insert(path, meta);
            }
        }
        self.index = index;
        self.cache = cache;
        self.poll_snapshot = snapshot;
        // Перенос мог унести и конфликтные копии: до ближайшего полного обхода
        // список берётся честным обходом (011 T081).
        self.conflict_copies = None;
    }

    /// Паритет с `NativeBackend::publish_links_invalidated` (FR-030, T087).
    async fn publish_links_invalidated(&mut self, targets: &[PathBuf]) {
        let mut affected = Vec::new();
        for target in targets {
            affected.extend(self.cache.backlinks(target).await);
        }
        if let Some(event) = links_invalidated(affected) {
            self.events.push_back(event);
        }
    }

    /// Хендл существующего файла по пути относительно корня vault.
    async fn find_handle(&self, path: &Path) -> Option<JsValue> {
        self.resolve_handle(path, false, false).await
    }

    /// Хендл папки по пути относительно корня vault.
    async fn find_folder_handle(&self, path: &Path) -> Option<JsValue> {
        let mut current = self.root_handle.clone();
        for component in path.components() {
            let name = component.as_os_str().to_string_lossy().into_owned();
            current = child_handle(&current, "getDirectoryHandle", &name, false).await?;
        }
        Some(current)
    }

    /// Хендл файла по пути.
    ///
    /// Промежуточные компоненты пути запрашиваются как **папки**, последний — как
    /// файл. До этого разделения все компоненты запрашивались через
    /// `getFileHandle`, поэтому ни один вложенный путь не разрешался вовсе, а
    /// запись в новый файл была невозможна: `getFileHandle` без `{create:true}`
    /// отказывает на отсутствующем файле, в отличие от `std::fs::write` на
    /// native (**дефект фичи 001**, найденный при реализации 002 — снимок
    /// вставки всегда лежит во вложенной папке `<заметка>.assets/`).
    ///
    /// Флаги раздельные и это существенно: `create_file` даёт паритет с
    /// `std::fs::write`, который сам создаёт файл, а папки не создаются нигде,
    /// кроме явного [`VaultBackend::ensure_folder`] — «запись в несуществующую
    /// папку» обязана остаться отказом на обоих таргетах (001 FR-014).
    async fn resolve_handle(
        &self,
        path: &Path,
        create_file: bool,
        create_folders: bool,
    ) -> Option<JsValue> {
        let names: Vec<String> = path
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect();
        let (file_name, folders) = names.split_last()?;

        let mut current = self.root_handle.clone();
        for folder in folders {
            current = child_handle(&current, "getDirectoryHandle", folder, create_folders).await?;
        }
        child_handle(&current, "getFileHandle", file_name, create_file).await
    }
}

/// Вызывает `getFileHandle`/`getDirectoryHandle` на хендле папки.
///
/// `None` — записи нет (и создавать не просили) либо метод недоступен: отличать
/// эти случаи вызывающим сторонам не нужно, для них и то и другое означает «по
/// этому пути ничего не открыть».
async fn child_handle(parent: &JsValue, method: &str, name: &str, create: bool) -> Option<JsValue> {
    let handle_fn = Reflect::get(parent, &JsValue::from_str(method)).ok()?;
    let handle_fn: js_sys::Function = handle_fn.dyn_into().ok()?;
    let opts = Object::new();
    if create {
        Reflect::set(
            &opts,
            &JsValue::from_str("create"),
            &JsValue::from_bool(true),
        )
        .ok()?;
    }
    let promise = handle_fn
        .call2(parent, &JsValue::from_str(name), &opts)
        .ok()?;
    JsFuture::from(js_sys::Promise::from(promise)).await.ok()
}

impl VaultBackend for WebBackend {
    async fn open(_root_path: &Path) -> Result<Self, OpenError> {
        // Реальный выбор папки происходит на JS-стороне через
        // `window.showDirectoryPicker()` до вызова этого метода; сюда передаётся
        // уже полученный хендл через отдельный конструктор `from_handle`
        // (см. `WebBackend::from_handle`), поскольку `showDirectoryPicker` — это
        // жест пользователя, которым управляет UI, а не ядро.
        Err(OpenError::AccessDenied)
    }

    fn root_path(&self) -> &Path {
        &self.root_path
    }

    fn access_state(&self) -> AccessState {
        self.access_state
    }

    fn service_dir_existed_at_open(&self) -> bool {
        self.service_dir_existed
    }

    async fn list(&self) -> Vec<Note> {
        self.index.all()
    }

    async fn list_folder(&self, folder: &Path) -> Vec<Note> {
        self.index.in_folder(folder)
    }

    /// Обход по хендлам, как у `pollForChanges`: `getFile()` отдаёт размер и
    /// время изменения без чтения содержимого. Отказ обхода — пустой список,
    /// а не ошибка: без доступа к папке сборка всё равно назовёт причину сама.
    async fn list_conflict_copies(&self) -> Vec<PathBuf> {
        // Синхронизация спрашивает о копиях на каждом такте; обход всего OPFS
        // ради этого держал очередь вызовов секундами (011 T081). Копии берутся
        // из последнего полного обхода; разобранные с тех пор самим приложением
        // отсеиваются проверкой, а новые извне появятся с ближайшим циклом
        // опроса — как и любое внешнее изменение на web.
        if let Some(known) = &self.conflict_copies {
            let mut alive = Vec::with_capacity(known.len());
            for copy in known {
                if self.find_handle(copy).await.is_some() {
                    alive.push(copy.clone());
                }
            }
            return alive;
        }
        let mut found = Vec::new();
        if walk_conflict_copies(&self.root_handle, PathBuf::new(), &mut found)
            .await
            .is_err()
        {
            return Vec::new();
        }
        found.sort();
        found
    }

    async fn list_source_files(&self) -> super::SourceListing {
        let mut listing = super::SourceListing::default();
        if walk_source_files(&self.root_handle, PathBuf::new(), &mut listing)
            .await
            .is_err()
        {
            return super::SourceListing::default();
        }
        listing
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        listing
    }

    async fn list_fonts(&self) -> Vec<super::FontFile> {
        let mut found = Vec::new();
        if walk_directory_matching(
            &self.root_handle,
            PathBuf::new(),
            &mut found,
            usize::MAX,
            super::is_font_file,
        )
        .await
        .is_err()
        {
            return Vec::new();
        }
        let mut fonts = Vec::with_capacity(found.len());
        for (path, handle) in found {
            if let Some((size, modified)) = file_meta(&handle).await {
                fonts.push(super::FontFile {
                    path,
                    size,
                    modified_ms: modified as i64,
                });
            }
        }
        fonts.sort_by(|left, right| left.path.cmp(&right.path));
        fonts
    }

    async fn list_source_candidates(&self) -> Vec<crate::sources::SourceCandidate> {
        let mut found = Vec::new();
        if walk_directory_matching(
            &self.root_handle,
            PathBuf::new(),
            &mut found,
            usize::MAX,
            crate::sources::discover::is_source_candidate,
        )
        .await
        .is_err()
        {
            return Vec::new();
        }
        let mut candidates = Vec::with_capacity(found.len());
        for (path, handle) in found {
            if let Some((size, modified)) = file_meta(&handle).await {
                candidates.push(crate::sources::SourceCandidate {
                    path: path.to_string_lossy().replace('\\', "/"),
                    size,
                    modified_ms: modified as i64,
                });
            }
        }
        candidates.sort_by(|left, right| left.path.cmp(&right.path));
        candidates
    }

    async fn list_images(&self, folder: Option<&Path>) -> Vec<super::ImageFile> {
        let mut found = Vec::new();
        match folder {
            // Весь vault — тот же обход, что у шрифтов (паритет с native).
            None => {
                if walk_directory_matching(
                    &self.root_handle,
                    PathBuf::new(),
                    &mut found,
                    usize::MAX,
                    super::is_image_file,
                )
                .await
                .is_err()
                {
                    return Vec::new();
                }
            }
            // Одна папка без вложенных; сама папка может быть скрытой.
            Some(folder) => {
                let Some(dir) = self.find_folder_handle(folder).await else {
                    return Vec::new();
                };
                if collect_folder_files(&dir, folder, &mut found)
                    .await
                    .is_err()
                {
                    return Vec::new();
                }
            }
        }
        let mut images = Vec::with_capacity(found.len());
        for (path, handle) in found {
            if let Some((size, _)) = file_meta(&handle).await {
                images.push(super::ImageFile { path, size });
            }
        }
        images.sort_by(|left, right| left.path.cmp(&right.path));
        images
    }

    /// Папка есть тогда и только тогда, когда на неё выдаётся хендл.
    ///
    /// `Unreadable` и `NotWritable` здесь недостижимы, и это свойство File
    /// System Access API, а не упрощение: разрешение выдаётся на корень vault
    /// целиком и наследуется всем поддеревом — отдельной папке отказать в
    /// чтении браузер не может. Расхождения с `NativeBackend` тут нет: там эти
    /// варианты возможны, здесь их порождать нечему, и выдумывать их значило бы
    /// сообщать о поломке, которой не существует.
    ///
    /// Отсутствие права записи на web выражается состоянием vault целиком
    /// (`accessState`), и его складывает с ответом отсюда
    /// [`crate::templates::library::folder_state`].
    async fn folder_access(&self, folder: &Path) -> crate::vault::FolderAccess {
        match self.find_folder_handle(folder).await {
            Some(_) => crate::vault::FolderAccess::Present,
            None => crate::vault::FolderAccess::Absent,
        }
    }

    async fn read(&self, path: &Path) -> Result<String, ReadError> {
        let handle = self.find_handle(path).await.ok_or(ReadError::NotFound)?;
        // Порог проверяется по `File.size`, ДО `text()`: весь смысл предела в
        // том, чтобы не втягивать в память файл, который мы всё равно
        // отказываемся открывать (spec Assumptions, Edge Cases). Заметка при
        // этом остаётся в индексе и видна в дереве — отказ касается только
        // открытия. Паритет с `NativeBackend::read` (принцип II).
        if file_meta(&handle)
            .await
            .is_some_and(|(size, _)| size > NOTE_SIZE_LIMIT_BYTES)
        {
            return Err(ReadError::TooLarge);
        }
        read_file_handle_text(&handle)
            .await
            .map_err(|_| ReadError::Io)
    }

    async fn file_mark(&self, path: &Path) -> Option<(u64, i64)> {
        let handle = self.find_handle(path).await?;
        let (size, modified) = file_meta(&handle).await?;
        Some((size, modified as i64))
    }

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, ReadError> {
        let handle = self.find_handle(path).await.ok_or(ReadError::NotFound)?;
        // Порядок проверок тот же, что в `read` и у `NativeBackend` (принцип II).
        if file_meta(&handle)
            .await
            .is_some_and(|(size, _)| size > NOTE_SIZE_LIMIT_BYTES)
        {
            return Err(ReadError::TooLarge);
        }
        read_file_handle_bytes(&handle)
            .await
            .map_err(|_| ReadError::Io)
    }

    async fn read_bytes_any_size(&self, path: &Path) -> Result<Vec<u8>, ReadError> {
        let handle = self.find_handle(path).await.ok_or(ReadError::NotFound)?;
        read_file_handle_bytes(&handle)
            .await
            .map_err(|_| ReadError::Io)
    }

    async fn write_bytes(&mut self, path: &Path, bytes: &[u8]) -> Result<(), WriteError> {
        if self.write_denied() {
            return Err(WriteError::AccessDenied);
        }
        let handle = self
            .resolve_handle(path, true, false)
            .await
            .ok_or(WriteError::Io)?;
        let writable = js_call0(&handle, "createWritable")
            .await
            .map_err(|_| WriteError::Io)?;
        // `Uint8Array` вместо строки: `FileSystemWritableFileStream.write`
        // принимает и то, и другое, но строка прошла бы через UTF-8 и испортила
        // всё, что им не является.
        let view = js_sys::Uint8Array::from(bytes);
        let write_fn =
            Reflect::get(&writable, &JsValue::from_str("write")).map_err(|_| WriteError::Io)?;
        let write_fn: js_sys::Function = write_fn.dyn_into().map_err(|_| WriteError::Io)?;
        let promise = write_fn
            .call1(&writable, &view)
            .map_err(|_| WriteError::Io)?;
        JsFuture::from(js_sys::Promise::from(promise))
            .await
            .map_err(|_| WriteError::Io)?;
        js_call0(&writable, "close")
            .await
            .map_err(|_| WriteError::Io)?;
        // Файл источников, записанный самим приложением (017, research.md §7).
        if crate::sources::discover::is_source_candidate(path) {
            self.events.push_back(VaultEvent::SourceFilesChanged);
        }
        Ok(())
    }

    async fn write(
        &mut self,
        path: &Path,
        content: &str,
        expected: Option<FileStamp>,
    ) -> Result<(), WriteError> {
        if self.write_denied() {
            return Err(WriteError::AccessDenied);
        }
        if let Some(expected) = expected {
            if let Ok(current) = self.read(path).await {
                if stamp_from_content(&current) != expected {
                    return Err(WriteError::Conflict);
                }
            }
        }
        // Файл создаётся — паритет с `std::fs::write` на native; папка не
        // создаётся — паритет с ним же (см. `ensure_folder`).
        let handle = self
            .resolve_handle(path, true, false)
            .await
            .ok_or(WriteError::Io)?;
        let writable = js_call0(&handle, "createWritable")
            .await
            .map_err(|_| WriteError::Io)?;
        let write_fn =
            Reflect::get(&writable, &JsValue::from_str("write")).map_err(|_| WriteError::Io)?;
        let write_fn: js_sys::Function = write_fn.dyn_into().map_err(|_| WriteError::Io)?;
        let promise = write_fn
            .call1(&writable, &JsValue::from_str(content))
            .map_err(|_| WriteError::Io)?;
        JsFuture::from(js_sys::Promise::from(promise))
            .await
            .map_err(|_| WriteError::Io)?;
        let _ = js_call0(&writable, "close").await;

        // Паритет с `NativeBackend::write`: сохранение снимает несохранённое
        // состояние и закрывает конфликт по этой заметке.
        self.conflicts.clear(path);
        // Паритет с native и FR-021 фичи 002: снимок вставки — файл vault, но не
        // запись дерева, и его содержимое не разбирается как Typst.
        if crate::vault::classify(path).is_listed() {
            let note = Note::new(path.to_path_buf(), stamp_from_content(content));
            self.index.insert(note.clone());
            self.cache.upsert_note(&note).await;
            record_links(&mut self.cache, path, content).await;
        }
        Ok(())
    }

    async fn ensure_folder(&mut self, path: &Path) -> Result<(), WriteError> {
        if self.write_denied() {
            return Err(WriteError::AccessDenied);
        }
        let mut current = self.root_handle.clone();
        for component in path.components() {
            let name = component.as_os_str().to_string_lossy().into_owned();
            current = child_handle(&current, "getDirectoryHandle", &name, true)
                .await
                .ok_or(WriteError::Io)?;
        }
        Ok(())
    }

    async fn create(&mut self, path: &Path) -> Result<(), CreateError> {
        // FR-036 + принцип II: тот же гейт и в том же месте, что и в
        // `NativeBackend::create` — до валидации имени, чтобы отказ по правам
        // не зависел от того, каким именем пользователь его вызвал.
        if self.write_denied() {
            return Err(CreateError::AccessDenied);
        }
        // FR-013 + принцип II: та же валидация (включая NUL-байт) и тот же
        // порядок — имя проверяется прежде запроса хендла — что и в
        // `NativeBackend::create` (см. `backend::valid_note_name`).
        if valid_note_name(path).is_none() {
            return Err(CreateError::InvalidName);
        }
        if self.find_handle(path).await.is_some() {
            return Err(CreateError::NameConflict);
        }
        // Принцип II: `NativeBackend::create` заводит недостающие папки
        // (`create_dir_all`), значит и здесь. `write` папок не создаёт намеренно —
        // паритет с `std::fs::write`, поэтому папка заводится до него.
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            self.ensure_folder(parent).await.map_err(|e| match e {
                WriteError::Io | WriteError::Conflict => CreateError::Io,
                WriteError::AccessDenied => CreateError::AccessDenied,
            })?;
        }
        self.write(path, "", None).await.map_err(|e| match e {
            WriteError::Io | WriteError::Conflict => CreateError::Io,
            WriteError::AccessDenied => CreateError::AccessDenied,
        })?;
        // Появление заметки чинит ранее битые ссылки на неё (FR-030).
        self.publish_links_invalidated(&[path.to_path_buf()]).await;
        Ok(())
    }

    async fn rename(&mut self, from: &Path, to: &Path) -> Result<(), RenameError> {
        // FR-036 и FR-020, паритет с `NativeBackend::rename`. Гейт стоит до
        // чтения исходной заметки: rename здесь неатомарен (чтение + запись +
        // удаление), и начинать эту последовательность без прав на запись
        // означало бы удалить исходный файл после провалившейся записи.
        if self.write_denied() {
            return Err(RenameError::AccessDenied);
        }
        // File System Access API не имеет атомарного rename — реализуется как
        // чтение + запись под новым именем + удаление старого файла.
        let content = self.read(from).await.map_err(|_| RenameError::NotFound)?;
        if self.find_handle(to).await.is_some() {
            return Err(RenameError::NameConflict);
        }
        self.create(to).await.map_err(|e| match e {
            CreateError::NameConflict => RenameError::NameConflict,
            CreateError::InvalidName => RenameError::InvalidName,
            CreateError::AccessDenied => RenameError::AccessDenied,
            CreateError::Io => RenameError::Io,
        })?;
        self.write(to, &content, None)
            .await
            .map_err(|_| RenameError::Io)?;
        let _ = self.delete(from).await;
        self.index.rename(from, to);
        self.cache.rename_note(from, to).await;

        // FR-018, паритет с `NativeBackend::rename`: пометка раздела едет за
        // папкой, переименованной средствами приложения.
        //
        // На практике сюда не доходит: rename здесь реализован как чтение
        // файла + запись + удаление, и на папке он отказывает выше, на
        // `self.read(from)`. Вызов стоит всё равно — правило переноса пометки
        // обязано быть одним на оба таргета, а не появиться на web задним
        // числом вместе с переименованием папок.
        let _ = super::vault_settings::move_sections(self, from, to).await;
        // Фича 028: отметка «намеренно без ссылок» едет так же.
        let _ = super::vault_settings::move_intentional_orphans(self, from, to).await;
        // Фича 029: история снимков едет за заметкой.
        let _ = crate::snapshots::move_snapshots(self, from, to).await;
        // Фича 032: закладки едут за заметкой или папкой.
        let _ = crate::bookmarks::move_bookmarks(self, from, to).await;

        self.publish_links_invalidated(&[from.to_path_buf(), to.to_path_buf()])
            .await;
        Ok(())
    }

    async fn delete(&mut self, path: &Path) -> Result<(), DeleteError> {
        // FR-036 и FR-020, паритет с `NativeBackend::delete`.
        if self.write_denied() {
            return Err(DeleteError::AccessDenied);
        }
        let Some(parent_handle) = (match path.parent() {
            Some(p) if p != Path::new("") => self.find_folder_handle(p).await,
            _ => Some(self.root_handle.clone()),
        }) else {
            return Err(DeleteError::NotFound);
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Фича 029: снимок и отметка «удалена» — до того, как файла не станет.
        crate::snapshots::before_delete(self, path, crate::properties::now_ms()).await;
        let remove_fn = Reflect::get(&parent_handle, &JsValue::from_str("removeEntry"))
            .map_err(|_| DeleteError::Io)?;
        let remove_fn: js_sys::Function = remove_fn.dyn_into().map_err(|_| DeleteError::Io)?;
        let promise = remove_fn
            .call1(&parent_handle, &JsValue::from_str(&name))
            .map_err(|_| DeleteError::Io)?;
        JsFuture::from(js_sys::Promise::from(promise))
            .await
            .map_err(|_| DeleteError::Io)?;
        self.index.remove(path);
        self.cache.remove_note(path).await;
        self.publish_links_invalidated(&[path.to_path_buf()]).await;
        Ok(())
    }

    async fn remove_service_file(&mut self, path: &Path) -> Result<(), WriteError> {
        // Паритет с `NativeBackend::remove_service_file`.
        if self.write_denied() || !super::service_dir::is_inside_service_dir(path) {
            return Err(WriteError::AccessDenied);
        }
        let Some(parent_handle) = (match path.parent() {
            Some(p) if p != Path::new("") => self.find_folder_handle(p).await,
            _ => Some(self.root_handle.clone()),
        }) else {
            return Ok(());
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let remove_fn = Reflect::get(&parent_handle, &JsValue::from_str("removeEntry"))
            .map_err(|_| WriteError::Io)?;
        let remove_fn: js_sys::Function = remove_fn.dyn_into().map_err(|_| WriteError::Io)?;
        let promise = remove_fn
            .call1(&parent_handle, &JsValue::from_str(&name))
            .map_err(|_| WriteError::Io)?;
        // Файла уже нет (`NotFoundError`) — не ошибка, как и на native.
        let _ = JsFuture::from(js_sys::Promise::from(promise)).await;
        Ok(())
    }

    async fn move_targets(&self, from: &Path, query: &str) -> Vec<MoveTarget> {
        super::relocate::move_targets_of(self, from, query).await
    }

    async fn move_impact(&self, request: &MoveRequest) -> Result<MoveImpact, MoveRefusal> {
        let plan = self.plan_move(request).await?;
        let notes = self.index.all();
        Ok(super::relocate::impact_of(self, &notes, &plan.notes).await)
    }

    /// Перенос на web: «скопировать всё в цель → убедиться → удалить источник»
    /// (research.md §6). У File System Access API нет переноса папок.
    ///
    /// До удаления источника отмена и любой сбой удаляют скопированное:
    /// исход `Cancelled` либо `Io`, vault прежний. Сбой удаления источника
    /// оставляет полную копию в цели и остаток в источнике — `Partial`.
    async fn move_entry(
        &mut self,
        request: &MoveRequest,
        control: &dyn MoveControl,
    ) -> Result<MoveOutcome, MoveError> {
        let plan = self.plan_move(request).await?;
        if control.cancelled() {
            return Ok(MoveOutcome::Cancelled);
        }

        // Опись источника до первой записи: число файлов нужно полосе хода.
        let mut inventory = Vec::with_capacity(plan.steps.len());
        for (source, dest) in &plan.steps {
            let (mut files, mut dirs) = (Vec::new(), Vec::new());
            match self.find_folder_handle(source).await {
                Some(handle) => {
                    walk_everything(&handle, PathBuf::new(), &mut files, &mut dirs)
                        .await
                        .map_err(|_| MoveError::Io)?;
                }
                None => files.push(PathBuf::new()),
            }
            inventory.push(StepInventory {
                source: source.clone(),
                dest: dest.clone(),
                dirs,
                files,
            });
        }

        // Что создаст копирование: откат удаляет самый верхний новый предок
        // каждого шага. Существовавшее до переноса не трогается никогда.
        let mut created_roots = Vec::new();
        for (_, dest) in &plan.steps {
            let mut top = dest.clone();
            let mut current = dest.parent();
            while let Some(parent) = current {
                if parent.as_os_str().is_empty() || self.find_folder_handle(parent).await.is_some()
                {
                    break;
                }
                top = parent.to_path_buf();
                current = parent.parent();
            }
            created_roots.push(top);
        }

        let report = match self.copy_steps(&inventory, control).await {
            Ok(report) if !control.cancelled() => report,
            Ok(_) | Err(CopyStop::Cancelled) => {
                self.discard_copies(&created_roots).await;
                return Ok(MoveOutcome::Cancelled);
            }
            Err(CopyStop::Failed) => {
                self.discard_copies(&created_roots).await;
                return Err(MoveError::Io);
            }
        };

        // Дальше пути назад нет: копия полная и проверена.
        let mut left: Vec<PathBuf> = Vec::new();
        let mut failed = false;
        for step in &inventory {
            if !failed {
                let parent = match step.source.parent() {
                    Some(parent) if !parent.as_os_str().is_empty() => {
                        self.find_folder_handle(parent).await
                    }
                    _ => Some(self.root_handle.clone()),
                };
                let name = step
                    .source
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let removed = match parent {
                    Some(parent) => remove_entry_recursive(&parent, &name).await.is_ok(),
                    None => false,
                };
                if removed {
                    continue;
                }
                failed = true;
            }
            left.extend(self.leftovers(&step.source).await);
        }
        let partial = !left.is_empty();

        // Состояние приложения следует за путём (FR-020).
        self.conflicts.rename_prefix(&plan.from, &plan.new_path);
        let mut affected = Vec::new();
        if partial {
            self.reindex_from_disk().await;
        } else {
            self.index.rename_prefix(&plan.from, &plan.new_path);
            let cache_moves: Vec<CacheMove> = plan
                .notes
                .iter()
                .map(|change| CacheMove {
                    from: change.from.clone(),
                    to: change.to.clone(),
                    links: report
                        .texts
                        .get(&change.to)
                        .map(|text| crate::graph::links_in(&change.to, text)),
                    embeds: report
                        .texts
                        .get(&change.to)
                        .map(|text| crate::graph::embeds_in(&change.to, text)),
                })
                .collect();
            self.cache.relocate(&cache_moves).await;
            self.poll_snapshot
                .retain(|path, _| !path.starts_with(&plan.from));
            // Копии уже известны опросу: иначе ближайший `pollForChanges` принял
            // бы собственный перенос за внешнюю правку каждого файла (FR-008).
            self.poll_snapshot.extend(report.metas.iter().cloned());
        }
        for change in &plan.notes {
            affected.extend(self.cache.backlinks(&change.from).await);
            affected.extend(self.cache.backlinks(&change.to).await);
        }
        super::relocate::move_section_marks(self, plan.is_folder, &plan.from, &plan.new_path).await;

        // Без `wrap_bulk_if_needed`: перенос — не внешнее изменение (FR-008).
        self.events
            .extend(super::relocate::rename_events(&plan.notes));
        self.events.extend(links_invalidated(affected));

        Ok(if partial {
            MoveOutcome::Partial {
                moved: plan.notes,
                folders: plan.folders,
                left,
            }
        } else {
            plan.done()
        })
    }

    async fn backlinks(&self, path: &Path) -> Vec<Note> {
        let sources = self.cache.backlinks(path).await;
        sources
            .into_iter()
            .filter_map(|p| self.index.get(&p).cloned())
            .collect()
    }

    async fn broken_links(&self, path: &Path) -> Vec<PathBuf> {
        self.cache
            .links_from(path)
            .await
            .into_iter()
            .filter(|target| self.index.get(target).is_none())
            .collect()
    }

    async fn indexed_notes(&self) -> Vec<crate::properties::IndexedNote> {
        let properties = self.cache.all_properties().await;
        // В индексе web время изменения не хранится (отпечаток заметки
        // сравнивается по хэшу), а опрос vault помнит `lastModified` файла.
        crate::properties::index::join_notes(self.index.all(), properties, |note| {
            self.poll_snapshot
                .get(&note.path)
                .map(|(_, modified)| *modified as i64)
        })
    }

    async fn graph_source(&self) -> crate::graph::view::GraphSource {
        crate::graph::view::GraphSource {
            notes: self.indexed_notes().await,
            boards: self
                .index
                .all()
                .into_iter()
                .filter(|note| note.kind == crate::vault::FileKind::Board)
                .map(|note| crate::properties::IndexedNote::new(&note.path, &note.name, None))
                .collect(),
            links: self.cache.all_links().await,
            embeds: self.cache.all_embeds().await,
        }
    }

    async fn set_indexed_properties(
        &mut self,
        path: &Path,
        entries: Vec<crate::properties::PropertyEntry>,
    ) {
        self.cache.set_properties(path, &entries).await;
    }

    fn poll_events(&mut self) -> Vec<VaultEvent> {
        self.events.drain(..).collect()
    }

    async fn begin_edit(&mut self, path: &Path, buffer: String) {
        self.conflicts.set_buffer(path, buffer);
        if let Some(note) = self.index.get_mut(path) {
            note.has_unsaved_edits = true;
        }
    }

    async fn discard_edit(&mut self, path: &Path) {
        self.conflicts.clear(path);
        if let Some(note) = self.index.get_mut(path) {
            note.has_unsaved_edits = false;
        }
    }

    fn has_unsaved_edits(&self, path: &Path) -> bool {
        self.conflicts.has_unsaved_edits(path)
    }

    fn pending_conflict(&self, path: &Path) -> Option<EditConflict> {
        self.conflicts.conflict(path).cloned()
    }

    fn unresolved_edit_paths(&self) -> Vec<PathBuf> {
        self.conflicts.unresolved_paths()
    }

    async fn conflict_diff(&self, path: &Path) -> Vec<DiffLine> {
        let Some(local) = self.conflicts.buffer(path).map(str::to_string) else {
            return Vec::new();
        };
        let external = self.read(path).await.unwrap_or_default();
        line_diff(&local, &external)
    }

    async fn resolve_conflict_keep_local(&mut self, path: &Path) -> Result<(), ResolveError> {
        let Some(buffer) = self.conflicts.buffer(path).map(str::to_string) else {
            return Err(ResolveError::NoPendingConflict);
        };
        // Файл мог быть удалён снаружи (FR-025) — в отличие от native, где
        // `std::fs::write` сам создаёт файл, File System Access API требует
        // явного `create`, иначе `find_handle` не найдёт хендл для записи.
        if self.find_handle(path).await.is_none() {
            VaultBackend::create(self, path)
                .await
                .map_err(|_| ResolveError::Io)?;
        }
        self.write(path, &buffer, None).await?;
        self.conflicts
            .resolve(path, ConflictStatus::ResolvedKeepLocal);
        Ok(())
    }

    async fn resolve_conflict_accept_external(
        &mut self,
        path: &Path,
    ) -> Result<Option<String>, ResolveError> {
        if !self.conflicts.has_unsaved_edits(path)
            && self.conflicts.conflict(path).is_none()
            && !self.conflicts.was_deleted_with_edits(path)
        {
            return Err(ResolveError::NoPendingConflict);
        }
        let external = self.read(path).await.ok();
        self.conflicts
            .resolve(path, ConflictStatus::ResolvedAcceptExternal);
        if let Some(note) = self.index.get_mut(path) {
            note.has_unsaved_edits = false;
        }
        Ok(external)
    }
}

#[wasm_bindgen]
impl WebBackend {
    /// Конструктор из уже полученного (жестом пользователя) хендла папки —
    /// `VaultBackend::open` для web не может сам вызвать `showDirectoryPicker`,
    /// поскольку это должно происходить в UI по явному действию пользователя.
    ///
    /// Отказ типизирован [`OpenError`], а не голым `JsValue`: FR-008 и FR-038
    /// требуют назвать пользователю причину, а `String(cause)` над
    /// JS-исключением такой причиной не является.
    #[wasm_bindgen(js_name = fromHandle)]
    pub async fn from_handle(name: String, handle: JsValue) -> Result<WebBackend, OpenError> {
        let mut backend = WebBackend {
            root_path: PathBuf::from(name),
            root_handle: handle.clone(),
            access_state: AccessState::ReadWrite,
            index: Index::new(),
            cache: MemoryCache::new(),
            conflicts: ConflictRegistry::new(),
            events: VecDeque::new(),
            // FR-033: веб-клиент рисует наравне с рабочим столом.
            board_write_profile: crate::boards::BoardWriteProfile::ReadWrite,
            template_previews: crate::templates::PreviewCache::new(),
            poll_snapshot: HashMap::new(),
            poll_cycle: None,
            conflict_copies: None,
            source_snapshot: None,
            // Уточняется ниже, до создания папки.
            service_dir_existed: false,
        };
        // FR-038: обход прерывается на превышении предела, и ни индекс, ни
        // кэш не наполняются — частичное дерево не показывается как рабочий
        // vault. Обход тот же, что у опроса: он попутно собирает конфликтные
        // копии, и первому такту синхронизации не нужен свой обход (011 T081).
        // Обход исправной папки не проваливается: отказ здесь означает, что
        // доступа к ней уже нет.
        let mut walk = StepWalk::start(&handle)
            .await
            .map_err(|_| OpenError::AccessDenied)?;
        loop {
            let done = walk
                .step(f64::NEG_INFINITY)
                .await
                .map_err(|_| OpenError::AccessDenied)?;
            if walk.found.len() > SUPPORTED_NOTE_COUNT {
                return Err(OpenError::TooManyNotes);
            }
            if done {
                break;
            }
        }
        let mut copies = std::mem::take(&mut walk.copies);
        copies.sort();
        backend.conflict_copies = Some(copies);
        for (path, file_handle) in walk.found {
            if let Ok(content) = read_file_handle_text(&file_handle).await {
                let stamp = stamp_from_content(&content);
                backend.index.insert(Note::new(path.clone(), stamp));
                // Ссылки берутся из того же чтения, что и снимок (T085) —
                // второго обхода vault ради таблицы `links` не происходит.
                record_links(&mut backend.cache, &path, &content).await;
                // Начальный снимок {size, lastModified} — первый вызов
                // pollForChanges после open() не должен заново перечитывать
                // содержимое каждого файла только потому, что снимок был
                // пуст (research.md §3: дешёвый предфильтр).
                if let Some(meta) = file_meta(&file_handle).await {
                    backend.poll_snapshot.insert(path, meta);
                }
            }
        }

        backend.service_dir_existed = backend
            .find_folder_handle(&super::service_dir::service_dir())
            .await
            .is_some();

        // Служебное место заводится при открытии — так же, как на native, где
        // внутрь него сразу ложится файл кэша (фича 006, research.md §12).
        // Здесь папка остаётся пустой до первой записи настроек, и это
        // осознанно: одно и то же действие пользователя обязано давать
        // одинаковый наблюдаемый результат на обоих клиентах.
        //
        // Отказ открытие не блокирует: невозможность завести служебное место
        // проявится там, где её надо назвать пользователю, — при записи
        // настроек (FR-025).
        let _ = super::vault_settings::ensure_service_dir(&mut backend).await;

        Ok(backend)
    }

    /// Один цикл сверки с диском (T054, research.md §3, FR-016, SC-004):
    /// web-эквивалент `NativeBackend::watch()` — File System Access API не
    /// даёт подписки на изменения, поэтому WASM (однопоточный, без
    /// собственного таймера) не может сам себя периодически будить. JS-сторона
    /// вызывает сверку по своему таймеру и сама решает об интервале.
    ///
    /// Цикл целиком, без уступок: шаги `poll_step` с неограниченным бюджетом.
    /// Клиент web зовёт `pollStep`, чтобы цикл не держал очередь вызовов.
    #[wasm_bindgen(js_name = pollForChanges)]
    pub async fn poll_for_changes(&mut self) {
        while !self.poll_step(f64::INFINITY).await {}
    }

    /// Шаг цикла сверки не дольше `budget_ms` (001 T136); `true` — цикл
    /// завершён (или прерван недоступностью), следующий шаг начнёт новый.
    ///
    /// Шаги проходят обход `walk_directory_step` и дешёвый предфильтр
    /// `{size, lastModified}` (только кандидаты на изменение читаются целиком):
    /// метаданные читаются для КАЖДОГО файла в КАЖДОМ цикле
    /// (`FileSystemFileHandle.getFile()` не требует полного чтения содержимого).
    /// Сверка с индексом и события — только по завершении полного обхода:
    /// по незавершённому нельзя судить, какие заметки пропали.
    #[wasm_bindgen(js_name = pollStep)]
    pub async fn poll_step(&mut self, budget_ms: f64) -> bool {
        // `Unavailable` — терминальное состояние для этого экземпляра backend
        // (T100, FR-020, data-model.md «Vault»): выйти из него можно только
        // явным переоткрытием vault, а не тем, что папка снова начала
        // читаться. `NativeBackend` обеспечивает это, останавливая
        // `spawn_access_watch` на первом же `Unavailable`
        // (`native.rs:533`); здесь опрос вызывается снаружи по таймеру
        // JS-стороны, поэтому терминальность приходится проверять на входе.
        // Без этой проверки таргеты расходились бы в поведении, что
        // запрещено принципом II конституции (NON-NEGOTIABLE).
        if self.access_state == AccessState::Unavailable {
            self.poll_cycle = None;
            return true;
        }
        let deadline = js_sys::Date::now() + budget_ms.max(0.0);

        let mut cycle = match self.poll_cycle.take() {
            Some(cycle) => cycle,
            None => {
                // Дешёвая (без пользовательского жеста) оценка доступа —
                // фактический провал обхода ниже переопределяет её до
                // Unavailable (T056, FR-020, FR-036).
                let permission_guess = probe_permission_access_state(&self.root_handle).await;
                let Ok(walk) = StepWalk::start(&self.root_handle).await else {
                    self.poll_unavailable();
                    return true;
                };
                PollCycle {
                    permission_guess,
                    walk,
                    metas: Vec::new(),
                    start: self
                        .index
                        .all()
                        .into_iter()
                        .map(|note| (note.path, note.stamp))
                        .collect(),
                }
            }
        };

        // Предел FR-038 применён при открытии; сверка уже открытого vault не
        // может закрыть его задним числом, поэтому обход без ограничения.
        match cycle.walk.step(deadline).await {
            Err(_) => {
                self.poll_unavailable();
                return true;
            }
            Ok(false) => {
                self.poll_cycle = Some(cycle);
                return false;
            }
            Ok(true) => {}
        }

        while cycle.metas.len() < cycle.walk.found.len() {
            let meta = file_meta(&cycle.walk.found[cycle.metas.len()].1).await;
            cycle.metas.push(meta);
            if cycle.metas.len() < cycle.walk.found.len() && js_sys::Date::now() >= deadline {
                self.poll_cycle = Some(cycle);
                return false;
            }
        }

        self.finish_poll_cycle(cycle).await;
        true
    }

    /// Провал обхода: недоступность диска не трактуется как удаление всех
    /// заметок (FR-020) — индекс не трогаем, цикл бросаем.
    fn poll_unavailable(&mut self) {
        self.poll_cycle = None;
        if self.access_state != AccessState::Unavailable {
            self.access_state = AccessState::Unavailable;
            self.events.push_back(VaultEvent::VaultAccessChanged {
                access_state: AccessState::Unavailable,
            });
        }
    }

    /// Сверка полного обхода с индексом и события (бывшая вторая половина
    /// `poll_for_changes`).
    async fn finish_poll_cycle(&mut self, cycle: PollCycle) {
        let mut copies = cycle.walk.copies.clone();
        copies.sort();
        self.conflict_copies = Some(copies);

        // Снимок кандидатов в источники (017): расхождение — событие; первый
        // цикл только запоминает — библиотека и так читается при открытии.
        let mut sources = HashMap::new();
        for (path, handle) in &cycle.walk.sources {
            if let Some(meta) = file_meta(handle).await {
                sources.insert(path.clone(), meta);
            }
        }
        if self
            .source_snapshot
            .as_ref()
            .is_some_and(|known| *known != sources)
        {
            self.events.push_back(VaultEvent::SourceFilesChanged);
        }
        self.source_snapshot = Some(sources);

        let observed_access = cycle.permission_guess;
        if observed_access != self.access_state {
            self.access_state = observed_access;
            self.events.push_back(VaultEvent::VaultAccessChanged {
                access_state: observed_access,
            });
        }

        let index = &self.index;
        let touched =
            |path: &Path| index.get(path).map(|note| &note.stamp) != cycle.start.get(path);

        // Дешёвый предфильтр {path, size, lastModified} (research.md §3) —
        // полное чтение содержимого выполняется только для кандидатов на
        // изменение, не для каждого файла на каждом цикле опроса.
        let mut current_paths: HashSet<PathBuf> = HashSet::new();
        let mut changed_candidates: Vec<(PathBuf, JsValue)> = Vec::new();
        let mut fresh_metas: Vec<(PathBuf, (u64, f64))> = Vec::new();
        for ((path, handle), meta) in cycle.walk.found.iter().zip(&cycle.metas) {
            current_paths.insert(path.clone());
            let Some(meta) = *meta else {
                continue;
            };
            if touched(path) {
                continue;
            }
            if self.poll_snapshot.get(path).copied() != Some(meta) {
                changed_candidates.push((path.clone(), handle.clone()));
            }
            fresh_metas.push((path.clone(), meta));
        }

        let known_paths: HashSet<PathBuf> = self.index.all().into_iter().map(|n| n.path).collect();
        let removed_paths: Vec<PathBuf> = known_paths
            .difference(&current_paths)
            .filter(|path| !touched(path))
            .cloned()
            .collect();
        self.poll_snapshot.extend(fresh_metas);
        for path in &removed_paths {
            self.poll_snapshot.remove(path);
        }

        let mut created_or_updated: Vec<(PathBuf, FileStamp, String)> = Vec::new();
        for (path, handle) in &changed_candidates {
            if let Ok(content) = read_file_handle_text(handle).await {
                created_or_updated.push((path.clone(), stamp_from_content(&content), content));
            }
        }

        // Best-effort корреляция rename по совпадению content_hash между
        // пропавшим и новым путём: File System Access API не даёт устойчивой
        // идентичности файла между путями без отдельного `isSameEntry`-протокола
        // (research.md §1 Alternatives — платформенное ограничение,
        // задокументированный неполный паритет, тот же прецедент, что и
        // "корзина" в research.md §7). Если содержимое совпадает 1:1 между
        // ровно одним пропавшим и одним новым путём — честный `NoteRenamed`,
        // иначе (в т.ч. при неоднозначном совпадении) — обычная пара
        // Deleted+Created, не теряющая сам факт изменения.
        let mut pending = Vec::new();
        let mut consumed_removed: HashSet<PathBuf> = HashSet::new();
        let mut consumed_created: HashSet<PathBuf> = HashSet::new();
        // Пути, появление/исчезновение/переезд которых меняет статус входящих
        // ссылок (FR-030, T087) — паритет с native.
        let mut link_targets_touched: Vec<PathBuf> = Vec::new();
        for (new_path, stamp, content) in &created_or_updated {
            if known_paths.contains(new_path) {
                continue; // это Updated, не кандидат в цель переименования
            }
            let rename_source = removed_paths.iter().find(|old_path| {
                !consumed_removed.contains(*old_path)
                    && self.index.get(old_path).map(|n| n.stamp.content_hash)
                        == Some(stamp.content_hash)
            });
            if let Some(old_path) = rename_source.cloned() {
                consumed_removed.insert(old_path.clone());
                consumed_created.insert(new_path.clone());
                self.index.rename(&old_path, new_path);
                if let Some(entry) = self.index.get_mut(new_path) {
                    entry.stamp = stamp.clone();
                }
                self.cache.rename_note(&old_path, new_path).await;
                record_links(&mut self.cache, new_path, content).await;
                link_targets_touched.push(old_path.clone());
                link_targets_touched.push(new_path.clone());
                pending.push(VaultEvent::NoteRenamed {
                    from_path: old_path,
                    to_path: new_path.clone(),
                });
            }
        }

        for (path, stamp, content) in &created_or_updated {
            if consumed_created.contains(path) {
                continue;
            }
            let is_new = !known_paths.contains(path);
            // US4/T074, паритет с native: конфликт вместо тихого обновления,
            // если у заметки есть несохранённые правки и содержимое диска
            // действительно разошлось (FR-021).
            let conflicted = match self.index.get(path).map(|n| n.stamp.clone()) {
                Some(known) => self.conflicts.observe_disk_change(path, &known, stamp),
                None => false,
            };
            let mut note = Note::new(path.clone(), stamp.clone());
            // Внешний конвейер не имеет права затирать состояние открытой в
            // редакторе заметки — то же правило, что в `update_index_entry`
            // на native.
            note.has_unsaved_edits = self
                .index
                .get(path)
                .map(|known| known.has_unsaved_edits)
                .unwrap_or(false);
            self.index.insert(note.clone());
            self.cache.upsert_note(&note).await;
            record_links(&mut self.cache, path, content).await;
            if is_new {
                link_targets_touched.push(path.clone());
            }
            pending.push(if conflicted {
                VaultEvent::ConflictDetected {
                    note_path: path.clone(),
                    disk_snapshot: stamp.clone(),
                }
            } else if is_new {
                VaultEvent::NoteCreated { path: path.clone() }
            } else {
                VaultEvent::NoteUpdated { path: path.clone() }
            });
        }

        for path in &removed_paths {
            if consumed_removed.contains(path) {
                continue;
            }
            let had_edits = self.conflicts.observe_disk_deletion(path);
            self.index.remove(path);
            self.cache.remove_note(path).await;
            link_targets_touched.push(path.clone());
            pending.push(if had_edits {
                VaultEvent::NoteDeletedWithUnsavedEdits {
                    note_path: path.clone(),
                }
            } else {
                VaultEvent::NoteDeleted { path: path.clone() }
            });
        }

        if !pending.is_empty() {
            self.events.extend(wrap_bulk_if_needed(pending));
            let mut affected = Vec::new();
            for target in &link_targets_touched {
                affected.extend(self.cache.backlinks(target).await);
            }
            self.events.extend(links_invalidated(affected));
        }
    }

    /// JS-facing обёртки над методами `VaultBackend` — сам трейт не может
    /// нести `#[wasm_bindgen]` (это ограничение wasm-bindgen: атрибут
    /// применяется только к конкретным impl-блокам, не к трейтам), поэтому
    /// это единственная точка, через которую UI вызывает ядро на web
    /// (contracts/vault-backend-trait.md, contracts/frontend-events.md).

    #[wasm_bindgen(js_name = list)]
    pub async fn list_js(&self) -> Vec<Note> {
        VaultBackend::list(self).await
    }

    #[wasm_bindgen(js_name = listFolder)]
    pub async fn list_folder_js(&self, folder: &str) -> Vec<Note> {
        VaultBackend::list_folder(self, Path::new(folder)).await
    }

    /// Расширенный поиск по vault (макет «Утилиты» § Поиск).
    ///
    /// Метод бэкенда, а не отдельного объекта: поиск читает vault и без него
    /// не существует. Правила совпадения при этом целиком в ядре — интерфейс
    /// получает готовые группы и куски строк.
    #[wasm_bindgen(js_name = search)]
    pub async fn search_js(
        &self,
        query: crate::search::SearchQuery,
    ) -> Result<crate::search::SearchResults, crate::search::SearchError> {
        crate::search::search(self, &query).await
    }

    /// Предпросмотр замены по vault (фича 027): каждое совпадение с кусками
    /// «было/станет». Открытые заметки — по тексту редактора.
    #[wasm_bindgen(js_name = replacePreview)]
    pub async fn replace_preview_js(
        &self,
        query: crate::search::replace::ReplaceQuery,
        open_texts: Vec<crate::links::OpenText>,
    ) -> Result<crate::search::replace::ReplacePreview, crate::search::replace::ReplaceError> {
        crate::search::replace::replace_preview(self, &query, &open_texts).await
    }

    /// Замена отмеченных совпадений одной заметки (фича 027).
    #[wasm_bindgen(js_name = replaceInNote)]
    pub async fn replace_in_note_js(
        &mut self,
        request: crate::search::replace::ReplaceNoteRequest,
        editor_text: Option<String>,
    ) -> crate::search::replace::ReplaceNoteOutcome {
        crate::search::replace::replace_in_note(self, &request, editor_text.as_deref()).await
    }

    /// Отмена замены в одной заметке (фича 027).
    #[wasm_bindgen(js_name = replaceUndo)]
    pub async fn replace_undo_js(
        &mut self,
        record: crate::search::replace::UndoRecord,
        editor_text: Option<String>,
    ) -> crate::search::replace::UndoOutcome {
        crate::search::replace::replace_undo(self, &record, editor_text.as_deref()).await
    }

    #[wasm_bindgen(js_name = read)]
    pub async fn read_js(&self, path: &str) -> Result<String, ReadError> {
        VaultBackend::read(self, Path::new(path)).await
    }

    #[wasm_bindgen(js_name = write)]
    pub async fn write_js(
        &mut self,
        path: &str,
        content: &str,
        expected: Option<FileStamp>,
    ) -> Result<(), WriteError> {
        VaultBackend::write(self, Path::new(path), content, expected).await
    }

    #[wasm_bindgen(js_name = create)]
    pub async fn create_js(&mut self, path: &str) -> Result<(), CreateError> {
        VaultBackend::create(self, Path::new(path)).await
    }

    /// Чем является файл по своему пути (фича 002, FR-004).
    ///
    /// Дерево спрашивает ядро вместо того, чтобы сверять расширение в
    /// TypeScript: правило «`.excalidraw` — доска, файл в `*.assets` — снимок»
    /// живёт в одном месте, иначе оно появится вторым в UI и разойдётся
    /// (принцип I).
    #[wasm_bindgen(js_name = fileKind)]
    pub fn file_kind_js(path: &str) -> crate::vault::FileKind {
        crate::vault::classify(Path::new(path))
    }

    // ── Доски (фича 002) ────────────────────────────────────────────────
    //
    // Операции досок обобщены по `VaultBackend` и живут в `crate::boards`;
    // здесь только проброс через границу WASM/JS. Профиль записи веб-клиента —
    // `ReadWrite` (FR-033), а просмотр без записи — свойство мобильного
    // клиента, у которого своя оболочка. Параметром `writeBoard` профиль не
    // принимает: это отдало бы интерфейсу решение, которое FR-035 требует
    // держать в ядре. Через границу он проходит только наружу, геттером ниже.

    /// Право этой оболочки писать доски — интерфейсу, чтобы объявить
    /// ограничение пользователю (FR-034).
    ///
    /// Читается из того же поля, что и гейт в `writeBoard`: сказанное
    /// пользователю и применённое к записи — одно значение, а не два
    /// согласованных вручную.
    #[wasm_bindgen(getter, js_name = boardWriteProfile)]
    pub fn board_write_profile_js(&self) -> crate::boards::BoardWriteProfile {
        self.board_write_profile
    }

    #[wasm_bindgen(js_name = createBoard)]
    pub async fn create_board_js(
        &mut self,
        path: &str,
    ) -> Result<crate::boards::Board, crate::boards::CreateBoardError> {
        crate::boards::create_board(self, Path::new(path)).await
    }

    /// Содержимое доски пересекает границу строкой: ядро его не разбирает
    /// дальше нормализации для хэша, а владеет им движок (research.md §9).
    #[wasm_bindgen(js_name = readBoard)]
    pub async fn read_board_js(&self, path: &str) -> Result<String, crate::boards::ReadBoardError> {
        let content = crate::boards::read_board(self, Path::new(path)).await?;
        String::from_utf8(content.as_bytes().to_vec())
            .map_err(|_| crate::boards::ReadBoardError::Unreadable)
    }

    #[wasm_bindgen(js_name = writeBoard)]
    pub async fn write_board_js(
        &mut self,
        path: &str,
        content: &str,
        expected: Option<FileStamp>,
    ) -> Result<(), crate::boards::WriteBoardError> {
        let profile = self.board_write_profile;
        crate::boards::write_board(
            self,
            profile,
            Path::new(path),
            &crate::boards::BoardContent::new(content.as_bytes().to_vec()),
            expected,
        )
        .await
    }

    /// Хэш нормализованного содержимого (research.md §5). Считает ядро: правило
    /// «что является содержимым» не должно появиться второй раз в TypeScript.
    #[wasm_bindgen(js_name = boardDigest)]
    pub fn board_digest_js(
        content: &str,
    ) -> Result<crate::boards::BoardDigest, crate::boards::DigestError> {
        crate::boards::digest(content.as_bytes())
    }

    // ── Шаблоны и сниппеты (фича 003) ───────────────────────────────────
    //
    // Операции библиотеки обобщены по `VaultBackend` и живут в
    // `crate::templates`; здесь только проброс через границу WASM/JS.
    //
    // Кэш превью — поле бэкенда, а не отдельного объекта: он привязан к
    // открытому vault и умирает вместе с ним. Производное восстановимое
    // состояние (research.md §4), поэтому переживать перезапуск не обязано.

    /// Состав библиотеки заготовок и состояние её папки (FR-004, FR-007).
    #[wasm_bindgen(js_name = templateLibrary)]
    pub async fn template_library_js(&self) -> crate::templates::TemplateLibraryView {
        crate::templates::library(self).await
    }

    /// Содержимое заготовки для применения (FR-017, FR-023).
    #[wasm_bindgen(js_name = readTemplate)]
    pub async fn read_template_js(
        &self,
        path: &str,
    ) -> Result<String, crate::templates::ReadTemplateError> {
        crate::templates::read_template(self, Path::new(path)).await
    }

    /// Превью одной заготовки (FR-011, FR-012, FR-014).
    ///
    /// `&mut self` из-за кэша: сам порт превью ничего не меняет, но результат
    /// запоминается, чтобы прокрутка галереи не пересчитывала одно и то же.
    #[wasm_bindgen(js_name = templatePreview)]
    pub async fn template_preview_js(
        &mut self,
        template: crate::templates::Template,
    ) -> crate::templates::PreviewState {
        // Кэш вынимается из поля на время вызова: `preview_of` берёт `&self`
        // ради чтения файла и `&mut PreviewCache` ради записи результата, а
        // держать оба заимствования одного объекта нельзя.
        let mut cache = std::mem::take(&mut self.template_previews);
        let state = crate::templates::preview_of(
            self,
            &crate::templates::ExcerptOnlySource,
            &mut cache,
            &template,
        )
        .await;
        self.template_previews = cache;
        state
    }

    /// Проверка имени будущей заготовки — до любых изменений на диске (FR-029).
    #[wasm_bindgen(js_name = checkTemplateName)]
    pub async fn check_template_name_js(&self, name: &str) -> crate::templates::NameCheck {
        crate::templates::check_name(self, name).await
    }

    /// Сохраняет содержимое как заготовку (FR-002, FR-028, FR-030).
    #[wasm_bindgen(js_name = saveAsTemplate)]
    pub async fn save_as_template_js(
        &mut self,
        name: &str,
        content: &str,
        confirm_overwrite: bool,
    ) -> Result<crate::templates::Template, crate::templates::SaveTemplateError> {
        crate::templates::save_as_template(self, name, content, confirm_overwrite).await
    }

    /// Какую правку совершить над текстом, чтобы вставить заготовку
    /// (FR-031…FR-035).
    ///
    /// Статический метод: правило вставки текста от vault не зависит вовсе.
    #[wasm_bindgen(js_name = planInsertion)]
    pub fn plan_insertion_js(
        text_length: usize,
        point: crate::templates::InsertionPoint,
        content: &str,
    ) -> crate::templates::InsertionEdit {
        crate::templates::plan_insertion(text_length, &point, content)
    }

    // ── Параметры элементов (фича 008, contracts/element-params.md) ────────
    //
    // Статические: функции чистые, vault не нужен — как `planInsertion`.

    /// Схема элемента по ключу (`call:figure`) — для правки найденного
    /// элемента; `undefined` — параметров нет.
    #[wasm_bindgen(js_name = elementSchema)]
    pub fn element_schema_js(id: &str) -> Option<crate::elements::ElementSpec> {
        crate::elements::element_schema(id)
    }

    /// Схема элемента, который вставляет шаблон каталога (текст без полей
    /// сниппета); `undefined` — вставляется сразу (FR-003).
    #[wasm_bindgen(js_name = elementSchemaFor)]
    pub fn element_schema_for_js(template: &str) -> Option<crate::elements::ElementSpec> {
        crate::elements::element_schema_for(template)
    }

    /// Элемент, непосредственно содержащий каретку (FR-006, FR-016).
    #[wasm_bindgen(js_name = elementAt)]
    pub fn element_at_js(text: &str, offset: usize) -> crate::elements::ElementAtResult {
        crate::elements::element_at(text, offset)
    }

    /// «Будет вставлено» (FR-004, FR-005).
    #[wasm_bindgen(js_name = renderElement)]
    pub fn render_element_js(
        request: crate::elements::RenderRequest,
    ) -> crate::elements::RenderOutcome {
        crate::elements::render(&request)
    }

    /// Точечные замены для правки на месте (FR-008, FR-009).
    #[wasm_bindgen(js_name = planElementEdit)]
    pub fn plan_element_edit_js(
        request: crate::elements::EditRequest,
    ) -> crate::elements::EditOutcome {
        crate::elements::plan(&request)
    }

    /// Затрагивает ли событие файла библиотеку заготовок (FR-003).
    ///
    /// Спрашивается у ядра, а не выводится в интерфейсе сверкой пути с
    /// «templates/»: правило «что считается заготовкой» обязано жить в одном
    /// месте (принцип I).
    #[wasm_bindgen(js_name = templateEvent)]
    pub fn template_event_js(event: VaultEvent) -> Option<crate::templates::TemplateEvent> {
        crate::templates::library_event(&event)
    }

    // ── Настройки уровня vault и разделы (фича 006) ─────────────────────
    //
    // Та же дыра, что закрывал T070 фичи 002: логика в
    // `crate::vault::vault_settings` есть, но без этих вызовов UI физически не
    // может ни прочитать политику расположения, ни пометить папку разделом.

    /// Настройки уровня vault (FR-002, FR-015, FR-026).
    #[wasm_bindgen(js_name = vaultSettings)]
    pub async fn vault_settings_js(
        &self,
    ) -> Result<
        crate::vault::vault_settings::VaultOverrides,
        crate::vault::vault_settings::VaultSettingsError,
    > {
        crate::vault::vault_settings::read_vault_settings(self).await
    }

    /// Записывает настройки уровня vault (FR-029).
    #[wasm_bindgen(js_name = setVaultSettings)]
    pub async fn set_vault_settings_js(
        &mut self,
        overrides: crate::vault::vault_settings::VaultOverrides,
    ) -> Result<(), crate::vault::vault_settings::VaultSettingsError> {
        crate::vault::vault_settings::write_vault_settings(self, &overrides).await
    }

    /// Помечает папку как корень раздела (FR-013).
    ///
    /// Возвращает обновлённый набор настроек, а не `()`: UI обязан показать
    /// новое состояние дерева, и второй запрос за тем, что ядро уже знает,
    /// был бы лишним кругом через границу.
    #[wasm_bindgen(js_name = markSection)]
    pub async fn mark_section_js(
        &mut self,
        folder: &str,
    ) -> Result<
        crate::vault::vault_settings::VaultOverrides,
        crate::vault::vault_settings::VaultSettingsError,
    > {
        crate::vault::vault_settings::set_section(self, Path::new(folder), true).await
    }

    /// Снимает пометку раздела (FR-013). Уже созданные файлы вставок при этом
    /// не двигаются (FR-017).
    #[wasm_bindgen(js_name = unmarkSection)]
    pub async fn unmark_section_js(
        &mut self,
        folder: &str,
    ) -> Result<
        crate::vault::vault_settings::VaultOverrides,
        crate::vault::vault_settings::VaultSettingsError,
    > {
        crate::vault::vault_settings::set_section(self, Path::new(folder), false).await
    }

    // ── Сочетания клавиш ────────────────────────────────────────────────
    //
    // Та же дыра, о которой сказано выше: правила в
    // `crate::settings::keybindings` есть, но без этих трёх вызовов ни панель
    // настроек, ни обработчик нажатий до них не добираются.

    /// Действующие сочетания всех команд.
    ///
    /// Чистая функция от уже прочитанных настроек, а не ещё одно чтение файла:
    /// UI держит `VaultOverrides` у себя, и второй поход на диск дал бы второй
    /// источник истины о том же.
    ///
    /// `mac` приходит снаружи: платформу ядро не знает, а подпись сочетания от
    /// неё зависит.
    #[wasm_bindgen(js_name = keybindings)]
    pub fn keybindings_js(
        overrides: crate::vault::vault_settings::VaultOverrides,
        mac: bool,
    ) -> Vec<crate::settings::keybindings::ResolvedBinding> {
        crate::settings::keybindings::resolve(&overrides.keybindings, mac)
    }

    /// Назначает команде сочетание либо снимает его (`shortcut: undefined`).
    #[wasm_bindgen(js_name = setKeybinding)]
    pub async fn set_keybinding_js(
        &mut self,
        command: crate::settings::keybindings::CommandId,
        shortcut: Option<crate::settings::keybindings::Shortcut>,
    ) -> Result<
        crate::vault::vault_settings::VaultOverrides,
        crate::vault::vault_settings::SetKeybindingError,
    > {
        crate::vault::vault_settings::set_keybinding(self, command, shortcut).await
    }

    /// Возвращает команду к умолчанию ядра.
    #[wasm_bindgen(js_name = resetKeybinding)]
    pub async fn reset_keybinding_js(
        &mut self,
        command: crate::settings::keybindings::CommandId,
    ) -> Result<
        crate::vault::vault_settings::VaultOverrides,
        crate::vault::vault_settings::VaultSettingsError,
    > {
        crate::vault::vault_settings::reset_keybinding(self, command).await
    }

    // ── Вставки досок в заметки (фича 002, US2 и US3) ───────────────────
    //
    // Проброс тем же правилом, что и у досок выше: решения — в ядре, здесь
    // только граница. Без этих четырёх вызовов UI физически не может вставить
    // доску, хотя логика в `crate::boards` есть, — ровно та дыра, которую
    // закрыл T070.

    /// Путь и имя файла для новой вставки (FR-018, FR-020). Файл не создаётся.
    ///
    /// Настройки vault приходят целиком, а не разобранными на политику и
    /// список разделов: UI их уже прочитал через `vaultSettings()`, а решение
    /// «что из них влияет на путь» принадлежит ядру (фича 006, принцип I).
    #[wasm_bindgen(js_name = planEmbed)]
    pub fn plan_embed_js(
        note_path: &str,
        note_text: &str,
        board_path: &str,
        overrides: crate::vault::vault_settings::VaultOverrides,
    ) -> Result<crate::boards::EmbedPlan, crate::boards::EmbedError> {
        crate::boards::plan_embed(
            Path::new(note_path),
            note_text,
            Path::new(board_path),
            overrides.placement(),
            &overrides.sections,
        )
    }

    /// Текст вызова `#image(...)` для вставки в заметку (FR-022).
    #[wasm_bindgen(js_name = embedMarkup)]
    pub fn embed_markup_js(plan: crate::boards::EmbedPlan) -> String {
        crate::boards::embed_markup(&plan)
    }

    /// Записывает снимок вставки (FR-019, FR-028).
    ///
    /// Хэш считается здесь по содержимому доски, а не приходит из UI: снимок и
    /// его метаданные обязаны описывать одно и то же состояние, и вычисление
    /// хэша на стороне JS было бы вторым правилом «что является содержимым».
    #[wasm_bindgen(js_name = writeSnapshot)]
    pub async fn write_snapshot_js(
        &mut self,
        snapshot_path: &str,
        svg: &str,
        board_path: &str,
        board_content: &str,
    ) -> Result<(), crate::boards::SnapshotWriteError> {
        let digest = crate::boards::digest(board_content.as_bytes())
            .map_err(|_| crate::boards::SnapshotWriteError::NotAnSvg)?;
        let metadata = crate::boards::snapshot_metadata(Path::new(board_path), &digest);
        crate::boards::write_snapshot(self, Path::new(snapshot_path), svg, &metadata).await
    }

    /// Вставки открытой заметки вместе с состоянием каждой (FR-026, FR-027).
    /// Ничего не пишет: снимок обновляется только явным действием (FR-030).
    #[wasm_bindgen(js_name = embedsOf)]
    pub async fn embeds_of_js(
        &self,
        note_path: &str,
        note_text: &str,
    ) -> Vec<crate::boards::Embed> {
        crate::boards::embeds_with_state(self, Path::new(note_path), note_text).await
    }

    /// Вставки в тексте заметки **без** чтения файлов и без состояния.
    ///
    /// Дешёвый вызов на каждый ввод символа: по нему UI решает, изменился ли
    /// состав вставок и нужно ли перечитывать снимки. Правило «что считается
    /// вставкой» при этом остаётся одно и то же — здешнее (research.md §11).
    #[wasm_bindgen(js_name = embedsInText)]
    pub fn embeds_in_text_js(note_path: &str, note_text: &str) -> Vec<crate::boards::EmbedRef> {
        crate::boards::embeds_of(Path::new(note_path), note_text)
    }

    /// Могло ли изменение файла затронуть вставки заметки (T056).
    ///
    /// `undefined` — пересчитывать нечего. Решение принимает ядро: правило
    /// «изменение доски устаревает вставку» не должно появиться второй раз в
    /// TypeScript.
    #[wasm_bindgen(js_name = embedsChanged)]
    pub fn embeds_changed_js(
        note_path: &str,
        note_text: &str,
        changed: &str,
    ) -> Option<crate::boards::BoardEvent> {
        crate::boards::embeds_changed(Path::new(note_path), note_text, Path::new(changed))
    }

    #[wasm_bindgen(js_name = rename)]
    pub async fn rename_js(&mut self, from: &str, to: &str) -> Result<(), RenameError> {
        VaultBackend::rename(self, Path::new(from), Path::new(to)).await
    }

    #[wasm_bindgen(js_name = delete)]
    pub async fn delete_js(&mut self, path: &str) -> Result<(), DeleteError> {
        VaultBackend::delete(self, Path::new(path)).await
    }

    #[wasm_bindgen(js_name = moveTargets)]
    pub async fn move_targets_js(&self, from: &str, query: &str) -> Vec<MoveTarget> {
        VaultBackend::move_targets(self, Path::new(from), query).await
    }

    /// Отказ — reject с [`MoveRefusal`].
    #[wasm_bindgen(js_name = moveImpact)]
    pub async fn move_impact_js(&self, request: MoveRequest) -> Result<MoveImpact, MoveRefusal> {
        VaultBackend::move_impact(self, &request).await
    }

    /// `on_progress(done, total)` вызывается после каждого файла; `cancel` —
    /// объект-флаг `{ cancelled: boolean }`, который интерфейс держит у себя и
    /// ставит в `true` для отмены (ядро читает его через `Reflect` между
    /// файлами). Оба необязательны. Ошибка — reject с [`MoveError`].
    #[wasm_bindgen(js_name = moveEntry)]
    pub async fn move_entry_js(
        &mut self,
        request: MoveRequest,
        on_progress: Option<js_sys::Function>,
        cancel: JsValue,
    ) -> Result<MoveOutcome, MoveError> {
        let control = JsMoveControl {
            on_progress,
            cancel,
        };
        VaultBackend::move_entry(self, &request, &control).await
    }

    #[wasm_bindgen(js_name = backlinks)]
    pub async fn backlinks_js(&self, path: &str) -> Vec<Note> {
        VaultBackend::backlinks(self, Path::new(path)).await
    }

    /// FR-030: цели ссылок этой заметки, которых нет в vault.
    #[wasm_bindgen(js_name = brokenLinks)]
    pub async fn broken_links_js(&self, path: &str) -> Vec<String> {
        VaultBackend::broken_links(self, Path::new(path))
            .await
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    }

    #[wasm_bindgen(js_name = pollEvents)]
    pub fn poll_events_js(&mut self) -> Vec<VaultEvent> {
        VaultBackend::poll_events(self)
    }

    #[wasm_bindgen(js_name = beginEdit)]
    pub async fn begin_edit_js(&mut self, path: &str, buffer: String) {
        VaultBackend::begin_edit(self, Path::new(path), buffer).await
    }

    #[wasm_bindgen(js_name = discardEdit)]
    pub async fn discard_edit_js(&mut self, path: &str) {
        VaultBackend::discard_edit(self, Path::new(path)).await
    }

    #[wasm_bindgen(js_name = hasUnsavedEdits)]
    pub fn has_unsaved_edits_js(&self, path: &str) -> bool {
        VaultBackend::has_unsaved_edits(self, Path::new(path))
    }

    #[wasm_bindgen(js_name = pendingConflict)]
    pub fn pending_conflict_js(&self, path: &str) -> Option<EditConflict> {
        VaultBackend::pending_conflict(self, Path::new(path))
    }

    /// FR-037: то, что будет потеряно при закрытии окна прямо сейчас. Пустой
    /// массив — закрывать безопасно.
    #[wasm_bindgen(js_name = unresolvedEditPaths)]
    pub fn unresolved_edit_paths_js(&self) -> Vec<String> {
        VaultBackend::unresolved_edit_paths(self)
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    }

    #[wasm_bindgen(js_name = conflictDiff)]
    pub async fn conflict_diff_js(&self, path: &str) -> Vec<DiffLine> {
        VaultBackend::conflict_diff(self, Path::new(path)).await
    }

    #[wasm_bindgen(js_name = resolveConflictKeepLocal)]
    pub async fn resolve_conflict_keep_local_js(&mut self, path: &str) -> Result<(), ResolveError> {
        VaultBackend::resolve_conflict_keep_local(self, Path::new(path)).await
    }

    #[wasm_bindgen(js_name = resolveConflictAcceptExternal)]
    pub async fn resolve_conflict_accept_external_js(
        &mut self,
        path: &str,
    ) -> Result<Option<String>, ResolveError> {
        VaultBackend::resolve_conflict_accept_external(self, Path::new(path)).await
    }

    #[wasm_bindgen(js_name = accessState)]
    pub fn access_state_js(&self) -> AccessState {
        VaultBackend::access_state(self)
    }

    /// Что сказать о служебном месте сразу после открытия (006 FR-024,
    /// FR-025): настройки уровня vault утрачены либо vault открыт только на
    /// чтение. `was_known` — открывали ли эту папку на этом устройстве раньше;
    /// знает это хранилище устройства, а не backend.
    #[wasm_bindgen(js_name = serviceDirStatus)]
    pub fn service_dir_status_js(
        &self,
        was_known: bool,
    ) -> Option<crate::vault::events::VaultEvent> {
        crate::vault::vault_settings::service_dir_status(self, was_known)
    }

    #[wasm_bindgen(js_name = rootPath)]
    pub fn root_path_js(&self) -> String {
        self.root_path.to_string_lossy().into_owned()
    }
}
