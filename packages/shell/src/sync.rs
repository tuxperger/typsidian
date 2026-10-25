//! Синхронизация в оболочке (фича 011; с фичи 038 команды общие с сервером).
//!
//! Команды `sync_*` над открытой сессией живут в `typsidian_host`
//! (`commands.rs`), обёртки Tauri к ним — сгенерированные `commands.rs`
//! оболочки. Здесь — то, что есть только у устройства: секреты в keyring ОС
//! (на Android — приватный файл приложения), открытие сессии с наблюдателем
//! Syncthing, Syncthing и клонирование удалённого репозитория в папку
//! устройства.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};

use typsidian_core::sync::engine::native::NativeGitEngine;
use typsidian_core::sync::engine::CancelToken;
#[cfg(not(target_os = "android"))]
use typsidian_core::sync::secrets::{entry_name, MemorySecretStore, SecretStore};
use typsidian_core::sync::secrets::{SecretKind, SecretStoreError};
use typsidian_core::sync::session::{self as sync_session, Credentials};
use typsidian_core::sync::syncthing::{self, SyncthingEndpoint};
use typsidian_core::sync::{ShellCapabilities, SyncError};
use typsidian_core::vault::backend::VaultBackend;
use typsidian_host::sync::{
    self as host_sync, DevicePlace, DynSecretStore, SecretFuture, SyncAnswer, SyncCancel,
    SyncFailure, SyncHome, SyncProgressSlot, SyncState,
};
use typsidian_host::with_sync;

use super::{Device, Vault};

fn capabilities() -> ShellCapabilities {
    ShellCapabilities::current(cfg!(target_os = "android"))
}

// ── Хранилище секретов устройства (011 research §10) ────────────────────────

/// Секреты синхронизации этого устройства в keyring ОС. Если keyring недоступен
/// (нет службы секретов в сеансе), секрет живёт в памяти до закрытия
/// приложения — не на диске и не в vault.
#[cfg(not(target_os = "android"))]
struct KeyringSecrets(MemorySecretStore);

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

#[cfg(not(target_os = "android"))]
impl DynSecretStore for KeyringSecrets {
    fn get<'a>(&'a self, vault_id: &'a str, kind: SecretKind) -> SecretFuture<'a, Option<String>> {
        Box::pin(async move {
            if let Some(value) = self.0.get(vault_id, kind).await? {
                return Ok(Some(value));
            }
            keyring_call(entry_name(vault_id, kind), |entry| {
                match entry.get_password() {
                    Ok(value) => Ok(Some(value)),
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(error) => Err(error),
                }
            })
            .await
            .or(Ok(None))
        })
    }

    fn set<'a>(
        &'a self,
        vault_id: &'a str,
        kind: SecretKind,
        value: &'a str,
    ) -> SecretFuture<'a, ()> {
        Box::pin(async move {
            let owned = value.to_string();
            match keyring_call(entry_name(vault_id, kind), move |entry| {
                entry.set_password(&owned)
            })
            .await
            {
                Ok(()) => {
                    let _ = self.0.delete(vault_id, kind).await;
                    Ok(())
                }
                Err(_) => self.0.set(vault_id, kind, value).await,
            }
        })
    }

    fn delete<'a>(&'a self, vault_id: &'a str, kind: SecretKind) -> SecretFuture<'a, ()> {
        Box::pin(async move {
            let _ = self.0.delete(vault_id, kind).await;
            // Отказ keyring — отказ удаления (011 T077): секрет остался в
            // хранилище ОС, и называть его удалённым нельзя.
            keyring_call(entry_name(vault_id, kind), |entry| {
                match entry.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(error) => Err(error),
                }
            })
            .await
        })
    }
}

/// Хранилище секретов окна — одно на всё время работы приложения.
fn secrets_for_app(app: &AppHandle) -> Arc<dyn DynSecretStore> {
    #[cfg(target_os = "android")]
    {
        let dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        Arc::new(typsidian_host::sync::FileSecrets::new(
            dir.join("sync-secrets.json"),
        ))
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Arc::new(KeyringSecrets(MemorySecretStore::new()))
    }
}

// ── Команды устройства ──────────────────────────────────────────────────────

/// Открывает сессию синхронизации для только что открытого vault.
#[tauri::command]
pub async fn sync_open(
    app: AppHandle,
    vault: State<'_, Vault>,
    home: State<'_, SyncHome>,
    sync_state: State<'_, SyncState>,
    cancel: State<'_, SyncCancel>,
    progress: State<'_, SyncProgressSlot>,
    vault_id: String,
) -> SyncAnswer<()> {
    host_sync::open_session(&vault, &home, &sync_state, &cancel, &progress, vault_id).await?;
    restart_syncthing_watcher(&app);
    Ok(())
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
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
    endpoint: SyncthingEndpoint,
    api_key: Option<String>,
) -> SyncAnswer<()> {
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
    let (home, vault, sync_state) = (&*home, &*vault, &*sync_state);
    let result: SyncAnswer<()> = async {
        with_sync!(home, vault, sync_state, |_backend, session, ctx| Ok(
            session
                .syncthing_connect(home.secrets(), &ctx, endpoint, &api_key)
                .await?
        ))
    }
    .await;
    restart_syncthing_watcher(&app);
    result
}

#[tauri::command]
pub async fn sync_syncthing_disconnect(
    home: State<'_, SyncHome>,
    vault: State<'_, Vault>,
    sync_state: State<'_, SyncState>,
) -> SyncAnswer<()> {
    let (home, vault, sync_state) = (&*home, &*vault, &*sync_state);
    with_sync!(home, vault, sync_state, |_backend, session, ctx| {
        session.syncthing_disconnect(home.secrets(), &ctx).await;
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
    let home = app.state::<SyncHome>();
    let vault = app.state::<Vault>();
    let sync_state = app.state::<SyncState>();
    let ctx = host_sync::sync_context(&home, &sync_state).await?;
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
            host_sync::persist_device_update(&home, &sync_state).await;
        }
    }
    let (root, detected) = {
        let guard = vault.0.lock().await;
        let backend = guard.as_ref().ok_or_else(host_sync::not_open)?;
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
        let open = guard.as_ref().ok_or_else(host_sync::not_open)?;
        open.session.syncthing_api_key(home.secrets()).await
    };
    let Some(key) = key else {
        return Ok(None);
    };
    let folder = syncthing::query(&endpoint, &key, &root.to_string_lossy(), detected).await;
    let changed = {
        let mut guard = sync_state.0.lock().await;
        let open = guard.as_mut().ok_or_else(host_sync::not_open)?;
        open.session.set_syncthing_state(folder)
    };
    if changed {
        // Окно перечитывает сводку сразу, не дожидаясь такта.
        let _ = app.emit(SYNC_CHANGED_EVENT, ());
    }
    Ok(Some((endpoint, key)))
}

/// Клонирует удалённый репозиторий в пустую папку будущего vault (FR-013).
#[tauri::command]
pub async fn sync_clone_vault(
    home: State<'_, SyncHome>,
    cancel: State<'_, SyncCancel>,
    path: String,
    vault_id: String,
    url: String,
    credentials: Credentials,
) -> SyncAnswer<()> {
    host_sync::require_vault_id(&vault_id)?;
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
        home.secrets(),
        &vault_id,
        &home.capabilities(),
        &url,
        credentials,
        None,
        &token,
    )
    .await?)
}

/// Регистрирует состояние синхронизации в приложении. Окружение собирается при
/// запуске: ему нужны папки приложения и то же хранилище устройства, что у
/// остальных команд (`Device`).
pub fn manage(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
        .manage(SyncState::default())
        .manage(SyncCancel::default())
        .manage(SyncProgressSlot::default())
        .manage(SyncthingWatcher::default())
        .setup(|app| {
            let handle = app.handle();
            let device = handle.state::<Device>().0.clone();
            let fallback = handle.path().app_config_dir().ok();
            let mirrors = handle
                .path()
                .app_data_dir()
                .ok()
                .map(|dir| dir.join("mirrors"));
            app.manage(SyncHome::new(
                device,
                DevicePlace::System { fallback },
                mirrors,
                secrets_for_app(handle),
                capabilities(),
            ));
            Ok(())
        })
}
