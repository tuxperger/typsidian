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
                    typsidian_core::stamp::FileStamp::from_content(0, bytes),
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

    async fn write(
        &mut self,
        path: &Path,
        content: &str,
        _expected: Option<typsidian_core::stamp::FileStamp>,
    ) -> Result<(), WriteError> {
        // Гейт записи повторяет оба настоящих backend: без него проверить
        // FR-025 было бы нечем — прав файловой системы на wasm32 не существует.
        if self.access_state != AccessState::ReadWrite {
            return Err(WriteError::AccessDenied);
        }
        self.files
            .borrow_mut()
            .insert(path.to_path_buf(), content.as_bytes().to_vec());
        Ok(())
    }

    async fn write_bytes(&mut self, path: &Path, bytes: &[u8]) -> Result<(), WriteError> {
        if self.access_state != AccessState::ReadWrite {
            return Err(WriteError::AccessDenied);
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
            let Ok(content) = self.read(&note.path).await else {
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

    fn has_unsaved_edits(&self, _path: &Path) -> bool {
        false
    }

    fn pending_conflict(&self, _path: &Path) -> Option<EditConflict> {
        None
    }

    fn unresolved_edit_paths(&self) -> Vec<PathBuf> {
        Vec::new()
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
pub mod element_cases;
pub mod export_cases;
pub mod install_cases;
pub mod language_cases;
pub mod message_cases;
pub mod package_cases;
pub mod placement_cases;
pub mod preview_cases;
pub mod search_cases;
pub mod sync_cases;
pub mod template_cases;
pub mod textlayer_cases;
pub mod vault_settings_cases;
