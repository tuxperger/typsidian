//! Поверхность синхронизации для web (фича 011, T016, T023, T032, T045, T054).
//!
//! Тонкая обёртка над [`SyncSession`] с движком [`JsGitEngine`]: каждое тело —
//! вызов сессии, без решений (принцип I). Vault приходит на время вызова, как
//! у `PreviewEngine`; **вызывать только через очередь `wasm-call-queue`** —
//! пока идёт операция, заимствование `WebBackend` занято.

use wasm_bindgen::prelude::*;

use super::engine::web::JsGitEngine;
use super::engine::CancelToken;
use super::ignore::{IgnoreFile, IgnoreReport};
use super::secrets::web::IdbSecretStore;
use super::session::{self, Conflict, Credentials, PullOutcome, Resolution, TickOutcome};
use super::settings::VaultSyncSettingsPatch;
use super::syncthing::SyncthingEndpoint;
use super::{
    Change, Content, DeviceVaultSync, FileDiff, HistoryPage, Keeper, RemoteStatus, Revision,
    ShellCapabilities, Snapshot, SyncContext, SyncError, SyncEvent, SyncOverview, SyncSession,
    VaultSyncSettings,
};
use crate::vault::web::WebBackend;

#[wasm_bindgen]
pub struct WebSyncSession {
    inner: SyncSession<JsGitEngine>,
    engine: JsGitEngine,
}

/// Отмена операции сессии; живёт отдельно от неё — см. [`WebSyncSession::cancel_handle`].
#[wasm_bindgen]
pub struct SyncCancelHandle {
    token: CancelToken,
    engine: JsGitEngine,
}

#[wasm_bindgen]
impl SyncCancelHandle {
    pub fn cancel(&self) {
        self.token.cancel();
        self.engine.abort();
    }
}

fn js_error(error: SyncError) -> JsValue {
    serde_wasm_bindgen::to_value(&error).unwrap_or_else(|_| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
impl WebSyncSession {
    /// `engine` — адаптер истории папки vault, `mirror` — адаптер зеркала
    /// в OPFS для не-хранителя (FR-033).
    #[wasm_bindgen(constructor)]
    pub fn new(engine: JsValue, mirror: JsValue, vault_id: String) -> WebSyncSession {
        let engine = JsGitEngine::new(engine);
        let mirror =
            (!mirror.is_null() && !mirror.is_undefined()).then(|| JsGitEngine::new(mirror));
        WebSyncSession {
            inner: SyncSession::new(engine.clone(), mirror, vault_id),
            engine,
        }
    }

    /// Возможности браузерной оболочки (FR-036).
    #[wasm_bindgen(js_name = capabilities)]
    pub fn capabilities() -> ShellCapabilities {
        ShellCapabilities::current(false)
    }

    #[wasm_bindgen(js_name = takeDeviceUpdate)]
    pub fn take_device_update(&mut self) -> Option<DeviceVaultSync> {
        self.inner.take_device_update()
    }

    #[wasm_bindgen(js_name = pollEvents)]
    pub fn poll_events(&mut self) -> Vec<SyncEvent> {
        self.inner.poll_events()
    }

    /// Ручка отмены текущей операции (принцип X). Отдельный объект, а не метод
    /// сессии: пока операция идёт, сессия заимствована, и вызов её метода из
    /// обработчика кнопки упал бы на границе wasm-bindgen.
    #[wasm_bindgen(js_name = cancelHandle)]
    pub fn cancel_handle(&self) -> SyncCancelHandle {
        SyncCancelHandle {
            token: self.inner.cancel_token(),
            engine: self.engine.clone(),
        }
    }

    #[wasm_bindgen(js_name = noteChange)]
    pub fn note_change(&mut self, now: f64) {
        self.inner.note_change(now as i64);
    }

    pub async fn overview(&mut self, backend: &WebBackend, ctx: SyncContext) -> SyncOverview {
        self.inner.overview(backend, &ctx).await
    }

    #[wasm_bindgen(js_name = ignoreReports)]
    pub async fn ignore_reports(&self, backend: &WebBackend) -> Vec<IgnoreReport> {
        self.inner.ignore_reports(backend).await
    }

    #[wasm_bindgen(js_name = applyIgnore)]
    pub async fn apply_ignore(
        &mut self,
        backend: &mut WebBackend,
        ctx: SyncContext,
        file: IgnoreFile,
    ) -> Result<(), JsValue> {
        self.inner
            .apply_ignore(backend, &ctx, file)
            .await
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = declineStignore)]
    pub fn decline_stignore(&mut self, ctx: SyncContext) {
        self.inner.decline_stignore(&ctx);
    }

    #[wasm_bindgen(js_name = enableHistory)]
    pub async fn enable_history(
        &mut self,
        backend: &mut WebBackend,
        ctx: SyncContext,
        message: String,
    ) -> Result<Snapshot, JsValue> {
        self.inner
            .enable_history(backend, &ctx, &message)
            .await
            .map_err(js_error)
    }

    pub async fn changes(&self, backend: &WebBackend) -> Result<Vec<Change>, JsValue> {
        self.inner.changes(backend).await.map_err(js_error)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn snapshot(
        &mut self,
        backend: &WebBackend,
        ctx: SyncContext,
        paths: JsValue,
        message: String,
        auto_prefix: String,
        flushed: bool,
        confirm_not_keeper: bool,
    ) -> Result<Snapshot, JsValue> {
        // `null` — снять все изменения, массив — только выбранные.
        let paths: Option<Vec<String>> = serde_wasm_bindgen::from_value(paths)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        self.inner
            .snapshot(
                backend,
                &ctx,
                paths,
                &message,
                &auto_prefix,
                flushed,
                confirm_not_keeper,
            )
            .await
            .map_err(js_error)
    }

    pub async fn history(
        &self,
        backend: &WebBackend,
        path: Option<String>,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<HistoryPage, JsValue> {
        self.inner
            .history(backend, path.as_deref(), cursor.as_deref(), limit)
            .await
            .map_err(js_error)
    }

    pub async fn diff(
        &self,
        backend: &WebBackend,
        path: String,
        from: Revision,
        to: Revision,
    ) -> Result<FileDiff, JsValue> {
        self.inner
            .diff(backend, &path, &from, &to)
            .await
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = snapshotChanges)]
    pub async fn snapshot_changes(
        &self,
        backend: &WebBackend,
        snapshot: String,
    ) -> Result<Vec<Change>, JsValue> {
        self.inner
            .snapshot_changes(backend, &snapshot)
            .await
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = fileAt)]
    pub async fn file_at(
        &self,
        backend: &WebBackend,
        path: String,
        revision: Revision,
    ) -> Result<Content, JsValue> {
        self.inner
            .file_at(backend, &path, &revision)
            .await
            .map_err(js_error)
    }

    pub async fn restore(
        &mut self,
        backend: &mut WebBackend,
        path: String,
        snapshot: String,
    ) -> Result<Content, JsValue> {
        self.inner
            .restore(backend, &path, &snapshot)
            .await
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = connectRemote)]
    pub async fn connect_remote(
        &mut self,
        backend: &mut WebBackend,
        ctx: SyncContext,
        url: String,
        credentials: Credentials,
    ) -> Result<RemoteStatus, JsValue> {
        self.inner
            .connect_remote(backend, &IdbSecretStore, &ctx, &url, credentials)
            .await
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = disconnectRemote)]
    pub async fn disconnect_remote(&mut self, backend: &mut WebBackend) -> Result<(), JsValue> {
        self.inner
            .disconnect_remote(backend, &IdbSecretStore)
            .await
            .map_err(js_error)
    }

    pub async fn push(
        &mut self,
        backend: &WebBackend,
        ctx: SyncContext,
        confirm_not_keeper: bool,
    ) -> Result<RemoteStatus, JsValue> {
        self.inner
            .push(backend, &IdbSecretStore, &ctx, confirm_not_keeper)
            .await
            .map_err(js_error)
    }

    pub async fn pull(
        &mut self,
        backend: &mut WebBackend,
        ctx: SyncContext,
        confirm_not_keeper: bool,
    ) -> Result<PullOutcome, JsValue> {
        self.inner
            .pull(backend, &IdbSecretStore, &ctx, confirm_not_keeper)
            .await
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = abortPull)]
    pub async fn abort_pull(&mut self, backend: &mut WebBackend) -> Result<(), JsValue> {
        self.inner.abort_pull(backend).await.map_err(js_error)
    }

    pub async fn conflicts(&self, backend: &WebBackend) -> Vec<Conflict> {
        self.inner.conflicts(backend).await
    }

    pub async fn resolve(
        &mut self,
        backend: &mut WebBackend,
        ctx: SyncContext,
        id: String,
        resolution: Resolution,
    ) -> Result<(), JsValue> {
        self.inner
            .resolve(backend, &ctx, &id, resolution)
            .await
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = setKeeper)]
    pub async fn set_keeper(
        &mut self,
        backend: &mut WebBackend,
        ctx: SyncContext,
    ) -> Result<Keeper, JsValue> {
        self.inner.set_keeper(backend, &ctx).await.map_err(js_error)
    }

    #[wasm_bindgen(js_name = clearKeeper)]
    pub async fn clear_keeper(&mut self, backend: &mut WebBackend) -> Result<(), JsValue> {
        self.inner.clear_keeper(backend).await.map_err(js_error)
    }

    #[wasm_bindgen(js_name = updateSettings)]
    pub async fn update_settings(
        &mut self,
        backend: &mut WebBackend,
        patch: VaultSyncSettingsPatch,
    ) -> Result<VaultSyncSettings, JsValue> {
        self.inner
            .update_settings(backend, &patch)
            .await
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = refreshMirror)]
    pub async fn refresh_mirror(
        &mut self,
        backend: &WebBackend,
        ctx: SyncContext,
    ) -> Result<(), JsValue> {
        self.inner
            .refresh_mirror(backend, &IdbSecretStore, &ctx)
            .await
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = syncthingConnect)]
    pub async fn syncthing_connect(
        &mut self,
        ctx: SyncContext,
        endpoint: SyncthingEndpoint,
        api_key: String,
    ) -> Result<(), JsValue> {
        self.inner
            .syncthing_connect(&IdbSecretStore, &ctx, endpoint, &api_key)
            .await
            .map_err(js_error)
    }

    pub async fn tick(
        &mut self,
        backend: &mut WebBackend,
        ctx: SyncContext,
        auto_prefix: String,
    ) -> TickOutcome {
        self.inner
            .tick(backend, &IdbSecretStore, &ctx, &auto_prefix)
            .await
    }
}

/// Клонирует удалённый репозиторий в пустую папку будущего vault (FR-013).
#[wasm_bindgen(js_name = cloneVault)]
pub async fn clone_vault(
    engine: JsValue,
    vault_id: String,
    url: String,
    credentials: Credentials,
    cors_proxy: Option<String>,
) -> Result<(), JsValue> {
    session::clone_vault(
        &JsGitEngine::new(engine),
        &IdbSecretStore,
        &vault_id,
        &ShellCapabilities::current(false),
        &url,
        credentials,
        cors_proxy.as_deref(),
        &CancelToken::new(),
    )
    .await
    .map_err(js_error)
}

/// Изменение настроек устройства, выдающее ему тождество (research §4);
/// `undefined` — оно уже есть.
#[wasm_bindgen(js_name = ensureDeviceChange)]
pub fn ensure_device_change(
    settings: crate::settings::Settings,
) -> Option<crate::settings::apply::SettingsChange> {
    super::ensure_device_change(&settings)
}

/// Изменение настроек устройства с новыми значениями синхронизации vault.
#[wasm_bindgen(js_name = deviceSyncChange)]
pub fn device_sync_change(
    vault: String,
    value: DeviceVaultSync,
) -> crate::settings::apply::SettingsChange {
    super::device_sync_change(crate::settings::VaultId::from_raw(vault), value)
}

/// Контекст вызова сессии из настроек устройства.
#[wasm_bindgen(js_name = syncContext)]
pub fn sync_context(settings: crate::settings::Settings, vault: String, now: f64) -> SyncContext {
    super::context_for(
        &settings,
        &crate::settings::VaultId::from_raw(vault),
        now as i64,
        ShellCapabilities::current(false),
    )
}
