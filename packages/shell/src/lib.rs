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

mod sync;

use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use typsidian_core::boards::{
    self, Board, BoardEvent, CreateBoardError, Embed, EmbedError, EmbedPlan, EmbedRef,
    ReadBoardError, SnapshotWriteError, WriteBoardError,
};
use typsidian_core::export;
use typsidian_core::preview::{self, CompiledDocument, SnapshotError};
use typsidian_core::settings::apply::{
    self as settings_apply, ApplyContext, ApplyPlan, ChangeTarget, RoutedChange, SettingsChange,
};
use typsidian_core::settings::diagnostics::{self, Diagnostics};
use typsidian_core::settings::editor_mode::{EditorModeBridge, NullBridge};
use typsidian_core::settings::events::SettingsEvent;
use typsidian_core::settings::folder_ref::{FolderRegistry, Resolved};
use typsidian_core::settings::keybindings::{
    self as keybindings_core, CommandId, ResolvedBinding, Shortcut,
};
use typsidian_core::settings::native::NativeStore;
use typsidian_core::settings::store::{SettingsStore, StorageState, StoreCapabilities, StoreError};
use typsidian_core::settings::vaults::{VaultId, VaultRecord};
use typsidian_core::settings::Settings;
use typsidian_core::stamp::FileStamp;
use typsidian_core::templates::{
    self, NameCheck, PreviewState, ReadTemplateError, SaveTemplateError, TemplateLibraryView,
};
use typsidian_core::vault::backend::{
    CreateError, DeleteError, OpenError, ReadError, RenameError, ResolveError, VaultBackend,
    WriteError,
};
use typsidian_core::vault::conflict::{DiffLine, EditConflict};
use typsidian_core::vault::events::VaultEvent;
use typsidian_core::vault::index::Note;
use typsidian_core::vault::native::NativeBackend;
use typsidian_core::vault::vault_settings::{
    self, SetKeybindingError, VaultOverrides, VaultSettingsError,
};
use typsidian_core::vault::{classify, AccessState, FileKind};

/// Имя события, по которому окно получает пачку `VaultEvent`.
///
/// Одно на все варианты: разбор варианта — работа общего UI
/// (`applyVaultEvent`), и заводить по событию на вариант значило бы описать
/// размеченное объединение вторым способом.
const VAULT_EVENTS: &str = "vault://events";

/// Право этой сборки писать доски (фича 002, FR-033, FR-034).
///
/// Здесь больше не ветвления по таргету. Раньше сборка под Android/iOS брала
/// `ViewOnly` — так требовал FR-034 в первой редакции спеки: рисование на
/// мобильном не входило в скоуп версии. Ограничение снято 2026-09-23: путь
/// записи доски (автосохранение, гейты конфликта, снимки) платформенно
/// нейтрален и на рабочем столе отработан, а единственным его следствием на
/// планшете был холст без единого инструмента.
///
/// Константой, а не полем состояния, это остаётся намеренно: профиль —
/// свойство клиента, и точкой, где он назначается, окно быть не должно. Сам
/// тип не исчезает и гейт в ядре тоже: `ViewOnly` — по-прежнему выразимое
/// состояние, и тесты ядра проверяют, что запись под ним отклоняется.
const BOARD_WRITE_PROFILE: boards::BoardWriteProfile = boards::BoardWriteProfile::ReadWrite;

/// Открытый vault. `None` — пользователь ещё не выбрал папку.
///
/// `Mutex`, а не `RwLock`: половина операций `VaultBackend` требует `&mut self`
/// (запись, `begin_edit`, разрешение конфликта), поэтому разделяемого чтения
/// всё равно не получится.
#[derive(Default)]
pub struct Vault(Mutex<Option<NativeBackend>>);

/// Кэш превью заготовок (фича 003, research.md §4).
///
/// Отдельным состоянием окна, а не полем `Vault`: он не принадлежит бэкенду и
/// не обязан переживать его — производное восстановимое состояние, ключуемое
/// хэшем содержимого. Отдельный `Mutex` затем, чтобы построение превью не
/// становилось вторым поводом держать блокировку самого vault.
#[derive(Default)]
pub struct TemplatePreviews(Mutex<templates::PreviewCache>);

/// Движок предпросмотра (фича 005, T042): встроенные шрифты и кэш файлов
/// внешних шаблонов.
///
/// То же, чем на web является `PreviewEngine` — и по той же причине отдельным
/// состоянием, а не полем `Vault`: предпросмотр не читает vault и не владеет
/// им, а vault получает на время одного вызова. Своего `Mutex` здесь нет
/// намеренно: `PackageCache` синхронизируется изнутри, а `FontSet` только
/// клонируется на вызов — лишняя блокировка сделала бы сборку документа
/// поводом ждать другую сборку.
///
/// Время жизни — всё окно, и именно оно делает кэш шаблонов полезным: без него
/// заметка с большим внешним шаблоном поднимала бы его заново на каждое
/// нажатие клавиши.
pub struct Preview {
    fonts: preview::FontSet,
    packages: preview::PackageCache,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            fonts: preview::FontSet::builtin(),
            packages: preview::PackageCache::new(),
        }
    }
}

/// Vault ещё не открыт, а команда требует его.
///
/// Отдельный вариант, а не общий `Io`: UI обязан различать «папка не выбрана» и
/// «файл не прочитался» (FR-008).
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind")]
pub enum NoVault {
    NoVaultOpen,
}

type Answer<T, E> = Result<T, E>;

macro_rules! with_vault {
    ($state:expr, |$backend:ident| $body:expr) => {{
        let mut guard = $state.0.lock().await;
        let Some($backend) = guard.as_mut() else {
            return Err(NoVault::NoVaultOpen.into());
        };
        $body
    }};
}

/// Отказ команды: либо vault не открыт, либо причина от ядра.
///
/// Сериализуется как то же размеченное объединение, что и ошибки ядра
/// (`{"kind": …}`), поэтому общий UI разбирает отказы desktop и web одним кодом
/// (contracts/frontend-events.md).
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum Failure<E> {
    NoVault(NoVault),
    Core(E),
}

impl<E> From<NoVault> for Failure<E> {
    fn from(value: NoVault) -> Self {
        Failure::NoVault(value)
    }
}

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
    path: String,
) -> Answer<(), OpenError> {
    let backend = NativeBackend::open(Path::new(&path)).await?;
    let ready = backend.events_ready();
    *state.0.lock().await = Some(backend);
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
pub struct Device(Mutex<Option<NativeStore>>);

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
) -> Result<(), StoreError> {
    let location = path.to_string_lossy().into_owned();
    // Вторая запись о той же папке не появится: сведение к существующей делает
    // реестр ядра по канонизированному пути (FR-020).
    let id = store.add(path, location).await?;
    // Отказа этот набор не знает: единственная проверка `current_vault` — что
    // запись известна, а её только что добавили. Предусловия, которые могут
    // отклонить набор (несохранённая заметка, конфликт), относятся к смене
    // режима редактора и здесь не срабатывают.
    store
        .apply(
            &SettingsChange::current_vault(id),
            &ApplyContext::unobstructed(),
        )
        .await;
    Ok(())
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

/// Запоминает открытую папку как последний vault.
#[tauri::command]
async fn remember_vault(
    app: AppHandle,
    device: State<'_, Device>,
    path: String,
) -> Answer<(), StoreError> {
    let mut guard = device.0.lock().await;
    let store = device_store(&app, &mut guard).await?;
    remember_opened_vault(store, PathBuf::from(path)).await
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
///
/// Моста к Neovim в оболочке пока нет, поэтому состояние режима берётся у
/// `NullBridge` — ядро само называет причину недоступности, а не окно.
fn native_diagnostics(store: &NativeStore) -> Diagnostics {
    let bridge = NullBridge::default();
    diagnostics::collect(
        bridge.current_mode(),
        bridge.availability(),
        store.capabilities(),
        store.storage_state(),
    )
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
    // Контекст — тот же, что у `WebStore.apply`: предусловия смены режима
    // проверяет вызывающая сторона через `RequiresNoteSave` (research.md §9).
    let plan = store.apply(&change, &ApplyContext::unobstructed()).await;
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

// ── Дерево и файлы ──────────────────────────────────────────────────────────

#[tauri::command]
async fn list(state: State<'_, Vault>) -> Answer<Vec<Note>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend.list().await))
}

#[tauri::command]
async fn read(state: State<'_, Vault>, path: String) -> Answer<String, Failure<ReadError>> {
    with_vault!(state, |backend| backend
        .read(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn write(
    state: State<'_, Vault>,
    path: String,
    content: String,
    expected: Option<FileStamp>,
) -> Answer<(), Failure<WriteError>> {
    with_vault!(state, |backend| backend
        .write(Path::new(&path), &content, expected)
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn create(state: State<'_, Vault>, path: String) -> Answer<(), Failure<CreateError>> {
    with_vault!(state, |backend| backend
        .create(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn rename(
    state: State<'_, Vault>,
    from: String,
    to: String,
) -> Answer<(), Failure<RenameError>> {
    with_vault!(state, |backend| backend
        .rename(Path::new(&from), Path::new(&to))
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn delete(state: State<'_, Vault>, path: String) -> Answer<(), Failure<DeleteError>> {
    with_vault!(state, |backend| backend
        .delete(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn backlinks(state: State<'_, Vault>, path: String) -> Answer<Vec<Note>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .backlinks(Path::new(&path))
        .await))
}

#[tauri::command]
async fn broken_links(state: State<'_, Vault>, path: String) -> Answer<Vec<PathBuf>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .broken_links(Path::new(&path))
        .await))
}

#[tauri::command]
async fn access_state(state: State<'_, Vault>) -> Answer<AccessState, Failure<()>> {
    with_vault!(state, |backend| Ok(backend.access_state()))
}

/// Расширенный поиск по vault (макет «Утилиты» § Поиск).
///
/// Обёртка, как и остальные команды: правила совпадения живут в ядре, здесь
/// только пересылка через границу процесса (принцип I).
#[tauri::command]
async fn search(
    state: State<'_, Vault>,
    query: typsidian_core::search::SearchQuery,
) -> Answer<typsidian_core::search::SearchResults, Failure<typsidian_core::search::SearchError>> {
    with_vault!(state, |backend| typsidian_core::search::search(
        backend, &query
    )
    .await
    .map_err(Failure::Core))
}

/// Путь папки, открытой как vault — окну для хлебных крошек в шапке.
///
/// Спрашивается у бэкенда, а не запоминается окном при выборе папки: тогда
/// «какой vault открыт» знали бы два места, и после переоткрытия они могли бы
/// разойтись.
#[tauri::command]
async fn root_path(state: State<'_, Vault>) -> Answer<String, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .root_path()
        .to_string_lossy()
        .into_owned()))
}

/// Право сборки писать доски — окну, чтобы объявить ограничение пользователю
/// (фича 002, FR-034).
///
/// Не требует открытого vault: профиль выбран таргетом при сборке, а не
/// состоянием папки, и окно спрашивает о нём до открытия чего-либо. Возвращает
/// ровно ту константу, которую применяет `write_board`, — сказанное
/// пользователю и применённое к записи не могут разойтись.
#[tauri::command]
fn board_write_profile() -> boards::BoardWriteProfile {
    BOARD_WRITE_PROFILE
}

/// Вид файла по пути. Не требует открытого vault: классификация идёт по пути,
/// без чтения содержимого (фича 002, research.md §3).
#[tauri::command]
fn file_kind(path: String) -> FileKind {
    classify(Path::new(&path))
}

// ── Несохранённые правки и конфликты ────────────────────────────────────────

#[tauri::command]
async fn begin_edit(
    state: State<'_, Vault>,
    path: String,
    buffer: String,
) -> Answer<(), Failure<()>> {
    with_vault!(state, |backend| {
        backend.begin_edit(Path::new(&path), buffer).await;
        Ok(())
    })
}

#[tauri::command]
async fn pending_conflict(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Option<EditConflict>, Failure<()>> {
    with_vault!(state, |backend| Ok(
        backend.pending_conflict(Path::new(&path))
    ))
}

#[tauri::command]
async fn unresolved_edit_paths(state: State<'_, Vault>) -> Answer<Vec<PathBuf>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend.unresolved_edit_paths()))
}

#[tauri::command]
async fn conflict_diff(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Vec<DiffLine>, Failure<()>> {
    with_vault!(state, |backend| Ok(backend
        .conflict_diff(Path::new(&path))
        .await))
}

#[tauri::command]
async fn resolve_conflict_keep_local(
    state: State<'_, Vault>,
    path: String,
) -> Answer<(), Failure<ResolveError>> {
    with_vault!(state, |backend| backend
        .resolve_conflict_keep_local(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

#[tauri::command]
async fn resolve_conflict_accept_external(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Option<String>, Failure<ResolveError>> {
    with_vault!(state, |backend| backend
        .resolve_conflict_accept_external(Path::new(&path))
        .await
        .map_err(Failure::Core))
}

// ── Доски и вставки (фича 002) ──────────────────────────────────────────────

/// Профиль записи зашит `ReadWrite` по той же причине, что и в web-биндинге:
/// рабочий стол рисует (FR-033 фичи 002), а просмотр без записи — свойство
/// мобильного клиента, у которого будет своя оболочка.
#[tauri::command]
async fn create_board(
    state: State<'_, Vault>,
    path: String,
) -> Answer<Board, Failure<CreateBoardError>> {
    with_vault!(state, |backend| boards::create_board(
        backend,
        Path::new(&path)
    )
    .await
    .map_err(Failure::Core))
}

#[tauri::command]
async fn read_board(
    state: State<'_, Vault>,
    path: String,
) -> Answer<String, Failure<ReadBoardError>> {
    with_vault!(state, |backend| {
        let content = boards::read_board(backend, Path::new(&path))
            .await
            .map_err(Failure::Core)?;
        String::from_utf8(content.as_bytes().to_vec())
            .map_err(|_| Failure::Core(ReadBoardError::Unreadable))
    })
}

#[tauri::command]
async fn write_board(
    state: State<'_, Vault>,
    path: String,
    content: String,
    expected: Option<FileStamp>,
) -> Answer<(), Failure<WriteBoardError>> {
    with_vault!(state, |backend| boards::write_board(
        backend,
        BOARD_WRITE_PROFILE,
        Path::new(&path),
        &boards::BoardContent::new(content.as_bytes().to_vec()),
        expected,
    )
    .await
    .map_err(Failure::Core))
}

#[tauri::command]
async fn write_snapshot(
    state: State<'_, Vault>,
    snapshot_path: String,
    svg: String,
    board_path: String,
    board_content: String,
) -> Answer<(), Failure<SnapshotWriteError>> {
    with_vault!(state, |backend| {
        let digest = boards::digest(board_content.as_bytes())
            .map_err(|_| Failure::Core(SnapshotWriteError::NotAnSvg))?;
        let metadata = boards::snapshot_metadata(Path::new(&board_path), &digest);
        boards::write_snapshot(backend, Path::new(&snapshot_path), &svg, &metadata)
            .await
            .map_err(Failure::Core)
    })
}

#[tauri::command]
async fn embeds_of(
    state: State<'_, Vault>,
    note_path: String,
    note_text: String,
) -> Answer<Vec<Embed>, Failure<()>> {
    with_vault!(state, |backend| Ok(boards::embeds_with_state(
        backend,
        Path::new(&note_path),
        &note_text
    )
    .await))
}

/// Разбор текста заметки. Vault не требуется: файлы не читаются.
#[tauri::command]
fn embeds_in_text(note_path: String, note_text: String) -> Vec<EmbedRef> {
    boards::embeds_of(Path::new(&note_path), &note_text)
}

#[tauri::command]
fn embeds_changed(note_path: String, note_text: String, changed: String) -> Option<BoardEvent> {
    boards::embeds_changed(Path::new(&note_path), &note_text, Path::new(&changed))
}

/// Настройки vault приходят целиком, а не разобранными на политику и список
/// разделов: UI их уже прочитал, а решение «что из них влияет на путь»
/// принадлежит ядру (фича 006, принцип I).
#[tauri::command]
fn plan_embed(
    note_path: String,
    note_text: String,
    board_path: String,
    overrides: VaultOverrides,
) -> Answer<EmbedPlan, EmbedError> {
    boards::plan_embed(
        Path::new(&note_path),
        &note_text,
        Path::new(&board_path),
        overrides.placement(),
        &overrides.sections,
    )
}

// ── Настройки уровня vault и разделы (фича 006) ─────────────────────────────

/// Настройки уровня vault (FR-002, FR-015, FR-026).
#[tauri::command]
async fn vault_settings(
    state: State<'_, Vault>,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::read_vault_settings(backend)
            .await
            .map_err(Failure::Core)
    })
}

/// Записывает настройки уровня vault (FR-029).
#[tauri::command]
async fn set_vault_settings(
    state: State<'_, Vault>,
    overrides: VaultOverrides,
) -> Answer<(), Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::write_vault_settings(backend, &overrides)
            .await
            .map_err(Failure::Core)
    })
}

/// Помечает папку как корень раздела (FR-013).
#[tauri::command]
async fn mark_section(
    state: State<'_, Vault>,
    folder: String,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::set_section(backend, Path::new(&folder), true)
            .await
            .map_err(Failure::Core)
    })
}

/// Снимает пометку раздела (FR-013). Уже созданные файлы вставок при этом не
/// двигаются (FR-017).
#[tauri::command]
async fn unmark_section(
    state: State<'_, Vault>,
    folder: String,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::set_section(backend, Path::new(&folder), false)
            .await
            .map_err(Failure::Core)
    })
}

#[tauri::command]
fn embed_markup(plan: EmbedPlan) -> String {
    boards::embed_markup(&plan)
}

// ── Сочетания клавиш ────────────────────────────────────────────────────────

/// Действующие сочетания всех команд.
///
/// Не команда над vault: чистая функция от уже прочитанных настроек. Через
/// границу процесса она ходит только потому, что на этой стороне ядро живёт в
/// нативном процессе, а разбирать сочетания второй раз в интерфейсе значило бы
/// завести вторую копию правил (принцип I).
#[tauri::command]
fn keybindings(overrides: VaultOverrides, mac: bool) -> Vec<ResolvedBinding> {
    keybindings_core::resolve(&overrides.keybindings, mac)
}

/// Назначает команде сочетание либо снимает его (`shortcut: null`).
#[tauri::command]
async fn set_keybinding(
    state: State<'_, Vault>,
    command: CommandId,
    shortcut: Option<Shortcut>,
) -> Answer<VaultOverrides, Failure<SetKeybindingError>> {
    with_vault!(state, |backend| {
        vault_settings::set_keybinding(backend, command, shortcut)
            .await
            .map_err(Failure::Core)
    })
}

/// Возвращает команду к умолчанию ядра.
#[tauri::command]
async fn reset_keybinding(
    state: State<'_, Vault>,
    command: CommandId,
) -> Answer<VaultOverrides, Failure<VaultSettingsError>> {
    with_vault!(state, |backend| {
        vault_settings::reset_keybinding(backend, command)
            .await
            .map_err(Failure::Core)
    })
}

// ── Шаблоны и сниппеты (фича 003) ───────────────────────────────────────────
//
// Те же обёртки, что и всё выше: правила библиотеки живут в
// `typsidian_core::templates`, здесь только пересылка через границу процесса.

#[tauri::command]
async fn template_library(state: State<'_, Vault>) -> Answer<TemplateLibraryView, Failure<()>> {
    with_vault!(state, |backend| Ok(templates::library(backend).await))
}

#[tauri::command]
async fn read_template(
    state: State<'_, Vault>,
    path: String,
) -> Answer<String, Failure<ReadTemplateError>> {
    with_vault!(state, |backend| templates::read_template(
        backend,
        Path::new(&path)
    )
    .await
    .map_err(Failure::Core))
}

/// Превью одной заготовки.
///
/// Кэш — состояние оболочки, а не ядра: на web им владеет `WebBackend`, здесь —
/// эта команда. Разница вынужденная и безобидная: кэш производный,
/// восстановимый и авторитетной копией ничему не является (research.md §4).
/// Правило, по которому он ключуется хэшем содержимого, при этом одно и живёт в
/// ядре.
#[tauri::command]
async fn template_preview(
    state: State<'_, Vault>,
    cache: State<'_, TemplatePreviews>,
    template: templates::Template,
) -> Answer<PreviewState, Failure<()>> {
    let mut cache = cache.0.lock().await;
    with_vault!(state, |backend| Ok(templates::preview_of(
        backend,
        &templates::ExcerptOnlySource,
        &mut cache,
        &template
    )
    .await))
}

// ── Предпросмотр документа (фича 005, T042) ─────────────────────────────────

/// Собирает заметку из текста редактора.
///
/// Текст приходит из буфера окна, а не с диска: иначе требование «без кнопки
/// сохранить» невыполнимо (FR-002, FR-006). Файл vault при этом не читается и
/// не изменяется.
///
/// Vault всё же нужен — фазе резолва: связанные файлы (изображения, вставки
/// досок, импорты) читаются через него, своего доступа к диску у предпросмотра
/// нет (принцип I).
///
/// Очереди вызовов, обязательной на web, здесь нет и не требуется: ограничение
/// «recursive use of an object» — свойство границы wasm-bindgen, а не ядра.
/// Нативный бэкенд защищён обычным `Mutex` состояния vault.
#[tauri::command]
async fn preview_compile(
    state: State<'_, Vault>,
    engine: State<'_, Preview>,
    note_path: String,
    text: String,
    revision: u64,
) -> Answer<CompiledDocument, Failure<SnapshotError>> {
    with_vault!(state, |backend| preview::compile_note_resolved(
        backend,
        &engine.packages,
        &note_path,
        &text,
        revision,
        engine.fonts.clone(),
    )
    .await
    .map_err(Failure::Core))
}

/// То же, но недостающие внешние шаблоны доустанавливаются (фича 007, FR-001).
///
/// Отдельная команда, а не флаг у `preview_compile`: этот путь может **писать**
/// в vault и обращаться в сеть, и разница обязана быть видна на границе, а не
/// прятаться в параметре. Заметка без подключений проходит здесь так же, как
/// через `preview_compile`, и в сеть не заходит (FR-008).
///
/// Правила загрузки приходят настройками уровня vault: решение «идти ли в
/// сеть» принимает ядро по ним, а не окно (принцип I).
#[tauri::command]
async fn preview_compile_with_templates(
    state: State<'_, Vault>,
    engine: State<'_, Preview>,
    note_path: String,
    text: String,
    revision: u64,
    overrides: VaultOverrides,
) -> Answer<CompiledDocument, Failure<SnapshotError>> {
    with_vault!(state, |backend| preview::compile_note_with_templates(
        backend,
        &engine.packages,
        &note_path,
        &text,
        revision,
        engine.fonts.clone(),
        &overrides.template_policy(),
    )
    .await
    .map_err(Failure::Core))
}

/// Установить шаблон по явному запросу пользователя (фича 007, FR-010).
///
/// Отдельно от сборки: выключенная автозагрузка запрещает приложению ходить в
/// сеть **само**, но не отнимает у пользователя возможность попросить об этом.
#[tauri::command]
async fn preview_install_template(
    state: State<'_, Vault>,
    engine: State<'_, Preview>,
    spec: String,
    overrides: VaultOverrides,
) -> Answer<preview::packages::InstallReport, Failure<preview::packages::InstallError>> {
    let result = with_vault!(state, |backend| preview::install_template(
        backend,
        &spec,
        &overrides.template_policy(),
    )
    .await
    .map_err(Failure::Core));

    // Ту же версию могли поставить поверх: кэш и отметка о сверке отпечатка
    // относятся к прежнему содержимому.
    if let Some(parsed) = preview::packages::parse_spec(&spec) {
        engine.packages.forget(&parsed);
        engine.packages.forget_verified(&parsed);
    }
    result
}

/// Установленные шаблоны — для перечня в настройках папки (фича 007, US5).
#[tauri::command]
async fn preview_installed_templates(
    state: State<'_, Vault>,
) -> Answer<Vec<preview::packages::InstalledTemplate>, Failure<()>> {
    with_vault!(state, |backend| Ok(preview::packages::installed_templates(
        backend
    )
    .await))
}

// ── Экспорт в PDF (фича 009) ────────────────────────────────────────────────

/// Экспортирует заметку в PDF (contracts/core-export-api.md).
///
/// Тот же движок, что у предпросмотра: шрифты и кэш шаблонов общие, иначе
/// заметка собиралась бы в одном месте и не собиралась в другом. Команда
/// асинхронная и идёт в пуле tokio, а не в потоке окна: пакет не блокирует
/// интерфейс (принцип X).
#[tauri::command]
async fn export_pdf(
    state: State<'_, Vault>,
    engine: State<'_, Preview>,
    note_path: String,
    text: Option<String>,
    overrides: VaultOverrides,
) -> Answer<export::ExportReport, Failure<()>> {
    with_vault!(state, |backend| Ok(export::export_note_pdf(
        backend,
        &engine.packages,
        &note_path,
        text.as_deref(),
        engine.fonts.clone(),
        &overrides,
    )
    .await))
}

#[tauri::command]
async fn export_candidates(
    state: State<'_, Vault>,
    overrides: VaultOverrides,
) -> Answer<Vec<export::ExportCandidate>, Failure<()>> {
    with_vault!(state, |backend| Ok(export::batch::candidates_of(
        backend, &overrides
    )
    .await))
}

// Переходы пакета — чистые функции ядра без состояния в оболочке
// (research.md §5): значение пакета держит окно.

#[tauri::command]
fn export_batch_start(
    paths: Vec<String>,
    stop_on_error: bool,
    overrides: VaultOverrides,
) -> export::BatchState {
    export::batch::start(paths, stop_on_error, &overrides)
}

#[tauri::command]
fn export_batch_next(state: export::BatchState) -> export::BatchStep {
    export::batch::step(state)
}

#[tauri::command]
fn export_batch_record(
    state: export::BatchState,
    report: export::ExportReport,
    elapsed_ms: u32,
) -> export::BatchState {
    export::batch::record(state, report, elapsed_ms)
}

#[tauri::command]
fn export_batch_skip(state: export::BatchState, note_path: String) -> export::BatchState {
    export::batch::skip(state, &note_path)
}

#[tauri::command]
fn export_batch_cancel(state: export::BatchState) -> export::BatchState {
    export::batch::cancel(state)
}

#[tauri::command]
fn export_batch_view(state: export::BatchState) -> export::BatchView {
    export::batch::view(state)
}

#[tauri::command]
async fn check_template_name(
    state: State<'_, Vault>,
    name: String,
) -> Answer<NameCheck, Failure<()>> {
    with_vault!(state, |backend| Ok(
        templates::check_name(backend, &name).await
    ))
}

#[tauri::command]
async fn save_as_template(
    state: State<'_, Vault>,
    name: String,
    content: String,
    confirm_overwrite: bool,
) -> Answer<templates::Template, Failure<SaveTemplateError>> {
    with_vault!(state, |backend| templates::save_as_template(
        backend,
        &name,
        &content,
        confirm_overwrite
    )
    .await
    .map_err(Failure::Core))
}

/// Какую правку совершить над текстом, чтобы вставить заготовку.
///
/// Открытого vault не требует: правило вставки текста от папки не зависит.
#[tauri::command]
fn plan_insertion(
    text_length: usize,
    point: templates::InsertionPoint,
    content: String,
) -> templates::InsertionEdit {
    templates::plan_insertion(text_length, &point, &content)
}

// ── Параметры элементов (фича 008, contracts/element-params.md) ─────────
//
// Открытого vault не требуют: функции чистые. Те же формы запроса и ответа,
// что на wasm-границе (FR-022).

/// Схема элемента по ключу (`call:figure`) — для правки; `None` — параметров
/// нет.
#[tauri::command]
fn element_schema(id: String) -> Option<typsidian_core::elements::ElementSpec> {
    typsidian_core::elements::element_schema(&id)
}

/// Схема элемента, который вставляет шаблон каталога; `None` — вставляется
/// сразу (FR-003).
#[tauri::command]
fn element_schema_for(template: String) -> Option<typsidian_core::elements::ElementSpec> {
    typsidian_core::elements::element_schema_for(&template)
}

/// Элемент под кареткой (FR-006, FR-016).
#[tauri::command]
fn element_at(text: String, offset: usize) -> typsidian_core::elements::ElementAtResult {
    typsidian_core::elements::element_at(&text, offset)
}

/// «Будет вставлено» (FR-004).
#[tauri::command]
fn render_element(
    request: typsidian_core::elements::RenderRequest,
) -> typsidian_core::elements::RenderOutcome {
    typsidian_core::elements::render(&request)
}

/// Точечные замены для правки на месте (FR-008).
#[tauri::command]
fn plan_element_edit(
    request: typsidian_core::elements::EditRequest,
) -> typsidian_core::elements::EditOutcome {
    typsidian_core::elements::plan(&request)
}

/// Затрагивает ли событие файла библиотеку заготовок.
#[tauri::command]
fn template_event(event: VaultEvent) -> Option<templates::TemplateEvent> {
    templates::library_event(&event)
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
        .manage(Device::default())
        .manage(TemplatePreviews::default())
        .manage(Preview::default())
        .invoke_handler(tauri::generate_handler![
            open_vault,
            browse_root,
            browse_dir,
            browse_create_dir,
            remember_vault,
            last_vault,
            device_settings,
            device_settings_events,
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
            list,
            read,
            write,
            create,
            rename,
            delete,
            backlinks,
            broken_links,
            access_state,
            search,
            file_kind,
            begin_edit,
            pending_conflict,
            unresolved_edit_paths,
            conflict_diff,
            resolve_conflict_keep_local,
            resolve_conflict_accept_external,
            root_path,
            board_write_profile,
            create_board,
            read_board,
            write_board,
            write_snapshot,
            embeds_of,
            embeds_in_text,
            embeds_changed,
            plan_embed,
            embed_markup,
            vault_settings,
            set_vault_settings,
            mark_section,
            unmark_section,
            keybindings,
            set_keybinding,
            reset_keybinding,
            template_library,
            read_template,
            template_preview,
            check_template_name,
            save_as_template,
            plan_insertion,
            element_schema,
            element_schema_for,
            element_at,
            render_element,
            plan_element_edit,
            template_event,
            preview_compile,
            preview_compile_with_templates,
            preview_install_template,
            preview_installed_templates,
            export_pdf,
            export_candidates,
            export_batch_start,
            export_batch_next,
            export_batch_record,
            export_batch_skip,
            export_batch_cancel,
            export_batch_view,
            sync::sync_open,
            sync::sync_capabilities,
            sync::sync_overview,
            sync::sync_poll_events,
            sync::sync_note_change,
            sync::sync_cancel,
            sync::sync_ignore_reports,
            sync::sync_apply_ignore,
            sync::sync_decline_stignore,
            sync::sync_enable_history,
            sync::sync_changes,
            sync::sync_snapshot,
            sync::sync_history,
            sync::sync_diff,
            sync::sync_file_at,
            sync::sync_restore,
            sync::sync_connect_remote,
            sync::sync_disconnect_remote,
            sync::sync_push,
            sync::sync_pull,
            sync::sync_abort_pull,
            sync::sync_conflicts,
            sync::sync_resolve,
            sync::sync_set_keeper,
            sync::sync_clear_keeper,
            sync::sync_update_settings,
            sync::sync_refresh_mirror,
            sync::sync_syncthing_discover,
            sync::sync_syncthing_connect,
            sync::sync_syncthing_disconnect,
            sync::sync_tick,
            sync::sync_clone_vault,
        ])
}
