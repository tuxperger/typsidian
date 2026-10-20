//! Сессия синхронизации открытого vault (фича 011, contracts/core-sync-api.md).
//!
//! Вся политика фичи — здесь, одинаково для обоих таргетов (принципы I, II):
//! движок ([`GitEngine`]) только хранит объекты и ходит по сети, файлы vault
//! пишутся через [`VaultBackend`], а значения устройства приходят в
//! [`SyncContext`] на время вызова и возвращаются через
//! [`SyncSession::take_device_update`] — хранилище настроек устройства сессия
//! не знает (то же разделение, что у `settings::apply`).
//!
//! Записи файлов vault, сделанные получением и откатом, видны остальному
//! приложению как внешние изменения (001): открытая заметка обновляется или
//! поднимает обычный конфликт с несохранёнными правками.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::vault::backend::{DeleteError, ReadError, VaultBackend};
use crate::vault::conflict::line_diff;
use crate::vault::FolderAccess;

use super::autosnapshot::{self, AutoDecision, SnapshotFacts};
use super::conflict_copy;
use super::engine::{Auth, CancelToken, GitEngine, RawChange, RawStatus, RemoteSpec, RepoProbe};
use super::ignore::{
    self, Editability, IgnoreDocument, IgnoreFile, IgnoreRefusal, IgnoreReport, IgnoreSaveOutcome,
    IgnoreSaveRequest, LineEnding,
};
use super::keeper::{self, KeeperFacts, KeeperState};
use super::merge::{self, MergeResult};
use super::secrets::{SecretKind, SecretStore};
use super::settings::{self as sync_settings, remote_url, VaultSyncSettings};
use super::syncthing::{self, LinkState, SyncthingFolder};
use super::{
    Change, ChangeKind, Content, DeviceVaultSync, FileDiff, HistoryPage, HistoryState, Millis,
    Operation, RemoteStatus, Revision, Snapshot, SyncContext, SyncError, SyncEvent, SyncOverview,
};

/// Ветка новой истории.
pub const DEFAULT_BRANCH: &str = "main";
/// Сколько снимков просматривается за один вызов истории файла, прежде чем
/// вернуть страницу с продолжением: история в десятки тысяч снимков не должна
/// превращаться в одно бесконечное ожидание (принцип X).
const HISTORY_SCAN_LIMIT: u32 = 2_000;
const LOG_CHUNK: u32 = 100;
/// Сколько изменённых заметок перечисляется в автоматическом сообщении.
const AUTO_MESSAGE_NAMES: usize = 5;

/// Конфликт для показа (data-model «Conflict»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Conflict {
    /// `pull:<путь>` или `st:<путь копии>`.
    pub id: String,
    pub source: ConflictSource,
    /// Файл, о котором спор.
    pub path: String,
    pub ours: Content,
    pub theirs: Content,
    /// Общая версия из истории (FR-016, FR-034); `None` — истории у файла нет.
    pub base: Option<Content>,
    pub auto_merge: Option<MergeResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ConflictSource {
    Pull {
        theirs_snapshot: String,
    },
    Syncthing {
        copy_path: String,
        stamp: String,
        device: String,
    },
}

/// Решение по конфликту.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum Resolution {
    KeepOurs,
    TakeTheirs,
    Merged {
        text: String,
    },
    /// Сохранить конфликтную копию отдельным файлом (только Syncthing).
    KeepBothAsFile {
        new_path: String,
    },
}

/// Итог получения (FR-016).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PullOutcome {
    /// Сколько файлов обновлено.
    pub applied: u32,
    /// Сколько конфликтов ждут решения; пока их больше нуля, снимки и
    /// отправка заблокированы (FR-017).
    pub conflicts: u32,
    pub up_to_date: bool,
}

/// Итог такта автоматики (research §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TickOutcome {
    pub decision: AutoDecision,
    pub snapshot: Option<Snapshot>,
    pub pulled: bool,
    pub pushed: bool,
    /// Когда спросить снова, мс от эпохи.
    pub next_tick_at: Millis,
}

impl TickOutcome {
    /// Такт, который ещё ничего не сделал: снимок запрещён, следующий — через
    /// минуту.
    pub fn idle(now: Millis) -> Self {
        Self {
            decision: AutoDecision::Blocked {
                reason: autosnapshot::BlockReason::Disabled,
            },
            snapshot: None,
            pulled: false,
            pushed: false,
            next_tick_at: now + 60_000,
        }
    }
}

/// Всё, что нужно сетевому шагу обмена, без vault и без сессии (T071):
/// оболочка получает план под блокировками, отпускает их и идёт в сеть.
/// Сохранение заметок тем временем не ждёт обмена (FR-020, принцип X).
#[derive(Debug, Clone)]
pub struct ExchangePlan {
    pub spec: RemoteSpec,
    pub auth: Auth,
    pub cancel: CancelToken,
}

impl ExchangePlan {
    /// Получение снимков сервера; `None` — ветки на сервере нет.
    pub async fn fetch<E: GitEngine>(&self, engine: &E) -> Result<Option<String>, SyncError> {
        engine
            .fetch(&self.spec, &self.auth, &self.cancel)
            .await
            .map_err(SyncError::from)
    }

    pub async fn push<E: GitEngine>(&self, engine: &E) -> Result<(), SyncError> {
        engine
            .push(&self.spec, &self.auth, &self.cancel)
            .await
            .map_err(SyncError::from)
    }

    /// Обновление зеркала не-хранителя: клон при первом обращении (FR-033).
    pub async fn mirror<E: GitEngine>(&self, mirror: &E) -> Result<(), SyncError> {
        match mirror.probe().await? {
            RepoProbe::Mirror => mirror
                .fetch(&self.spec, &self.auth, &self.cancel)
                .await
                .map(|_| ()),
            _ => {
                mirror
                    .clone_into(&self.spec, &self.auth, true, &self.cancel)
                    .await
            }
        }
        .map_err(SyncError::from)
    }
}

/// План получения: обмен и автор будущего снимка слияния.
#[derive(Debug, Clone)]
pub struct PullPlan {
    pub exchange: ExchangePlan,
    author: super::Author,
}

/// Данные доступа, введённые пользователем при подключении (FR-013).
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Credentials {
    pub user: Option<String>,
    pub token: Option<String>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("user", &self.user)
            .field("token", &self.token.as_ref().map(|_| "***"))
            .finish()
    }
}

/// Незавершённое получение: журнал исходных версий и конфликты (research §9).
#[derive(Debug, Clone)]
struct PendingPull {
    theirs: String,
    head_before: Option<String>,
    /// Что лежало в файле до получения; `None` — файла не было.
    journal: BTreeMap<String, Option<Vec<u8>>>,
    /// Все пути, пришедшие с сервера, — они входят в снимок слияния.
    incoming: BTreeSet<String>,
    conflicts: BTreeMap<String, PullConflict>,
    fast_forward: bool,
}

#[derive(Debug, Clone)]
struct PullConflict {
    base: Option<Vec<u8>>,
    ours: Option<Vec<u8>>,
    theirs: Option<Vec<u8>>,
}

/// Сессия синхронизации одного открытого vault.
pub struct SyncSession<E: GitEngine> {
    engine: E,
    /// Зеркало удалённого репозитория для не-хранителя (FR-033).
    mirror: Option<E>,
    vault_id: String,
    pull: Option<PendingPull>,
    fetched_at: Option<Millis>,
    mirror_fetched_at: Option<Millis>,
    last_error: Option<SyncError>,
    last_change_at: Option<Millis>,
    pull_on_open_done: bool,
    syncthing: Option<SyncthingFolder>,
    device_update: Option<DeviceVaultSync>,
    events: Vec<SyncEvent>,
    cancel: CancelToken,
    /// Ход идущей операции для оболочки — см. [`super::ProgressSink`].
    progress_sink: super::ProgressSink,
}

impl<E: GitEngine> SyncSession<E> {
    pub fn new(engine: E, mirror: Option<E>, vault_id: impl Into<String>) -> Self {
        Self {
            engine,
            mirror,
            vault_id: vault_id.into(),
            pull: None,
            fetched_at: None,
            mirror_fetched_at: None,
            last_error: None,
            last_change_at: None,
            pull_on_open_done: false,
            syncthing: None,
            device_update: None,
            events: Vec::new(),
            cancel: CancelToken::new(),
            progress_sink: super::ProgressSink::default(),
        }
    }

    pub fn engine(&self) -> &E {
        &self.engine
    }

    /// Изменённые значения устройства, которые вызывающая сторона обязана
    /// сохранить в настройках устройства (004). Забирается после каждого вызова.
    pub fn take_device_update(&mut self) -> Option<DeviceVaultSync> {
        self.device_update.take()
    }

    pub fn poll_events(&mut self) -> Vec<SyncEvent> {
        std::mem::take(&mut self.events)
    }

    /// Токен отмены текущей сетевой или массовой операции (принцип X).
    pub fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }

    /// Отменяет текущую операцию; следующая начнётся со снятой отменой.
    pub fn cancel(&mut self) {
        self.cancel.cancel();
    }

    fn fresh_cancel(&mut self) -> CancelToken {
        // Новая операция — ход прежней больше не показывается.
        self.progress_sink.set(None);
        self.cancel.reset();
        self.cancel.clone()
    }

    /// Файлы vault изменились (свои правки или доставка Syncthing) — отсчёт
    /// паузы автоснимка начинается заново (FR-031).
    pub fn note_change(&mut self, now: Millis) {
        self.last_change_at = Some(now);
    }

    fn update_device(&mut self, ctx: &SyncContext, change: impl FnOnce(&mut DeviceVaultSync)) {
        let mut next = self
            .device_update
            .clone()
            .unwrap_or_else(|| ctx.device_sync.clone());
        change(&mut next);
        if next != ctx.device_sync || self.device_update.is_some() {
            self.device_update = Some(next);
        }
    }

    // ── Сводка ─────────────────────────────────────────────────────────────

    pub async fn overview<B: VaultBackend>(
        &mut self,
        backend: &B,
        ctx: &SyncContext,
    ) -> SyncOverview {
        let settings = sync_settings::read(backend).await;
        let detected = syncthing_detected(backend).await;
        let copies = conflict_copies(backend).await;
        let keeper_state = self.keeper_state(&settings, ctx, &copies);

        let history = self.history_state(ctx, &settings, &keeper_state).await;
        let has_local = matches!(history, HistoryState::Local { .. });
        if has_local != ctx.device_sync.last_seen_history && !matches!(history, HistoryState::Lost)
        {
            self.update_device(ctx, |device| device.last_seen_history = has_local);
        }
        if let (KeeperState::ThisDevice, Some(device)) = (&keeper_state, &ctx.device) {
            if ctx.device_sync.last_seen_keeper.as_deref() != Some(device.id.as_str()) {
                let id = device.id.clone();
                self.update_device(ctx, |value| value.last_seen_keeper = Some(id));
            }
        }

        let mut ignore_warnings = Vec::new();
        if detected && !ctx.device_sync.stignore_declined {
            let report = ignore::report(
                IgnoreFile::StIgnore,
                read_text(backend, IgnoreFile::StIgnore.path())
                    .await
                    .as_deref(),
            );
            // Без истории исключать `.git` нечего, но кэши — всегда: SQLite,
            // разнесённый Syncthing, — источник конфликтных копий сам по себе.
            if !report.is_complete() && (has_local || only_cache_rules_missing(&report)) {
                ignore_warnings.push(report);
            }
        }

        let remote = match &settings.remote_url {
            Some(url) => Some(self.remote_status(url, &settings, has_local).await),
            None => None,
        };
        let last_snapshot_at = if has_local {
            self.engine
                .log(None, 0, 1)
                .await
                .ok()
                .and_then(|page| page.entries.first().map(|entry| entry.time))
        } else {
            None
        };
        let pull_conflicts = self.pull.as_ref().map_or(0, |pull| pull.conflicts.len());
        let syncthing = self.syncthing_view(detected, ctx);

        SyncOverview {
            history,
            remote,
            syncthing,
            keeper: settings.keeper.clone(),
            keeper_state,
            last_snapshot_at,
            conflicts: (copies.len() + pull_conflicts) as u32,
            pull_in_progress: self.pull.is_some(),
            ignore_warnings,
            settings,
            capabilities: ctx.capabilities,
        }
    }

    fn syncthing_view(&self, detected: bool, ctx: &SyncContext) -> SyncthingFolder {
        if let Some(folder) = &self.syncthing {
            let mut folder = folder.clone();
            folder.detected = detected;
            return folder;
        }
        let link = if !ctx.capabilities.syncthing_status {
            LinkState::UnavailableInShell
        } else if ctx.device_sync.syncthing.is_some() {
            LinkState::Unreachable
        } else {
            LinkState::NotConfigured
        };
        SyncthingFolder::unlinked(detected, link)
    }

    fn keeper_state(
        &self,
        settings: &VaultSyncSettings,
        ctx: &SyncContext,
        copies: &[PathBuf],
    ) -> KeeperState {
        let settings_copy = copies.iter().any(|copy| is_settings_conflict_copy(copy));
        keeper::assess(&KeeperFacts {
            keeper: settings.keeper.as_ref(),
            device: ctx.device.as_ref(),
            last_seen: ctx.device_sync.last_seen_keeper.as_deref(),
            settings_conflict_copy: settings_copy,
        })
    }

    async fn history_state(
        &self,
        ctx: &SyncContext,
        settings: &VaultSyncSettings,
        keeper_state: &KeeperState,
    ) -> HistoryState {
        match self.engine.probe().await {
            Ok(RepoProbe::Local) => {
                let head = self.engine.head().await.unwrap_or_default();
                let changes = self
                    .engine
                    .status()
                    .await
                    .map(|changes| changes.len() as u32)
                    .unwrap_or(0);
                HistoryState::Local {
                    branch: head.branch,
                    in_progress: head.in_progress,
                    changes,
                }
            }
            Ok(RepoProbe::ForeignParent { repo_root, .. }) => {
                HistoryState::ForeignParent { repo_root }
            }
            _ => {
                if *keeper_state == KeeperState::OtherDevice && settings.remote_url.is_some() {
                    if self.mirror.is_some() {
                        return HistoryState::Mirror {
                            fetched_at: self.mirror_fetched_at,
                        };
                    }
                }
                if ctx.device_sync.last_seen_history {
                    HistoryState::Lost
                } else {
                    HistoryState::Absent
                }
            }
        }
    }

    async fn remote_status(
        &self,
        url: &str,
        settings: &VaultSyncSettings,
        has_local: bool,
    ) -> RemoteStatus {
        let mut status = RemoteStatus {
            url: url.to_string(),
            ahead: None,
            behind: None,
            fetched_at: self.fetched_at,
            last_error: self.last_error.clone(),
        };
        if !has_local {
            return status;
        }
        let head = self.engine.head().await.unwrap_or_default();
        let branch = branch_of(settings, head.branch.as_deref());
        let remote = self.engine.remote_head(&branch).await.ok().flatten();
        match (head.oid, remote) {
            (Some(local), Some(remote)) => {
                if let Ok((ahead, behind)) = self.engine.ahead_behind(&local, &remote).await {
                    status.ahead = Some(ahead);
                    status.behind = Some(behind);
                }
            }
            (Some(local), None) => {
                // На сервере ветки ещё нет — всё локальное не отправлено.
                if let Ok(page) = self.engine.log(Some(&local), 0, 1_000).await {
                    status.ahead = Some(page.entries.len() as u32);
                    status.behind = Some(0);
                }
            }
            _ => {}
        }
        status
    }

    // ── Правила исключения ─────────────────────────────────────────────────

    pub async fn ignore_reports<B: VaultBackend>(&self, backend: &B) -> Vec<IgnoreReport> {
        let mut reports = vec![ignore::report(
            IgnoreFile::GitIgnore,
            read_text(backend, IgnoreFile::GitIgnore.path())
                .await
                .as_deref(),
        )];
        if syncthing_detected(backend).await {
            reports.insert(
                0,
                ignore::report(
                    IgnoreFile::StIgnore,
                    read_text(backend, IgnoreFile::StIgnore.path())
                        .await
                        .as_deref(),
                ),
            );
        }
        reports
    }

    /// Дописывает блок правил приложения (FR-027).
    pub async fn apply_ignore<B: VaultBackend>(
        &mut self,
        backend: &mut B,
        ctx: &SyncContext,
        file: IgnoreFile,
    ) -> Result<(), SyncError> {
        // «Не прочитался» — не «файла нет»: по второму файл создаётся заново, и
        // правила пользователя в нечитаемом файле (не UTF-8, нет прав) пропали
        // бы (T068).
        let current = match backend.read(Path::new(file.path())).await {
            Ok(text) => Some(text),
            Err(ReadError::NotFound) => None,
            Err(ReadError::Io | ReadError::TooLarge) => {
                return Err(SyncError::Io {
                    message: format!("файл правил не читается: {}", file.path()),
                })
            }
        };
        let next = ignore::apply(file, current.as_deref());
        if current.as_deref() != Some(next.as_str()) {
            write_file(backend, file.path(), Some(next.as_bytes())).await?;
        }
        if file == IgnoreFile::StIgnore && ctx.device_sync.stignore_declined {
            self.update_device(ctx, |device| device.stignore_declined = false);
        }
        self.events.push(SyncEvent::OverviewChanged);
        Ok(())
    }

    /// Пользователь явно отказался от правил Syncthing (FR-028).
    pub fn decline_stignore(&mut self, ctx: &SyncContext) {
        self.update_device(ctx, |device| device.stignore_declined = true);
        self.events.push(SyncEvent::OverviewChanged);
    }

    // ── Редактор правил исключения (фича 012) ──────────────────────────────

    /// Файл правил для редактора (012 §FR-001). Не отказывает: всё, что
    /// мешает правке, названо в `editable`.
    pub async fn ignore_open<B: VaultBackend>(
        &self,
        backend: &B,
        file: IgnoreFile,
    ) -> IgnoreDocument {
        let probe = self.engine.probe().await.ok();
        let foreign_parent =
            file == IgnoreFile::GitIgnore && matches!(probe, Some(RepoProbe::ForeignParent { .. }));
        let in_effect = match file {
            IgnoreFile::GitIgnore => matches!(
                probe,
                Some(RepoProbe::Local | RepoProbe::ForeignParent { .. })
            ),
            IgnoreFile::StIgnore => syncthing_detected(backend).await,
        };
        let on_disk = read_ignore(backend, file).await;
        let read_only = backend.access_state() != crate::vault::AccessState::ReadWrite;
        let document = |exists, text: String, line_ending, base, editable| IgnoreDocument {
            file,
            exists,
            check: ignore::check(file, &text),
            text,
            line_ending,
            base,
            editable: match editable {
                Editability::Editable if read_only => Editability::ReadOnly,
                other => other,
            },
            in_effect,
            foreign_parent,
        };
        match on_disk {
            // Файла нет — редактор открывается с блоком приложения, а сам
            // файл появится только при сохранении (FR-005).
            OnDisk::Absent => document(
                false,
                ignore::restore_block(file, ""),
                LineEnding::Lf,
                None,
                Editability::Editable,
            ),
            OnDisk::Text { text, mark } => document(
                true,
                LineEnding::to_editor(&text),
                LineEnding::detect(&text),
                Some(mark),
                Editability::Editable,
            ),
            OnDisk::Unusable(reason) => document(true, String::new(), LineEnding::Lf, None, reason),
        }
    }

    /// Сохраняет текст редактора в файл правил (012 §FR-003).
    ///
    /// Файл пишется только при исходе `Saved`. Роль хранителя не
    /// спрашивается: это правка файла vault, а не операция истории (FR-020).
    pub async fn ignore_save<B: VaultBackend>(
        &mut self,
        backend: &mut B,
        ctx: &SyncContext,
        request: IgnoreSaveRequest,
    ) -> Result<IgnoreSaveOutcome, SyncError> {
        let file = request.file;
        let current = self.ignore_open(backend, file).await;
        if let Some(reason) = IgnoreRefusal::from_editability(current.editable) {
            return Ok(IgnoreSaveOutcome::Refused { reason });
        }
        // Посреди слияния или получения рабочее дерево принадлежит операции:
        // правило, сменившееся на полпути, изменило бы её набор файлов
        // (FR-017).
        if file == IgnoreFile::GitIgnore && self.history_busy().await {
            return Ok(IgnoreSaveOutcome::Refused {
                reason: IgnoreRefusal::HistoryBusy,
            });
        }
        if current.base != request.base && !request.overwrite {
            // Тот же текст, что на диске, расхождением не считается, даже
            // если правку начинали с другой версии: терять нечего.
            let same = current.exists && current.text == request.text;
            if !same {
                let diff = line_diff(&request.text, &current.text);
                return Ok(IgnoreSaveOutcome::Diverged {
                    disk: current,
                    diff,
                });
            }
        }
        let missing = ignore::check(file, &request.text).missing;
        if !missing.is_empty() && !request.confirm_incomplete {
            return Ok(IgnoreSaveOutcome::NeedsConfirmation { missing });
        }

        let bytes = request.line_ending.to_file(&request.text).into_bytes();
        write_file(backend, file.path(), Some(&bytes)).await?;
        // Полные правила Syncthing снимают прежний отказ от них, как и
        // `apply_ignore`; неполные его не трогают — предупреждение остаётся
        // (FR-012).
        if file == IgnoreFile::StIgnore && missing.is_empty() && ctx.device_sync.stignore_declined {
            self.update_device(ctx, |device| device.stignore_declined = false);
        }
        self.events.push(SyncEvent::OverviewChanged);
        Ok(IgnoreSaveOutcome::Saved {
            document: self.ignore_open(backend, file).await,
        })
    }

    /// Есть ли у истории незавершённая операция.
    async fn history_busy(&self) -> bool {
        if self.pull.is_some() {
            return true;
        }
        matches!(self.engine.probe().await, Ok(RepoProbe::Local))
            && self
                .engine
                .head()
                .await
                .is_ok_and(|head| head.in_progress.is_some())
    }

    // ── История и снимки (US1) ────────────────────────────────────────────

    /// Включает историю (FR-001, FR-028). Правила `.gitignore` дописываются
    /// здесь же: мастер уже показал их текст, вызов и есть подтверждение.
    pub async fn enable_history<B: VaultBackend>(
        &mut self,
        backend: &mut B,
        ctx: &SyncContext,
        message: &str,
    ) -> Result<Snapshot, SyncError> {
        match self.engine.probe().await? {
            RepoProbe::Absent => {}
            RepoProbe::Local | RepoProbe::Mirror => return Err(SyncError::HistoryExists),
            RepoProbe::ForeignParent { .. } => return Err(SyncError::ForeignRepository),
        }
        if syncthing_detected(backend).await && !ctx.device_sync.stignore_declined {
            let report = ignore::report(
                IgnoreFile::StIgnore,
                read_text(backend, IgnoreFile::StIgnore.path())
                    .await
                    .as_deref(),
            );
            if !report.is_complete() {
                return Err(SyncError::StIgnoreNotConfirmed);
            }
        }
        let author = self.author(ctx).await?;
        let cancel = self.fresh_cancel();
        self.apply_ignore(backend, ctx, IgnoreFile::GitIgnore)
            .await?;
        self.engine.init(DEFAULT_BRANCH).await?;
        if cancel.is_cancelled() {
            return Err(SyncError::Cancelled);
        }
        self.progress(Operation::FirstSnapshot, 0, 1);
        let paths: Vec<String> = self
            .engine
            .status()
            .await?
            .into_iter()
            .map(|change| change.path)
            .collect();
        if cancel.is_cancelled() {
            // История включена, но без снимка: отмена не оставляет коммита.
            return Err(SyncError::Cancelled);
        }
        let snapshot = self.commit(&paths, message, &author, ctx.now, None).await?;
        self.progress(Operation::FirstSnapshot, 1, 1);
        self.update_device(ctx, |device| device.last_seen_history = true);
        self.events.push(SyncEvent::OverviewChanged);
        Ok(snapshot)
    }

    async fn author(&self, ctx: &SyncContext) -> Result<super::Author, SyncError> {
        if let Some(author) = ctx.device_sync.author.as_ref().filter(|a| a.is_valid()) {
            return Ok(author.clone());
        }
        self.engine
            .global_author()
            .await
            .filter(|author| author.is_valid())
            .ok_or(SyncError::AuthorRequired)
    }

    /// Изменения с последнего снимка (FR-004), с переименованиями.
    pub async fn changes<B: VaultBackend>(&self, backend: &B) -> Result<Vec<Change>, SyncError> {
        self.require_local().await?;
        let raw = self.engine.status().await?;
        Ok(pair_renames(raw, |old, new| async move {
            let before = self.engine.read_blob("HEAD", &old).await.ok().flatten();
            let after = backend.read_bytes(Path::new(&new)).await.ok();
            before.is_some() && before == after
        })
        .await)
    }

    async fn require_local(&self) -> Result<(), SyncError> {
        match self.engine.probe().await? {
            RepoProbe::Local => Ok(()),
            // Чужая история показывается, но приложение в неё не пишет: снимок
            // в репозитории, который пользователь завёл для другого, —
            // самоуправство (Edge Cases).
            RepoProbe::ForeignParent { .. } => Err(SyncError::ForeignRepository),
            _ => Err(SyncError::NoHistory),
        }
    }

    /// Снимок всех или выбранных изменений (FR-005…FR-007, FR-030).
    ///
    /// `flushed` — вызывающая сторона сохранила открытые заметки (FR-006):
    /// снимок того, чего пользователь не видит, хуже отказа.
    #[allow(clippy::too_many_arguments)]
    pub async fn snapshot<B: VaultBackend>(
        &mut self,
        backend: &B,
        ctx: &SyncContext,
        paths: Option<Vec<String>>,
        message: &str,
        auto_prefix: &str,
        flushed: bool,
        confirm_not_keeper: bool,
    ) -> Result<Snapshot, SyncError> {
        if !flushed {
            return Err(SyncError::NotFlushed);
        }
        self.require_local().await?;
        self.gate_write(backend, ctx, confirm_not_keeper).await?;
        let author = self.author(ctx).await?;
        let changes = self.changes(backend).await?;
        let mut selected: Vec<String> = match paths {
            Some(paths) => paths,
            None => changes.iter().map(|change| change.path.clone()).collect(),
        };
        // У переименования в снимок обязаны попасть обе стороны, иначе старое
        // имя останется в истории висеть удалённым не тем снимком.
        for change in &changes {
            if let ChangeKind::Renamed { from } = &change.change {
                if selected.contains(&change.path) && !selected.contains(from) {
                    selected.push(from.clone());
                }
            }
        }
        if selected.is_empty() {
            return Err(SyncError::NothingToSnapshot);
        }
        let message = if message.trim().is_empty() {
            auto_message(auto_prefix, &changes, &selected)
        } else {
            message.trim().to_string()
        };
        let snapshot = self
            .commit(&selected, &message, &author, ctx.now, None)
            .await?;
        self.events.push(SyncEvent::OverviewChanged);
        Ok(snapshot)
    }

    async fn commit(
        &mut self,
        paths: &[String],
        message: &str,
        author: &super::Author,
        now: Millis,
        merge_parent: Option<&str>,
    ) -> Result<Snapshot, SyncError> {
        let id = self
            .engine
            .commit(paths, message, author, now, merge_parent)
            .await?;
        let entry = self
            .engine
            .log(Some(&id), 0, 1)
            .await?
            .entries
            .into_iter()
            .next()
            .ok_or_else(|| SyncError::Io {
                message: "снимок не найден после записи".to_string(),
            })?;
        Ok(snapshot_of(entry, None, None))
    }

    /// Проверки перед записью истории: незавершённая операция, конфликты
    /// получения, хранитель (FR-017, FR-030, FR-032).
    async fn gate_write<B: VaultBackend>(
        &self,
        backend: &B,
        ctx: &SyncContext,
        confirm_not_keeper: bool,
    ) -> Result<(), SyncError> {
        if self.engine.head().await?.in_progress.is_some() {
            return Err(SyncError::InProgressOperation);
        }
        if self
            .pull
            .as_ref()
            .is_some_and(|pull| !pull.conflicts.is_empty())
        {
            return Err(SyncError::PullConflictsPending);
        }
        let settings = sync_settings::read(backend).await;
        let copies = conflict_copies(backend).await;
        match self.keeper_state(&settings, ctx, &copies) {
            KeeperState::Disputed if !confirm_not_keeper => Err(SyncError::KeeperDisputed),
            KeeperState::OtherDevice if !confirm_not_keeper => Err(SyncError::KeeperElsewhere {
                keeper: settings.keeper.clone().expect("OtherDevice без хранителя"),
            }),
            _ => Ok(()),
        }
    }

    // ── Просмотр и откат (US2) ─────────────────────────────────────────────

    /// История vault или файла (FR-009), с прослеживанием переименований.
    pub async fn history<B: VaultBackend>(
        &self,
        backend: &B,
        path: Option<&str>,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<HistoryPage, SyncError> {
        let (engine, start) = self.reading_engine(backend).await?;
        let limit = limit.clamp(1, 200);
        let Some(path) = path else {
            let offset = cursor.and_then(|c| c.parse().ok()).unwrap_or(0);
            let page = engine.log(start.as_deref(), offset, limit).await?;
            return Ok(HistoryPage {
                snapshots: page
                    .entries
                    .into_iter()
                    .map(|entry| snapshot_of(entry, None, None))
                    .collect(),
                cursor: page.next.map(|next| next.to_string()),
            });
        };

        // Курсор истории файла: смещение в журнале и имя файла на этот момент.
        let (mut offset, mut current) = match cursor.and_then(|c| c.split_once('\n')) {
            Some((offset, name)) => (offset.parse().unwrap_or(0), name.to_string()),
            None => (0u32, path.to_string()),
        };
        let mut found = Vec::new();
        let mut scanned = 0;
        loop {
            let page = engine.log(start.as_deref(), offset, LOG_CHUNK).await?;
            let exhausted = page.next.is_none();
            for entry in page.entries {
                offset += 1;
                scanned += 1;
                let raw = engine
                    .tree_diff(entry.parents.first().map(String::as_str), &entry.id)
                    .await?;
                let changes = pair_renames(raw, |_, _| async { false }).await;
                let hit = changes.into_iter().find(|change| change.path == current);
                if let Some(change) = hit {
                    let name = current.clone();
                    if let ChangeKind::Renamed { from } = &change.change {
                        current = from.clone();
                    }
                    found.push(snapshot_of(entry, Some(name), Some(change.change)));
                    if found.len() as u32 >= limit {
                        return Ok(HistoryPage {
                            snapshots: found,
                            cursor: Some(format!("{offset}\n{current}")),
                        });
                    }
                }
            }
            if exhausted {
                return Ok(HistoryPage {
                    snapshots: found,
                    cursor: None,
                });
            }
            if scanned >= HISTORY_SCAN_LIMIT {
                return Ok(HistoryPage {
                    snapshots: found,
                    cursor: Some(format!("{offset}\n{current}")),
                });
            }
        }
    }

    /// Где читать историю: своя или зеркало (FR-033). Второй элемент —
    /// начальная ревизия.
    async fn reading_engine<B: VaultBackend>(
        &self,
        _backend: &B,
    ) -> Result<(&E, Option<String>), SyncError> {
        match self.engine.probe().await? {
            RepoProbe::Local => Ok((&self.engine, None)),
            _ => match &self.mirror {
                Some(mirror) if matches!(mirror.probe().await, Ok(RepoProbe::Mirror)) => {
                    Ok((mirror, None))
                }
                _ => Err(SyncError::NoHistory),
            },
        }
    }

    async fn version<B: VaultBackend>(
        &self,
        backend: &B,
        path: &str,
        revision: &Revision,
    ) -> Result<Option<Vec<u8>>, SyncError> {
        let (engine, _) = match revision {
            // Путь приходит из окна: рабочий файл читается только внутри vault.
            Revision::Working => {
                let path = safe_path(path).ok_or(SyncError::Io {
                    message: "недопустимый путь".to_string(),
                })?;
                return Ok(read_bytes(backend, &path).await);
            }
            _ => self.reading_engine(backend).await?,
        };
        let rev = match revision {
            Revision::Working => unreachable!(),
            Revision::Head => "HEAD".to_string(),
            Revision::Snapshot { id } => id.clone(),
            Revision::ParentOf { id } => {
                let parent = engine
                    .log(Some(id), 0, 1)
                    .await?
                    .entries
                    .into_iter()
                    .next()
                    .and_then(|entry| entry.parents.into_iter().next());
                match parent {
                    Some(parent) => parent,
                    None => return Ok(None),
                }
            }
        };
        Ok(engine.read_blob(&rev, path).await?)
    }

    /// Сравнение двух версий файла (FR-010).
    pub async fn diff<B: VaultBackend>(
        &self,
        backend: &B,
        path: &str,
        from: &Revision,
        to: &Revision,
    ) -> Result<FileDiff, SyncError> {
        let old = self.version(backend, path, from).await?;
        let new = self.version(backend, path, to).await?;
        Ok(file_diff(old, new))
    }

    /// Содержимое файла в снимке.
    pub async fn file_at<B: VaultBackend>(
        &self,
        backend: &B,
        path: &str,
        revision: &Revision,
    ) -> Result<Content, SyncError> {
        Ok(Content::from_bytes(
            self.version(backend, path, revision).await?,
        ))
    }

    /// Что изменил снимок относительно предыдущего (FR-009, FR-011): по этому
    /// списку в истории vault находят удалённый файл, чтобы вернуть его.
    pub async fn snapshot_changes<B: VaultBackend>(
        &self,
        backend: &B,
        snapshot: &str,
    ) -> Result<Vec<Change>, SyncError> {
        let (engine, _) = self.reading_engine(backend).await?;
        let entry = engine
            .log(Some(snapshot), 0, 1)
            .await?
            .entries
            .into_iter()
            .next()
            .ok_or_else(|| SyncError::Io {
                message: "снимок не найден".to_string(),
            })?;
        let raw = engine
            .tree_diff(entry.parents.first().map(String::as_str), &entry.id)
            .await?;
        Ok(pair_renames(raw, |_, _| async { false }).await)
    }

    /// Возвращает файл к версии из снимка или восстанавливает удалённый
    /// (FR-011). Это обычная запись файла: истории она не меняет.
    pub async fn restore<B: VaultBackend>(
        &mut self,
        backend: &mut B,
        path: &str,
        snapshot: &str,
    ) -> Result<Content, SyncError> {
        let safe = safe_path(path).ok_or(SyncError::Io {
            message: "недопустимый путь".to_string(),
        })?;
        let bytes = self
            .version(
                backend,
                path,
                &Revision::Snapshot {
                    id: snapshot.to_string(),
                },
            )
            .await?;
        write_file(backend, &safe, bytes.as_deref()).await?;
        self.events.push(SyncEvent::FilesChanged {
            paths: vec![safe.clone()],
        });
        self.events.push(SyncEvent::OverviewChanged);
        Ok(Content::from_bytes(bytes))
    }

    // ── Удалённый репозиторий (US3) ────────────────────────────────────────

    /// Подключает удалённый репозиторий (FR-013, FR-014, FR-014a).
    pub async fn connect_remote<B: VaultBackend, S: SecretStore>(
        &mut self,
        backend: &mut B,
        secrets: &S,
        ctx: &SyncContext,
        url: &str,
        credentials: Credentials,
    ) -> Result<RemoteStatus, SyncError> {
        let parsed = parse_remote(url, ctx)?;
        let user = credentials.user.or(parsed.user.clone());
        let token = credentials.token.or(parsed.secret.clone());
        let store_err = |_| SyncError::Io {
            message: "хранилище секретов недоступно".to_string(),
        };
        // Подключение — и есть подтверждение адреса на этом устройстве: дальше
        // обмен и токен идут только ему (см. `remote_spec`).
        secrets
            .set(&self.vault_id, SecretKind::RemoteOrigin, &parsed.url)
            .await
            .map_err(store_err)?;
        if parsed.scheme != remote_url::Scheme::Ssh {
            match &user {
                Some(user) => secrets
                    .set(&self.vault_id, SecretKind::RemoteUser, user)
                    .await
                    .map_err(store_err)?,
                None => secrets
                    .delete(&self.vault_id, SecretKind::RemoteUser)
                    .await
                    .map_err(store_err)?,
            }
            match &token {
                Some(token) => secrets
                    .set(&self.vault_id, SecretKind::RemoteToken, token)
                    .await
                    .map_err(store_err)?,
                None => secrets
                    .delete(&self.vault_id, SecretKind::RemoteToken)
                    .await
                    .map_err(store_err)?,
            }
        }
        let mut settings = sync_settings::read(backend).await;
        settings.remote_url = Some(parsed.url.clone());
        sync_settings::write(backend, &settings).await?;
        if matches!(self.engine.probe().await?, RepoProbe::Local) {
            self.engine.set_remote_url(Some(&parsed.url)).await?;
            // Проверка доступа (FR-013 AS1): получение снимков без изменения
            // файлов. Ошибка не отменяет подключения — адрес сохранён, а
            // причина видна в индикаторе.
            if let Err(error) = self.fetch(backend, secrets, ctx).await {
                self.events.push(SyncEvent::OverviewChanged);
                return Err(error);
            }
        }
        self.events.push(SyncEvent::OverviewChanged);
        Ok(self.remote_status(&parsed.url, &settings, true).await)
    }

    /// Отключает удалённый репозиторий: история и файлы остаются (FR-038).
    pub async fn disconnect_remote<B: VaultBackend, S: SecretStore>(
        &mut self,
        backend: &mut B,
        secrets: &S,
    ) -> Result<(), SyncError> {
        let mut settings = sync_settings::read(backend).await;
        settings.remote_url = None;
        sync_settings::write(backend, &settings).await?;
        if matches!(self.engine.probe().await?, RepoProbe::Local) {
            self.engine.set_remote_url(None).await?;
        }
        for kind in [
            SecretKind::RemoteUser,
            SecretKind::RemoteToken,
            SecretKind::RemoteOrigin,
        ] {
            let _ = secrets.delete(&self.vault_id, kind).await;
        }
        self.fetched_at = None;
        self.last_error = None;
        self.events.push(SyncEvent::OverviewChanged);
        Ok(())
    }

    async fn auth<S: SecretStore>(&self, secrets: &S) -> Auth {
        Auth {
            user: secrets
                .get(&self.vault_id, SecretKind::RemoteUser)
                .await
                .ok()
                .flatten(),
            token: secrets
                .get(&self.vault_id, SecretKind::RemoteToken)
                .await
                .ok()
                .flatten(),
        }
    }

    /// Куда идёт обмен. Адрес лежит в настройках vault — файле, который
    /// доставляется вместе с vault (историей, Syncthing) и доступен на запись
    /// всем, с кем vault общий. Поэтому обмен идёт только с адресом,
    /// подтверждённым на этом устройстве подключением: иначе правка чужими
    /// руками увела бы токен и заметки на чужой сервер (T068).
    async fn remote_spec<B: VaultBackend, S: SecretStore>(
        &self,
        backend: &B,
        secrets: &S,
        ctx: &SyncContext,
    ) -> Result<RemoteSpec, SyncError> {
        let settings = sync_settings::read(backend).await;
        let url = settings.remote_url.clone().ok_or(SyncError::NoRemote)?;
        let confirmed = secrets
            .get(&self.vault_id, SecretKind::RemoteOrigin)
            .await
            .ok()
            .flatten();
        if confirmed.as_deref() != Some(url.as_str()) {
            return Err(SyncError::RemoteChanged { url });
        }
        // Схема обязана быть доступной этой оболочке и при обмене, а не только
        // при подключении.
        parse_remote(&url, ctx)?;
        let head_branch = self.engine.head().await.ok().and_then(|head| head.branch);
        Ok(RemoteSpec {
            url,
            branch: branch_of(&settings, head_branch.as_deref()),
            cors_proxy: ctx.device_sync.cors_proxy.clone(),
        })
    }

    fn record_exchange(&mut self, now: Millis, result: &Result<(), SyncError>) {
        match result {
            Ok(()) => {
                self.fetched_at = Some(now);
                self.last_error = None;
            }
            Err(SyncError::Cancelled) => {}
            Err(error) => self.last_error = Some(error.clone()),
        }
        self.events.push(SyncEvent::OverviewChanged);
    }

    async fn fetch<B: VaultBackend, S: SecretStore>(
        &mut self,
        backend: &B,
        secrets: &S,
        ctx: &SyncContext,
    ) -> Result<Option<String>, SyncError> {
        let plan = self.plan_exchange(backend, secrets, ctx).await?;
        let result = plan.fetch(&self.engine).await;
        self.record_exchange(ctx.now, &result.as_ref().map(|_| ()).map_err(Clone::clone));
        result
    }

    /// Адрес, данные доступа и свежий токен отмены — всё, что нужно сети.
    async fn plan_exchange<B: VaultBackend, S: SecretStore>(
        &mut self,
        backend: &B,
        secrets: &S,
        ctx: &SyncContext,
    ) -> Result<ExchangePlan, SyncError> {
        let spec = self.remote_spec(backend, secrets, ctx).await?;
        let auth = self.auth(secrets).await;
        let cancel = self.fresh_cancel();
        Ok(ExchangePlan { spec, auth, cancel })
    }

    /// Отправка снимков на сервер (FR-015, FR-018). Три шага подряд; оболочка,
    /// которой важно не держать vault на время сети, зовёт их по отдельности
    /// (T071): [`Self::begin_push`], [`ExchangePlan::push`],
    /// [`Self::complete_push`].
    pub async fn push<B: VaultBackend, S: SecretStore>(
        &mut self,
        backend: &B,
        secrets: &S,
        ctx: &SyncContext,
        confirm_not_keeper: bool,
    ) -> Result<RemoteStatus, SyncError> {
        let plan = self
            .begin_push(backend, secrets, ctx, confirm_not_keeper)
            .await?;
        let result = plan.push(&self.engine).await;
        self.complete_push(backend, ctx, &plan, result).await
    }

    /// Проверки и план отправки — под блокировкой vault.
    pub async fn begin_push<B: VaultBackend, S: SecretStore>(
        &mut self,
        backend: &B,
        secrets: &S,
        ctx: &SyncContext,
        confirm_not_keeper: bool,
    ) -> Result<ExchangePlan, SyncError> {
        self.require_local().await?;
        self.gate_write(backend, ctx, confirm_not_keeper).await?;
        let plan = self.plan_exchange(backend, secrets, ctx).await?;
        self.progress(Operation::Push, 0, 1);
        Ok(plan)
    }

    /// Итог отправки: запись об обмене и состояние удалённого репозитория.
    pub async fn complete_push<B: VaultBackend>(
        &mut self,
        backend: &B,
        ctx: &SyncContext,
        plan: &ExchangePlan,
        result: Result<(), SyncError>,
    ) -> Result<RemoteStatus, SyncError> {
        self.record_exchange(ctx.now, &result);
        result?;
        self.progress(Operation::Push, 1, 1);
        let settings = sync_settings::read(backend).await;
        Ok(self.remote_status(&plan.spec.url, &settings, true).await)
    }

    /// Получение снимков с сервера со слиянием (FR-016, FR-017, research §9).
    /// Возвращает утраченную историю из удалённого репозитория (Edge Cases
    /// «история удалена», T079).
    ///
    /// Локальной истории нет, но адрес удалённого репозитория записан в
    /// настройках vault — значит, история где-то цела. Заводится пустая
    /// локальная, и в неё идёт обычное получение: файлы, совпавшие с
    /// удалённой версией, остаются как есть, разошедшиеся становятся
    /// конфликтами того же рода, что при любом получении. Ничего не
    /// перезаписывается молча.
    ///
    /// Если получение не удалось (нет сети, отказ доступа), пустая история
    /// остаётся — получение можно повторить обычным порядком.
    pub async fn restore_history<B: VaultBackend, S: SecretStore>(
        &mut self,
        backend: &mut B,
        secrets: &S,
        ctx: &SyncContext,
        confirm_not_keeper: bool,
    ) -> Result<PullOutcome, SyncError> {
        match self.engine.probe().await? {
            RepoProbe::Absent => {}
            RepoProbe::Local | RepoProbe::Mirror => return Err(SyncError::HistoryExists),
            RepoProbe::ForeignParent { .. } => return Err(SyncError::ForeignRepository),
        }
        // До создания истории: адрес обязан быть записан и подтверждён на этом
        // устройстве (T068). Иначе отказ оставил бы после себя пустую историю,
        // которую вернуть уже нечем.
        self.remote_spec(backend, secrets, ctx).await?;
        self.engine.init(DEFAULT_BRANCH).await?;
        self.events.push(SyncEvent::OverviewChanged);
        self.pull(backend, secrets, ctx, confirm_not_keeper).await
    }

    /// Получение снимков: подготовка, сеть, запись файлов. Оболочка, которой
    /// важно не держать vault на время сети, зовёт шаги по отдельности (T071):
    /// [`Self::begin_pull`], [`ExchangePlan::fetch`], [`Self::complete_pull`].
    pub async fn pull<B: VaultBackend, S: SecretStore>(
        &mut self,
        backend: &mut B,
        secrets: &S,
        ctx: &SyncContext,
        confirm_not_keeper: bool,
    ) -> Result<PullOutcome, SyncError> {
        let plan = self
            .begin_pull(backend, secrets, ctx, confirm_not_keeper)
            .await?;
        let fetched = plan.exchange.fetch(&self.engine).await;
        self.complete_pull(backend, ctx, plan, fetched).await
    }

    /// Проверки и план получения — под блокировкой vault.
    pub async fn begin_pull<B: VaultBackend, S: SecretStore>(
        &mut self,
        backend: &B,
        secrets: &S,
        ctx: &SyncContext,
        confirm_not_keeper: bool,
    ) -> Result<PullPlan, SyncError> {
        if self.pull.is_some() {
            return Err(SyncError::PullConflictsPending);
        }
        self.require_local().await?;
        self.gate_write(backend, ctx, confirm_not_keeper).await?;
        let author = self.author(ctx).await?;
        let exchange = self.plan_exchange(backend, secrets, ctx).await?;
        Ok(PullPlan { exchange, author })
    }

    /// Слияние полученного с файлами vault — под блокировкой vault. `fetched`
    /// — итог сетевого шага: снимок сервера или отказ.
    pub async fn complete_pull<B: VaultBackend>(
        &mut self,
        backend: &mut B,
        ctx: &SyncContext,
        plan: PullPlan,
        fetched: Result<Option<String>, SyncError>,
    ) -> Result<PullOutcome, SyncError> {
        self.record_exchange(ctx.now, &fetched.as_ref().map(|_| ()).map_err(Clone::clone));
        let author = plan.author;
        // Пока шла сеть, получение могли начать другим путём.
        if self.pull.is_some() {
            return Err(SyncError::PullConflictsPending);
        }
        let Some(theirs) = fetched? else {
            return Ok(PullOutcome {
                applied: 0,
                conflicts: 0,
                up_to_date: true,
            });
        };
        let head = self.engine.head().await?.oid;
        let base = match &head {
            Some(head) if *head == theirs => {
                return Ok(PullOutcome {
                    applied: 0,
                    conflicts: 0,
                    up_to_date: true,
                })
            }
            Some(head) => self.engine.merge_base(head, &theirs).await?,
            None => None,
        };
        if base.as_deref() == Some(theirs.as_str()) {
            // Сервер позади — получать нечего, всё уже здесь.
            return Ok(PullOutcome {
                applied: 0,
                conflicts: 0,
                up_to_date: true,
            });
        }

        let incoming: Vec<RawChange> = self.engine.tree_diff(base.as_deref(), &theirs).await?;
        let cancel = self.fresh_cancel();
        let mut pending = PendingPull {
            theirs: theirs.clone(),
            head_before: head.clone(),
            journal: BTreeMap::new(),
            incoming: BTreeSet::new(),
            conflicts: BTreeMap::new(),
            fast_forward: head.is_none() || head == base,
        };
        let total = incoming.len() as u32;
        let mut applied = 0;
        for (index, change) in incoming.iter().enumerate() {
            if cancel.is_cancelled() {
                self.rollback(backend, &pending).await;
                return Err(SyncError::Cancelled);
            }
            let Some(path) = safe_path(&change.path) else {
                self.rollback(backend, &pending).await;
                return Err(SyncError::Io {
                    message: format!("недопустимый путь с сервера: {}", change.path),
                });
            };
            pending.incoming.insert(path.clone());
            // Три версии файла. Любой отказ чтения прерывает получение с
            // откатом уже записанного: «не прочиталось» нельзя принять за
            // «файла нет» — тогда нечитаемый локальный файл (слишком большой,
            // без прав, каталог на месте файла) был бы молча перезаписан.
            let versions = async {
                let ours = read_local(backend, &path).await?;
                let base_bytes = match &base {
                    Some(base) => self.engine.read_blob(base, &path).await?,
                    None => None,
                };
                let theirs_bytes = if change.status == RawStatus::Deleted {
                    None
                } else {
                    self.engine.read_blob(&theirs, &path).await?
                };
                Ok::<_, SyncError>((ours, base_bytes, theirs_bytes))
            }
            .await;
            let (ours, base_bytes, theirs_bytes) = match versions {
                Ok(versions) => versions,
                Err(error) => {
                    self.rollback(backend, &pending).await;
                    return Err(error);
                }
            };
            let decision = decide_file(&base_bytes, &ours, &theirs_bytes);
            match decision {
                FileDecision::Keep => {}
                FileDecision::Write(bytes) => {
                    pending.journal.insert(path.clone(), ours.clone());
                    if let Err(error) = write_file(backend, &path, bytes.as_deref()).await {
                        self.rollback(backend, &pending).await;
                        return Err(error);
                    }
                    if ours != base_bytes {
                        // Наши незафиксированные правки слились — это уже не
                        // перемотка, а слияние.
                        pending.fast_forward = false;
                    }
                    applied += 1;
                }
                FileDecision::Conflict => {
                    pending.fast_forward = false;
                    pending.conflicts.insert(
                        path.clone(),
                        PullConflict {
                            base: base_bytes,
                            ours,
                            theirs: theirs_bytes,
                        },
                    );
                }
            }
            self.progress(Operation::Pull, index as u32 + 1, total);
        }

        let written: Vec<String> = pending.journal.keys().cloned().collect();
        let conflicts = pending.conflicts.len() as u32;
        if conflicts == 0 {
            self.finish_pull(pending, &author, ctx.now).await?;
        } else {
            self.pull = Some(pending);
        }
        if !written.is_empty() {
            self.events.push(SyncEvent::FilesChanged { paths: written });
        }
        self.events.push(SyncEvent::OverviewChanged);
        Ok(PullOutcome {
            applied,
            conflicts,
            up_to_date: false,
        })
    }

    async fn finish_pull(
        &mut self,
        pending: PendingPull,
        author: &super::Author,
        now: Millis,
    ) -> Result<(), SyncError> {
        if pending.fast_forward {
            self.engine.set_head(&pending.theirs).await?;
        } else {
            let paths: Vec<String> = pending.incoming.iter().cloned().collect();
            let message = format!("Merge {}", short(&pending.theirs));
            self.engine
                .commit(&paths, &message, author, now, Some(&pending.theirs))
                .await?;
        }
        self.pull = None;
        Ok(())
    }

    /// Отменяет незавершённое получение: файлы возвращаются к версиям до него
    /// (FR-016).
    pub async fn abort_pull<B: VaultBackend>(&mut self, backend: &mut B) -> Result<(), SyncError> {
        let pending = self.pull.take().ok_or(SyncError::ConflictNotFound)?;
        self.rollback(backend, &pending).await;
        let _ = pending.head_before;
        self.events.push(SyncEvent::FilesChanged {
            paths: pending.journal.keys().cloned().collect(),
        });
        self.events.push(SyncEvent::OverviewChanged);
        Ok(())
    }

    async fn rollback<B: VaultBackend>(&mut self, backend: &mut B, pending: &PendingPull) {
        self.progress_sink.set(None);
        for (path, before) in &pending.journal {
            let _ = write_file(backend, path, before.as_deref()).await;
        }
    }

    // ── Конфликты (US3, US4, US5) ──────────────────────────────────────────

    pub async fn conflicts<B: VaultBackend>(&self, backend: &B) -> Vec<Conflict> {
        let mut list = Vec::new();
        if let Some(pull) = &self.pull {
            for (path, conflict) in &pull.conflicts {
                list.push(build_conflict(
                    format!("pull:{path}"),
                    ConflictSource::Pull {
                        theirs_snapshot: pull.theirs.clone(),
                    },
                    path.clone(),
                    conflict.ours.clone(),
                    conflict.theirs.clone(),
                    Some(conflict.base.clone()),
                ));
            }
        }
        for copy in conflict_copies(backend).await {
            let Some(name) = copy
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(conflict_copy::parse)
            else {
                continue;
            };
            let original = copy.with_file_name(&name.original);
            let original = to_slash(&original);
            let copy_path = to_slash(&copy);
            let ours = read_bytes(backend, &original).await;
            let theirs = read_bytes(backend, &copy_path).await;
            // Общая версия — последний снимок файла (FR-034).
            let base = match self.reading_engine(backend).await {
                Ok((engine, _)) => Some(engine.read_blob("HEAD", &original).await.ok().flatten()),
                Err(_) => None,
            };
            let device = self
                .syncthing
                .as_ref()
                .and_then(|folder| folder.device_name(&name.device))
                .unwrap_or(&name.device)
                .to_string();
            list.push(build_conflict(
                format!("st:{copy_path}"),
                ConflictSource::Syncthing {
                    copy_path,
                    stamp: name.stamp,
                    device,
                },
                original,
                ours,
                theirs,
                base,
            ));
        }
        list
    }

    /// Решение по конфликту (FR-016, FR-025).
    pub async fn resolve<B: VaultBackend>(
        &mut self,
        backend: &mut B,
        ctx: &SyncContext,
        id: &str,
        resolution: Resolution,
    ) -> Result<(), SyncError> {
        if let Some(path) = id.strip_prefix("pull:") {
            return self.resolve_pull(backend, ctx, path, resolution).await;
        }
        let copy = id.strip_prefix("st:").ok_or(SyncError::ConflictNotFound)?;
        let copy = safe_path(copy).ok_or(SyncError::ConflictNotFound)?;
        let name = Path::new(&copy)
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(conflict_copy::parse)
            .ok_or(SyncError::ConflictNotFound)?;
        // Имя исходного файла выведено из имени копии, то есть из чужих данных:
        // `..sync-conflict-…` дало бы `..`, а `.git.sync-conflict-…` — `.git`.
        let original = safe_path(&to_slash(&Path::new(&copy).with_file_name(&name.original)))
            .ok_or(SyncError::ConflictNotFound)?;
        let copy_bytes = read_bytes(backend, &copy)
            .await
            .ok_or(SyncError::ConflictNotFound)?;
        match resolution {
            Resolution::KeepOurs => {}
            Resolution::TakeTheirs => {
                write_file(backend, &original, Some(&copy_bytes)).await?;
            }
            Resolution::Merged { text } => {
                write_file(backend, &original, Some(text.as_bytes())).await?;
            }
            Resolution::KeepBothAsFile { new_path } => {
                let target = safe_path(&new_path).ok_or(SyncError::Io {
                    message: "недопустимый путь".to_string(),
                })?;
                if read_bytes(backend, &target).await.is_some() {
                    return Err(SyncError::Io {
                        message: "файл с таким именем уже есть".to_string(),
                    });
                }
                write_file(backend, &target, Some(&copy_bytes)).await?;
            }
        }
        write_file(backend, &copy, None).await?;
        self.events.push(SyncEvent::FilesChanged {
            paths: vec![original],
        });
        self.events.push(SyncEvent::OverviewChanged);
        Ok(())
    }

    async fn resolve_pull<B: VaultBackend>(
        &mut self,
        backend: &mut B,
        ctx: &SyncContext,
        path: &str,
        resolution: Resolution,
    ) -> Result<(), SyncError> {
        let pending = self.pull.as_mut().ok_or(SyncError::ConflictNotFound)?;
        let conflict = pending
            .conflicts
            .get(path)
            .cloned()
            .ok_or(SyncError::ConflictNotFound)?;
        let content: Option<Vec<u8>> = match resolution {
            Resolution::KeepOurs => conflict.ours.clone(),
            Resolution::TakeTheirs => conflict.theirs.clone(),
            Resolution::Merged { text } => Some(text.into_bytes()),
            Resolution::KeepBothAsFile { .. } => return Err(SyncError::ConflictNotFound),
        };
        pending
            .journal
            .entry(path.to_string())
            .or_insert(conflict.ours.clone());
        write_file(backend, path, content.as_deref()).await?;
        let pending = self.pull.as_mut().expect("проверено выше");
        pending.conflicts.remove(path);
        if pending.conflicts.is_empty() {
            let pending = self.pull.take().expect("проверено выше");
            let author = self.author(ctx).await?;
            self.finish_pull(pending, &author, ctx.now).await?;
        }
        self.events.push(SyncEvent::FilesChanged {
            paths: vec![path.to_string()],
        });
        self.events.push(SyncEvent::OverviewChanged);
        Ok(())
    }

    // ── Хранитель и настройки (US5) ────────────────────────────────────────

    /// Назначает это устройство хранителем (FR-029).
    pub async fn set_keeper<B: VaultBackend>(
        &mut self,
        backend: &mut B,
        ctx: &SyncContext,
    ) -> Result<super::Keeper, SyncError> {
        let device = ctx.device.as_ref().ok_or(SyncError::Io {
            message: "у устройства нет идентификатора".to_string(),
        })?;
        let mut settings = sync_settings::read(backend).await;
        let keeper = keeper::appoint(device, ctx.now);
        settings.keeper = Some(keeper.clone());
        sync_settings::write(backend, &settings).await?;
        let id = device.id.clone();
        self.update_device(ctx, |value| value.last_seen_keeper = Some(id));
        self.events.push(SyncEvent::OverviewChanged);
        Ok(keeper)
    }

    pub async fn clear_keeper<B: VaultBackend>(
        &mut self,
        backend: &mut B,
    ) -> Result<(), SyncError> {
        let mut settings = sync_settings::read(backend).await;
        settings.keeper = None;
        sync_settings::write(backend, &settings).await?;
        self.events.push(SyncEvent::OverviewChanged);
        Ok(())
    }

    pub async fn update_settings<B: VaultBackend>(
        &mut self,
        backend: &mut B,
        patch: &sync_settings::VaultSyncSettingsPatch,
    ) -> Result<VaultSyncSettings, SyncError> {
        let settings = patch.apply_to(&sync_settings::read(backend).await);
        sync_settings::write(backend, &settings).await?;
        self.events.push(SyncEvent::OverviewChanged);
        Ok(settings)
    }

    /// Обновляет зеркало не-хранителя (FR-033): клонирует при первом
    /// обращении, дальше получает новые снимки.
    pub async fn refresh_mirror<B: VaultBackend, S: SecretStore>(
        &mut self,
        backend: &B,
        secrets: &S,
        ctx: &SyncContext,
    ) -> Result<(), SyncError> {
        let plan = self.begin_mirror(backend, secrets, ctx).await?;
        let mirror = self.mirror.as_ref().ok_or(SyncError::NoHistory)?;
        let result = plan.mirror(mirror).await;
        self.complete_mirror(ctx, result)
    }

    /// План обновления зеркала — под блокировкой vault (T071).
    pub async fn begin_mirror<B: VaultBackend, S: SecretStore>(
        &mut self,
        backend: &B,
        secrets: &S,
        ctx: &SyncContext,
    ) -> Result<ExchangePlan, SyncError> {
        if self.mirror.is_none() {
            return Err(SyncError::NoHistory);
        }
        self.plan_exchange(backend, secrets, ctx).await
    }

    /// Движок зеркала — оболочка запускает сетевой шаг на его копии.
    pub fn mirror_engine(&self) -> Option<&E> {
        self.mirror.as_ref()
    }

    pub fn complete_mirror(
        &mut self,
        ctx: &SyncContext,
        result: Result<(), SyncError>,
    ) -> Result<(), SyncError> {
        if result.is_ok() {
            self.mirror_fetched_at = Some(ctx.now);
        }
        self.record_exchange(ctx.now, &result);
        result
    }

    // ── Syncthing (US4) ────────────────────────────────────────────────────

    /// Запоминает последнее известное состояние папки в Syncthing.
    /// `true` — состояние изменилось, окну пора перечитать сводку.
    pub fn set_syncthing_state(&mut self, folder: SyncthingFolder) -> bool {
        if self.syncthing.as_ref() == Some(&folder) {
            return false;
        }
        self.syncthing = Some(folder);
        self.events.push(SyncEvent::OverviewChanged);
        true
    }

    /// Сохраняет параметры доступа к Syncthing (FR-022): адрес — в значениях
    /// устройства, ключ — в хранилище секретов.
    pub async fn syncthing_connect<S: SecretStore>(
        &mut self,
        secrets: &S,
        ctx: &SyncContext,
        endpoint: syncthing::SyncthingEndpoint,
        api_key: &str,
    ) -> Result<(), SyncError> {
        if !endpoint.is_loopback() {
            return Err(SyncError::UnsupportedScheme {
                scheme: endpoint.address.clone(),
            });
        }
        secrets
            .set(&self.vault_id, SecretKind::SyncthingApiKey, api_key)
            .await
            .map_err(|_| SyncError::Io {
                message: "хранилище секретов недоступно".to_string(),
            })?;
        self.update_device(ctx, |device| device.syncthing = Some(endpoint));
        self.syncthing = None;
        self.events.push(SyncEvent::OverviewChanged);
        Ok(())
    }

    /// Закрепляет сертификат подключённого Syncthing при первом обращении
    /// (research §8). Отпечаток добывает оболочка — сеть есть только у неё.
    pub fn syncthing_pinned(&mut self, ctx: &SyncContext, fingerprint: String) {
        self.update_device(ctx, |device| {
            if let Some(endpoint) = device.syncthing.as_mut() {
                if endpoint.tls && endpoint.cert_fingerprint.is_none() {
                    endpoint.cert_fingerprint = Some(fingerprint);
                }
            }
        });
    }

    pub async fn syncthing_disconnect<S: SecretStore>(&mut self, secrets: &S, ctx: &SyncContext) {
        let _ = secrets
            .delete(&self.vault_id, SecretKind::SyncthingApiKey)
            .await;
        self.update_device(ctx, |device| device.syncthing = None);
        self.syncthing = None;
        self.events.push(SyncEvent::OverviewChanged);
    }

    /// Ключ API Syncthing для запроса состояния оболочкой.
    pub async fn syncthing_api_key<S: SecretStore>(&self, secrets: &S) -> Option<String> {
        secrets
            .get(&self.vault_id, SecretKind::SyncthingApiKey)
            .await
            .ok()
            .flatten()
    }

    // ── Автоматика (FR-019, FR-031) ────────────────────────────────────────

    /// Такт автоматики: автополучение, автоснимок, автоотправка. Ошибки
    /// обмена уходят в индикатор событием, а не в отказ (FR-019).
    ///
    /// Оболочка, которой важно не держать vault на время сети, проходит те же
    /// шаги по отдельности (T071): [`Self::tick_pull_due`], получение по
    /// шагам, [`Self::tick_local`], отправка по шагам, [`Self::tick_finish`].
    pub async fn tick<B: VaultBackend, S: SecretStore>(
        &mut self,
        backend: &mut B,
        secrets: &S,
        ctx: &SyncContext,
        auto_prefix: &str,
    ) -> TickOutcome {
        let mut outcome = TickOutcome::idle(ctx.now);
        if self.tick_pull_due(backend, ctx).await {
            let result = self.pull(backend, secrets, ctx, false).await;
            self.tick_pulled(&mut outcome, result);
        }
        if self
            .tick_local(backend, ctx, auto_prefix, &mut outcome)
            .await
        {
            let result = self.push(backend, secrets, ctx, false).await;
            self.tick_pushed(&mut outcome, result);
        }
        self.tick_finish(backend, ctx, &mut outcome).await;
        outcome
    }

    /// Обмен с сервером разрешён: история своя, адрес есть, писать можно,
    /// незавершённого получения и операции git нет.
    async fn exchange_allowed<B: VaultBackend>(
        &self,
        backend: &B,
        ctx: &SyncContext,
        settings: &VaultSyncSettings,
    ) -> bool {
        let copies = conflict_copies(backend).await;
        let keeper_state = self.keeper_state(settings, ctx, &copies);
        let local = matches!(self.engine.probe().await, Ok(RepoProbe::Local));
        let head = self.engine.head().await.unwrap_or_default();
        local
            && settings.remote_url.is_some()
            && keeper_state.may_write_history()
            && self.pull.is_none()
            && head.in_progress.is_none()
    }

    /// Пора ли автополучение — при открытии и по интервалу.
    pub async fn tick_pull_due<B: VaultBackend>(&mut self, backend: &B, ctx: &SyncContext) -> bool {
        let settings = sync_settings::read(backend).await;
        let allowed = self.exchange_allowed(backend, ctx, &settings).await;
        let interval = settings.auto_pull_interval_secs.map(|s| s as Millis * 1000);
        let due = allowed
            && ((settings.auto_pull_on_open && !self.pull_on_open_done)
                || autosnapshot::pull_due(ctx.now, self.fetched_at, interval));
        self.pull_on_open_done = true;
        due
    }

    pub fn tick_pulled(
        &mut self,
        outcome: &mut TickOutcome,
        result: Result<PullOutcome, SyncError>,
    ) {
        match result {
            Ok(result) => outcome.pulled = result.applied > 0 || result.conflicts > 0,
            Err(error) => self.events.push(SyncEvent::ExchangeFailed { error }),
        }
    }

    /// Автоснимок без сети. Возвращает, пора ли автоотправка.
    pub async fn tick_local<B: VaultBackend>(
        &mut self,
        backend: &mut B,
        ctx: &SyncContext,
        auto_prefix: &str,
        outcome: &mut TickOutcome,
    ) -> bool {
        let settings = sync_settings::read(backend).await;
        let copies = conflict_copies(backend).await;
        let keeper_state = self.keeper_state(&settings, ctx, &copies);
        let local = matches!(self.engine.probe().await, Ok(RepoProbe::Local));
        let head = self.engine.head().await.unwrap_or_default();
        let has_changes = local
            && self
                .engine
                .status()
                .await
                .map(|changes| !changes.is_empty())
                .unwrap_or(false);
        // Изменения, о которых оболочка не сообщала (были уже при открытии,
        // пришли мимо вотчера): пауза отсчитывается с такта, который их увидел,
        // — иначе «тихо с» сдвигалось бы на каждом такте и снимок не наступал.
        if !has_changes {
            self.last_change_at = None;
        } else if self.last_change_at.is_none() {
            self.last_change_at = Some(ctx.now);
        }
        let facts = SnapshotFacts {
            enabled: settings.auto_snapshot,
            has_history: local,
            keeper: keeper_state.clone(),
            syncthing: self.syncthing.as_ref().and_then(|folder| folder.state),
            pull_conflicts: self.pull.is_some(),
            in_progress: head.in_progress.is_some(),
            has_changes,
            last_change_at: self.last_change_at,
            pause_ms: settings.auto_snapshot_pause_secs as Millis * 1000,
        };
        outcome.decision = autosnapshot::decide(ctx.now, &facts);
        match &outcome.decision {
            AutoDecision::Wait { until } => {
                outcome.next_tick_at = (*until).min(outcome.next_tick_at)
            }
            AutoDecision::Snapshot => {
                match self
                    .snapshot(backend, ctx, None, "", auto_prefix, true, false)
                    .await
                {
                    Ok(snapshot) => {
                        self.events.push(SyncEvent::AutoSnapshotDone {
                            snapshot: snapshot.clone(),
                        });
                        outcome.snapshot = Some(snapshot);
                    }
                    Err(error) => self.events.push(SyncEvent::ExchangeFailed { error }),
                }
            }
            AutoDecision::Blocked { reason } => {
                if *reason == autosnapshot::BlockReason::SyncthingBusy {
                    outcome.next_tick_at = ctx.now + 10_000;
                }
            }
        }

        // Автоотправка: после снимка или если есть неотправленное.
        if !(settings.auto_push && self.exchange_allowed(backend, ctx, &settings).await) {
            return false;
        }
        let status = self
            .remote_status(
                settings.remote_url.as_deref().unwrap_or(""),
                &settings,
                true,
            )
            .await;
        outcome.snapshot.is_some() || status.ahead.unwrap_or(0) > 0
    }

    pub fn tick_pushed(
        &mut self,
        outcome: &mut TickOutcome,
        result: Result<RemoteStatus, SyncError>,
    ) {
        match result {
            Ok(_) => outcome.pushed = true,
            Err(error) => self.events.push(SyncEvent::ExchangeFailed { error }),
        }
    }

    /// Когда спросить снова: не позже следующего автополучения.
    pub async fn tick_finish<B: VaultBackend>(
        &self,
        backend: &B,
        ctx: &SyncContext,
        outcome: &mut TickOutcome,
    ) {
        let settings = sync_settings::read(backend).await;
        if let Some(interval) = settings.auto_pull_interval_secs.map(|s| s as Millis * 1000) {
            let next_pull = self.fetched_at.unwrap_or(ctx.now) + interval;
            outcome.next_tick_at = outcome.next_tick_at.min(next_pull.max(ctx.now + 1_000));
        }
    }

    /// Приёмник хода операций: оболочка читает его, пока сессия занята (T075).
    pub fn progress_sink(&self) -> super::ProgressSink {
        self.progress_sink.clone()
    }

    fn progress(&mut self, operation: Operation, done: u32, total: u32) {
        // Операция закончилась — хода больше нет.
        self.progress_sink
            .set((done < total).then_some(super::SyncProgress {
                operation,
                done,
                total,
            }));
        self.events.push(SyncEvent::Progress {
            operation,
            done,
            total,
        });
    }
}

/// Клонирует удалённый репозиторий в пустую папку будущего vault (FR-013).
/// Данные доступа сохраняются под идентификатором будущего vault.
pub async fn clone_vault<E: GitEngine, S: SecretStore>(
    engine: &E,
    secrets: &S,
    vault_id: &str,
    capabilities: &super::ShellCapabilities,
    url: &str,
    credentials: Credentials,
    cors_proxy: Option<&str>,
    cancel: &CancelToken,
) -> Result<(), SyncError> {
    let ctx = SyncContext {
        device: None,
        device_sync: DeviceVaultSync::default(),
        now: 0,
        capabilities: *capabilities,
    };
    let parsed = parse_remote(url, &ctx)?;
    if cors_proxy.is_some_and(|proxy| !remote_url::is_proxy(proxy)) {
        return Err(SyncError::InvalidRemoteUrl);
    }
    let auth = Auth {
        user: credentials.user.or(parsed.user.clone()),
        token: credentials.token.or(parsed.secret.clone()),
    };
    if !matches!(engine.probe().await?, RepoProbe::Absent) {
        return Err(SyncError::TargetNotEmpty);
    }
    let spec = RemoteSpec {
        url: parsed.url.clone(),
        branch: DEFAULT_BRANCH.to_string(),
        cors_proxy: cors_proxy.map(str::to_string),
    };
    engine.clone_into(&spec, &auth, false, cancel).await?;
    let store_err = |_| SyncError::Io {
        message: "хранилище секретов недоступно".to_string(),
    };
    secrets
        .set(vault_id, SecretKind::RemoteOrigin, &parsed.url)
        .await
        .map_err(store_err)?;
    if let Some(user) = &auth.user {
        secrets
            .set(vault_id, SecretKind::RemoteUser, user)
            .await
            .map_err(store_err)?;
    }
    if let Some(token) = &auth.token {
        secrets
            .set(vault_id, SecretKind::RemoteToken, token)
            .await
            .map_err(store_err)?;
    }
    Ok(())
}

fn parse_remote(url: &str, ctx: &SyncContext) -> Result<remote_url::RemoteUrl, SyncError> {
    let parsed = remote_url::normalize(url).map_err(|problem| match problem {
        remote_url::UrlProblem::Unsupported(scheme) => SyncError::UnsupportedScheme { scheme },
        _ => SyncError::InvalidRemoteUrl,
    })?;
    match parsed.scheme {
        remote_url::Scheme::Ssh if !ctx.capabilities.ssh => Err(SyncError::UnsupportedScheme {
            scheme: "ssh".to_string(),
        }),
        remote_url::Scheme::File if !ctx.capabilities.direct_remote => {
            Err(SyncError::UnsupportedScheme {
                scheme: "file".to_string(),
            })
        }
        _ => Ok(parsed),
    }
}

enum FileDecision {
    Keep,
    Write(Option<Vec<u8>>),
    Conflict,
}

/// Что делать с одним файлом при получении: как у git, только с рабочим
/// файлом вместо индекса — незафиксированные правки тоже «наши».
fn decide_file(
    base: &Option<Vec<u8>>,
    ours: &Option<Vec<u8>>,
    theirs: &Option<Vec<u8>>,
) -> FileDecision {
    if ours == theirs {
        return FileDecision::Keep;
    }
    if ours == base {
        return FileDecision::Write(theirs.clone());
    }
    if theirs == base {
        return FileDecision::Keep;
    }
    // Обе стороны правили: совместить, если обе версии — текст.
    let as_text = |bytes: &Option<Vec<u8>>| -> Option<String> {
        match bytes {
            None => Some(String::new()),
            Some(bytes) if merge::is_binary(bytes) => None,
            Some(bytes) => String::from_utf8(bytes.clone()).ok(),
        }
    };
    if ours.is_none() || theirs.is_none() {
        // Удаление против правки — решает пользователь.
        return FileDecision::Conflict;
    }
    match (as_text(base), as_text(ours), as_text(theirs)) {
        (Some(b), Some(o), Some(t)) => match merge::merge3(&b, &o, &t) {
            MergeResult::Clean { text } => FileDecision::Write(Some(text.into_bytes())),
            MergeResult::Conflicted { .. } => FileDecision::Conflict,
        },
        _ => FileDecision::Conflict,
    }
}

fn build_conflict(
    id: String,
    source: ConflictSource,
    path: String,
    ours: Option<Vec<u8>>,
    theirs: Option<Vec<u8>>,
    base: Option<Option<Vec<u8>>>,
) -> Conflict {
    let ours_content = Content::from_bytes(ours);
    let theirs_content = Content::from_bytes(theirs);
    let base_content = base.map(Content::from_bytes);
    let auto_merge = match (
        ours_content.text(),
        theirs_content.text(),
        base_content.as_ref().and_then(|base| match base {
            Content::Text { text } => Some(text.as_str()),
            Content::Missing => Some(""),
            Content::Binary { .. } => None,
        }),
    ) {
        (Some(ours), Some(theirs), Some(base)) => Some(merge::merge3(base, ours, theirs)),
        _ => None,
    };
    Conflict {
        id,
        source,
        path,
        ours: ours_content,
        theirs: theirs_content,
        base: base_content,
        auto_merge,
    }
}

fn file_diff(old: Option<Vec<u8>>, new: Option<Vec<u8>>) -> FileDiff {
    let binary = |bytes: &Option<Vec<u8>>| bytes.as_ref().is_some_and(|b| merge::is_binary(b));
    if binary(&old) || binary(&new) {
        return FileDiff::Binary {
            old_size: old.as_ref().map(|b| b.len() as u64),
            new_size: new.as_ref().map(|b| b.len() as u64),
        };
    }
    let text = |bytes: Option<Vec<u8>>| {
        bytes
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_default()
    };
    FileDiff::Text {
        lines: line_diff(&text(new), &text(old)),
    }
}

/// Сводит пары «удалён + добавлен» с одинаковым содержимым в переименования.
/// Для снимков сравниваются идентификаторы объектов, для рабочих файлов —
/// содержимое (`same_content`).
async fn pair_renames<F, Fut>(raw: Vec<RawChange>, same_content: F) -> Vec<Change>
where
    F: Fn(String, String) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let mut deleted: Vec<&RawChange> = raw
        .iter()
        .filter(|change| change.status == RawStatus::Deleted)
        .collect();
    let mut result = Vec::with_capacity(raw.len());
    let mut consumed: BTreeSet<String> = BTreeSet::new();
    for change in raw
        .iter()
        .filter(|change| change.status == RawStatus::Added)
    {
        let mut matched = None;
        for (index, gone) in deleted.iter().enumerate() {
            let same = match (&gone.old_oid, &change.new_oid) {
                (Some(old), Some(new)) => old == new,
                _ => {
                    // Сравнение содержимого дорого — только для файлов одного
                    // типа и не больше разумного числа пар.
                    extension(&gone.path) == extension(&change.path)
                        && same_content(gone.path.clone(), change.path.clone()).await
                }
            };
            if same {
                matched = Some(index);
                break;
            }
        }
        match matched {
            Some(index) => {
                let gone = deleted.remove(index);
                consumed.insert(gone.path.clone());
                result.push(Change {
                    path: change.path.clone(),
                    change: ChangeKind::Renamed {
                        from: gone.path.clone(),
                    },
                });
            }
            None => result.push(Change {
                path: change.path.clone(),
                change: ChangeKind::Added,
            }),
        }
    }
    for change in &raw {
        match change.status {
            RawStatus::Modified => result.push(Change {
                path: change.path.clone(),
                change: ChangeKind::Modified,
            }),
            RawStatus::Deleted if !consumed.contains(&change.path) => result.push(Change {
                path: change.path.clone(),
                change: ChangeKind::Deleted,
            }),
            _ => {}
        }
    }
    result.sort_by(|a, b| a.path.cmp(&b.path));
    result
}

fn extension(path: &str) -> Option<&str> {
    Path::new(path).extension().and_then(|ext| ext.to_str())
}

/// Автоматическое сообщение: перечень изменённых заметок (FR-005).
fn auto_message(prefix: &str, changes: &[Change], selected: &[String]) -> String {
    let names: Vec<String> = changes
        .iter()
        .filter(|change| selected.contains(&change.path))
        .map(|change| {
            Path::new(&change.path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(&change.path)
                .to_string()
        })
        .collect();
    let shown: Vec<&str> = names
        .iter()
        .take(AUTO_MESSAGE_NAMES)
        .map(String::as_str)
        .collect();
    let rest = names.len().saturating_sub(AUTO_MESSAGE_NAMES);
    let prefix = if prefix.trim().is_empty() {
        "Snapshot"
    } else {
        prefix.trim()
    };
    if rest > 0 {
        format!("{prefix}: {} (+{rest})", shown.join(", "))
    } else {
        format!("{prefix}: {}", shown.join(", "))
    }
}

fn snapshot_of(
    entry: super::engine::LogEntry,
    path: Option<String>,
    change: Option<ChangeKind>,
) -> Snapshot {
    Snapshot {
        summary: entry.message.lines().next().unwrap_or("").to_string(),
        id: entry.id,
        message: entry.message,
        author: entry.author,
        time: entry.time,
        parents: entry.parents,
        path,
        change,
    }
}

fn branch_of(settings: &VaultSyncSettings, head_branch: Option<&str>) -> String {
    settings
        .branch
        .clone()
        .or_else(|| head_branch.map(str::to_string))
        .unwrap_or_else(|| DEFAULT_BRANCH.to_string())
}

fn short(oid: &str) -> &str {
    &oid[..oid.len().min(7)]
}

/// Путь, пришедший извне (с сервера, из идентификатора конфликта, из имени
/// конфликтной копии), годится для записи в vault: относительный, без `..`,
/// не внутрь `.git` — включая имена, которые файловая система Windows или
/// macOS считает тем же `.git` (T068).
pub fn safe_path(path: &str) -> Option<String> {
    if path.is_empty() || path.contains('\0') || path.contains('\\') {
        return None;
    }
    let candidate = Path::new(path);
    let mut parts = Vec::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if parts.is_empty() || parts.iter().any(|part| names_git_dir(part)) {
        return None;
    }
    Some(parts.join("/"))
}

/// Называет ли компонент пути каталог истории — прямо или так, как его
/// прочтёт файловая система: NTFS отбрасывает хвостовые точки и пробелы,
/// знает короткое имя `GIT~1` и потоки `имя:поток`; HFS+ и APFS игнорируют
/// невидимые знаки внутри имени. Запись в такой путь — это запись в
/// `.git/config`, то есть исполнение чужой команды обычным git.
fn names_git_dir(part: &str) -> bool {
    // Поток NTFS (`.git::$INDEX_ALLOCATION`) — тот же каталог; двоеточие в
    // обычном имени заметки на остальных системах законно, поэтому смотрим
    // только на имя до него.
    let visible: String = part
        .split(':')
        .next()
        .unwrap_or(part)
        .chars()
        .filter(|c| {
            !matches!(*c,
                '\u{200C}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{206A}'..='\u{206F}'
                | '\u{FEFF}')
        })
        .collect();
    let name = visible.trim_end_matches(['.', ' ']).to_ascii_lowercase();
    name == ".git"
        || name
            .strip_prefix("git~")
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

fn to_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

async fn read_text<B: VaultBackend>(backend: &B, path: &str) -> Option<String> {
    backend.read(Path::new(path)).await.ok()
}

/// Файл правил на диске глазами редактора (фича 012).
enum OnDisk {
    Absent,
    Text { text: String, mark: String },
    Unusable(Editability),
}

async fn read_ignore<B: VaultBackend>(backend: &B, file: IgnoreFile) -> OnDisk {
    let path = Path::new(file.path());
    // Раньше чтения: сквозь ссылку файл прочитался бы как обычный.
    if backend.is_symlink(path).await {
        return OnDisk::Unusable(Editability::NotAFile);
    }
    match backend.read_bytes(path).await {
        Ok(bytes) => {
            let mark = ignore::version_mark(&bytes);
            match String::from_utf8(bytes) {
                Ok(text) => OnDisk::Text { text, mark },
                Err(_) => OnDisk::Unusable(Editability::NotText),
            }
        }
        Err(ReadError::NotFound) => OnDisk::Absent,
        Err(ReadError::TooLarge) => OnDisk::Unusable(Editability::TooLarge),
        Err(ReadError::Io) => OnDisk::Unusable(Editability::NotAFile),
    }
}

/// Локальный файл перед тем, как его менять: `Ok(None)` — файла нет, отказ —
/// файл есть, но не прочитан. Различать обязательно: по «нет» файл
/// перезаписывается без вопросов (T068).
async fn read_local<B: VaultBackend>(
    backend: &B,
    path: &str,
) -> Result<Option<Vec<u8>>, SyncError> {
    match backend.read_bytes_any_size(Path::new(path)).await {
        Ok(bytes) => Ok(Some(bytes)),
        Err(ReadError::NotFound) => Ok(None),
        Err(ReadError::Io | ReadError::TooLarge) => Err(SyncError::Io {
            message: format!("локальный файл не читается: {path}"),
        }),
    }
}

async fn read_bytes<B: VaultBackend>(backend: &B, path: &str) -> Option<Vec<u8>> {
    match backend.read_bytes_any_size(Path::new(path)).await {
        Ok(bytes) => Some(bytes),
        Err(ReadError::NotFound | ReadError::Io | ReadError::TooLarge) => None,
    }
}

/// Пишет файл vault (атомарно, 009) или удаляет его; папки создаются.
async fn write_file<B: VaultBackend>(
    backend: &mut B,
    path: &str,
    bytes: Option<&[u8]>,
) -> Result<(), SyncError> {
    let io = |message: &str| SyncError::Io {
        message: format!("{message}: {path}"),
    };
    let target = Path::new(path);
    // Строка пути проверена `safe_path`, но путь на диске может вести за
    // корень через символическую ссылку-каталог (пришла клонированием, от
    // Syncthing, заведена руками). Писать и удалять сквозь неё нельзя (T068).
    if backend.crosses_symlink(target).await {
        return Err(io("путь проходит через символическую ссылку"));
    }
    match bytes {
        Some(bytes) => {
            let mut folder = PathBuf::new();
            if let Some(parent) = target.parent() {
                for component in parent.components() {
                    folder.push(component);
                    backend
                        .ensure_folder(&folder)
                        .await
                        .map_err(|_| io("не удалось создать папку"))?;
                }
            }
            backend
                .write_bytes(target, bytes)
                .await
                .map_err(|_| io("не удалось записать файл"))
        }
        None => match backend.delete(target).await {
            Ok(()) | Err(DeleteError::NotFound) => Ok(()),
            Err(_) => Err(io("не удалось удалить файл")),
        },
    }
}

async fn syncthing_detected<B: VaultBackend>(backend: &B) -> bool {
    matches!(
        backend
            .folder_access(Path::new(syncthing::FOLDER_MARKER))
            .await,
        FolderAccess::Present | FolderAccess::NotWritable { .. }
    )
}

async fn conflict_copies<B: VaultBackend>(backend: &B) -> Vec<PathBuf> {
    backend.list_conflict_copies().await
}

fn is_settings_conflict_copy(copy: &Path) -> bool {
    crate::vault::service_dir::is_inside_service_dir(copy)
        && copy
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(conflict_copy::parse)
            .is_some_and(|name| {
                name.original == crate::vault::service_dir::VAULT_SETTINGS_FILE_NAME
            })
}

fn only_cache_rules_missing(report: &IgnoreReport) -> bool {
    report.missing.iter().all(|rule| rule != "/.git")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_paths_are_rejected() {
        for bad in [
            "",
            "../x",
            "/etc/passwd",
            "a/../../b",
            ".git/config",
            "a/.GIT/x",
            "a\\b",
            // Тот же каталог истории глазами NTFS и HFS+ (T068).
            ".git./config",
            ".git /config",
            "GIT~1/config",
            ".git::$INDEX_ALLOCATION/config",
            ".g\u{200c}it/config",
        ] {
            assert!(safe_path(bad).is_none(), "{bad}");
        }
        assert_eq!(safe_path("./a/b.typ").as_deref(), Some("a/b.typ"));
        // Похожее, но законное: служебное место, двоеточие в имени заметки.
        for good in [".gitignore", ".github/x", "git~x/a", "Заметка: тема.typ"] {
            assert_eq!(safe_path(good).as_deref(), Some(good), "{good}");
        }
    }

    #[test]
    fn auto_message_lists_notes() {
        let changes: Vec<Change> = (1..=7)
            .map(|i| Change {
                path: format!("n{i}.typ"),
                change: ChangeKind::Modified,
            })
            .collect();
        let selected: Vec<String> = changes.iter().map(|c| c.path.clone()).collect();
        assert_eq!(
            auto_message("Снимок", &changes, &selected),
            "Снимок: n1, n2, n3, n4, n5 (+2)"
        );
    }

    #[test]
    fn file_decisions_follow_git_rules() {
        let a = Some(b"a\n".to_vec());
        let b = Some(b"b\n".to_vec());
        assert!(matches!(
            decide_file(&a, &a, &b),
            FileDecision::Write(Some(_))
        ));
        assert!(matches!(decide_file(&a, &b, &a), FileDecision::Keep));
        assert!(matches!(decide_file(&a, &None, &b), FileDecision::Conflict));
        assert!(matches!(
            decide_file(&a, &b, &Some(b"c\n".to_vec())),
            FileDecision::Conflict
        ));
    }
}
