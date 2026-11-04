//! Реестр фоновых заданий (фича 054, research.md §4). Задание — многошаговая работа с
//! прогрессом и отменой, которую интерфейс опрашивает. Реестр общий: тип задания
//! подставляется параметром ([`Job`]), в 056 он переедет в `ai/jobs.rs` без изменений.
//!
//! Реестр живёт в памяти процесса, по `vault_id` (на сервере его подставляет сервер, и
//! один пользователь чужих заданий не видит). Отмена — та же `cancel` из 050: отметка
//! отмены открывается вместе с заданием и снимается, когда оно кончилось.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};

use crate::ai::cancel::{flag_cancelled, flag_close, flag_open};
use crate::ai::AiFailure;

/// Заданий на vault, которые помнит реестр: больше — выбрасываются самые старые из
/// законченных.
pub const MAX_JOBS_PER_VAULT: usize = 30;
/// Через сколько без единого обновления идущее задание считается брошенным (закрыта
/// вкладка посреди цикла wasm).
pub const STALE_RUNNING_MS: u64 = 30 * 60 * 1000;

/// Состояние задания.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum JobState {
    Running,
    Done,
    Failed,
    Cancelled,
}

/// Задание в реестре.
pub trait Job: Clone {
    /// Идентификатор (он же `request_id` отмены).
    fn id(&self) -> &str;
    fn state(&self) -> JobState;
    /// Переводит идущее задание в конечное состояние.
    fn finish(&mut self, state: JobState, failure: Option<AiFailure>, at: i64);
}

struct Entry<T> {
    job: T,
    touched_ms: u64,
}

/// Реестр заданий одного типа.
pub struct Registry<T> {
    inner: Mutex<HashMap<String, Vec<Entry<T>>>>,
}

fn now_ms() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static START: LazyLock<std::time::Instant> = LazyLock::new(std::time::Instant::now);
        START.elapsed().as_millis() as u64
    }
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64
    }
}

impl<T: Job> Registry<T> {
    pub fn new() -> Self {
        Registry {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Бросившие задания убираются из «идущих»: они никогда не закончатся сами. Возвращает id
    /// закончившихся: тексты их частей убирает вызывающий, уже без замка реестра.
    fn sweep(vault: &mut [Entry<T>], vault_id: &str, at: i64) -> Vec<String> {
        let now = now_ms();
        let mut stale = Vec::new();
        for entry in vault.iter_mut() {
            if entry.job.state() == JobState::Running
                && now.saturating_sub(entry.touched_ms) > STALE_RUNNING_MS
            {
                entry
                    .job
                    .finish(JobState::Failed, Some(AiFailure::Timeout), at);
                flag_close(vault_id, entry.job.id());
                stale.push(entry.job.id().to_owned());
            }
        }
        stale
    }

    /// Забывает тексты частей импорта законченных заданий (ревью 055, L1). Замок реестра
    /// при этом не держится: порядок замков везде «тексты, потом реестр».
    fn forget_runs(vault_id: &str, ids: &[String]) {
        for id in ids {
            super::import::drop_run(vault_id, id);
        }
    }

    /// Заносит новое идущее задание. `conflict` — не идёт ли уже такое же по смыслу
    /// (то же происхождение). Отказ: id занят или `conflict` истинно.
    pub fn start(
        &self,
        vault_id: &str,
        job: T,
        at: i64,
        conflict: impl Fn(&T) -> bool,
    ) -> Result<(), AiFailure> {
        let mut all = self.inner.lock().map_err(|_| AiFailure::Cancelled)?;
        let list = all.entry(vault_id.to_owned()).or_default();
        let stale = Self::sweep(list, vault_id, at);
        let started = Self::push(list, job, conflict);
        drop(all);
        Self::forget_runs(vault_id, &stale);
        started
    }

    fn push(
        list: &mut Vec<Entry<T>>,
        job: T,
        conflict: impl Fn(&T) -> bool,
    ) -> Result<(), AiFailure> {
        if list.iter().any(|entry| entry.job.id() == job.id())
            || list
                .iter()
                .any(|entry| entry.job.state() == JobState::Running && conflict(&entry.job))
        {
            return Err(AiFailure::Cancelled);
        }
        list.push(Entry {
            job,
            touched_ms: now_ms(),
        });
        while list.len() > MAX_JOBS_PER_VAULT {
            match list
                .iter()
                .position(|entry| entry.job.state() != JobState::Running)
            {
                Some(index) => {
                    list.remove(index);
                }
                None => break,
            }
        }
        Ok(())
    }

    pub fn get(&self, vault_id: &str, id: &str) -> Option<T> {
        let all = self.inner.lock().ok()?;
        all.get(vault_id)?
            .iter()
            .find(|entry| entry.job.id() == id)
            .map(|entry| entry.job.clone())
    }

    /// Меняет задание; возвращает его новое состояние. Конечное задание не меняется.
    pub fn update(&self, vault_id: &str, id: &str, change: impl FnOnce(&mut T)) -> Option<T> {
        let mut all = self.inner.lock().ok()?;
        let entry = all
            .get_mut(vault_id)?
            .iter_mut()
            .find(|entry| entry.job.id() == id)?;
        if entry.job.state() == JobState::Running {
            change(&mut entry.job);
            entry.touched_ms = now_ms();
        }
        Some(entry.job.clone())
    }

    /// Задания vault: идущие первыми, потом законченные, новые раньше.
    pub fn list(&self, vault_id: &str, at: i64) -> Vec<T> {
        let Ok(mut all) = self.inner.lock() else {
            return Vec::new();
        };
        let Some(list) = all.get_mut(vault_id) else {
            return Vec::new();
        };
        let stale = Self::sweep(list, vault_id, at);
        let mut jobs: Vec<T> = list.iter().rev().map(|entry| entry.job.clone()).collect();
        drop(all);
        Self::forget_runs(vault_id, &stale);
        jobs.sort_by_key(|job| job.state() != JobState::Running);
        jobs
    }

    /// Заканчивает идущее задание и снимает отметку отмены.
    pub fn end(
        &self,
        vault_id: &str,
        id: &str,
        state: JobState,
        failure: Option<AiFailure>,
        at: i64,
    ) -> Option<T> {
        // Отметка отмены снимается только вместе с идущим заданием: такой же id мог занять
        // цикл урока или другое задание, их отметку трогать нельзя (ревью 054, L5).
        let was_running = self
            .get(vault_id, id)
            .is_some_and(|job| job.state() == JobState::Running);
        let job = self.update(vault_id, id, |job| job.finish(state, failure, at));
        if was_running {
            flag_close(vault_id, id);
            Self::forget_runs(vault_id, &[id.to_owned()]);
        }
        job
    }

    /// Идущие задания, подходящие под `matches`: `vault_id` задан — только этого vault.
    /// Возвращает пары (vault, задание).
    pub fn running_where(
        &self,
        vault_id: Option<&str>,
        matches: impl Fn(&T) -> bool,
    ) -> Vec<(String, T)> {
        let Ok(all) = self.inner.lock() else {
            return Vec::new();
        };
        all.iter()
            .filter(|(vault, _)| vault_id.is_none_or(|wanted| wanted == vault.as_str()))
            .flat_map(|(vault, list)| {
                list.iter()
                    .filter(|entry| entry.job.state() == JobState::Running && matches(&entry.job))
                    .map(|entry| (vault.clone(), entry.job.clone()))
            })
            .collect()
    }

    /// Отменено ли задание: отметка снимается только вместе с заданием.
    pub fn is_cancelled(&self, vault_id: &str, id: &str) -> bool {
        flag_cancelled(vault_id, id)
    }

    /// Открывает отметку отмены нового задания; `false` — такой id уже идёт.
    pub fn open_flag(&self, vault_id: &str, id: &str) -> bool {
        flag_open(vault_id, id)
    }
}

impl<T: Job> Default for Registry<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Задание пополнения графа знаний.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct KnowledgeJob {
    /// `request_id` запроса: по нему же отменяется (`ai_cancel`).
    pub id: String,
    /// Путь урока.
    pub source: String,
    pub state: JobState,
    pub batches_total: u32,
    pub batches_done: u32,
    /// Утверждения, которых не было в графе.
    pub propositions_added: u32,
    /// Утверждения, которые уже были (в том числе по `same_as`): им дописан источник.
    pub propositions_merged: u32,
    pub chains_added: u32,
    /// Цепочки, отклонённые из-за ссылок на несуществующее.
    pub chains_rejected: u32,
    pub failure: Option<AiFailure>,
    /// Файлы импорта заметок (фича 055); у пополнения из урока пусто.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<super::import::ImportFileJob>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

impl KnowledgeJob {
    pub fn new(id: &str, source: &str, now: i64) -> Self {
        KnowledgeJob {
            id: id.to_owned(),
            source: source.to_owned(),
            state: JobState::Running,
            batches_total: 0,
            batches_done: 0,
            propositions_added: 0,
            propositions_merged: 0,
            chains_added: 0,
            chains_rejected: 0,
            failure: None,
            files: Vec::new(),
            started_at: now,
            finished_at: None,
        }
    }
}

impl Job for KnowledgeJob {
    fn id(&self) -> &str {
        &self.id
    }

    fn state(&self) -> JobState {
        self.state
    }

    fn finish(&mut self, state: JobState, failure: Option<AiFailure>, at: i64) {
        self.state = state;
        self.failure = failure;
        self.finished_at = Some(at);
    }
}

/// Реестр заданий графа знаний на процесс.
pub static KNOWLEDGE_JOBS: LazyLock<Registry<KnowledgeJob>> = LazyLock::new(Registry::new);

/// Задания vault для интерфейса.
pub fn jobs(vault_id: &str) -> Vec<KnowledgeJob> {
    KNOWLEDGE_JOBS.list(vault_id, crate::properties::now_ms())
}

/// Бросает задание, если вызов, который его вёл, оборвали (закрыто соединение): идущее
/// задание становится отменённым и отметка отмены снимается.
pub struct JobGuard {
    vault_id: String,
    id: String,
    armed: bool,
}

impl JobGuard {
    pub fn new(vault_id: &str, id: &str) -> Self {
        JobGuard {
            vault_id: vault_id.to_owned(),
            id: id.to_owned(),
            armed: true,
        }
    }

    /// Снимает охрану: задание с этим id не наше (отказ при начале), трогать его нельзя.
    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        if self.armed
            && KNOWLEDGE_JOBS
                .get(&self.vault_id, &self.id)
                .is_some_and(|job| job.state == JobState::Running)
        {
            KNOWLEDGE_JOBS.end(
                &self.vault_id,
                &self.id,
                JobState::Cancelled,
                Some(AiFailure::Cancelled),
                0,
            );
        }
    }
}
