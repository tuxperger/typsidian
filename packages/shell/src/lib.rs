//! Нативная оболочка поверх ядра (001 T112-T115), общая для рабочего стола и
//! мобильного клиента.
//!
//! Здесь нет ни одного правила про vault: каждая команда — тонкая обёртка над
//! `typsidian_core` (принцип I). Проверить это можно по объёму тел: в них нет ни
//! одного `if`, который решал бы что-то за ядро, — только получение backend из
//! состояния и вызов.
//!
//! Чем эта оболочка отличается от web:
//! - backend — `NativeBackend` (`std::fs` + `notify`), а не WASM поверх File
//!   System Access API;
//! - события идут в окно **push**-ом: ядро сигналит `events_ready()`, задача
//!   ниже просыпается и пересылает `poll_events()` в окно (T114). Опроса по
//!   таймеру на native нет и не должно быть — обнаружение здесь событийное;
//! - папку выбирает платформа, а не `showDirectoryPicker` (T115): на рабочем
//!   столе — нативный диалог ОС, на Android — обзор папок поверх команд
//!   `browse_*` ниже.
//!
//! Крейт не собирается сам по себе в приложение: он отдаёт [`builder`], а
//! точку входа, плагины и `Context` (а с ним `tauri.conf.json`, иконки и
//! capabilities) добавляет оболочка — `apps/desktop` или `apps/mobile`.
//! Почему именно так — в комментарии к [`builder`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};

mod commands;

use tokio::sync::Mutex;
use typsidian_host::prelude::*;
use typsidian_host::StoreBodyProblem;
pub use typsidian_host::{
    Answer, Failure, HighlightSessions, ImportSource, NoVault, Preview, PropertiesReindex, Sources,
    Speller, TemplatePreviews, Vault,
};

mod sync;

/// Имя события, по которому окно получает пачку `VaultEvent`.
///
/// Одно на все варианты: разбор варианта — работа общего UI
/// (`applyVaultEvent`), и заводить по событию на вариант значило бы описать
/// размеченное объединение вторым способом.
const VAULT_EVENTS: &str = "vault://events";

// ── Открытие vault (FR-001, T115) ───────────────────────────────────────────

/// Открывает выбранную пользователем папку как vault.
///
/// Сам выбор папки делает нативный диалог на стороне окна
/// (`tauri-plugin-dialog`): это жест пользователя, и ядро его не инициирует —
/// то же правило, по которому `showDirectoryPicker` вызывается в UI (research.md
/// §1). На Android нативного диалога нет, и папку выбирают обзором поверх
/// [`browse_dir`] — сама команда от этого не меняется: ей передают путь.
///
/// Пересылка событий запускается здесь же: подписываться на них до открытия
/// vault не на что.
#[tauri::command]
async fn open_vault(
    app: AppHandle,
    state: State<'_, Vault>,
    sources_state: State<'_, Sources>,
    path: String,
) -> Answer<(), OpenError> {
    let backend = NativeBackend::open(Path::new(&path)).await?;
    let ready = backend.events_ready();
    *state.0.lock().await = Some(backend);
    // Источники и цитаты прежнего vault к новому отношения не имеют (017).
    *sources_state.0.lock().await = sources::SourceEngine::new();
    spawn_event_forwarding(app, ready);
    Ok(())
}

/// Пересылка `VaultEvent` в окно (T114).
///
/// Ждёт сигнала ядра и отдаёт накопившееся окну одной пачкой. Пачкой, а не по
/// событию: массовое изменение снаружи (`git checkout`) обёрнуто в
/// `BulkChangeStarted`/`Settled`, и разрывать эту обёртку на отдельные emit'ы
/// значило бы отдать UI шторм уведомлений, который US3 AS5 запрещает.
fn spawn_event_forwarding(app: AppHandle, ready: Arc<tokio::sync::Notify>) {
    tauri::async_runtime::spawn(async move {
        loop {
            ready.notified().await;
            let Some(state) = app.try_state::<Vault>() else {
                return;
            };
            let events: Vec<VaultEvent> = {
                let mut guard = state.0.lock().await;
                match guard.as_mut() {
                    Some(backend) => backend.poll_events(),
                    None => return,
                }
            };
            if events.is_empty() {
                continue;
            }
            if app.emit(VAULT_EVENTS, events).is_err() {
                // Окна больше нет — пересылать некуда.
                return;
            }
        }
    });
}

// ── Обзор папок устройства (выбор vault там, где нет диалога ОС) ────────────
//
// Это не работа с vault и не логика ядра, а реализация **жеста выбора папки**
// — того самого, который на рабочем столе целиком делает нативный диалог ОС, а
// в браузере `showDirectoryPicker`. На Android ни того, ни другого нет:
// `pick_folder` у `tauri-plugin-dialog` реализован только под desktop, а
// системный SAF отдаёт `content://`-идентификаторы, которых `NativeBackend` не
// понимает — он построен на `std::fs`. Поэтому обзор папок приходится показать
// своим окном, а этим командам — отдать ему содержимое каталогов.
//
// Раньше выбора здесь не было вовсе: команда `default_vault_dir` возвращала
// один фиксированный путь `Documents/Typsidian`, и папку с заметками, лежащую
// где-то ещё, открыть было нельзя. Её не стало вместе с появлением обзора —
// «создать папку» покрывает и первый запуск на чистом устройстве, ради
// которого она заводила каталог сама.
//
// Смотрят команды в обычную файловую систему устройства, а не в vault:
// открытый vault им не нужен и `Vault` они не трогают.

/// Папка в списке обзора: имя для показа и полный путь для перехода.
#[derive(Debug, serde::Serialize)]
pub struct DirEntryView {
    name: String,
    path: String,
}

/// Содержимое одной папки для окна обзора.
#[derive(Debug, serde::Serialize)]
pub struct DirListing {
    /// Путь самой папки: окно показывает его и его же предлагает выбрать.
    path: String,
    /// Родитель; `None` — выше подниматься некуда.
    parent: Option<String>,
    /// Вложенные папки по возрастанию имени. Файлов здесь нет: выбирается
    /// папка, и тысяча снимков камеры рядом с ней только мешала бы.
    dirs: Vec<DirEntryView>,
}

/// Отказ обзора — тем же размеченным объединением, что и ошибки ядра
/// (`{"kind": …}`), чтобы окно разбирало его сужением по `kind`.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind")]
pub enum BrowseError {
    NotFound,
    NotADirectory,
    /// На Android это в первую очередь не выданное разрешение на общую память,
    /// а не запрет на конкретный каталог (см. `MainActivity.kt`).
    AccessDenied,
    AlreadyExists,
    /// Имя новой папки пустое, состоит из точек или содержит разделитель пути.
    InvalidName,
    Io {
        message: String,
    },
}

impl From<std::io::Error> for BrowseError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => BrowseError::NotFound,
            std::io::ErrorKind::PermissionDenied => BrowseError::AccessDenied,
            std::io::ErrorKind::AlreadyExists => BrowseError::AlreadyExists,
            // Причина остаётся словами системы: общего «что-то пошло не так»
            // пользователю не показывают (FR-008).
            _ => BrowseError::Io {
                message: error.to_string(),
            },
        }
    }
}

/// С какой папки начинается обзор, пока пользователь ничего не выбирал.
///
/// На Android — корень общей памяти из `EXTERNAL_STORAGE` (эту переменную
/// процессу выставляет система; `/storage/emulated/0` — то, чем она является
/// на всех поддерживаемых версиях, и запасной вариант). Именно общая память, а
/// не приватный каталог приложения: vault обязан переживать удаление
/// приложения и быть виден файловому менеджеру и синхронизации (принцип VI).
///
/// На остальных нативных таргетах — домашний каталог. Рабочий стол этих команд
/// не зовёт (у него диалог ОС), и значение там нужно ровно затем, чтобы
/// команда оставалась осмысленной на любом таргете, а не отвечала пустотой.
#[tauri::command]
fn browse_root() -> String {
    #[cfg(target_os = "android")]
    {
        std::env::var("EXTERNAL_STORAGE").unwrap_or_else(|_| "/storage/emulated/0".to_owned())
    }
    #[cfg(not(target_os = "android"))]
    {
        std::env::var("HOME").unwrap_or_else(|_| "/".to_owned())
    }
}

/// Вложенные папки указанного каталога.
///
/// Скрытые (`.`-в начале имени) пропускаются: это служебные каталоги системы и
/// синхронизаций, а не место для заметок. Ссылки на папки показываются как
/// папки — на Android общая память видна и через `/sdcard`, который ссылкой и
/// является.
#[tauri::command]
async fn browse_dir(path: String) -> Answer<DirListing, BrowseError> {
    let path = PathBuf::from(path);
    // Отдельный `metadata` до чтения: иначе «это файл, а не папка» пришло бы
    // тем же `Io`, что и настоящий сбой ввода-вывода.
    if !tokio::fs::metadata(&path).await?.is_dir() {
        return Err(BrowseError::NotADirectory);
    }

    let mut entries = tokio::fs::read_dir(&path).await?;
    let mut dirs = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let kind = match entry.file_type().await {
            Ok(kind) => kind,
            // Запись исчезла между перечислением и вопросом о ней. Пропуск, а
            // не отказ: содержимое чужого каталога меняется без нас.
            Err(_) => continue,
        };
        let is_dir = if kind.is_symlink() {
            tokio::fs::metadata(entry.path())
                .await
                .is_ok_and(|target| target.is_dir())
        } else {
            kind.is_dir()
        };
        if !is_dir {
            continue;
        }
        dirs.push(DirEntryView {
            name,
            path: entry.path().to_string_lossy().into_owned(),
        });
    }
    // Порядок чтения каталога — порядок файловой системы, то есть никакой.
    // Регистр не должен разбивать список надвое: «Documents» и «documents»
    // стоят рядом.
    dirs.sort_by_key(|dir| dir.name.to_lowercase());

    Ok(DirListing {
        parent: path
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned()),
        path: path.to_string_lossy().into_owned(),
        dirs,
    })
}

/// Создаёт папку внутри показанной и возвращает её путь.
///
/// Нужна первому запуску: на чистом устройстве каталога для заметок может не
/// быть вовсе, и без этого пользователю пришлось бы заводить его файловым
/// менеджером, чтобы приложение вообще открылось.
///
/// Имя проверяется здесь, а не только системой: `..` и `a/b` система приняла
/// бы молча и создала папку не там, где показано пользователю.
#[tauri::command]
async fn browse_create_dir(parent: String, name: String) -> Answer<String, BrowseError> {
    let name = name.trim();
    if name.is_empty()
        || name.chars().all(|character| character == '.')
        || name.contains(['/', '\\', '\0'])
    {
        return Err(BrowseError::InvalidName);
    }
    let dir = PathBuf::from(parent).join(name);
    tokio::fs::create_dir(&dir).await?;
    Ok(dir.to_string_lossy().into_owned())
}

// ── Последний открытый vault (FR-007 фичи 001) ──────────────────────────────
//
// Хранилище настроек устройства — `NativeStore` фичи 004: `settings.json` в
// каталоге конфигурации ОС плюс реестр папок, где по `VaultId` лежит путь.
// Ядро всё это умеет давно; чего не было — вот этих двух команд, и без них
// нативные оболочки к хранилищу не дотягивались вовсе. Запоминание последнего
// vault на них поэтому не работало: `vault::last_vault` фича 004 упразднила,
// передав обязанность вызывающей стороне, а вызывающая сторона о хранилище не
// знала.
//
// Правил здесь по-прежнему нет: тождество папки устанавливает реестр ядра
// (канонизированные пути, FR-020), «эта папка ещё на месте?» отвечает
// `resolve`, а розыск перенесённой папки запрещён (FR-023) — команда ниже
// просто не вернёт её.

/// Хранилище настроек устройства. `None` — ещё не открывали.
///
/// Открывается лениво, а не на старте: до первого вопроса о последнем vault
/// читать `settings.json` незачем, а отказ хранилища не должен мешать
/// приложению запуститься.
#[derive(Default)]
pub struct Device(Arc<Mutex<Option<NativeStore>>>);

/// Последний открытый vault — окну, чтобы открыть его при запуске.
#[derive(Debug, serde::Serialize)]
pub struct LastVaultView {
    /// Подпись пользователя, если он её задавал.
    pub label: Option<String>,
    /// Показываемое расположение из записи.
    pub location: String,
    /// Путь, которым его открывать.
    pub path: String,
}

/// Запоминает папку как последний открытый vault.
///
/// Отдельно от команды, а не в её теле: команда добавляет к этому ровно
/// получение хранилища из состояния окна, и всё, что можно проверить тестом,
/// находится здесь (`packages/shell/tests/last_vault.rs`). Без этого «последний
/// vault запоминается» проверялось бы только запуском окна и разглядыванием
/// файла настроек руками.
pub async fn remember_opened_vault(
    store: &mut NativeStore,
    path: PathBuf,
) -> Result<VaultId, StoreError> {
    let location = path.to_string_lossy().into_owned();
    // Вторая запись о той же папке не появится: сведение к существующей делает
    // реестр ядра по канонизированному пути (FR-020).
    let id = store.add(path, location).await?;
    // Отказа этот набор не знает: единственная проверка `current_vault` — что
    // запись известна, а её только что добавили. Предусловия, которые могут
    // отклонить набор (несохранённая заметка, конфликт), относятся к смене
    // режима редактора и здесь не срабатывают.
    store
        .apply(&SettingsChange::current_vault(id.clone()))
        .await;
    Ok(id)
}

/// Последний открытый vault, если он ещё на месте.
pub async fn last_opened_vault(store: &NativeStore) -> Option<LastVaultView> {
    let id = SettingsStore::settings(store).current_vault.clone()?;
    let Resolved::Resolved(path) = store.resolve(&id).await else {
        // Розыск перенесённой папки запрещён (FR-023): связь утрачена, и
        // предлагать нечего.
        return None;
    };
    let record = SettingsStore::settings(store)
        .known_vaults
        .iter()
        .find(|record| record.id == id)
        .cloned();
    Some(LastVaultView {
        label: record.as_ref().and_then(|record| record.label.clone()),
        location: record
            .map(|record| record.location)
            .unwrap_or_else(|| path.to_string_lossy().into_owned()),
        path: path.to_string_lossy().into_owned(),
    })
}

/// Открывает хранилище настроек устройства.
///
/// `NativeStore::open()` — основной путь: он же приносит перенос значений из
/// упразднённого файла последнего vault. Запасной нужен Android: каталог
/// конфигурации в том смысле, в котором его знает `directories`, там может не
/// определиться вовсе, и тогда каталог называет сама платформа.
async fn open_device_store(app: &AppHandle) -> Result<NativeStore, StoreError> {
    match NativeStore::open().await {
        Err(StoreError::NoConfigDirectory) => {
            let dir = app
                .path()
                .app_config_dir()
                .map_err(|_| StoreError::NoConfigDirectory)?;
            NativeStore::open_in(dir).await
        }
        other => other,
    }
}

/// Хранилище устройства из состояния окна; открывает его при первом вопросе.
///
/// Одно место на все команды ниже: ленивое открытие, повторённое в каждой,
/// разошлось бы при первой же правке запасного пути Android.
async fn device_store<'a>(
    app: &AppHandle,
    guard: &'a mut Option<NativeStore>,
) -> Result<&'a mut NativeStore, StoreError> {
    if guard.is_none() {
        *guard = Some(open_device_store(app).await?);
    }
    Ok(guard.as_mut().expect("хранилище только что открыто"))
}

/// Что окно узнаёт, запомнив открытый vault.
#[derive(Debug, serde::Serialize)]
struct RememberedVault {
    /// Идентификатор записи: по нему окно открывает сессию синхронизации
    /// (фича 011).
    id: VaultId,
    /// Что сказать о служебном месте (006 FR-024, FR-025): настройки уровня
    /// vault утрачены либо vault открыт только на чтение.
    #[serde(skip_serializing_if = "Option::is_none")]
    notice: Option<VaultEvent>,
}

/// Запоминает открытую папку как последний vault.
///
/// Здесь же решается, было ли в этой папке что терять: «открывали раньше»
/// значит «запись о ней уже была», а это видно только в момент, когда её
/// пытаются добавить, — после него ответ всегда «да».
#[tauri::command]
async fn remember_vault(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    path: String,
) -> Answer<RememberedVault, StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    let known_before = SettingsStore::settings(store).known_vaults.len();
    let id = remember_opened_vault(store, PathBuf::from(path)).await?;
    let was_known = SettingsStore::settings(store).known_vaults.len() == known_before;
    let notice = vault.0.lock().await.as_ref().and_then(|backend| {
        typsidian_core::vault::vault_settings::service_dir_status(backend, was_known)
    });
    Ok(RememberedVault { id, notice })
}

/// Последний открытый vault, если он ещё на месте.
///
/// `None` — его нет или связь с папкой утрачена. Это не ошибка: окно в таком
/// случае просто показывает экран приветствия.
#[tauri::command]
async fn last_vault(
    app: AppHandle,
    device: State<'_, Device>,
) -> Answer<Option<LastVaultView>, StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    Ok(last_opened_vault(store).await)
}

// ── Панель настроек устройства (фича 004) ───────────────────────────────────
//
// Те же операции, что `WebStore` отдаёт через wasm-bindgen, — чтобы панель
// настроек на рабочем столе и Android была той же самой, что в браузере, а не
// урезанной до настроек папки. Решения по-прежнему принимает ядро: куда
// уходит правка (`target_for`), какой набор возвращает к умолчаниям, как
// слои складываются в действующие значения.

/// Всё, что панель читает из хранилища разом, — ответ одной команды.
///
/// Одним снимком, а не пятью командами: между пятью вызовами наблюдатель мог
/// бы перечитать файл, и панель показала бы значения из двух разных версий.
#[derive(Debug, serde::Serialize)]
pub struct DeviceSnapshot {
    settings: Settings,
    capabilities: StoreCapabilities,
    storage_state: StorageState,
    diagnostics: Diagnostics,
    known_vaults: Vec<VaultRecord>,
}

async fn snapshot_of(store: &NativeStore) -> DeviceSnapshot {
    DeviceSnapshot {
        settings: SettingsStore::settings(store).clone(),
        capabilities: store.capabilities(),
        storage_state: store.storage_state(),
        diagnostics: native_diagnostics(store),
        known_vaults: store.records_with_availability().await,
    }
}

/// Диагностика раздела «Дополнительно» (FR-039).
fn native_diagnostics(store: &NativeStore) -> Diagnostics {
    diagnostics::collect(store.capabilities(), store.storage_state())
}

#[tauri::command]
async fn device_settings(
    app: AppHandle,
    device: State<'_, Device>,
) -> Answer<DeviceSnapshot, StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    Ok(snapshot_of(store).await)
}

/// Журнал приложения целиком (004 FR-040). Отдельной командой, а не полем
/// снимка: журнал — до двух мегабайт текста, а снимок запрашивается постоянно.
#[tauri::command]
async fn device_journal(app: AppHandle, device: State<'_, Device>) -> Answer<String, StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    Ok(store.journal_text())
}

/// События хранилища с прошлого вызова (FR-013). Pull, как и на web: наблюдатель
/// ядра только взводит флаг, разбор — здесь.
#[tauri::command]
async fn device_settings_events(
    app: AppHandle,
    device: State<'_, Device>,
) -> Answer<Vec<SettingsEvent>, StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    Ok(store.poll_events().await)
}

/// Применить набор и вернуть план вместе со свежим снимком.
#[derive(Debug, serde::Serialize)]
pub struct AppliedSettings {
    plan: ApplyPlan,
    snapshot: DeviceSnapshot,
}

#[tauri::command]
async fn apply_device_settings(
    app: AppHandle,
    device: State<'_, Device>,
    change: SettingsChange,
) -> Answer<AppliedSettings, StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    let plan = store.apply(&change).await;
    Ok(AppliedSettings {
        plan,
        snapshot: snapshot_of(store).await,
    })
}

/// Набор, возвращающий значения к умолчаниям (FR-037).
#[tauri::command]
fn settings_reset_change() -> SettingsChange {
    settings_apply::reset_to_defaults()
}

/// Куда уходит правка — в устройство или в vault (фича 006, FR-029).
#[tauri::command]
fn settings_change_target(change: SettingsChange, vault_is_open: bool) -> ChangeTarget {
    settings_apply::target_for(&change, vault_is_open)
}

/// Раскладка набора по хранилищам (фича 010, FR-016): язык — всегда в
/// устройство. Интерфейс исполняет записи по порядку.
#[tauri::command]
fn settings_route_change(change: SettingsChange, vault_is_open: bool) -> Vec<RoutedChange> {
    settings_apply::route(&change, vault_is_open)
}

/// Действующие значения: устройство под перекрытием vault (FR-030).
#[tauri::command]
async fn resolve_device_settings(
    app: AppHandle,
    device: State<'_, Device>,
    overrides: VaultOverrides,
) -> Answer<Settings, StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    Ok(typsidian_core::settings::resolve(
        SettingsStore::settings(store),
        &overrides,
    ))
}

/// Набор, делающий действующие значения умолчаниями устройства (FR-031).
#[tauri::command]
async fn device_defaults_change(
    app: AppHandle,
    device: State<'_, Device>,
    overrides: VaultOverrides,
) -> Answer<SettingsChange, StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    let effective = typsidian_core::settings::resolve(SettingsStore::settings(store), &overrides);
    Ok(settings_apply::device_defaults_change(&effective))
}

/// Показать каталог настроек в файловом менеджере (FR-036).
#[tauri::command]
async fn reveal_device_settings(
    app: AppHandle,
    device: State<'_, Device>,
) -> Answer<(), StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    store.reveal()
}

/// Добавить папку в список известных vault (FR-020): ту же папку реестр ядра
/// сведёт к существующей записи.
#[tauri::command]
async fn add_known_vault(
    app: AppHandle,
    device: State<'_, Device>,
    path: String,
) -> Answer<String, StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    let id = store.add(PathBuf::from(&path), path).await?;
    Ok(id.as_str().to_string())
}

#[tauri::command]
async fn set_known_vault_label(
    app: AppHandle,
    device: State<'_, Device>,
    id: String,
    label: Option<String>,
) -> Answer<(), StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    store.set_label(&VaultId::from_raw(id), label)
}

/// Забыть запись; папка на диске не трогается (FR-024).
#[tauri::command]
async fn forget_known_vault(
    app: AppHandle,
    device: State<'_, Device>,
    id: String,
) -> Answer<(), StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    store.forget(&VaultId::from_raw(id)).await
}

/// Путь известного vault, чтобы его открыть. `None` — связь утрачена, а
/// розыск запрещён (FR-023).
#[tauri::command]
async fn resolve_known_vault(
    app: AppHandle,
    device: State<'_, Device>,
    id: String,
) -> Answer<Option<String>, StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    Ok(match store.resolve(&VaultId::from_raw(id)).await {
        Resolved::Resolved(path) => Some(path.to_string_lossy().into_owned()),
        _ => None,
    })
}

#[tauri::command]
async fn attachments_batch_store(
    state: State<'_, Vault>,
    request: tauri::ipc::Request<'_>,
) -> Answer<attachments::AttachmentBatchState, Failure<StoreBodyProblem>> {
    let tauri::ipc::InvokeBody::Raw(body) = request.body() else {
        return Err(Failure::Core(StoreBodyProblem::Truncated));
    };
    typsidian_host::store_attachment_body(&state, body).await
}

/// Открывает выбранную диалогом папку как источник импорта. Ничего в ней не
/// создаёт: ни `.typsidian/`, ни кэша.
#[tauri::command]
async fn import_open_source(state: State<'_, ImportSource>, path: String) -> Answer<(), OpenError> {
    let backend = NativeBackend::open_read_only(Path::new(&path))?;
    *state.0.lock().await = Some(ReadOnly::new(backend));
    Ok(())
}

// ── Свойства заметок и выборки (фича 020) ───────────────────────────────────
//
// Обёртки 1:1 над `typsidian_core::properties`; правил здесь нет. Команды без
// vault работают с текстом; остальные читают индекс открытого vault. Имена
// аргументов — camelCase на стороне окна (`knownKeys`, `onProgress` как событие).

/// Имя события хода пересчёта свойств: `{ done, total }`.
const PROPERTIES_REINDEX_PROGRESS: &str = "properties://reindex-progress";

/// Пакетный пересчёт свойств с ходом (событие) и отменой
/// (`vault_cancel_reindex_properties`). Идёт шагами: между ними замок vault
/// отпускается, и набор текста с остальными командами не ждёт конца пересчёта.
#[tauri::command]
async fn vault_reindex_properties(
    app: AppHandle,
    state: State<'_, Vault>,
    cancel: State<'_, PropertiesReindex>,
) -> Answer<properties::ReindexOutcome, Failure<()>> {
    typsidian_host::reindex_properties(&state, &cancel, |done, total| {
        let _ = app.emit(
            PROPERTIES_REINDEX_PROGRESS,
            serde_json::json!({ "done": done, "total": total }),
        );
    })
    .await
}

/// Обход бага WebKitGTK на проприетарном драйвере NVIDIA.
///
/// Под Wayland с явной синхронизацией (`linux-drm-syncobj-v1`) DMABUF-рендерер
/// WebKit коммитит буфер, не выставив acquire-точку, и композитор отвечает
/// `wp_linux_drm_syncobj_surface_v1: "Missing acquire timeline"`. Окно не
/// доживает до первого кадра: GTK получает протокольную ошибку и завершает
/// процесс — воспроизводится 3 раза из 3 на NVIDIA + Hyprland.
///
/// Путь DMABUF выключается **только там, где он сломан**: на AMD и Intel он
/// исправен и нужен, чтобы полотно доски укладывалось в бюджет отклика ≤100 мс
/// (фича 002, SC-005). Значение, заданное пользователем снаружи, не трогается —
/// обход можно и отключить, и включить принудительно.
#[cfg(target_os = "linux")]
fn disable_dmabuf_renderer_on_nvidia() {
    const KEY: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
    if Path::new("/sys/module/nvidia_drm").exists() && std::env::var_os(KEY).is_none() {
        std::env::set_var(KEY, "1");
    }
}

#[cfg(not(target_os = "linux"))]
fn disable_dmabuf_renderer_on_nvidia() {}

/// Приложение Tauri со всеми командами, но без плагинов и без `Context`.
///
/// Ни того, ни другого этот крейт дать не может, и это не недоделка, а
/// граница:
///
/// - `Context` рождается макросом `generate_context!()`, который читает
///   `tauri.conf.json` рядом с `Cargo.toml` того крейта, где раскрыт. Раскрытый
///   здесь, он намертво привязал бы обе оболочки к одному идентификатору
///   приложения, одному набору иконок и одним capabilities.
/// - Плагины `tauri-build` сверяет с capabilities **того крейта, который их
///   объявил зависимостью**: право `dialog:allow-open` существует только там,
///   где `tauri-plugin-dialog` записан в `Cargo.toml` самой оболочки. Отсюда
///   же и польза от разделения — диалог выбора папки нужен рабочему столу и
///   бессмыслен на Android, где выбирать папку нечем.
///
/// Общими остаются ровно команды: две копии одного списка из сорока обёрток
/// разошлись бы молча и по-разному на разных платформах.
pub fn builder() -> tauri::Builder<tauri::Wry> {
    // До `Builder`, а не внутри него: переменную обязан увидеть WebKit при
    // инициализации, и менять окружение можно только пока процесс однопоточный.
    disable_dmabuf_renderer_on_nvidia();

    sync::manage(tauri::Builder::default())
        .manage(Vault::default())
        .manage(ImportSource::default())
        .manage(PropertiesReindex::default())
        .manage(Device::default())
        .manage(TemplatePreviews::default())
        .manage(Sources::default())
        .manage(Preview::default())
        .manage(HighlightSessions::default())
        .manage(Speller::default())
        .invoke_handler(tauri::generate_handler![
            open_vault,
            browse_root,
            browse_dir,
            browse_create_dir,
            remember_vault,
            last_vault,
            device_settings,
            device_settings_events,
            device_journal,
            apply_device_settings,
            settings_reset_change,
            settings_change_target,
            settings_route_change,
            resolve_device_settings,
            device_defaults_change,
            reveal_device_settings,
            add_known_vault,
            set_known_vault_label,
            forget_known_vault,
            resolve_known_vault,
            commands::list,
            commands::read,
            commands::write,
            commands::create,
            commands::rename,
            commands::move_targets,
            commands::move_impact,
            commands::move_entry,
            commands::delete,
            commands::backlinks,
            commands::broken_links,
            commands::access_state,
            commands::search,
            commands::replace_preview,
            commands::replace_in_note,
            commands::replace_undo,
            commands::links_health,
            commands::links_fix,
            commands::snapshot_take,
            commands::snapshot_history,
            commands::snapshot_diff,
            commands::snapshot_text,
            commands::snapshot_restore,
            commands::snapshot_deleted_notes,
            commands::snapshot_recover_note,
            commands::snapshot_usage,
            commands::snapshots_clear,
            commands::file_kind,
            commands::begin_edit,
            commands::pending_conflict,
            commands::unresolved_edit_paths,
            commands::conflict_diff,
            commands::resolve_conflict_keep_local,
            commands::resolve_conflict_accept_external,
            commands::root_path,
            commands::board_write_profile,
            commands::create_board,
            commands::read_board,
            commands::write_board,
            commands::write_snapshot,
            commands::embeds_of,
            commands::embeds_in_text,
            commands::embeds_changed,
            commands::plan_embed,
            commands::embed_markup,
            commands::vault_settings,
            commands::set_vault_settings,
            commands::mark_section,
            commands::unmark_section,
            commands::keybindings,
            commands::set_keybinding,
            commands::reset_keybinding,
            commands::template_library,
            commands::read_template,
            commands::template_preview,
            commands::check_template_name,
            commands::save_as_template,
            commands::plan_insertion,
            commands::element_schema,
            commands::element_schema_for,
            commands::element_at,
            commands::syntax_session_open,
            commands::syntax_session_update,
            commands::syntax_session_close,
            commands::syntax_session_spelling,
            commands::embed_insert,
            commands::tasks_view,
            commands::task_toggle,
            commands::task_toggle_at,
            commands::bookmarks_view,
            commands::bookmark_add,
            commands::bookmark_remove,
            commands::bookmark_rename,
            commands::bookmark_move,
            commands::bookmark_group_create,
            commands::bookmark_group_rename,
            commands::bookmark_group_toggle,
            commands::bookmark_group_delete,
            commands::spell_configure,
            commands::spell_add_word,
            commands::spell_remove_word,
            commands::spell_suggest,
            commands::render_element,
            commands::plan_element_edit,
            commands::template_event,
            commands::preview_compile,
            commands::preview_compile_with_templates,
            commands::preview_dependency_mark,
            commands::preview_install_template,
            commands::preview_installed_templates,
            commands::export_pdf,
            commands::export_html,
            commands::export_slides,
            commands::export_site_index,
            commands::export_batch_with_format,
            commands::export_candidates,
            commands::export_batch_start,
            commands::sources_refresh,
            commands::sources_summary,
            commands::sources_search,
            commands::sources_for_completion,
            commands::sources_scan_start,
            commands::sources_scan_step,
            commands::sources_scan_cancel,
            commands::sources_scan_view,
            commands::sources_note_changed,
            commands::sources_note_removed,
            commands::sources_note_renamed,
            commands::sources_citing,
            commands::sources_missing_keys,
            commands::sources_check_note,
            commands::sources_plan_citation,
            commands::sources_bibliography_offer,
            commands::sources_create_file,
            commands::attachments_batch_start,
            commands::attachments_batch_next,
            commands::attachments_batch_confirm,
            attachments_batch_store,
            commands::attachments_batch_source_failed,
            commands::attachments_batch_cancel,
            commands::attachments_batch_view,
            commands::attachments_plan_markup,
            commands::attachments_validate_folder,
            commands::export_batch_next,
            commands::export_batch_record,
            commands::export_batch_skip,
            commands::export_batch_cancel,
            commands::export_batch_view,
            import_open_source,
            commands::import_close_source,
            commands::import_survey,
            commands::import_begin,
            commands::import_step,
            commands::import_finish,
            commands::links_backlinks_page,
            commands::links_mentions_availability,
            commands::links_mentions_start,
            commands::links_mentions_step,
            commands::links_mentions_cancel,
            commands::links_mentions_in,
            commands::links_link_mention,
            commands::workspace_empty,
            commands::workspace_apply,
            commands::workspace_view,
            commands::workspace_snapshot,
            commands::workspace_restore,
            commands::structure_outline,
            commands::structure_outline_view,
            commands::structure_outline_active,
            commands::structure_outline_carry,
            commands::preview_locate_heading,
            commands::slides_start,
            commands::slides_navigate,
            commands::slides_carry,
            commands::slides_caret,
            commands::slides_label,
            commands::slides_keymap,
            commands::capture_settings,
            commands::capture_entry,
            commands::capture_target_path,
            commands::daily_path,
            commands::daily_date_of,
            commands::capture_write,
            commands::open_daily,
            commands::daily_neighbours,
            commands::properties_read,
            commands::properties_edit,
            commands::select_blocks,
            commands::select_import_edits,
            commands::select_library,
            commands::search_parse_conditions,
            commands::vault_select,
            commands::vault_property_keys,
            commands::vault_graph,
            commands::completion_linked_index,
            commands::completions,
            commands::definition_at,
            commands::reference_marks,
            commands::vault_properties_current,
            vault_reindex_properties,
            commands::vault_cancel_reindex_properties,
            commands::vault_ensure_select_library,
            commands::vault_select_digest,
            commands::discard_edit,
            sync::sync_open,
            commands::sync_capabilities,
            commands::sync_overview,
            commands::sync_poll_events,
            commands::sync_progress,
            commands::sync_note_change,
            commands::sync_cancel,
            commands::sync_ignore_reports,
            commands::sync_apply_ignore,
            commands::sync_decline_stignore,
            commands::sync_ignore_open,
            commands::sync_ignore_check,
            commands::sync_merge_annotate,
            commands::sync_ignore_restore,
            commands::sync_ignore_save,
            commands::sync_enable_history,
            commands::sync_changes,
            commands::sync_snapshot,
            commands::sync_history,
            commands::sync_diff,
            commands::sync_snapshot_changes,
            commands::sync_file_at,
            commands::sync_restore,
            commands::sync_connect_remote,
            commands::sync_disconnect_remote,
            commands::sync_push,
            commands::sync_pull,
            commands::sync_restore_history,
            commands::sync_abort_pull,
            commands::sync_conflicts,
            commands::sync_resolve,
            commands::sync_resolve_many,
            commands::sync_set_keeper,
            commands::sync_clear_keeper,
            commands::sync_update_settings,
            commands::sync_refresh_mirror,
            sync::sync_syncthing_discover,
            sync::sync_syncthing_connect,
            sync::sync_syncthing_disconnect,
            commands::sync_tick,
            commands::sync_device_values,
            commands::sync_set_author,
            sync::sync_clone_vault,
        ])
}
