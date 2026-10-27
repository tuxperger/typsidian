//! Общий для тестов бэкенд в памяти.
//!
//! Не заменяет `NativeBackend`/`WebBackend` в их собственных проверках — нужен
//! там, где тестируется логика **поверх** трейта и настоящий доступ к диску
//! только мешает: фаза резолва предпросмотра одинакова на обоих таргетах, а
//! файловой системы на wasm32 нет вовсе (принцип II).

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use typsidian_core::vault::backend::{
    CreateError, DeleteError, OpenError, ReadError, RenameError, ResolveError, VaultBackend,
    WriteError,
};
use typsidian_core::vault::conflict::{DiffLine, EditConflict};
use typsidian_core::vault::events::VaultEvent;
use typsidian_core::vault::index::Note;
use typsidian_core::vault::{AccessState, FolderAccess, FolderProblem};

/// Vault, который целиком лежит в памяти.
///
/// Читает то, что положили, и ничего больше: любой путь, которого нет в карте,
/// отвечает `NotFound` — ровно так же, как настоящий бэкенд.
pub struct MemoryVault {
    root: PathBuf,
    /// Общая с двойником движка git карта файлов (фича 011): движок читает
    /// «рабочие файлы» из той же памяти, в которую пишет сессия.
    files: std::rc::Rc<std::cell::RefCell<HashMap<PathBuf, Vec<u8>>>>,
    /// Пути, на которых чтение обязано отказать с этой причиной. Нужны, чтобы
    /// проверить, что причина доходит до пользователя названной (FR-021).
    failures: HashMap<PathBuf, ReadError>,
    /// Сколько раз спросили каждый файл — по этому счётчику видно, что цикл
    /// резолва не перечитывает одно и то же.
    pub reads: std::cell::RefCell<HashMap<PathBuf, u32>>,
    /// Доступность vault — чтобы проверять отказы FR-025 без настоящих прав
    /// файловой системы, которых на wasm32 нет.
    access_state: AccessState,
    /// Существовало ли служебное место при открытии (фича 006, FR-024).
    service_dir_existed: bool,
    /// Папки, которые обязаны отвечать «не читается». Объявляются тестом:
    /// прав файловой системы на wasm32 нет, а различать `Absent` и
    /// `Unreadable` галерея заготовок обязана (003 FR-009, FR-016).
    folder_problems: HashMap<PathBuf, FolderProblem>,
    /// Проверять ли `expected` в `write` (фича 013): расхождение с отпечатком
    /// файла в памяти — `WriteError::Conflict`, как у настоящих backend. По
    /// умолчанию выключено, чтобы прежние таблицы случаев вели себя как раньше.
    check_expected: bool,
    /// Пути, по которым тест объявил несохранённые правки (`begin_edit` двойник
    /// не помнит; нужен только ответ `has_unsaved_edits`).
    unsaved: std::collections::HashSet<PathBuf>,
    /// Сколько раз файл успешно записан через `write` — по счётчику видно, что
    /// отказ не тронул vault.
    pub writes: std::cell::RefCell<HashMap<PathBuf, u32>>,
    /// Отпечатки, которые индекс «запомнил» раньше, чем файл изменили снаружи
    /// (фича 013): `list`/`list_folder` отдают их вместо настоящих.
    lagging: HashMap<PathBuf, typsidian_core::stamp::FileStamp>,
    /// Первая запись по пути с `expected` отказывает конфликтом, а файл тем
    /// временем становится этими байтами (фича 022): так проверяется повтор на
    /// свежем тексте после правки «с другого устройства».
    conflict_once: HashMap<PathBuf, Vec<u8>>,
    /// Пути, запись байтов по которым отказывает `Io` (фича 016): так
    /// проверяется, что отказ не оставляет файла и текст не меняется (FR-014).
    write_failures: std::collections::HashSet<PathBuf>,
}

impl MemoryVault {
    pub fn new() -> Self {
        Self {
            root: PathBuf::from("/vault"),
            files: Default::default(),
            failures: HashMap::new(),
            reads: std::cell::RefCell::new(HashMap::new()),
            access_state: AccessState::ReadWrite,
            service_dir_existed: true,
            folder_problems: HashMap::new(),
            check_expected: false,
            unsaved: Default::default(),
            writes: std::cell::RefCell::new(HashMap::new()),
            lagging: HashMap::new(),
            conflict_once: HashMap::new(),
            write_failures: Default::default(),
        }
    }

    pub fn with_file(self, path: &str, bytes: impl Into<Vec<u8>>) -> Self {
        self.files
            .borrow_mut()
            .insert(PathBuf::from(path), bytes.into());
        self
    }

    /// Убирает файл — «кто-то удалил его мимо приложения».
    ///
    /// Нужен проверкам, где истина на диске важнее записанного нами: перечень
    /// установленных шаблонов обязан терять запись, если файла больше нет
    /// (фича 007, US5).
    pub fn remove(&mut self, path: &Path) {
        self.files.borrow_mut().remove(path);
    }

    /// Содержимое файла, если он есть — для проверок того, что записано.
    pub fn bytes_of(&self, path: &str) -> Option<Vec<u8>> {
        self.files.borrow().get(Path::new(path)).cloned()
    }

    /// Все файлы под папкой (`""` — весь vault), по порядку, через `/`.
    pub fn paths_under(&self, folder: &str) -> Vec<String> {
        let mut paths: Vec<String> = self
            .files
            .borrow()
            .keys()
            .filter(|path| path.starts_with(folder))
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect();
        paths.sort();
        paths
    }

    /// Папка, которая существует, но не читается.
    pub fn with_folder_problem(mut self, folder: &str, reason: FolderProblem) -> Self {
        self.folder_problems.insert(PathBuf::from(folder), reason);
        self
    }

    /// Отпечаток файла в том виде, в каком его отдаёт индекс, — для проверок
    /// «файл после операции не тронут» (003 SC-008, инвариант 1).
    pub fn stamp_of(&self, path: &str) -> Option<typsidian_core::stamp::FileStamp> {
        self.files
            .borrow()
            .get(&PathBuf::from(path))
            .map(|bytes| typsidian_core::stamp::FileStamp::from_content(0, bytes))
    }

    pub fn with_failure(mut self, path: &str, error: ReadError) -> Self {
        self.failures.insert(PathBuf::from(path), error);
        self
    }

    pub fn read_only(mut self) -> Self {
        self.access_state = AccessState::ReadOnly;
        self
    }

    /// Vault, папка которого стала недоступна (отмонтирована, отозван доступ).
    /// Запись байтов по этому пути отказывает `Io` (фича 016, FR-014).
    pub fn with_write_failure(mut self, path: &str) -> Self {
        self.write_failures.insert(PathBuf::from(path));
        self
    }

    pub fn unavailable(mut self) -> Self {
        self.access_state = AccessState::Unavailable;
        self
    }

    /// Vault, у которого служебного места при открытии не было.
    pub fn without_service_dir(mut self) -> Self {
        self.service_dir_existed = false;
        self
    }

    /// Карта файлов для двойника движка git (фича 011).
    pub fn files_handle(&self) -> std::rc::Rc<std::cell::RefCell<HashMap<PathBuf, Vec<u8>>>> {
        self.files.clone()
    }

    /// Включает проверку `expected` в `write` (фича 013).
    pub fn checking_expected(mut self) -> Self {
        self.check_expected = true;
        self
    }

    /// Индекс запоминает текущий отпечаток файла и не обновляет его, когда
    /// файл меняют снаружи (`replace_externally`): так проверяется запись по
    /// устаревшему снимку (фича 013).
    pub fn with_lagging_index(mut self, path: &str) -> Self {
        if let Some(stamp) = self.stamp_of(path) {
            self.lagging.insert(PathBuf::from(path), stamp);
        }
        self
    }

    /// Первая запись в файл провалится конфликтом, а файл станет `external`
    /// (фича 022).
    pub fn with_conflict_once(mut self, path: &str, external: impl Into<Vec<u8>>) -> Self {
        self.conflict_once
            .insert(PathBuf::from(path), external.into());
        self
    }

    /// Объявляет у заметки несохранённые правки вне редактора (фича 013).
    pub fn with_unsaved_edits(mut self, path: &str) -> Self {
        self.unsaved.insert(PathBuf::from(path));
        self
    }

    /// Сколько раз файл успешно записан через `write`.
    pub fn write_count(&self, path: &str) -> u32 {
        *self.writes.borrow().get(&PathBuf::from(path)).unwrap_or(&0)
    }

    /// Сколько записей через `write` во всём vault.
    pub fn total_writes(&self) -> u32 {
        self.writes.borrow().values().sum()
    }

    /// Сколько чтений во всём vault.
    pub fn total_reads(&self) -> u32 {
        self.reads.borrow().values().sum()
    }

    /// Подменяет содержимое «снаружи» — между вызовами ядра, мимо `write` и его
    /// счётчика: так приложение видит правку внешним редактором.
    pub fn replace_externally(&self, path: &str, bytes: impl Into<Vec<u8>>) {
        self.files
            .borrow_mut()
            .insert(PathBuf::from(path), bytes.into());
    }

    pub fn read_count(&self, path: &str) -> u32 {
        *self.reads.borrow().get(&PathBuf::from(path)).unwrap_or(&0)
    }
}

impl VaultBackend for MemoryVault {
    async fn open(_root_path: &Path) -> Result<Self, OpenError> {
        Ok(Self::new())
    }

    fn root_path(&self) -> &Path {
        &self.root
    }

    fn access_state(&self) -> AccessState {
        self.access_state
    }

    fn service_dir_existed_at_open(&self) -> bool {
        self.service_dir_existed
    }

    /// Все положенные файлы записями дерева, в порядке пути.
    ///
    /// Раньше здесь был пустой список: проверкам чтения он не мешал. Поиску
    /// мешает — он обходит именно `list()`, и фейк, отвечающий «vault пуст»,
    /// сделал бы зелёными любые правила совпадения.
    async fn list(&self) -> Vec<Note> {
        let mut notes: Vec<Note> = self
            .files
            .borrow()
            .iter()
            .map(|(path, bytes)| {
                Note::new(
                    path.clone(),
                    self.lagging.get(path).cloned().unwrap_or_else(|| {
                        typsidian_core::stamp::FileStamp::from_content(0, bytes)
                    }),
                )
            })
            .collect();
        notes.sort_by(|left, right| left.path.cmp(&right.path));
        notes
    }

    async fn file_mark(&self, path: &Path) -> Option<(u64, i64)> {
        // Времени изменения у файлов в памяти нет — его роль играет хэш
        // содержимого: отметка обязана меняться вместе с файлом.
        self.files.borrow().get(path).map(|bytes| {
            let hash = bytes.iter().fold(0i64, |hash, byte| {
                hash.wrapping_mul(31).wrapping_add(*byte as i64)
            });
            (bytes.len() as u64, hash)
        })
    }
    /// Шрифты среди положенных файлов. Время изменения — хэш
    /// содержимого: у памяти часов нет, а подмена файла тем же размером
    /// обязана выглядеть изменением, иначе кэш шрифтов её не заметит.
    async fn list_fonts(&self) -> Vec<typsidian_core::vault::FontFile> {
        let mut fonts: Vec<typsidian_core::vault::FontFile> = self
            .files
            .borrow()
            .iter()
            .filter(|(path, _)| typsidian_core::vault::is_font_file(path))
            .map(|(path, bytes)| typsidian_core::vault::FontFile {
                path: path.clone(),
                size: bytes.len() as u64,
                modified_ms: bytes.iter().fold(0i64, |hash, byte| {
                    hash.wrapping_mul(31).wrapping_add(*byte as i64)
                }),
            })
            .collect();
        fonts.sort_by(|left, right| left.path.cmp(&right.path));
        fonts
    }

    /// Все файлы, кроме лежащих под скрытыми папками и скрытых самих (фича 021);
    /// число скрытых записей — по их верхнему имени.
    async fn list_source_files(&self) -> typsidian_core::vault::SourceListing {
        let mut hidden = std::collections::BTreeSet::new();
        let mut files: Vec<typsidian_core::vault::SourceFile> = Vec::new();
        for (path, bytes) in self.files.borrow().iter() {
            let mut prefix = PathBuf::new();
            let mut is_hidden = false;
            for component in path.components() {
                prefix.push(component);
                if component.as_os_str().to_string_lossy().starts_with('.') {
                    hidden.insert(prefix.clone());
                    is_hidden = true;
                    break;
                }
            }
            if !is_hidden {
                files.push(typsidian_core::vault::SourceFile {
                    path: path.clone(),
                    size: bytes.len() as u64,
                });
            }
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        typsidian_core::vault::SourceListing {
            files,
            excluded: hidden.len() as u32,
        }
    }

    /// Изображения среди положенных файлов (фича 016) — по тем же правилам, что
    /// у настоящих backend: весь vault без скрытых, либо одна папка без вложенных.
    async fn list_images(&self, folder: Option<&Path>) -> Vec<typsidian_core::vault::ImageFile> {
        let mut images: Vec<typsidian_core::vault::ImageFile> = self
            .files
            .borrow()
            .iter()
            .filter(|(path, _)| typsidian_core::vault::is_image_file(path))
            .filter(|(path, _)| match folder {
                None => !path
                    .components()
                    .any(|part| part.as_os_str().to_string_lossy().starts_with('.')),
                Some(folder) => path.parent() == Some(folder),
            })
            .map(|(path, bytes)| typsidian_core::vault::ImageFile {
                path: path.clone(),
                size: bytes.len() as u64,
            })
            .collect();
        images.sort_by(|left, right| left.path.cmp(&right.path));
        images
    }

    /// Кандидаты в файлы источников (фича 017) — правило ядра, без скрытых
    /// папок; «время изменения» — хэш содержимого, как у шрифтов.
    async fn list_source_candidates(&self) -> Vec<typsidian_core::sources::SourceCandidate> {
        let mut found: Vec<typsidian_core::sources::SourceCandidate> = self
            .files
            .borrow()
            .iter()
            .filter(|(path, _)| typsidian_core::sources::discover::is_source_candidate(path))
            .filter(|(path, _)| {
                !path
                    .components()
                    .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
            })
            .map(|(path, bytes)| typsidian_core::sources::SourceCandidate {
                path: path.to_string_lossy().replace('\\', "/"),
                size: bytes.len() as u64,
                modified_ms: bytes.iter().fold(0i64, |hash, byte| {
                    hash.wrapping_mul(31).wrapping_add(*byte as i64)
                }),
            })
            .collect();
        found.sort_by(|left, right| left.path.cmp(&right.path));
        found
    }

    async fn list_conflict_copies(&self) -> Vec<PathBuf> {
        let mut copies: Vec<PathBuf> = self
            .files
            .borrow()
            .keys()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(typsidian_core::sync::conflict_copy::is_conflict_copy)
            })
            .cloned()
            .collect();
        copies.sort();
        copies
    }

    async fn list_folder(&self, folder: &Path) -> Vec<Note> {
        self.list()
            .await
            .into_iter()
            .filter(|note| note.path.parent() == Some(folder))
            .collect()
    }

    /// Папка существует, если в ней лежит хоть один файл, — иначе её нет.
    ///
    /// Это не упрощение, а точная модель памяти: пустых папок в `HashMap`
    /// путей не существует. Состояния `Unreadable` и `NotWritable`
    /// объявляются тестом явно (`with_folder_problem`, `read_only`): вывести
    /// их из содержимого карты нельзя, а проверять различимость вариантов
    /// нужно.
    async fn folder_access(&self, folder: &Path) -> FolderAccess {
        if let Some(reason) = self.folder_problems.get(folder) {
            return FolderAccess::Unreadable { reason: *reason };
        }
        let exists = self
            .files
            .borrow()
            .keys()
            .any(|path| path.starts_with(folder) && path != folder);
        if !exists {
            return FolderAccess::Absent;
        }
        if self.access_state == AccessState::ReadWrite {
            FolderAccess::Present
        } else {
            FolderAccess::NotWritable {
                reason: FolderProblem::VaultReadOnly,
            }
        }
    }

    async fn read(&self, path: &Path) -> Result<String, ReadError> {
        let bytes = self.read_bytes(path).await?;
        String::from_utf8(bytes).map_err(|_| ReadError::Io)
    }

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, ReadError> {
        *self
            .reads
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_insert(0) += 1;
        if let Some(error) = self.failures.get(path) {
            return Err(error.clone());
        }
        self.files
            .borrow()
            .get(path)
            .cloned()
            .ok_or(ReadError::NotFound)
    }

    async fn read_bytes_any_size(&self, path: &Path) -> Result<Vec<u8>, ReadError> {
        // Чтение без предела — такое же чтение: счётчик обязан его видеть
        // (изображения сборка читает так с фичи 016).
        *self
            .reads
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_insert(0) += 1;
        // «Слишком большой» файл здесь — обычный файл с объявленным отказом
        // `TooLarge`: чтение без предела отдаёт его содержимое.
        match self.failures.get(path) {
            Some(ReadError::TooLarge) | None => self
                .files
                .borrow()
                .get(path)
                .cloned()
                .ok_or(ReadError::NotFound),
            Some(error) => Err(error.clone()),
        }
    }

    async fn write(
        &mut self,
        path: &Path,
        content: &str,
        expected: Option<typsidian_core::stamp::FileStamp>,
    ) -> Result<(), WriteError> {
        // Гейт записи повторяет оба настоящих backend: без него проверить
        // FR-025 было бы нечем — прав файловой системы на wasm32 не существует.
        if self.access_state != AccessState::ReadWrite {
            return Err(WriteError::AccessDenied);
        }
        if expected.is_some() {
            if let Some(external) = self.conflict_once.remove(path) {
                self.files.borrow_mut().insert(path.to_path_buf(), external);
                return Err(WriteError::Conflict);
            }
        }
        if self.check_expected {
            if let Some(expected) = &expected {
                let current = self.stamp_of(&path.to_string_lossy());
                if current.as_ref() != Some(expected) {
                    return Err(WriteError::Conflict);
                }
            }
        }
        self.files
            .borrow_mut()
            .insert(path.to_path_buf(), content.as_bytes().to_vec());
        *self
            .writes
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_insert(0) += 1;
        Ok(())
    }

    async fn write_bytes(&mut self, path: &Path, bytes: &[u8]) -> Result<(), WriteError> {
        if self.access_state != AccessState::ReadWrite {
            return Err(WriteError::AccessDenied);
        }
        // Как атомарная запись настоящих backend: при отказе файла нет вовсе.
        if self.write_failures.contains(path) {
            return Err(WriteError::Io);
        }
        self.files
            .borrow_mut()
            .insert(path.to_path_buf(), bytes.to_vec());
        Ok(())
    }

    async fn create(&mut self, path: &Path) -> Result<(), CreateError> {
        if self.access_state != AccessState::ReadWrite {
            return Err(CreateError::AccessDenied);
        }
        self.files
            .borrow_mut()
            .insert(path.to_path_buf(), Vec::new());
        Ok(())
    }

    async fn ensure_folder(&mut self, _path: &Path) -> Result<(), WriteError> {
        if self.access_state != AccessState::ReadWrite {
            return Err(WriteError::AccessDenied);
        }
        Ok(())
    }

    async fn rename(&mut self, _from: &Path, _to: &Path) -> Result<(), RenameError> {
        Ok(())
    }

    async fn delete(&mut self, path: &Path) -> Result<(), DeleteError> {
        self.files.borrow_mut().remove(path);
        Ok(())
    }

    async fn remove_service_file(&mut self, path: &Path) -> Result<(), WriteError> {
        if self.access_state != AccessState::ReadWrite
            || !typsidian_core::vault::service_dir::is_inside_service_dir(path)
        {
            return Err(WriteError::AccessDenied);
        }
        self.files.borrow_mut().remove(path);
        Ok(())
    }

    /// Перенос здесь не моделируется: у памяти нет ни папок, ни индекса, а
    /// сценарии фичи 023 идут по настоящим backend (`relocate_cases`). Двойник
    /// отвечает как backend, у которого нечего переносить.
    async fn move_targets(
        &self,
        from: &Path,
        query: &str,
    ) -> Vec<typsidian_core::vault::relocate::MoveTarget> {
        typsidian_core::vault::relocate::move_targets_of(self, from, query).await
    }

    async fn move_impact(
        &self,
        _request: &typsidian_core::vault::relocate::MoveRequest,
    ) -> Result<
        typsidian_core::vault::relocate::MoveImpact,
        typsidian_core::vault::relocate::MoveRefusal,
    > {
        Err(typsidian_core::vault::relocate::MoveRefusal::NotFound)
    }

    async fn move_entry(
        &mut self,
        _request: &typsidian_core::vault::relocate::MoveRequest,
        _control: &dyn typsidian_core::vault::relocate::MoveControl,
    ) -> Result<
        typsidian_core::vault::relocate::MoveOutcome,
        typsidian_core::vault::relocate::MoveError,
    > {
        Err(typsidian_core::vault::relocate::MoveRefusal::NotFound.into())
    }

    /// Заметки, ссылающиеся на `path`.
    ///
    /// Ссылки разбираются тем же `graph::links_in`, что и в настоящих
    /// бэкендах: свой разбор `[[…]]` в фейке означал бы второе правило и
    /// зелёные тесты на неверном.
    async fn backlinks(&self, path: &Path) -> Vec<Note> {
        let mut linking: Vec<Note> = Vec::new();
        for note in self.list().await {
            if note.path == path {
                continue;
            }
            // Индекс ссылок не читает файлы через `read`: ни счётчик чтений, ни
            // объявленные отказы чтения на него не действуют (фича 013).
            let Some(content) = self
                .files
                .borrow()
                .get(&note.path)
                .and_then(|bytes| String::from_utf8(bytes.clone()).ok())
            else {
                continue;
            };
            if typsidian_core::graph::links_in(&note.path, &content)
                .iter()
                .any(|target| target == path)
            {
                linking.push(note);
            }
        }
        linking
    }

    async fn broken_links(&self, _path: &Path) -> Vec<PathBuf> {
        Vec::new()
    }

    fn poll_events(&mut self) -> Vec<VaultEvent> {
        Vec::new()
    }

    async fn begin_edit(&mut self, _path: &Path, _buffer: String) {}

    async fn discard_edit(&mut self, _path: &Path) {}

    fn has_unsaved_edits(&self, path: &Path) -> bool {
        self.unsaved.contains(path)
    }

    fn pending_conflict(&self, _path: &Path) -> Option<EditConflict> {
        None
    }

    /// Как настоящий реестр (`ConflictRegistry::unresolved_paths`): несохранённые
    /// буферы — тоже «есть что терять», а не только конфликты.
    fn unresolved_edit_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self.unsaved.iter().cloned().collect();
        paths.sort();
        paths
    }

    async fn conflict_diff(&self, _path: &Path) -> Vec<DiffLine> {
        Vec::new()
    }

    async fn resolve_conflict_keep_local(&mut self, _path: &Path) -> Result<(), ResolveError> {
        Ok(())
    }

    async fn resolve_conflict_accept_external(
        &mut self,
        _path: &Path,
    ) -> Result<Option<String>, ResolveError> {
        Ok(None)
    }
}

// Общие таблицы проверок фичи 006. Вынесены сюда, чтобы native и wasm
// сверялись с ОДНИМ набором ожиданий: в отличие от эталонного хэша досок, это
// таблица из контракта, и две её копии разошлись бы при первой же правке
// контракта, оставив один таргет проверенным по устаревшему правилу.
pub mod attachment_cases;
pub mod bookmarks_cases;
pub mod capture_cases;
pub mod completion_cases;
pub mod element_cases;
pub mod embeds_cases;
pub mod export_cases;
pub mod graph_cases;
pub mod highlight_cases;
pub mod html_export_cases;
pub mod import_cases;
pub mod install_cases;
pub mod language_cases;
pub mod links_cases;
pub mod merge_view_cases;
pub mod message_cases;
pub mod note_panel_cases;
pub mod package_cases;
pub mod placement_cases;
pub mod preview_cases;
pub mod properties_cases;
pub mod relocate_cases;
pub mod replace_cases;
pub mod search_cases;
pub mod slides_cases;
pub mod slides_export_cases;
pub mod snapshots_cases;
pub mod sources_cases;
pub mod spellcheck_cases;
pub mod structure_cases;
pub mod sync_cases;
pub mod tasks_cases;
pub mod template_cases;
pub mod textlayer_cases;
pub mod vault_settings_cases;
pub mod workspace_cases;
