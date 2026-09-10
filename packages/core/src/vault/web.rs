use crate::vault::backend::{
    valid_note_name, CreateError, DeleteError, OpenError, ReadError, RenameError, ResolveError,
    VaultBackend, WriteError,
};
use crate::vault::cache::{record_links, IndexCache, MemoryCache};
use crate::vault::conflict::{line_diff, ConflictRegistry, ConflictStatus, DiffLine, EditConflict};
use crate::vault::events::{links_invalidated, wrap_bulk_if_needed, VaultEvent};
use crate::vault::index::{FileStamp, Index, Note};
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
            Box::pin(walk_directory(&handle, child_path, out, limit)).await?;
        } else if kind == "file" && is_listed(&child_path) {
            out.push((child_path, handle));
            if out.len() > limit {
                return Err(WalkAbort::TooManyNotes);
            }
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
    /// Существовало ли `.typsidian/` до открытия (фича 006, FR-024).
    ///
    /// Снимок момента открытия, а не текущее состояние: открытие само эту
    /// папку создаёт, поэтому спросить позже — значит всегда получить `true`.
    service_dir_existed: bool,
}

// `JsValue` не Send/Sync, но wasm32 однопоточен — обёртка безопасна в этом контексте.
unsafe impl Send for WebBackend {}

impl WebBackend {
    /// Единственный гейт записи для всех четырёх операций, названных в FR-036
    /// (сохранение, создание, переименование, удаление) — паритет с
    /// `NativeBackend::write_denied` (принцип II).
    fn write_denied(&self) -> bool {
        self.access_state != AccessState::ReadWrite
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
async fn child_handle(
    parent: &JsValue,
    method: &str,
    name: &str,
    create: bool,
) -> Option<JsValue> {
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
            // Уточняется ниже, до создания папки.
            service_dir_existed: false,
        };
        let mut found = Vec::new();
        // FR-038: обход прерывается на превышении предела, и ни индекс, ни
        // кэш не наполняются — частичное дерево не показывается как рабочий
        // vault.
        walk_directory(&handle, PathBuf::new(), &mut found, SUPPORTED_NOTE_COUNT)
            .await
            .map_err(|abort| match abort {
                WalkAbort::TooManyNotes => OpenError::TooManyNotes,
                // Обход исправной папки не проваливается: отказ здесь означает,
                // что доступа к ней уже нет.
                WalkAbort::Js => OpenError::AccessDenied,
            })?;
        for (path, file_handle) in found {
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
    /// обязана вызывать этот метод по своему таймеру (см. отчёт агента —
    /// `useVault.ts`, уже существующий `setInterval` на 1000мс, должен
    /// переключиться с "просто читать `pollEvents()`" на "сначала
    /// `await pollForChanges()`, потом `pollEvents()`") — адаптивное
    /// увеличение интервала на больших vault (research.md §3) целиком на
    /// стороне JS, эта функция не знает и не хранит собственный интервал.
    ///
    /// Один проход обхода `walk_directory`+дешёвого предфильтра
    /// `{size, lastModified}` (только кандидаты на изменение читаются
    /// целиком) — метаданные читаются для КАЖДОГО файла на КАЖДЫЙ вызов
    /// (`FileSystemFileHandle.getFile()` не требует полного чтения
    /// содержимого), поэтому именно частота вызовов этого метода
    /// определяет фактическую задержку обнаружения (SC-004).
    #[wasm_bindgen(js_name = pollForChanges)]
    pub async fn poll_for_changes(&mut self) {
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
            return;
        }

        // Дешёвая (без пользовательского жеста) оценка доступа —
        // фактический провал обхода ниже переопределяет её до Unavailable,
        // если обход реально проваливается (T056, FR-020, FR-036).
        let permission_guess = probe_permission_access_state(&self.root_handle).await;

        let mut found = Vec::new();
        // Предел FR-038 применён при открытии; сверка уже открытого vault не
        // может закрыть его задним числом, поэтому обход без ограничения.
        let walk_result =
            walk_directory(&self.root_handle, PathBuf::new(), &mut found, usize::MAX).await;

        let observed_access = if walk_result.is_err() {
            AccessState::Unavailable
        } else {
            permission_guess
        };
        if observed_access != self.access_state {
            self.access_state = observed_access;
            self.events.push_back(VaultEvent::VaultAccessChanged {
                access_state: observed_access,
            });
        }
        if walk_result.is_err() {
            // Недоступность диска — не трактуем как удаление всех заметок
            // (FR-020): индекс не трогаем, просто останавливаем этот цикл.
            return;
        }

        // Дешёвый предфильтр {path, size, lastModified} (research.md §3) —
        // полное чтение содержимого выполняется только для кандидатов на
        // изменение, не для каждого файла на каждом цикле опроса.
        let mut current_paths: HashSet<PathBuf> = HashSet::new();
        let mut changed_candidates: Vec<(PathBuf, JsValue)> = Vec::new();
        for (path, handle) in &found {
            current_paths.insert(path.clone());
            let Some(meta) = file_meta(handle).await else {
                continue;
            };
            let previous = self.poll_snapshot.get(path).copied();
            if previous != Some(meta) {
                changed_candidates.push((path.clone(), handle.clone()));
            }
            self.poll_snapshot.insert(path.clone(), meta);
        }

        let known_paths: HashSet<PathBuf> = self.index.all().into_iter().map(|n| n.path).collect();
        let removed_paths: Vec<PathBuf> = known_paths.difference(&current_paths).cloned().collect();
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
    pub async fn read_board_js(
        &self,
        path: &str,
    ) -> Result<String, crate::boards::ReadBoardError> {
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

    #[wasm_bindgen(js_name = rootPath)]
    pub fn root_path_js(&self) -> String {
        self.root_path.to_string_lossy().into_owned()
    }
}
