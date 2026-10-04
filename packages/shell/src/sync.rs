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

use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex;

use typsidian_core::settings::apply::ApplyContext;
use typsidian_core::settings::store::SettingsStore;
use typsidian_core::settings::VaultId;
use typsidian_core::sync::engine::native::NativeGitEngine;
use typsidian_core::sync::engine::CancelToken;
use typsidian_core::sync::ignore::{IgnoreFile, IgnoreReport};
use typsidian_core::sync::secrets::{
    entry_name, MemorySecretStore, SecretKind, SecretStore, SecretStoreError,
};
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
    File(PathBuf),
}

impl ShellSecrets {
    fn for_app(app: &AppHandle) -> Self {
        #[cfg(target_os = "android")]
        {
            let dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            ShellSecrets::File(dir.join("sync-secrets.json"))
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

#[cfg(target_os = "android")]
fn read_file_secrets(path: &std::path::Path) -> std::collections::BTreeMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
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
            ShellSecrets::File(path) => Ok(read_file_secrets(path).get(&name).cloned()),
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
            ShellSecrets::File(path) => {
                let mut entries = read_file_secrets(path);
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
                let _ = keyring_call(name, |entry| match entry.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(error) => Err(error),
                })
                .await;
                Ok(())
            }
            #[cfg(target_os = "android")]
            ShellSecrets::File(path) => {
                let mut entries = read_file_secrets(path);
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

// ── Команды ────────────────────────────────────────────────────────────────

/// Открывает сессию синхронизации для только что открытого vault.
#[tauri::command]
pub async fn sync_open(
    app: AppHandle,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    cancel: State<'_, SyncCancel>,
    vault_id: String,
) -> Answer<()> {
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
    *sync_state.0.lock().await = Some(OpenSync {
        session,
        vault: VaultId::from_raw(vault_id),
    });
    Ok(())
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
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| Ok(
        session
            .push(backend, &secrets.0, &ctx, confirm_not_keeper)
            .await?
    ))
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
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| Ok(
        session
            .pull(backend, &secrets.0, &ctx, confirm_not_keeper)
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
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| Ok(
        session.refresh_mirror(backend, &secrets.0, &ctx).await?
    ))
}

/// Параметры локального Syncthing, найденные в его конфигурации (FR-022a).
/// Ключ возвращается окну один раз — для подтверждения подключения
/// пользователем; сохраняется он только через [`sync_syncthing_connect`].
#[derive(Debug, serde::Serialize)]
pub struct DiscoveredSyncthing {
    endpoint: SyncthingEndpoint,
    api_key: String,
}

#[tauri::command]
pub fn sync_syncthing_discover() -> Option<DiscoveredSyncthing> {
    syncthing::discover().map(|(endpoint, api_key)| DiscoveredSyncthing { endpoint, api_key })
}

#[tauri::command]
pub async fn sync_syncthing_connect(
    app: AppHandle,
    device: State<'_, Device>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    secrets: State<'_, Secrets>,
    endpoint: SyncthingEndpoint,
    api_key: String,
) -> Answer<()> {
    with_sync!(app, device, vault, sync_state, |_backend, session, ctx| Ok(
        session
            .syncthing_connect(&secrets.0, &ctx, endpoint, &api_key)
            .await?
    ))
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
    // Состояние Syncthing читается до такта: автоснимок ждёт окончания
    // доставки, если она идёт (FR-031).
    let vault_id = {
        let guard = sync_state.0.lock().await;
        guard.as_ref().ok_or_else(not_open)?.vault.clone()
    };
    let ctx = context(&app, &device, &vault_id).await?;
    if let Some(endpoint) = ctx.device_sync.syncthing.clone() {
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
            match guard.as_ref() {
                Some(open) => open.session.syncthing_api_key(&secrets.0).await,
                None => None,
            }
        };
        if let Some(key) = key {
            let folder = syncthing::query(&endpoint, &key, &root.to_string_lossy(), detected).await;
            if let Some(open) = sync_state.0.lock().await.as_mut() {
                open.session.set_syncthing_state(folder);
            }
        }
    }
    with_sync!(app, device, vault, sync_state, |backend, session, ctx| {
        Ok(session.tick(backend, &secrets.0, &ctx, &auto_prefix).await)
    })
}

/// Клонирует удалённый репозиторий в пустую папку будущего vault (FR-013).
#[tauri::command]
pub async fn sync_clone_vault(
    secrets: State<'_, Secrets>,
    path: String,
    vault_id: String,
    url: String,
    credentials: Credentials,
) -> Answer<()> {
    let target = PathBuf::from(&path);
    let is_empty = std::fs::read_dir(&target)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(true);
    if !is_empty {
        return Err(SyncError::TargetNotEmpty.into());
    }
    Ok(sync_session::clone_vault(
        &NativeGitEngine::open(target),
        &secrets.0,
        &vault_id,
        &capabilities(),
        &url,
        credentials,
        &CancelToken::new(),
    )
    .await?)
}

/// Регистрирует состояние синхронизации в приложении.
pub fn manage(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
        .manage(SyncState::default())
        .manage(SyncCancel::default())
        .setup(|app| {
            let secrets = ShellSecrets::for_app(app.handle());
            app.manage(Secrets(secrets));
            Ok(())
        })
}
