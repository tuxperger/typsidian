//! Команды синхронизации (фича 011, T018, T023, T032, T045, T054, T062).
//!
//! Тонкие обёртки над `typsidian_core::sync::SyncSession` — тот же контракт,
//! что у `WebSyncSession` в браузере (contracts/core-sync-api.md). Решений
//! здесь нет: команда собирает контекст из настроек устройства, вызывает
//! сессию и сохраняет изменённые значения устройства, если сессия их вернула.
//!
//! Порядок блокировок один на все команды — устройство, затем vault, затем
//! сессия, — и устройство отпускается до обращения к vault: иначе две команды,
//! взявшие их в разном порядке, ждали бы друг друга вечно.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use typsidian_core::settings::apply::ApplyContext;
use typsidian_core::settings::store::SettingsStore;
use typsidian_core::settings::VaultId;
use typsidian_core::sync::engine::native::NativeGitEngine;
use typsidian_core::sync::engine::CancelToken;
use typsidian_core::sync::ignore::{
    self, IgnoreCheck, IgnoreDocument, IgnoreFile, IgnoreReport, IgnoreSaveOutcome,
    IgnoreSaveRequest,
};
#[cfg(not(target_os = "android"))]
use typsidian_core::sync::secrets::MemorySecretStore;
use typsidian_core::sync::secrets::{entry_name, SecretKind, SecretStore, SecretStoreError};
use typsidian_core::sync::session::{
    self as sync_session, Conflict, Credentials, PullOutcome, Resolution, TickOutcome,
};
use typsidian_core::sync::settings::VaultSyncSettingsPatch;
use typsidian_core::sync::syncthing::{self, SyncthingEndpoint};
use typsidian_core::sync::{
    self, Change, Content, FileDiff, HistoryPage, Keeper, RemoteStatus, Revision,
    ShellCapabilities, Snapshot, SyncContext, SyncError, SyncEvent, SyncOverview, SyncSession,
    VaultSyncSettings,
};
use typsidian_core::vault::backend::VaultBackend;

use super::{device_store, Device, Vault};

/// Сессия синхронизации открытого vault.
#[derive(Default)]
pub struct SyncState(Mutex<Option<OpenSync>>);

pub struct OpenSync {
    session: SyncSession<NativeGitEngine>,
    vault: VaultId,
}

/// Токен отмены текущей операции — отдельно от сессии: пока операция идёт,
/// сессия заблокирована, а отменить её нужно именно тогда.
#[derive(Default)]
pub struct SyncCancel(std::sync::Mutex<Option<CancelToken>>);

/// Ход массовой операции открытой сессии (T075) — отдельно от сессии по той же
/// причине, что токен отмены: читается, пока операция идёт.
#[derive(Default)]
pub struct SyncProgressSlot(std::sync::Mutex<Option<sync::ProgressSink>>);

/// Отказ команды синхронизации.
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum SyncFailure {
    NotOpen(NotOpen),
    Core(SyncError),
}

#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind")]
pub enum NotOpen {
    NoVaultOpen,
}

impl From<SyncError> for SyncFailure {
    fn from(error: SyncError) -> Self {
        SyncFailure::Core(error)
    }
}

type Answer<T> = Result<T, SyncFailure>;

fn not_open() -> SyncFailure {
    SyncFailure::NotOpen(NotOpen::NoVaultOpen)
}

fn capabilities() -> ShellCapabilities {
    ShellCapabilities::current(cfg!(target_os = "android"))
}

/// Идентификатор vault приходит из окна и идёт в имя каталога зеркала и в имя
/// записи секрета — поэтому только в том виде, в каком его выдаёт реестр
/// ядра (`VaultId::mint`): шестнадцатеричная строка без разделителей (T068).
fn require_vault_id(vault_id: &str) -> Result<(), SyncFailure> {
    let well_formed =
        (1..=64).contains(&vault_id.len()) && vault_id.bytes().all(|b| b.is_ascii_hexdigit());
    if well_formed {
        Ok(())
    } else {
        Err(SyncError::Io {
            message: "неизвестный vault".to_string(),
        }
        .into())
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

// ── Хранилище секретов устройства (research §10) ───────────────────────────

/// Секреты синхронизации этого устройства: keyring ОС на рабочем столе,
/// приватный файл приложения на Android. Если keyring недоступен (нет службы
/// секретов в сеансе), секрет живёт в памяти до закрытия приложения — не на
/// диске и не в vault.
pub enum ShellSecrets {
    #[cfg(not(target_os = "android"))]
    Keyring(MemorySecretStore),
    #[cfg(target_os = "android")]
    File(PathBuf, tokio::sync::Mutex<()>),
}

impl ShellSecrets {
    fn for_app(app: &AppHandle) -> Self {
        #[cfg(target_os = "android")]
        {
            let dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            ShellSecrets::File(dir.join("sync-secrets.json"), tokio::sync::Mutex::new(()))
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = app;
            ShellSecrets::Keyring(MemorySecretStore::new())
        }
    }
}

#[cfg(not(target_os = "android"))]
const KEYRING_SERVICE: &str = "typsidian";

#[cfg(not(target_os = "android"))]
async fn keyring_call<T: Send + 'static>(
    name: String,
    action: impl FnOnce(keyring::Entry) -> keyring::Result<T> + Send + 'static,
) -> Result<T, SecretStoreError> {
    tokio::task::spawn_blocking(move || {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &name).map_err(|_| SecretStoreError)?;
        action(entry).map_err(|_| SecretStoreError)
    })
    .await
    .map_err(|_| SecretStoreError)?
}

/// Файл секретов: нет файла — пусто; файл есть, но не читается или не
/// разбирается — отказ, а не «пусто» (T077): иначе следующая запись затёрла
/// бы его вместе со всеми секретами.
#[cfg(target_os = "android")]
fn read_file_secrets(
    path: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, String>, SecretStoreError> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|_| SecretStoreError),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
        Err(_) => Err(SecretStoreError),
    }
}

/// Записи для изменения. Повреждённый файл откладывается рядом
/// (`sync-secrets.corrupt.json`), а не перезаписывается: секреты в нём,
/// возможно, ещё можно достать, а новые пишутся в чистый файл.
#[cfg(target_os = "android")]
fn entries_for_change(
    path: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, String>, SecretStoreError> {
    match read_file_secrets(path) {
        Ok(entries) => Ok(entries),
        Err(_) if path.exists() => {
            std::fs::rename(path, path.with_extension("corrupt.json"))
                .map_err(|_| SecretStoreError)?;
            Ok(Default::default())
        }
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "android")]
fn write_file_secrets(
    path: &std::path::Path,
    entries: &std::collections::BTreeMap<String, String>,
) -> Result<(), SecretStoreError> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| SecretStoreError)?;
    }
    let text = serde_json::to_string(entries).map_err(|_| SecretStoreError)?;
    let temporary = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|_| SecretStoreError)?;
    file.write_all(text.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|_| SecretStoreError)?;
    std::fs::rename(&temporary, path).map_err(|_| SecretStoreError)
}

impl SecretStore for ShellSecrets {
    async fn get(
        &self,
        vault_id: &str,
        kind: SecretKind,
    ) -> Result<Option<String>, SecretStoreError> {
        let name = entry_name(vault_id, kind);
        match self {
            #[cfg(not(target_os = "android"))]
            ShellSecrets::Keyring(fallback) => {
                if let Some(value) = fallback.get(vault_id, kind).await? {
                    return Ok(Some(value));
                }
                keyring_call(name, |entry| match entry.get_password() {
                    Ok(value) => Ok(Some(value)),
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(error) => Err(error),
                })
                .await
                .or(Ok(None))
            }
            #[cfg(target_os = "android")]
            ShellSecrets::File(path, lock) => {
                let _guard = lock.lock().await;
                Ok(read_file_secrets(path)?.get(&name).cloned())
            }
        }
    }

    async fn set(
        &self,
        vault_id: &str,
        kind: SecretKind,
        value: &str,
    ) -> Result<(), SecretStoreError> {
        let name = entry_name(vault_id, kind);
        match self {
            #[cfg(not(target_os = "android"))]
            ShellSecrets::Keyring(fallback) => {
                let owned = value.to_string();
                match keyring_call(name, move |entry| entry.set_password(&owned)).await {
                    Ok(()) => {
                        let _ = fallback.delete(vault_id, kind).await;
                        Ok(())
                    }
                    Err(_) => fallback.set(vault_id, kind, value).await,
                }
            }
            #[cfg(target_os = "android")]
            ShellSecrets::File(path, lock) => {
                // Прочитать-изменить-записать под одной блокировкой: две
                // записи подряд иначе теряли бы одна другую.
                let _guard = lock.lock().await;
                let mut entries = entries_for_change(path)?;
                entries.insert(name, value.to_string());
                write_file_secrets(path, &entries)
            }
        }
    }

    async fn delete(&self, vault_id: &str, kind: SecretKind) -> Result<(), SecretStoreError> {
        let name = entry_name(vault_id, kind);
        match self {
            #[cfg(not(target_os = "android"))]
            ShellSecrets::Keyring(fallback) => {
                let _ = fallback.delete(vault_id, kind).await;
                // Отказ keyring — отказ удаления (T077): секрет остался в
                // хранилище ОС, и называть его удалённым нельзя.
                keyring_call(name, |entry| match entry.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(error) => Err(error),
                })
                .await
            }
            #[cfg(target_os = "android")]
            ShellSecrets::File(path, lock) => {
                let _guard = lock.lock().await;
                let mut entries = entries_for_change(path)?;
                if entries.remove(&name).is_some() {
                    write_file_secrets(path, &entries)?;
                }
                Ok(())
            }
        }
    }
}

/// Хранилище секретов окна — одно на всё время работы приложения.
pub struct Secrets(pub ShellSecrets);

// ── Контекст и значения устройства ─────────────────────────────────────────

async fn context(
    app: &AppHandle,
    device: &State<'_, Device>,
    vault: &VaultId,
) -> Result<SyncContext, SyncFailure> {
    let mut guard = device.0.lock().await;
    let store = device_store(app, &mut guard)
        .await
        .map_err(|_| SyncError::Io {
            message: "настройки устройства недоступны".to_string(),
        })?;
    // Тождество устройства выдаётся при первом обращении (research §4).
    if let Some(change) = sync::ensure_device_change(SettingsStore::settings(store)) {
        store.apply(&change, &ApplyContext::unobstructed()).await;
    }
    Ok(sync::context_for(
        SettingsStore::settings(store),
        vault,
        now(),
        capabilities(),
    ))
}

/// Сохраняет значения устройства, изменённые сессией.
async fn persist_device_update(
    app: &AppHandle,
    device: &State<'_, Device>,
    sync_state: &State<'_, SyncState>,
) {
    let (update, vault) = {
        let mut guard = sync_state.0.lock().await;
        let Some(open) = guard.as_mut() else {
            return;
        };
        (open.session.take_device_update(), open.vault.clone())
    };
    let Some(update) = update else {
        return;
    };
    let mut guard = device.0.lock().await;
    if let Ok(store) = device_store(app, &mut guard).await {
        store
            .apply(
                &sync::device_sync_change(vault, update),
                &ApplyContext::unobstructed(),
            )
            .await;
    }
}

/// Тело команды над открытыми vault и сессией: контекст, вызов, сохранение
/// значений устройства.
macro_rules! with_sync {
    ($app:expr, $device:expr, $vault:expr, $sync:expr, |$backend:ident, $session:ident, $ctx:ident| $body:expr) => {{
        let vault_id = {
            let guard = $sync.0.lock().await;
            guard.as_ref().ok_or_else(not_open)?.vault.clone()
        };
        #[allow(unused_variables)]
        let $ctx = context(&$app, &$device, &vault_id).await?;
        let result = {
            let mut vault_guard = $vault.0.lock().await;
            let $backend = vault_guard.as_mut().ok_or_else(not_open)?;
            let mut sync_guard = $sync.0.lock().await;
            let open = sync_guard.as_mut().ok_or_else(not_open)?;
            let $session = &mut open.session;
            $body
        };
        persist_device_update(&$app, &$device, &$sync).await;
        result
    }};
}

/// Шаг под обеими блокировками — тот же порядок, что у `with_sync!`: vault,
/// затем сессия. Сетевой шаг обмена идёт между такими шагами без блокировок
/// (T071): сохранение заметок и опрос состояния не ждут сети.
macro_rules! locked {
    ($vault:expr, $sync:expr, |$backend:ident, $session:ident| $body:expr) => {{
        let mut vault_guard = $vault.0.lock().await;
        #[allow(unused_variables)]
        let $backend = vault_guard.as_mut().ok_or_else(not_open)?;
        let mut sync_guard = $sync.0.lock().await;
        let open = sync_guard.as_mut().ok_or_else(not_open)?;
        let $session = &mut open.session;
        $body
    }};
}

/// Контекст команды по идентификатору открытого vault.
async fn sync_context(
    app: &AppHandle,
    device: &State<'_, Device>,
    sync_state: &State<'_, SyncState>,
) -> Answer<SyncContext> {
    let vault_id = {
        let guard = sync_state.0.lock().await;
        guard.as_ref().ok_or_else(not_open)?.vault.clone()
    };
    context(app, device, &vault_id).await
}

// ── Команды ────────────────────────────────────────────────────────────────

/// Открывает сессию синхронизации для только что открытого vault.
#[tauri::command]
pub async fn sync_open(
    app: AppHandle,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    cancel: State<'_, SyncCancel>,
    progress: State<'_, SyncProgressSlot>,
    vault_id: String,
) -> Answer<()> {
    require_vault_id(&vault_id)?;
    let root = {
        let guard = vault.0.lock().await;
        guard
            .as_ref()
            .ok_or_else(not_open)?
            .root_path()
            .to_path_buf()
    };
    let mirror_dir = app
        .path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("mirrors").join(format!("{vault_id}.git")));
    let session = SyncSession::new(
        NativeGitEngine::open(root),
        mirror_dir.map(NativeGitEngine::mirror),
        vault_id.clone(),
    );
    if let Ok(mut slot) = cancel.0.lock() {
        *slot = Some(session.cancel_token());
    }
    if let Ok(mut slot) = progress.0.lock() {
        *slot = Some(session.progress_sink());
    }
    *sync_state.0.lock().await = Some(OpenSync {
        session,
        vault: VaultId::from_raw(vault_id),
    });
    restart_syncthing_watcher(&app);
    Ok(())
}

/// Ход идущей массовой операции — без блокировки сессии (T075, принцип X).
#[tauri::command]
pub fn sync_progress(progress: State<'_, SyncProgressSlot>) -> Option<sync::SyncProgress> {
    progress
        .0
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().and_then(sync::ProgressSink::current))
}

#[tauri::command]
pub fn sync_capabilities() -> ShellCapabilities {
    capabilities()
}

#[tauri::command]
pub async fn sync_overview(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> Answer<SyncOverview> {
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| Ok(
        session.overview(backend, &ctx).await
    ))
}

#[tauri::command]
pub async fn sync_poll_events(sync_state: State<'_, SyncState>) -> Answer<Vec<SyncEvent>> {
    let mut guard = sync_state.0.lock().await;
    Ok(guard
        .as_mut()
        .map(|open| open.session.poll_events())
        .unwrap_or_default())
}

#[tauri::command]
pub async fn sync_note_change(sync_state: State<'_, SyncState>) -> Answer<()> {
    if let Some(open) = sync_state.0.lock().await.as_mut() {
        open.session.note_change(now());
    }
    Ok(())
}

/// Отмена текущей сетевой или массовой операции (принцип X). Не ждёт
/// блокировки сессии: токен общий у сессии и движка.
#[tauri::command]
pub fn sync_cancel(cancel: State<'_, SyncCancel>) {
    if let Ok(slot) = cancel.0.lock() {
        if let Some(token) = slot.as_ref() {
            token.cancel();
        }
    }
}

#[tauri::command]
pub async fn sync_ignore_reports(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> Answer<Vec<IgnoreReport>> {
    let vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_ref().ok_or_else(not_open)?;
    let sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_ref().ok_or_else(not_open)?;
    Ok(open.session.ignore_reports(backend).await)
}

#[tauri::command]
pub async fn sync_apply_ignore(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    file: IgnoreFile,
) -> Answer<()> {
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| {
        session.apply_ignore(backend, &ctx, file).await?;
        Ok(())
    })
}

#[tauri::command]
pub async fn sync_decline_stignore(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> Answer<()> {
    with_sync!(app, device, vault, sync_state, |_backend, session, ctx| {
        session.decline_stignore(&ctx);
        Ok(())
    })
}

// ── Редактор правил исключения (фича 012) ──────────────────────────────────

#[tauri::command]
pub async fn sync_ignore_open(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    file: IgnoreFile,
) -> Answer<IgnoreDocument> {
    let vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_ref().ok_or_else(not_open)?;
    let sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_ref().ok_or_else(not_open)?;
    Ok(open.session.ignore_open(backend, file).await)
}

/// Чистая проверка текста: ни vault, ни сессия не нужны, поэтому набор в
/// редакторе не ждёт идущего обмена.
#[tauri::command]
pub fn sync_ignore_check(file: IgnoreFile, text: String) -> IgnoreCheck {
    ignore::check(file, &text)
}

#[tauri::command]
pub fn sync_ignore_restore(file: IgnoreFile, text: String) -> String {
    ignore::restore_block(file, &text)
}

#[tauri::command]
pub async fn sync_ignore_save(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    request: IgnoreSaveRequest,
) -> Answer<IgnoreSaveOutcome> {
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| {
        Ok(session.ignore_save(backend, &ctx, request).await?)
    })
}

#[tauri::command]
pub async fn sync_enable_history(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    message: String,
) -> Answer<Snapshot> {
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| Ok(
        session.enable_history(backend, &ctx, &message).await?
    ))
}

#[tauri::command]
pub async fn sync_changes(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> Answer<Vec<Change>> {
    let vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_ref().ok_or_else(not_open)?;
    let sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_ref().ok_or_else(not_open)?;
    Ok(open.session.changes(backend).await?)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn sync_snapshot(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    paths: Option<Vec<String>>,
    message: String,
    auto_prefix: String,
    flushed: bool,
    confirm_not_keeper: bool,
) -> Answer<Snapshot> {
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| Ok(
        session
            .snapshot(
                backend,
                &ctx,
                paths,
                &message,
                &auto_prefix,
                flushed,
                confirm_not_keeper
            )
            .await?
    ))
}

#[tauri::command]
pub async fn sync_history(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    path: Option<String>,
    cursor: Option<String>,
    limit: u32,
) -> Answer<HistoryPage> {
    let vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_ref().ok_or_else(not_open)?;
    let sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_ref().ok_or_else(not_open)?;
    Ok(open
        .session
        .history(backend, path.as_deref(), cursor.as_deref(), limit)
        .await?)
}

#[tauri::command]
pub async fn sync_diff(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    path: String,
    from: Revision,
    to: Revision,
) -> Answer<FileDiff> {
    let vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_ref().ok_or_else(not_open)?;
    let sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_ref().ok_or_else(not_open)?;
    Ok(open.session.diff(backend, &path, &from, &to).await?)
}

#[tauri::command]
pub async fn sync_snapshot_changes(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    snapshot: String,
) -> Answer<Vec<Change>> {
    let vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_ref().ok_or_else(not_open)?;
    let sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_ref().ok_or_else(not_open)?;
    Ok(open.session.snapshot_changes(backend, &snapshot).await?)
}

#[tauri::command]
pub async fn sync_file_at(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    path: String,
    revision: Revision,
) -> Answer<Content> {
    let vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_ref().ok_or_else(not_open)?;
    let sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_ref().ok_or_else(not_open)?;
    Ok(open.session.file_at(backend, &path, &revision).await?)
}

#[tauri::command]
pub async fn sync_restore(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    path: String,
    snapshot: String,
) -> Answer<Content> {
    let mut vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_mut().ok_or_else(not_open)?;
    let mut sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_mut().ok_or_else(not_open)?;
    Ok(open.session.restore(backend, &path, &snapshot).await?)
}

#[tauri::command]
pub async fn sync_connect_remote(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    secrets: State<'_, Secrets>,
    url: String,
    credentials: Credentials,
) -> Answer<RemoteStatus> {
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| Ok(
        session
            .connect_remote(backend, &secrets.0, &ctx, &url, credentials)
            .await?
    ))
}

#[tauri::command]
pub async fn sync_disconnect_remote(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    secrets: State<'_, Secrets>,
) -> Answer<()> {
    let mut vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_mut().ok_or_else(not_open)?;
    let mut sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_mut().ok_or_else(not_open)?;
    Ok(open.session.disconnect_remote(backend, &secrets.0).await?)
}

#[tauri::command]
pub async fn sync_push(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    secrets: State<'_, Secrets>,
    confirm_not_keeper: bool,
) -> Answer<RemoteStatus> {
    let ctx = sync_context(&app, &device, &sync_state).await?;
    let (plan, engine) = locked!(vault, sync_state, |backend, session| (
        session
            .begin_push(backend, &secrets.0, &ctx, confirm_not_keeper)
            .await?,
        session.engine().clone()
    ));
    let sent = plan.push(&engine).await;
    let result = locked!(vault, sync_state, |backend, session| session
        .complete_push(backend, &ctx, &plan, sent)
        .await);
    persist_device_update(&app, &device, &sync_state).await;
    Ok(result?)
}

#[tauri::command]
pub async fn sync_pull(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    secrets: State<'_, Secrets>,
    confirm_not_keeper: bool,
) -> Answer<PullOutcome> {
    let ctx = sync_context(&app, &device, &sync_state).await?;
    let (plan, engine) = locked!(vault, sync_state, |backend, session| (
        session
            .begin_pull(backend, &secrets.0, &ctx, confirm_not_keeper)
            .await?,
        session.engine().clone()
    ));
    let fetched = plan.exchange.fetch(&engine).await;
    let result = locked!(vault, sync_state, |backend, session| session
        .complete_pull(backend, &ctx, plan, fetched)
        .await);
    persist_device_update(&app, &device, &sync_state).await;
    Ok(result?)
}

#[tauri::command]
pub async fn sync_restore_history(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    secrets: State<'_, Secrets>,
    confirm_not_keeper: bool,
) -> Answer<PullOutcome> {
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| Ok(
        session
            .restore_history(backend, &secrets.0, &ctx, confirm_not_keeper)
            .await?
    ))
}

#[tauri::command]
pub async fn sync_abort_pull(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> Answer<()> {
    let mut vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_mut().ok_or_else(not_open)?;
    let mut sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_mut().ok_or_else(not_open)?;
    Ok(open.session.abort_pull(backend).await?)
}

#[tauri::command]
pub async fn sync_conflicts(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> Answer<Vec<Conflict>> {
    let vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_ref().ok_or_else(not_open)?;
    let sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_ref().ok_or_else(not_open)?;
    Ok(open.session.conflicts(backend).await)
}

#[tauri::command]
pub async fn sync_resolve(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    id: String,
    resolution: Resolution,
) -> Answer<()> {
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| Ok(
        session.resolve(backend, &ctx, &id, resolution).await?
    ))
}

#[tauri::command]
pub async fn sync_set_keeper(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> Answer<Keeper> {
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| Ok(
        session.set_keeper(backend, &ctx).await?
    ))
}

#[tauri::command]
pub async fn sync_clear_keeper(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> Answer<()> {
    let mut vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_mut().ok_or_else(not_open)?;
    let mut sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_mut().ok_or_else(not_open)?;
    Ok(open.session.clear_keeper(backend).await?)
}

#[tauri::command]
pub async fn sync_update_settings(
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    patch: VaultSyncSettingsPatch,
) -> Answer<VaultSyncSettings> {
    let mut vault_guard = vault.0.lock().await;
    let backend = vault_guard.as_mut().ok_or_else(not_open)?;
    let mut sync_guard = sync_state.0.lock().await;
    let open = sync_guard.as_mut().ok_or_else(not_open)?;
    Ok(open.session.update_settings(backend, &patch).await?)
}

#[tauri::command]
pub async fn sync_refresh_mirror(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    secrets: State<'_, Secrets>,
) -> Answer<()> {
    let ctx = sync_context(&app, &device, &sync_state).await?;
    let (plan, mirror) = locked!(vault, sync_state, |backend, session| (
        session.begin_mirror(backend, &secrets.0, &ctx).await?,
        session.mirror_engine().cloned()
    ));
    let fetched = match &mirror {
        Some(mirror) => plan.mirror(mirror).await,
        None => Err(SyncError::NoHistory),
    };
    let result = locked!(vault, sync_state, |backend, session| session
        .complete_mirror(&ctx, fetched));
    persist_device_update(&app, &device, &sync_state).await;
    Ok(result?)
}

/// Параметры локального Syncthing, найденные в его конфигурации (FR-022a).
///
/// Только адрес. Ключ API окну не отдаётся: он даёт полный доступ к Syncthing,
/// а окно показывает содержимое заметок и внешних шаблонов (T068). При
/// подтверждении [`sync_syncthing_connect`] перечитывает ключ сам.
#[derive(Debug, serde::Serialize)]
pub struct DiscoveredSyncthing {
    endpoint: SyncthingEndpoint,
}

#[tauri::command]
pub fn sync_syncthing_discover() -> Option<DiscoveredSyncthing> {
    syncthing::discover().map(|(endpoint, _)| DiscoveredSyncthing { endpoint })
}

#[tauri::command]
pub async fn sync_syncthing_connect(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    secrets: State<'_, Secrets>,
    endpoint: SyncthingEndpoint,
    api_key: Option<String>,
) -> Answer<()> {
    // Без ключа — подключение к найденному Syncthing: ключ берётся из его же
    // конфигурации, и только если адрес в ней тот, что подтвердил пользователь.
    let api_key = match api_key {
        Some(key) => key,
        None => syncthing::discover()
            .filter(|(found, _)| found.address == endpoint.address && found.tls == endpoint.tls)
            .map(|(_, key)| key)
            .ok_or_else(|| SyncError::Io {
                message: "ключ Syncthing не найден".to_string(),
            })?,
    };
    let result = with_sync!(app, device, vault, sync_state, |_backend, session, ctx| Ok(
        session
            .syncthing_connect(&secrets.0, &ctx, endpoint, &api_key)
            .await?
    ));
    restart_syncthing_watcher(&app);
    result
}

#[tauri::command]
pub async fn sync_syncthing_disconnect(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    secrets: State<'_, Secrets>,
) -> Answer<()> {
    with_sync!(app, device, vault, sync_state, |_backend, session, ctx| {
        session.syncthing_disconnect(&secrets.0, &ctx).await;
        Ok(())
    })
}

// ── Наблюдатель Syncthing (T078, FR-022, research §8) ───────────────────────

/// Фоновое ожидание событий Syncthing открытой сессии: одна задача на окно.
#[derive(Default)]
pub struct SyncthingWatcher(std::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>);

/// Запасной опрос, если ждать событий не выходит (Syncthing не запущен, ключа
/// ещё нет, сертификат не закреплён).
const SYNCTHING_RETRY: std::time::Duration = std::time::Duration::from_secs(10);

/// Событие окну: состояние синхронизации изменилось само.
pub const SYNC_CHANGED_EVENT: &str = "sync-changed";

/// Запускает наблюдателя заново: после открытия сессии и после подключения
/// Syncthing. Прежний останавливается.
fn restart_syncthing_watcher(app: &AppHandle) {
    let watcher = app.state::<SyncthingWatcher>();
    let Ok(mut slot) = watcher.0.lock() else {
        return;
    };
    if let Some(previous) = slot.take() {
        previous.abort();
    }
    let app = app.clone();
    *slot = Some(tauri::async_runtime::spawn(async move {
        let mut since = 0;
        loop {
            // Сессия закрыта — наблюдать нечего.
            let Ok(target) = refresh_syncthing(&app).await else {
                return;
            };
            match target {
                Some((endpoint, key)) => match syncthing::wait_events(&endpoint, &key, since).await
                {
                    Ok(next) => since = next,
                    Err(_) => tokio::time::sleep(SYNCTHING_RETRY).await,
                },
                None => tokio::time::sleep(SYNCTHING_RETRY).await,
            }
        }
    }));
}

/// Перечитывает состояние папки vault в Syncthing и сообщает окну, если оно
/// изменилось. Возвращает адрес и ключ, по которым ждать событий; `None` —
/// Syncthing не подключён или ключа нет. Отказ — сессия закрыта.
async fn refresh_syncthing(
    app: &AppHandle,
) -> Result<Option<(SyncthingEndpoint, String)>, SyncFailure> {
    let device = app.state::<Device>();
    let vault = app.state::<Vault>();
    let sync_state = app.state::<SyncState>();
    let secrets = app.state::<Secrets>();
    let ctx = sync_context(app, &device, &sync_state).await?;
    let Some(mut endpoint) = ctx.device_sync.syncthing.clone() else {
        return Ok(None);
    };
    // Первое обращение к Syncthing по TLS закрепляет его сертификат; ключ API
    // уходит только после этого (research §8).
    if endpoint.tls && endpoint.cert_fingerprint.is_none() {
        if let Some(fingerprint) = syncthing::pin(&endpoint).await {
            endpoint.cert_fingerprint = Some(fingerprint.clone());
            if let Some(open) = sync_state.0.lock().await.as_mut() {
                open.session.syncthing_pinned(&ctx, fingerprint);
            }
            persist_device_update(app, &device, &sync_state).await;
        }
    }
    let (root, detected) = {
        let guard = vault.0.lock().await;
        let backend = guard.as_ref().ok_or_else(not_open)?;
        let detected = matches!(
            backend
                .folder_access(std::path::Path::new(syncthing::FOLDER_MARKER))
                .await,
            typsidian_core::vault::FolderAccess::Present
                | typsidian_core::vault::FolderAccess::NotWritable { .. }
        );
        (backend.root_path().to_path_buf(), detected)
    };
    let key = {
        let guard = sync_state.0.lock().await;
        let open = guard.as_ref().ok_or_else(not_open)?;
        open.session.syncthing_api_key(&secrets.0).await
    };
    let Some(key) = key else {
        return Ok(None);
    };
    let folder = syncthing::query(&endpoint, &key, &root.to_string_lossy(), detected).await;
    let changed = {
        let mut guard = sync_state.0.lock().await;
        let open = guard.as_mut().ok_or_else(not_open)?;
        open.session.set_syncthing_state(folder)
    };
    if changed {
        // Окно перечитывает сводку сразу, не дожидаясь такта.
        let _ = app.emit(SYNC_CHANGED_EVENT, ());
    }
    Ok(Some((endpoint, key)))
}

/// Такт автоматики (FR-019, FR-031): обновляет состояние Syncthing и
/// спрашивает ядро, пора ли снимать и обмениваться. Таймер — у окна.
#[tauri::command]
pub async fn sync_tick(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    secrets: State<'_, Secrets>,
    auto_prefix: String,
) -> Answer<TickOutcome> {
    // Состояние Syncthing держит наблюдатель (T078): такт его не опрашивает.
    let ctx = sync_context(&app, &device, &sync_state).await?;
    // Такт по шагам (T071): сеть автополучения и автоотправки — без блокировок.
    let mut outcome = TickOutcome::idle(ctx.now);
    let pull = locked!(vault, sync_state, |backend, session| {
        if session.tick_pull_due(backend, &ctx).await {
            match session.begin_pull(backend, &secrets.0, &ctx, false).await {
                Ok(plan) => Some((plan, session.engine().clone())),
                Err(error) => {
                    session.tick_pulled(&mut outcome, Err(error));
                    None
                }
            }
        } else {
            None
        }
    });
    let fetched = match &pull {
        Some((plan, engine)) => Some(plan.exchange.fetch(engine).await),
        None => None,
    };
    let push = locked!(vault, sync_state, |backend, session| {
        if let (Some((plan, _)), Some(fetched)) = (pull, fetched) {
            let result = session.complete_pull(backend, &ctx, plan, fetched).await;
            session.tick_pulled(&mut outcome, result);
        }
        if session
            .tick_local(backend, &ctx, &auto_prefix, &mut outcome)
            .await
        {
            match session.begin_push(backend, &secrets.0, &ctx, false).await {
                Ok(plan) => Some((plan, session.engine().clone())),
                Err(error) => {
                    session.tick_pushed(&mut outcome, Err(error));
                    None
                }
            }
        } else {
            None
        }
    });
    let sent = match &push {
        Some((plan, engine)) => Some(plan.push(engine).await),
        None => None,
    };
    locked!(vault, sync_state, |backend, session| {
        if let (Some((plan, _)), Some(sent)) = (push, sent) {
            let result = session.complete_push(backend, &ctx, &plan, sent).await;
            session.tick_pushed(&mut outcome, result);
        }
        session.tick_finish(backend, &ctx, &mut outcome).await;
    });
    persist_device_update(&app, &device, &sync_state).await;
    Ok(outcome)
}

/// Клонирует удалённый репозиторий в пустую папку будущего vault (FR-013).
#[tauri::command]
pub async fn sync_clone_vault(
    secrets: State<'_, Secrets>,
    cancel: State<'_, SyncCancel>,
    path: String,
    vault_id: String,
    url: String,
    credentials: Credentials,
) -> Answer<()> {
    require_vault_id(&vault_id)?;
    let target = PathBuf::from(&path);
    let is_empty = std::fs::read_dir(&target)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(true);
    if !is_empty {
        return Err(SyncError::TargetNotEmpty.into());
    }
    // Сессии ещё нет — токен отмены кладётся в то же место, откуда его берёт
    // `sync_cancel`: клонирование отменяется той же командой (принцип X).
    let token = CancelToken::new();
    if let Ok(mut slot) = cancel.0.lock() {
        *slot = Some(token.clone());
    }
    Ok(sync_session::clone_vault(
        &NativeGitEngine::open(target),
        &secrets.0,
        &vault_id,
        &capabilities(),
        &url,
        credentials,
        None,
        &token,
    )
    .await?)
}

/// Регистрирует состояние синхронизации в приложении.
pub fn manage(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
        .manage(SyncState::default())
        .manage(SyncCancel::default())
        .manage(SyncProgressSlot::default())
        .manage(SyncthingWatcher::default())
        .setup(|app| {
            let secrets = ShellSecrets::for_app(app.handle());
            app.manage(Secrets(secrets));
            Ok(())
        })
}
