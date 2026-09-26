//! Веб-реализация хранилища настроек (research.md §1, §3, §5, §10).
//!
//! Значения — файл `settings.json` в OPFS, тот же формат и тот же публичный API,
//! что и на native. IndexedDB используется **только** для реестра ссылок на
//! папки vault: `FileSystemDirectoryHandle` невозможно сериализовать в JSON.
//! Значения настроек в IndexedDB не попадают никогда — иначе пользователь
//! лишился бы возможности править их руками (FR-013) и видеть, где они лежат
//! (FR-036).

use super::apply::{plan, reset_to_defaults, ApplyContext, ApplyPlan, SettingsChange};
use super::diagnostics::{self, Diagnostics};
use super::editor_mode::{BridgeAvailability, EditorMode, OutageReason};
use super::events::SettingsEvent;
use super::folder_ref::{FolderRegistry, Resolved};
use super::format::{self, FolderRefs};
use super::store::{
    SettingsStore, StorageState, StoreCapabilities, StoreError, SETTINGS_FILE_NAME,
};
use super::vaults::{self, Availability, VaultId, VaultRecord};
use super::Settings;
use js_sys::{Array, Object, Reflect};
use std::collections::VecDeque;
use std::path::PathBuf;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Имя базы и хранилища реестра папок.
const IDB_NAME: &str = "typsidian-settings";
const IDB_STORE: &str = "folder-refs";

fn now_millis() -> i64 {
    js_sys::Date::now() as i64
}

async fn js_call0(obj: &JsValue, method: &str) -> Result<JsValue, JsValue> {
    let function: js_sys::Function = Reflect::get(obj, &JsValue::from_str(method))?.dyn_into()?;
    let result = function.call0(obj)?;
    if result.has_type::<js_sys::Promise>() {
        JsFuture::from(js_sys::Promise::from(result)).await
    } else {
        Ok(result)
    }
}

async fn js_call1(obj: &JsValue, method: &str, arg: &JsValue) -> Result<JsValue, JsValue> {
    let function: js_sys::Function = Reflect::get(obj, &JsValue::from_str(method))?.dyn_into()?;
    let result = function.call1(obj, arg)?;
    if result.has_type::<js_sys::Promise>() {
        JsFuture::from(js_sys::Promise::from(result)).await
    } else {
        Ok(result)
    }
}

/// Корень OPFS: `navigator.storage.getDirectory()`.
async fn opfs_root() -> Result<JsValue, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("нет window"))?;
    let storage = window.navigator().storage();
    JsFuture::from(storage.get_directory()).await
}

/// Хендл файла настроек в OPFS. `create` — создавать ли его, если нет.
async fn settings_file_handle(root: &JsValue, create: bool) -> Result<JsValue, JsValue> {
    let options = Object::new();
    Reflect::set(
        &options,
        &JsValue::from_str("create"),
        &JsValue::from_bool(create),
    )?;
    let function: js_sys::Function =
        Reflect::get(root, &JsValue::from_str("getFileHandle"))?.dyn_into()?;
    let promise = function.call2(
        root,
        &JsValue::from_str(SETTINGS_FILE_NAME),
        &options.into(),
    )?;
    JsFuture::from(js_sys::Promise::from(promise)).await
}

async fn read_text(file_handle: &JsValue) -> Result<String, JsValue> {
    let file = js_call0(file_handle, "getFile").await?;
    let text = js_call0(&file, "text").await?;
    Ok(text.as_string().unwrap_or_default())
}

/// Дешёвый снимок `{size, lastModified}` — предфильтр сверки (research.md §3):
/// содержимое читается только при расхождении.
async fn file_meta(file_handle: &JsValue) -> Option<(f64, f64)> {
    let file = js_call0(file_handle, "getFile").await.ok()?;
    let size = Reflect::get(&file, &JsValue::from_str("size")).ok()?.as_f64()?;
    let last_modified = Reflect::get(&file, &JsValue::from_str("lastModified"))
        .ok()?
        .as_f64()?;
    Some((size, last_modified))
}

/// Атомарная запись (research.md §5): `FileSystemWritableFileStream` по
/// спецификации фиксирует изменения только при `close()`, поэтому обрыв на
/// середине оставляет прежнее содержимое целиком, а не обрезанный JSON.
async fn write_text(file_handle: &JsValue, text: &str) -> Result<(), JsValue> {
    let writable = js_call0(file_handle, "createWritable").await?;
    js_call1(&writable, "write", &JsValue::from_str(text)).await?;
    js_call0(&writable, "close").await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Реестр ссылок на папки в IndexedDB (research.md §1, §10)
// ---------------------------------------------------------------------------

/// Оборачивает `IDBRequest` в промис: у IndexedDB нет промис-API, завершение
/// приходит событиями `success`/`error`.
fn request_to_promise(request: &web_sys::IdbRequest) -> js_sys::Promise {
    let request = request.clone();
    js_sys::Promise::new(&mut |resolve, reject| {
        let success_target = request.clone();
        let on_success = Closure::once(Box::new(move |_event: JsValue| {
            let value = success_target.result().unwrap_or(JsValue::UNDEFINED);
            let _ = resolve.call1(&JsValue::NULL, &value);
        }) as Box<dyn FnOnce(JsValue)>);
        let on_error = Closure::once(Box::new(move |_event: JsValue| {
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str("IndexedDB request failed"));
        }) as Box<dyn FnOnce(JsValue)>);
        request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
        request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_success.forget();
        on_error.forget();
    })
}

async fn open_registry() -> Result<web_sys::IdbDatabase, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("нет window"))?;
    let factory = window
        .indexed_db()?
        .ok_or_else(|| JsValue::from_str("IndexedDB недоступен"))?;
    let request = factory.open_with_u32(IDB_NAME, 1)?;

    // Хранилище создаётся в `upgradeneeded` — единственном месте, где схему
    // менять разрешено.
    let upgrade_target = request.clone();
    let on_upgrade = Closure::once(Box::new(move |_event: JsValue| {
        if let Ok(result) = upgrade_target.result() {
            if let Ok(db) = result.dyn_into::<web_sys::IdbDatabase>() {
                // Повторное создание существующего хранилища возвращает
                // ошибку — это и есть проверка «уже есть», без обращения к
                // `objectStoreNames` (лишняя фича web-sys ради одного чтения).
                let _ = db.create_object_store(IDB_STORE);
            }
        }
    }) as Box<dyn FnOnce(JsValue)>);
    request.set_onupgradeneeded(Some(on_upgrade.as_ref().unchecked_ref()));
    on_upgrade.forget();

    let value = JsFuture::from(request_to_promise(request.as_ref())).await?;
    value.dyn_into::<web_sys::IdbDatabase>()
}

fn store_for(
    db: &web_sys::IdbDatabase,
    mode: web_sys::IdbTransactionMode,
) -> Result<web_sys::IdbObjectStore, JsValue> {
    let transaction =
        db.transaction_with_str_and_mode(IDB_STORE, mode)?;
    transaction.object_store(IDB_STORE)
}

async fn registry_put(db: &web_sys::IdbDatabase, key: &str, handle: &JsValue) -> Result<(), JsValue> {
    let store = store_for(db, web_sys::IdbTransactionMode::Readwrite)?;
    let request = store.put_with_key(handle, &JsValue::from_str(key))?;
    JsFuture::from(request_to_promise(&request)).await?;
    Ok(())
}

async fn registry_get(db: &web_sys::IdbDatabase, key: &str) -> Option<JsValue> {
    let store = store_for(db, web_sys::IdbTransactionMode::Readonly).ok()?;
    let request = store.get(&JsValue::from_str(key)).ok()?;
    let value = JsFuture::from(request_to_promise(&request)).await.ok()?;
    (!value.is_undefined() && !value.is_null()).then_some(value)
}

async fn registry_delete(db: &web_sys::IdbDatabase, key: &str) -> Result<(), JsValue> {
    let store = store_for(db, web_sys::IdbTransactionMode::Readwrite)?;
    let request = store.delete(&JsValue::from_str(key))?;
    JsFuture::from(request_to_promise(&request)).await?;
    Ok(())
}

/// Веб-реализация [`SettingsStore`].
#[wasm_bindgen]
pub struct WebStore {
    root: JsValue,
    file: JsValue,
    registry: Option<web_sys::IdbDatabase>,
    settings: Settings,
    state: StorageState,
    raw: String,
    pending_invalid: bool,
    events: VecDeque<SettingsEvent>,
    /// Последний дешёвый снимок файла — предфильтр сверки (research.md §3).
    meta: Option<(f64, f64)>,
}

// wasm32 однопоточен, `JsValue` не пересекает границ потоков — обёртка
// безопасна в этом контексте (тот же приём, что и в `vault::web`).
unsafe impl Send for WebStore {}

impl WebStore {
    async fn persist(&mut self) -> Result<(), StoreError> {
        // FR-013, research.md §6: испорченный файл не перезаписывается молча.
        // В OPFS нет файлового менеджера, но правило одно на оба таргета —
        // копия откладывается рядом под именем с UTC-меткой.
        if self.pending_invalid {
            let moved_to = PathBuf::from(super::store::invalid_file_name(now_millis()));
            if self.set_aside(&moved_to).await.is_ok() {
                self.events
                    .push_back(SettingsEvent::InvalidFileSetAside { moved_to });
                self.raw = String::new();
            }
            self.pending_invalid = false;
        }

        let text = if self.raw.trim().is_empty() {
            format::initial_document(&self.settings)
        } else {
            format::render(&self.raw, &self.settings, &FolderRefs::new())
        };
        write_text(&self.file, &text)
            .await
            .map_err(|_| StoreError::Io)?;
        self.raw = text;
        self.meta = file_meta(&self.file).await;
        Ok(())
    }

    async fn set_aside(&self, moved_to: &std::path::Path) -> Result<(), JsValue> {
        let name = moved_to.to_string_lossy().into_owned();
        let options = Object::new();
        Reflect::set(&options, &JsValue::from_str("create"), &JsValue::TRUE)?;
        let function: js_sys::Function =
            Reflect::get(&self.root, &JsValue::from_str("getFileHandle"))?.dyn_into()?;
        let promise = function.call2(&self.root, &JsValue::from_str(&name), &options.into())?;
        let copy = JsFuture::from(js_sys::Promise::from(promise)).await?;
        write_text(&copy, &self.raw).await
    }

    /// Один цикл сверки с диском (research.md §3). JS-сторона вызывает его по
    /// своему таймеру с интервалом ≤1 с, чтобы уложиться в SC-014: WASM
    /// однопоточен и не может будить себя сам.
    ///
    /// Честное ограничение веба: OPFS недоступен ни файловому менеджеру, ни
    /// стороннему редактору, поэтому единственный источник внешнего изменения
    /// здесь — запись из другой вкладки того же происхождения. Механизм
    /// обнаружения при этом тот же, что и на native.
    async fn reread(&mut self) -> Option<SettingsEvent> {
        let meta = file_meta(&self.file).await;
        if meta == self.meta && self.state != StorageState::DefaultsAfterUnreadable {
            return None;
        }
        self.meta = meta;

        let text = read_text(&self.file).await.ok()?;
        if text == self.raw {
            return None;
        }
        match format::parse(&text) {
            Ok(settings) => {
                let recovered = self.state == StorageState::DefaultsAfterUnreadable;
                self.raw = text;
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

    pub async fn set_label(
        &mut self,
        id: &VaultId,
        label: Option<String>,
    ) -> Result<(), StoreError> {
        if vaults::set_label(&mut self.settings.known_vaults, id, label) {
            self.persist().await?;
        }
        Ok(())
    }

    pub async fn availability_of(&self, id: &VaultId) -> Availability {
        self.resolve(id).await.availability()
    }
}

impl SettingsStore for WebStore {
    async fn open() -> Result<Self, StoreError> {
        let root = opfs_root().await.map_err(|_| StoreError::NoConfigDirectory)?;
        let file = settings_file_handle(&root, true)
            .await
            .map_err(|_| StoreError::Io)?;
        let raw = read_text(&file).await.unwrap_or_default();

        let (settings, state, pending_invalid) = if raw.trim().is_empty() {
            (Settings::default(), StorageState::Loaded, false)
        } else {
            match format::parse(&raw) {
                Ok(settings) => (settings, StorageState::Loaded, false),
                Err(_) => (
                    Settings::default(),
                    StorageState::DefaultsAfterUnreadable,
                    true,
                ),
            }
        };

        let meta = file_meta(&file).await;
        let registry = open_registry().await.ok();

        let mut store = Self {
            root,
            file,
            registry,
            settings,
            state,
            raw,
            pending_invalid,
            events: VecDeque::new(),
            meta,
        };
        if store.state == StorageState::DefaultsAfterUnreadable {
            store.events.push_back(SettingsEvent::StorageUnreadable);
        }
        Ok(store)
    }

    fn settings(&self) -> &Settings {
        &self.settings
    }

    async fn load(&mut self) -> Settings {
        if let Some(event) = self.reread().await {
            self.events.push_back(event);
        }
        self.settings.clone()
    }

    fn storage_state(&self) -> StorageState {
        self.state.clone()
    }

    async fn apply(&mut self, change: &SettingsChange, context: &ApplyContext) -> ApplyPlan {
        let mut context = context.clone();
        context.storage_writable = self.state != StorageState::ReadOnlyForSession;

        let outcome = plan(&self.settings, change, &context);
        if outcome != ApplyPlan::Applied {
            return outcome;
        }

        self.settings = change.apply_to(&self.settings);
        if self.persist().await.is_err() {
            // Квота исчерпана или доступ к OPFS отозван — паритет с native:
            // значение действует до конца сессии (FR-014), а не отвергается.
            self.state = StorageState::ReadOnlyForSession;
            self.events.push_back(SettingsEvent::StorageBecameReadOnly);
        }
        ApplyPlan::Applied
    }

    fn capabilities(&self) -> StoreCapabilities {
        StoreCapabilities {
            // Показывается справочным текстом: у OPFS нет пути, который
            // пользователь мог бы открыть (research.md §13).
            location: format!("OPFS: {SETTINGS_FILE_NAME}"),
            can_reveal: false,
            writable: self.state != StorageState::ReadOnlyForSession,
        }
    }

    fn reveal(&self) -> Result<(), StoreError> {
        // Отказ значением, а не отсутствием метода: одинаковая форма API на
        // всех таргетах — требование принципа II (research.md §13).
        Err(StoreError::RevealUnsupported)
    }

    async fn poll_events(&mut self) -> Vec<SettingsEvent> {
        if let Some(event) = self.reread().await {
            self.events.push_back(event);
        }
        self.events.drain(..).collect()
    }
}

impl FolderRegistry for WebStore {
    type FolderRef = JsValue;

    async fn find_existing(&self, candidate: &JsValue) -> Option<VaultId> {
        let registry = self.registry.as_ref()?;
        // Тождество — `isSameEntry()` (research.md §10). Сравнение по имени
        // папки схлопнуло бы две разные папки `notes` в одну запись: в браузере
        // абсолютного пути не существует, и имя — единственное, что видно.
        // Перебор до N асинхронных сравнений; при рабочей границе списка в 50
        // записей это несущественно.
        for record in &self.settings.known_vaults {
            let Some(stored) = registry_get(registry, record.id.as_str()).await else {
                continue;
            };
            let Ok(same) = js_call1(&stored, "isSameEntry", candidate).await else {
                continue;
            };
            if same.as_bool().unwrap_or(false) {
                return Some(record.id.clone());
            }
        }
        None
    }

    async fn add(&mut self, candidate: JsValue, location: String) -> Result<VaultId, StoreError> {
        if let Some(existing) = self.find_existing(&candidate).await {
            return Ok(existing);
        }
        let known: Vec<VaultId> = self
            .settings
            .known_vaults
            .iter()
            .map(|r| r.id.clone())
            .collect();
        let id = VaultId::mint(&known, &location, now_millis());
        if let Some(registry) = self.registry.as_ref() {
            registry_put(registry, id.as_str(), &candidate)
                .await
                .map_err(|_| StoreError::Io)?;
        }
        vaults::upsert(
            &mut self.settings.known_vaults,
            VaultRecord::new(id.clone(), location),
        );
        self.persist().await?;
        Ok(id)
    }

    async fn resolve(&self, id: &VaultId) -> Resolved<JsValue> {
        let Some(registry) = self.registry.as_ref() else {
            return Resolved::Missing;
        };
        let Some(handle) = registry_get(registry, id.as_str()).await else {
            return Resolved::Missing;
        };
        // Разрешение доступа к папке могло быть отозвано между сессиями —
        // это не ошибка, его переспрашивают по жесту пользователя.
        match js_call1(&handle, "queryPermission", &Object::new().into()).await {
            Ok(state) if state.as_string().as_deref() == Some("granted") => {
                Resolved::Resolved(handle)
            }
            // `prompt`, отсутствие метода в браузере или отказ запроса — всё
            // это «нужно переспросить», а не «папки нет».
            _ => Resolved::NeedsPermission,
        }
    }

    async fn forget(&mut self, id: &VaultId) -> Result<(), StoreError> {
        if let Some(registry) = self.registry.as_ref() {
            let _ = registry_delete(registry, id.as_str()).await;
        }
        vaults::forget(&mut self.settings.known_vaults, id);
        if self.settings.current_vault.as_ref() == Some(id) {
            self.settings.current_vault = None;
        }
        self.persist().await
    }
}

/// Помощники для интеграционных тестов. Под фичей, а не под голым
/// `#[cfg(test)]`: тесты в `packages/core/tests/` линкуются с крейтом,
/// собранным без `cfg(test)`, и не увидели бы их (research.md §15).
///
/// Нужны, потому что «испортить файл настроек снаружи» в OPFS невозможно ничем,
/// кроме самого приложения: файлового менеджера у OPFS нет (research.md §3).
#[cfg(any(test, feature = "test-support"))]
impl WebStore {
    /// Кладёт произвольный текст в файл настроек, минуя разбор, — так тест
    /// воспроизводит повреждённое хранилище.
    pub async fn overwrite_raw_for_test(text: &str) -> Result<(), StoreError> {
        let root = opfs_root().await.map_err(|_| StoreError::NoConfigDirectory)?;
        let file = settings_file_handle(&root, true)
            .await
            .map_err(|_| StoreError::Io)?;
        write_text(&file, text).await.map_err(|_| StoreError::Io)
    }

    /// Читает файл настроек как есть — проверка «испорченный файл не тронут».
    pub async fn read_raw_for_test() -> Option<String> {
        let root = opfs_root().await.ok()?;
        let file = settings_file_handle(&root, false).await.ok()?;
        read_text(&file).await.ok()
    }

    /// Существует ли отложенная копия испорченного файла (research.md §6).
    pub async fn set_aside_exists_for_test(name: &str) -> bool {
        let Ok(root) = opfs_root().await else {
            return false;
        };
        let Ok(function) =
            Reflect::get(&root, &JsValue::from_str("getFileHandle")).and_then(|f| f.dyn_into::<js_sys::Function>())
        else {
            return false;
        };
        let Ok(promise) = function.call1(&root, &JsValue::from_str(name)) else {
            return false;
        };
        JsFuture::from(js_sys::Promise::from(promise)).await.is_ok()
    }
}

/// JS-facing обёртки: трейт не может нести `#[wasm_bindgen]`, поэтому это
/// единственная точка, через которую UI вызывает хранилище на web
/// (contracts/frontend-settings.md).
#[wasm_bindgen]
impl WebStore {
    #[wasm_bindgen(js_name = create)]
    pub async fn create_js() -> Result<WebStore, StoreError> {
        <WebStore as SettingsStore>::open().await
    }

    /// Версия приложения — подвал панели настроек (макет «Утилиты» §
    /// Настройки).
    ///
    /// Берётся из манифеста ядра на этапе сборки, а не пишется в интерфейсе
    /// литералом: строка в подвале обязана меняться вместе с версией, а не
    /// вместе с чьей-то памятью о ней.
    #[wasm_bindgen(js_name = version)]
    pub fn version_js() -> String {
        crate::settings::VERSION.to_string()
    }

    #[wasm_bindgen(js_name = settings)]
    pub fn settings_js(&self) -> Settings {
        self.settings.clone()
    }

    #[wasm_bindgen(js_name = storageState)]
    pub fn storage_state_js(&self) -> StorageState {
        self.state.clone()
    }

    #[wasm_bindgen(js_name = capabilities)]
    pub fn capabilities_js(&self) -> StoreCapabilities {
        SettingsStore::capabilities(self)
    }

    #[wasm_bindgen(js_name = reveal)]
    pub fn reveal_js(&self) -> Result<(), StoreError> {
        SettingsStore::reveal(self)
    }

    #[wasm_bindgen(js_name = pollEvents)]
    pub async fn poll_events_js(&mut self) -> Result<JsValue, JsValue> {
        let events = SettingsStore::poll_events(self).await;
        let array = Array::new();
        for event in events {
            array.push(&serde_wasm_bindgen::to_value(&event)?);
        }
        Ok(array.into())
    }

    #[wasm_bindgen(js_name = apply)]
    pub async fn apply_js(&mut self, change: SettingsChange) -> ApplyPlan {
        // Контекст собирает вызывающая сторона: ядро настроек не имеет доступа
        // ни к редактору, ни к vault-бэкенду (research.md §9).
        SettingsStore::apply(self, &change, &ApplyContext::unobstructed()).await
    }

    /// Добавить папку как vault. Ссылку UI передаёт **не разбирая** её:
    /// повторное добавление той же папки ядро само сведёт к существующей
    /// записи (FR-020).
    #[wasm_bindgen(js_name = addVault)]
    pub async fn add_vault_js(
        &mut self,
        handle: JsValue,
        location: String,
    ) -> Result<String, StoreError> {
        let id = self.add(handle, location).await?;
        Ok(id.as_str().to_string())
    }

    #[wasm_bindgen(js_name = setVaultLabel)]
    pub async fn set_vault_label_js(
        &mut self,
        id: String,
        label: Option<String>,
    ) -> Result<(), StoreError> {
        self.set_label(&VaultId::from_raw(id), label).await
    }

    #[wasm_bindgen(js_name = forgetVault)]
    pub async fn forget_vault_js(&mut self, id: String) -> Result<(), StoreError> {
        self.forget(&VaultId::from_raw(id)).await
    }

    #[wasm_bindgen(js_name = knownVaults)]
    pub async fn known_vaults_js(&self) -> Result<JsValue, JsValue> {
        let records = self.records_with_availability().await;
        Ok(serde_wasm_bindgen::to_value(&records)?)
    }

    /// Ссылка на папку известного vault — чтобы его можно было **открыть**, а
    /// не только показать в списке (FR-007 фичи 001).
    ///
    /// Без этой обёртки реестр папок оставался недостижим из интерфейса:
    /// `FolderRegistry::resolve` существовал, но пересечь границу wasm-bindgen
    /// не мог — трейт не несёт `#[wasm_bindgen]`, как и в случае самого
    /// `SettingsStore`. Список известных vault от этого был списком, по
    /// которому нельзя открыть ни одной записи.
    ///
    /// Возвращает `{ access, handle }`, где `access` — одно из трёх состояний
    /// [`Resolved`]: `"granted"`, `"needsPermission"` или `"missing"`.
    ///
    /// Состояние возвращается отдельно от хендла, а не сводится к «есть или
    /// нет», потому что от него зависит **может ли приложение открыть vault
    /// само**. При `"granted"` открытие не требует ничего от пользователя и
    /// происходит сразу при запуске; при `"needsPermission"` хендл цел, но
    /// браузер выдаст разрешение только в обработчике жеста — там приложение и
    /// спрашивает его, кнопкой на экране приветствия. `"missing"` — ссылки
    /// больше нет: папка удалена или связь утрачена, а розыск запрещён
    /// (FR-023).
    #[wasm_bindgen(js_name = resolveVault)]
    pub async fn resolve_vault_js(&self, id: String) -> Result<JsValue, JsValue> {
        let id = VaultId::from_raw(id);
        let (access, handle) = match self.resolve(&id).await {
            Resolved::Resolved(handle) => ("granted", Some(handle)),
            Resolved::NeedsPermission => {
                let stored = match self.registry.as_ref() {
                    Some(registry) => registry_get(registry, id.as_str()).await,
                    None => None,
                };
                ("needsPermission", stored)
            }
            Resolved::Missing => ("missing", None),
        };

        let result = Object::new();
        Reflect::set(&result, &JsValue::from_str("access"), &JsValue::from_str(access))?;
        if let Some(handle) = handle {
            Reflect::set(&result, &JsValue::from_str("handle"), &handle)?;
        }
        Ok(result.into())
    }

    /// Набор изменений, возвращающий значения к умолчаниям (FR-037).
    ///
    /// Возвращается именно `SettingsChange`, а не результат: сброс обязан
    /// пройти тем же путём `apply`, что и обычное изменение, — иначе он
    /// разойдётся с ним в предусловиях (неразрешённый конфликт отменяет сброс
    /// целиком, кларификация 2026-08-09). Собирать этот объект на стороне TS
    /// запрещено: умолчания — знание ядра (принцип I).
    ///
    /// `current_vault` в набор не входит: сброс не закрывает открытый vault и
    /// не трогает список известных (FR-038, SC-011).
    #[wasm_bindgen(js_name = resetToDefaults)]
    pub fn reset_to_defaults_js() -> SettingsChange {
        reset_to_defaults()
    }

    /// Куда записать набор изменений (фича 006, FR-029).
    ///
    /// Решение принимает ядро, исполняет интерфейс — та же форма, что у
    /// `ApplyPlan`: панель не владеет обоими хранилищами и не должна решать
    /// сама, в какое из них уходит правка.
    #[wasm_bindgen(js_name = changeTarget)]
    pub fn change_target_js(
        change: SettingsChange,
        vault_is_open: bool,
    ) -> super::apply::ChangeTarget {
        super::apply::target_for(&change, vault_is_open)
    }

    /// Действующие значения при открытом vault: устройство как слой по
    /// умолчанию, vault перекрывает записанное в нём (фича 006, FR-030).
    #[wasm_bindgen(js_name = resolveWithVault)]
    pub fn resolve_with_vault_js(
        &self,
        overrides: crate::vault::vault_settings::VaultOverrides,
    ) -> super::Settings {
        super::resolve(SettingsStore::settings(self), &overrides)
    }

    /// Набор, делающий действующие значения умолчаниями устройства (FR-031).
    ///
    /// Возвращает набор, а не применяет его: применение идёт тем же путём
    /// `apply()`, что и любое другое изменение, — отдельной операции, обходящей
    /// предусловия, не существует (то же правило, что у сброса).
    #[wasm_bindgen(js_name = deviceDefaultsChange)]
    pub fn device_defaults_change_js(
        &self,
        overrides: crate::vault::vault_settings::VaultOverrides,
    ) -> SettingsChange {
        let effective = super::resolve(SettingsStore::settings(self), &overrides);
        super::apply::device_defaults_change(&effective)
    }

    /// Диагностика раздела «Дополнительно» (FR-039).
    ///
    /// Состояние моста здесь не параметр, а вычисляемое значение: в вебе
    /// полноценный режим недоступен постоянно и по одной причине — платформа не
    /// порождает внешние процессы (research.md §7). Принимать его от UI значило
    /// бы позволить интерфейсу назвать состояние, которого нет.
    #[wasm_bindgen(js_name = diagnostics)]
    pub fn diagnostics_js(&self) -> Diagnostics {
        diagnostics::collect(
            EditorMode::PlainInput {
                reason: OutageReason::ExternalProcessesUnavailable,
            },
            BridgeAvailability::Unavailable {
                reason: OutageReason::ExternalProcessesUnavailable,
            },
            SettingsStore::capabilities(self),
            self.state.clone(),
        )
    }
}
