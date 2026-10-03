//! Нативная реализация хранилища настроек (research.md §1, §3, §5, §6, §10).
//!
//! Файл `settings.json` в каталоге конфигурации ОС, полученном через
//! `directories`: путь показывается пользователю (FR-036), поэтому обязан быть
//! тем, который он ожидает увидеть на своей системе. Ручная сборка
//! `HOME/.config/...` — как делал упразднённый `vault::last_vault` — неверна на
//! macOS и Windows.

use super::apply::{plan, ApplyContext, ApplyPlan, SettingsChange};
use super::events::SettingsEvent;
use super::folder_ref::{FolderRegistry, Resolved};
use super::format::{self, FolderRefs};
use super::store::{
    invalid_file_path, SettingsStore, StorageState, StoreCapabilities, StoreError,
    SETTINGS_FILE_NAME,
};
use super::vaults::{self, Availability, VaultId, VaultRecord};
use super::Settings;
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, FileIdMap};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Дебаунс наблюдения за каталогом настроек. Заметно меньше бюджета SC-014
/// (≤2 с), но достаточно, чтобы схлопнуть пару событий от атомарной записи
/// «временный файл + rename» в одно.
const WATCH_DEBOUNCE: Duration = Duration::from_millis(200);

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Каталог настроек по правилам платформы (research.md §1).
pub fn config_dir() -> Result<PathBuf, StoreError> {
    directories::ProjectDirs::from("", "", "typsidian")
        .map(|dirs| dirs.config_dir().to_path_buf())
        .ok_or(StoreError::NoConfigDirectory)
}

/// Доступен ли каталог на запись (FR-014). Проверяется по факту, а не по битам
/// доступа: под root биты не отражают реальную возможность записи.
fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(".typsidian-write-probe");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Атомарная запись «временный файл + fsync + rename» (research.md §5).
///
/// Наблюдаемое состояние файла — либо прежнее целиком, либо новое целиком:
/// обрыв на середине не оставляет обрезанный JSON, то есть не создаёт
/// повреждённое хранилище на ровном месте. Побочно это же закрывает FR-037:
/// весь набор значений уходит одним `rename`, промежуточного состояния на
/// диске не существует.
fn write_atomically(path: &Path, contents: &str) -> Result<(), StoreError> {
    let dir = path.parent().ok_or(StoreError::Io)?;
    std::fs::create_dir_all(dir).map_err(|_| StoreError::Io)?;
    let temporary = path.with_extension(format!("tmp-{}", now_millis()));
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&temporary).map_err(|_| StoreError::Io)?;
        file.write_all(contents.as_bytes())
            .map_err(|_| StoreError::Io)?;
        // Без fsync rename может опередить попадание данных на носитель, и
        // при потере питания на месте окажется пустой файл вместо старого.
        file.sync_all().map_err(|_| StoreError::Io)?;
    }
    std::fs::rename(&temporary, path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        let _ = error;
        StoreError::Io
    })
}

pub struct NativeStore {
    dir: PathBuf,
    path: PathBuf,
    settings: Settings,
    state: StorageState,
    /// Текст файла на момент последнего успешного чтения — основа записи,
    /// сохраняющей дописанные пользователем ключи (research.md §2,
    /// пересмотрено): [`format::render`] правит их поверх, а не собирает файл
    /// заново.
    raw: String,
    /// Испорченный файл ещё не отложен: его нельзя перезаписывать, пока он не
    /// переименован рядом (FR-013, research.md §6).
    pending_invalid: bool,
    folder_refs: FolderRefs,
    events: VecDeque<SettingsEvent>,
    /// Взводится наблюдателем; разбирается в `poll_events`.
    dirty: Arc<AtomicBool>,
    _watcher: Option<notify_debouncer_full::Debouncer<notify::RecommendedWatcher, FileIdMap>>,
}

impl NativeStore {
    /// Путь, по которому упразднённый `vault::last_vault` хранил «последний
    /// открытый vault» (research.md §10). Он собирал его руками из
    /// `HOME/.config`, поэтому импорт обязан смотреть именно туда, а не в
    /// каталог конфигурации по правилам платформы.
    pub fn legacy_last_vault_path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
        Some(
            PathBuf::from(home)
                .join(".config")
                .join("typsidian")
                .join("last_vault.txt"),
        )
    }

    /// Открывает хранилище в заданном каталоге **без** импорта наследия.
    ///
    /// Отдельный конструктор нужен тестам: миграция читает файл из `HOME`, и
    /// хранилище, открытое «в песочнице», всё равно затягивало бы настоящий
    /// `last_vault.txt` пользователя — тесты переставали быть герметичными и
    /// начинали зависеть от порядка запуска.
    pub async fn open_in(dir: PathBuf) -> Result<Self, StoreError> {
        Self::open_in_with_legacy(dir, None).await
    }

    /// Открывает хранилище, импортируя наследие из указанного файла, если он
    /// существует (research.md §10).
    pub async fn open_in_with_legacy(
        dir: PathBuf,
        legacy_last_vault: Option<PathBuf>,
    ) -> Result<Self, StoreError> {
        let path = dir.join(SETTINGS_FILE_NAME);
        let _ = std::fs::create_dir_all(&dir);

        let raw = std::fs::read_to_string(&path).unwrap_or_default();
        let existed = path.exists();
        let (settings, state, pending_invalid) = match (existed, format::parse(&raw)) {
            // Файла нет — это не повреждение, а первый запуск.
            (false, _) => (Settings::default(), StorageState::Loaded, false),
            (true, Ok(settings)) => (settings, StorageState::Loaded, false),
            // FR-013: нечитаемый файл не ошибка открытия. Поднимаемся на
            // значениях по умолчанию и НЕ трогаем файл до первой записи.
            (true, Err(_)) => (
                Settings::default(),
                StorageState::DefaultsAfterUnreadable,
                true,
            ),
        };

        let folder_refs = format::parse_folder_refs(&raw);

        let mut store = Self {
            dir,
            path,
            settings,
            state,
            raw,
            pending_invalid,
            folder_refs,
            events: VecDeque::new(),
            dirty: Arc::new(AtomicBool::new(false)),
            _watcher: None,
        };

        if store.state == StorageState::DefaultsAfterUnreadable {
            store.events.push_back(SettingsEvent::StorageUnreadable);
        }
        if !is_writable(&store.dir) {
            store.state = StorageState::ReadOnlyForSession;
            store.events.push_back(SettingsEvent::StorageBecameReadOnly);
        }

        if let Some(legacy) = legacy_last_vault {
            store.migrate_last_vault(&legacy);
        }
        store.start_watch();
        Ok(store)
    }

    /// Импорт `last_vault.txt` в список известных vault (research.md §10).
    ///
    /// Упразднённый модуль `vault::last_vault` хранил «последний открытый
    /// vault» в собственном файле и только под native — то есть был вторым
    /// источником истины и нарушал принцип II. Его содержимое становится
    /// единственной известной записью, а файл удаляется, чтобы двух источников
    /// не осталось.
    fn migrate_last_vault(&mut self, legacy: &Path) {
        if !self.settings.known_vaults.is_empty() {
            return;
        }
        let Ok(content) = std::fs::read_to_string(legacy) else {
            return;
        };
        let trimmed = content.trim();
        if trimmed.is_empty() {
            let _ = std::fs::remove_file(legacy);
            return;
        }

        let id = VaultId::mint(&[], trimmed, now_millis());
        self.folder_refs
            .insert(id.as_str().to_string(), trimmed.to_string());
        self.settings
            .known_vaults
            .push(VaultRecord::new(id.clone(), trimmed.to_string()));
        self.settings.current_vault = Some(id);
        // Файл удаляется только после успешной записи нового хранилища:
        // иначе сбой записи потерял бы единственную запись о vault.
        if self.persist().is_ok() {
            let _ = std::fs::remove_file(legacy);
        }
    }

    /// Наблюдение за каталогом настроек (research.md §3). Наблюдается каталог,
    /// а не файл: атомарная запись заменяет inode, и подписка на сам файл
    /// перестала бы работать после первого же `rename`.
    fn start_watch(&mut self) {
        let dirty = self.dirty.clone();
        let Ok(mut debouncer) =
            new_debouncer(WATCH_DEBOUNCE, None, move |result: DebounceEventResult| {
                if result.is_ok() {
                    dirty.store(true, Ordering::Relaxed);
                }
            })
        else {
            return;
        };
        if debouncer
            .watcher()
            .watch(&self.dir, RecursiveMode::NonRecursive)
            .is_ok()
        {
            self._watcher = Some(debouncer);
        }
    }

    /// Откладывает испорченный файл рядом перед первой записью поверх него
    /// (FR-013, research.md §6) и сообщает, куда именно.
    fn set_aside_invalid(&mut self) {
        if !self.pending_invalid {
            return;
        }
        let target = invalid_file_path(&self.path, now_millis());
        if std::fs::rename(&self.path, &target).is_ok() {
            self.events
                .push_back(SettingsEvent::InvalidFileSetAside { moved_to: target });
            // Текст, поверх которого пишутся значения, ушёл вместе с файлом:
            // новый файл начинается с подсказки, а не с обломков.
            self.raw = String::new();
        }
        self.pending_invalid = false;
    }

    fn persist(&mut self) -> Result<(), StoreError> {
        self.set_aside_invalid();
        let text = if self.raw.trim().is_empty() {
            let mut text = format::initial_document(&self.settings);
            if !self.folder_refs.is_empty() {
                text = format::render(&text, &self.settings, &self.folder_refs);
            }
            text
        } else {
            format::render(&self.raw, &self.settings, &self.folder_refs)
        };
        write_atomically(&self.path, &text)?;
        self.raw = text;
        Ok(())
    }

    /// Перечитывает файл, если он изменился на диске. Возвращает событие,
    /// которое следует опубликовать.
    fn reread(&mut self) -> Option<SettingsEvent> {
        let raw = std::fs::read_to_string(&self.path).ok();
        match raw {
            Some(text) if text == self.raw => None,
            Some(text) => match format::parse(&text) {
                Ok(settings) => {
                    let recovered = self.state == StorageState::DefaultsAfterUnreadable;
                    self.raw = text.clone();
                    self.folder_refs = format::parse_folder_refs(&text);
                    self.settings = settings.clone();
                    self.pending_invalid = false;
                    if recovered {
                        self.state = StorageState::Loaded;
                        Some(SettingsEvent::StorageRecovered { settings })
                    } else {
                        Some(SettingsEvent::ValuesChangedExternally { settings })
                    }
                }
                Err(_) => {
                    if self.state == StorageState::DefaultsAfterUnreadable {
                        None
                    } else {
                        self.raw = text;
                        self.state = StorageState::DefaultsAfterUnreadable;
                        self.pending_invalid = true;
                        self.settings = Settings::default();
                        Some(SettingsEvent::StorageUnreadable)
                    }
                }
            },
            // Файл исчез: значения по умолчанию, как и при повреждении
            // (data-model.md «StorageState»).
            None => {
                if self.state == StorageState::DefaultsAfterUnreadable {
                    None
                } else {
                    self.state = StorageState::DefaultsAfterUnreadable;
                    self.settings = Settings::default();
                    self.raw = String::new();
                    self.pending_invalid = false;
                    Some(SettingsEvent::StorageUnreadable)
                }
            }
        }
    }

    /// Список известных vault с проставленной доступностью (FR-023).
    pub async fn records_with_availability(&self) -> Vec<VaultRecord> {
        let mut records = self.settings.known_vaults.clone();
        for record in &mut records {
            record.availability = self.resolve(&record.id).await.availability();
        }
        records
    }

    /// Задать или снять подпись записи (FR-018).
    pub fn set_label(&mut self, id: &VaultId, label: Option<String>) -> Result<(), StoreError> {
        if vaults::set_label(&mut self.settings.known_vaults, id, label) {
            self.persist()?;
        }
        Ok(())
    }

    /// Путь к файлу настроек — для показа пользователю (FR-036).
    pub fn settings_path(&self) -> &Path {
        &self.path
    }
}

impl SettingsStore for NativeStore {
    async fn open() -> Result<Self, StoreError> {
        Self::open_in_with_legacy(config_dir()?, Self::legacy_last_vault_path()).await
    }

    fn settings(&self) -> &Settings {
        &self.settings
    }

    async fn load(&mut self) -> Settings {
        if let Some(event) = self.reread() {
            self.events.push_back(event);
        }
        self.settings.clone()
    }

    async fn apply(&mut self, change: &SettingsChange, context: &ApplyContext) -> ApplyPlan {
        let mut context = context.clone();
        context.storage_writable = self.state != StorageState::ReadOnlyForSession;

        let outcome = plan(&self.settings, change, &context);
        if outcome != ApplyPlan::Applied {
            return outcome;
        }

        let next = change.apply_to(&self.settings);
        // FR-014: при недоступном на запись каталоге изменение всё равно
        // вступает в силу — до конца сессии. Отказ здесь означал бы, что
        // приложение теряет настраиваемость из-за прав на каталог.
        if self.state == StorageState::ReadOnlyForSession {
            self.settings = next;
            return ApplyPlan::Applied;
        }

        self.settings = next;
        match self.persist() {
            Ok(()) => ApplyPlan::Applied,
            Err(_) => {
                self.state = StorageState::ReadOnlyForSession;
                self.events.push_back(SettingsEvent::StorageBecameReadOnly);
                // Значение уже применено в памяти и продолжает действовать до
                // конца сессии — это FR-014, а не отказ.
                ApplyPlan::Applied
            }
        }
    }

    fn storage_state(&self) -> StorageState {
        self.state.clone()
    }

    fn capabilities(&self) -> StoreCapabilities {
        StoreCapabilities {
            location: self.path.to_string_lossy().into_owned(),
            can_reveal: true,
            writable: self.state != StorageState::ReadOnlyForSession,
        }
    }

    fn reveal(&self) -> Result<(), StoreError> {
        // Раскрытие делегируется системному обработчику: собственного знания о
        // файловых менеджерах у ядра нет и быть не должно.
        let command = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "explorer"
        } else {
            "xdg-open"
        };
        std::process::Command::new(command)
            .arg(&self.dir)
            .spawn()
            .map(|_| ())
            .map_err(|_| StoreError::Io)
    }

    async fn poll_events(&mut self) -> Vec<SettingsEvent> {
        if self.dirty.swap(false, Ordering::Relaxed) {
            if let Some(event) = self.reread() {
                self.events.push_back(event);
            }
        }
        self.events.drain(..).collect()
    }
}

impl FolderRegistry for NativeStore {
    type FolderRef = PathBuf;

    async fn find_existing(&self, candidate: &PathBuf) -> Option<VaultId> {
        // Тождество — сравнение канонизированных путей (research.md §10).
        // Сравнение имён и показываемых расположений запрещено: две разные
        // папки с одинаковым именем обязаны остаться разными записями.
        let canonical = std::fs::canonicalize(candidate).ok()?;
        self.folder_refs.iter().find_map(|(id, stored)| {
            let stored_canonical = std::fs::canonicalize(stored).ok()?;
            (stored_canonical == canonical).then(|| VaultId::from_raw(id.clone()))
        })
    }

    async fn add(&mut self, candidate: PathBuf, location: String) -> Result<VaultId, StoreError> {
        if let Some(existing) = self.find_existing(&candidate).await {
            // Та же самая папка переиспользует запись и не создаёт вторую
            // (FR-020).
            return Ok(existing);
        }
        let known: Vec<VaultId> = self
            .settings
            .known_vaults
            .iter()
            .map(|r| r.id.clone())
            .collect();
        let id = VaultId::mint(&known, &location, now_millis());
        self.folder_refs.insert(
            id.as_str().to_string(),
            candidate.to_string_lossy().into_owned(),
        );
        vaults::upsert(
            &mut self.settings.known_vaults,
            VaultRecord::new(id.clone(), location),
        );
        self.persist()?;
        Ok(id)
    }

    async fn resolve(&self, id: &VaultId) -> Resolved<PathBuf> {
        let Some(stored) = self.folder_refs.get(id.as_str()) else {
            return Resolved::Missing;
        };
        let path = PathBuf::from(stored);
        // Розыск запрещён (FR-023): перенесённая снаружи папка не разыскивается
        // по новому расположению — если прежний путь не разрешается, связь
        // считается утраченной.
        match std::fs::canonicalize(&path) {
            Ok(canonical) if canonical.is_dir() => Resolved::Resolved(canonical),
            _ => Resolved::Missing,
        }
    }

    async fn forget(&mut self, id: &VaultId) -> Result<(), StoreError> {
        // Папка на диске не трогается (FR-024) — убирается только запись и
        // ссылка на неё.
        self.folder_refs.remove(id.as_str());
        vaults::forget(&mut self.settings.known_vaults, id);
        if self.settings.current_vault.as_ref() == Some(id) {
            self.settings.current_vault = None;
        }
        self.persist()
    }
}

impl NativeStore {
    /// Доступность записи для показа списка (FR-023).
    pub async fn availability_of(&self, id: &VaultId) -> Availability {
        self.resolve(id).await.availability()
    }
}
