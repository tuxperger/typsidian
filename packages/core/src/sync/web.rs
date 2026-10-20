//! Поверхность синхронизации для web (фича 011, T016, T023, T032, T045, T054).
//!
//! Тонкая обёртка над [`SyncSession`] с движком [`JsGitEngine`]: каждое тело —
//! вызов сессии, без решений (принцип I). Vault приходит на время вызова, как
//! у `PreviewEngine`; **вызывать только через очередь `wasm-call-queue`** —
//! пока идёт операция, заимствование `WebBackend` занято.

use wasm_bindgen::prelude::*;

use super::engine::web::JsGitEngine;
use super::engine::CancelToken;
use super::ignore::{
    self, IgnoreCheck, IgnoreDocument, IgnoreFile, IgnoreReport, IgnoreSaveOutcome,
    IgnoreSaveRequest,
};
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
    /// Такт по шагам: итог, накопленный между `tickBegin` и `tickEnd`.
    tick: Option<TickOutcome>,
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
            tick: None,
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

    #[wasm_bindgen(js_name = ignoreOpen)]
    pub async fn ignore_open(&self, backend: &WebBackend, file: IgnoreFile) -> IgnoreDocument {
        self.inner.ignore_open(backend, file).await
    }

    #[wasm_bindgen(js_name = ignoreSave)]
    pub async fn ignore_save(
        &mut self,
        backend: &mut WebBackend,
        ctx: SyncContext,
        request: IgnoreSaveRequest,
    ) -> Result<IgnoreSaveOutcome, JsValue> {
        self.inner
            .ignore_save(backend, &ctx, request)
            .await
            .map_err(js_error)
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

    /// Вернуть утраченную историю из удалённого репозитория (T079).
    #[wasm_bindgen(js_name = restoreHistory)]
    pub async fn restore_history(
        &mut self,
        backend: &mut WebBackend,
        ctx: SyncContext,
        confirm_not_keeper: bool,
    ) -> Result<PullOutcome, JsValue> {
        self.inner
            .restore_history(backend, &IdbSecretStore, &ctx, confirm_not_keeper)
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

/// Проверка текста редактора правил (фича 012, FR-010). Свободная функция, а
/// не метод сессии: она чистая и зовётся по мере набора — в том числе пока
/// сессия занята сохранением.
#[wasm_bindgen(js_name = ignoreCheck)]
pub fn ignore_check(file: IgnoreFile, text: String) -> IgnoreCheck {
    ignore::check(file, &text)
}

/// Текст редактора с возвращённым блоком приложения (фича 012, FR-011).
#[wasm_bindgen(js_name = ignoreRestore)]
pub fn ignore_restore(file: IgnoreFile, text: String) -> String {
    ignore::restore_block(file, &text)
}

// ── Обмен по шагам (T071) ──────────────────────────────────────────────────
//
// Сеть — долгая часть обмена, и на её время нельзя занимать очередь вызовов
// wasm: сохранение заметок и опрос состояния ждали бы сервер (FR-020,
// принцип X). Поэтому обмен идёт тремя вызовами: план под очередью
// (`begin…`), сеть на отдельном объекте [`SyncExchange`] вне очереди — он не
// заимствует ни сессию, ни vault, — и итог снова под очередью (`complete…`).

enum ExchangeKind {
    Pull(session::PullPlan),
    Push(session::ExchangePlan),
    Mirror(session::ExchangePlan),
}

enum ExchangeResult {
    Fetched(Result<Option<String>, SyncError>),
    Done(Result<(), SyncError>),
}

/// Сетевой шаг обмена: копия движка, план и итог.
#[wasm_bindgen]
pub struct SyncExchange {
    engine: Option<JsGitEngine>,
    kind: ExchangeKind,
    result: Option<ExchangeResult>,
}

#[wasm_bindgen]
impl SyncExchange {
    /// Сеть. Звать вне очереди `wasm-call-queue`; итог остаётся в объекте.
    pub async fn run(&mut self) {
        let Some(engine) = self.engine.as_ref() else {
            self.result = Some(ExchangeResult::Done(Err(SyncError::NoHistory)));
            return;
        };
        self.result = Some(match &self.kind {
            ExchangeKind::Pull(plan) => ExchangeResult::Fetched(plan.exchange.fetch(engine).await),
            ExchangeKind::Push(plan) => ExchangeResult::Done(plan.push(engine).await),
            ExchangeKind::Mirror(plan) => ExchangeResult::Done(plan.mirror(engine).await),
        });
    }
}

impl SyncExchange {
    fn fetched(&mut self) -> Result<Option<String>, SyncError> {
        match self.result.take() {
            Some(ExchangeResult::Fetched(result)) => result,
            _ => Err(SyncError::Cancelled),
        }
    }

    fn done(&mut self) -> Result<(), SyncError> {
        match self.result.take() {
            Some(ExchangeResult::Done(result)) => result,
            _ => Err(SyncError::Cancelled),
        }
    }
}

#[wasm_bindgen]
impl WebSyncSession {
    #[wasm_bindgen(js_name = beginPull)]
    pub async fn begin_pull(
        &mut self,
        backend: &WebBackend,
        ctx: SyncContext,
        confirm_not_keeper: bool,
    ) -> Result<SyncExchange, JsValue> {
        let plan = self
            .inner
            .begin_pull(backend, &IdbSecretStore, &ctx, confirm_not_keeper)
            .await
            .map_err(js_error)?;
        Ok(self.exchange(ExchangeKind::Pull(plan)))
    }

    #[wasm_bindgen(js_name = completePull)]
    pub async fn complete_pull(
        &mut self,
        backend: &mut WebBackend,
        ctx: SyncContext,
        mut exchange: SyncExchange,
    ) -> Result<PullOutcome, JsValue> {
        let fetched = exchange.fetched();
        let ExchangeKind::Pull(plan) = exchange.kind else {
            return Err(js_error(SyncError::Cancelled));
        };
        self.inner
            .complete_pull(backend, &ctx, plan, fetched)
            .await
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = beginPush)]
    pub async fn begin_push(
        &mut self,
        backend: &WebBackend,
        ctx: SyncContext,
        confirm_not_keeper: bool,
    ) -> Result<SyncExchange, JsValue> {
        let plan = self
            .inner
            .begin_push(backend, &IdbSecretStore, &ctx, confirm_not_keeper)
            .await
            .map_err(js_error)?;
        Ok(self.exchange(ExchangeKind::Push(plan)))
    }

    #[wasm_bindgen(js_name = completePush)]
    pub async fn complete_push(
        &mut self,
        backend: &WebBackend,
        ctx: SyncContext,
        mut exchange: SyncExchange,
    ) -> Result<RemoteStatus, JsValue> {
        let sent = exchange.done();
        let ExchangeKind::Push(plan) = exchange.kind else {
            return Err(js_error(SyncError::Cancelled));
        };
        self.inner
            .complete_push(backend, &ctx, &plan, sent)
            .await
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = beginMirror)]
    pub async fn begin_mirror(
        &mut self,
        backend: &WebBackend,
        ctx: SyncContext,
    ) -> Result<SyncExchange, JsValue> {
        let plan = self
            .inner
            .begin_mirror(backend, &IdbSecretStore, &ctx)
            .await
            .map_err(js_error)?;
        Ok(SyncExchange {
            engine: self.inner.mirror_engine().cloned(),
            kind: ExchangeKind::Mirror(plan),
            result: None,
        })
    }

    #[wasm_bindgen(js_name = completeMirror)]
    pub fn complete_mirror(
        &mut self,
        ctx: SyncContext,
        mut exchange: SyncExchange,
    ) -> Result<(), JsValue> {
        let result = exchange.done();
        self.inner.complete_mirror(&ctx, result).map_err(js_error)
    }

    /// Такт по шагам: начало — автополучение, если пора.
    #[wasm_bindgen(js_name = tickBegin)]
    pub async fn tick_begin(
        &mut self,
        backend: &WebBackend,
        ctx: SyncContext,
    ) -> Option<SyncExchange> {
        self.tick = Some(TickOutcome::idle(ctx.now));
        if !self.inner.tick_pull_due(backend, &ctx).await {
            return None;
        }
        match self
            .inner
            .begin_pull(backend, &IdbSecretStore, &ctx, false)
            .await
        {
            Ok(plan) => Some(self.exchange(ExchangeKind::Pull(plan))),
            Err(error) => {
                let mut outcome = self.tick.take().unwrap_or(TickOutcome::idle(ctx.now));
                self.inner.tick_pulled(&mut outcome, Err(error));
                self.tick = Some(outcome);
                None
            }
        }
    }

    /// Середина такта: итог получения, автоснимок и, если пора, план отправки.
    #[wasm_bindgen(js_name = tickMiddle)]
    pub async fn tick_middle(
        &mut self,
        backend: &mut WebBackend,
        ctx: SyncContext,
        auto_prefix: String,
        pulled: Option<SyncExchange>,
    ) -> Option<SyncExchange> {
        let mut outcome = self.tick.take().unwrap_or(TickOutcome::idle(ctx.now));
        if let Some(mut exchange) = pulled {
            let fetched = exchange.fetched();
            if let ExchangeKind::Pull(plan) = exchange.kind {
                let result = self.inner.complete_pull(backend, &ctx, plan, fetched).await;
                self.inner.tick_pulled(&mut outcome, result);
            }
        }
        let push_due = self
            .inner
            .tick_local(backend, &ctx, &auto_prefix, &mut outcome)
            .await;
        let next = if push_due {
            match self
                .inner
                .begin_push(backend, &IdbSecretStore, &ctx, false)
                .await
            {
                Ok(plan) => Some(self.exchange(ExchangeKind::Push(plan))),
                Err(error) => {
                    self.inner.tick_pushed(&mut outcome, Err(error));
                    None
                }
            }
        } else {
            None
        };
        self.tick = Some(outcome);
        next
    }

    /// Конец такта: итог отправки и время следующего.
    #[wasm_bindgen(js_name = tickEnd)]
    pub async fn tick_end(
        &mut self,
        backend: &WebBackend,
        ctx: SyncContext,
        pushed: Option<SyncExchange>,
    ) -> TickOutcome {
        let mut outcome = self.tick.take().unwrap_or(TickOutcome::idle(ctx.now));
        if let Some(mut exchange) = pushed {
            let sent = exchange.done();
            if let ExchangeKind::Push(plan) = exchange.kind {
                let result = self.inner.complete_push(backend, &ctx, &plan, sent).await;
                self.inner.tick_pushed(&mut outcome, result);
            }
        }
        self.inner.tick_finish(backend, &ctx, &mut outcome).await;
        outcome
    }
}

impl WebSyncSession {
    fn exchange(&self, kind: ExchangeKind) -> SyncExchange {
        SyncExchange {
            engine: Some(self.engine.clone()),
            kind,
            result: None,
        }
    }
}

/// Ход массовой операции (T075): свой объект, а не метод сессии — пока
/// операция идёт, сессия заимствована, и вызов на ней упал бы.
#[wasm_bindgen]
pub struct SyncProgressHandle {
    sink: super::ProgressSink,
}

#[wasm_bindgen]
impl SyncProgressHandle {
    pub fn current(&self) -> Option<super::SyncProgress> {
        self.sink.current()
    }
}

#[wasm_bindgen]
impl WebSyncSession {
    /// Взять один раз при открытии; читать — когда угодно, вне очереди.
    #[wasm_bindgen(js_name = progressHandle)]
    pub fn progress_handle(&self) -> SyncProgressHandle {
        SyncProgressHandle {
            sink: self.inner.progress_sink(),
        }
    }
}
