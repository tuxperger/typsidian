//! Реестр фоновых заданий (фича 054, research.md §4; общий с фичи 056). Задание —
//! многошаговая работа с прогрессом и отменой, которую интерфейс опрашивает. Тип задания
//! подставляется параметром ([`Job`]): пополнение графа знаний (054, 055) и исследования
//! (056) живут в своих реестрах с одним поведением.
//!
//! Реестр живёт в памяти процесса, по `vault_id` (на сервере его подставляет сервер, и
//! один пользователь чужих заданий не видит). Отмена — та же `cancel` из 050: отметка
//! отмены открывается вместе с заданием и снимается, когда оно кончилось.

use std::collections::HashMap;
use std::sync::Mutex;

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
    /// Задание закончилось (или брошено): убрать то, что оно держит вне реестра. Зовётся без
    /// замка реестра.
    fn release(_vault_id: &str, _id: &str) {}
}

struct Entry<T> {
    job: T,
    touched_ms: u64,
}

/// Реестр заданий одного типа.
pub struct Registry<T> {
    inner: Mutex<HashMap<String, Vec<Entry<T>>>>,
}

pub(crate) fn now_ms() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static START: std::sync::LazyLock<std::time::Instant> =
            std::sync::LazyLock::new(std::time::Instant::now);
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

    /// Забывает то, что законченные задания держали вне реестра (тексты частей импорта,
    /// ревью 055, L1). Замок реестра при этом не держится: порядок замков везде «своё, потом
    /// реестр».
    fn forget_runs(vault_id: &str, ids: &[String]) {
        for id in ids {
            T::release(vault_id, id);
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

/// Задание любого вида: общий список для интерфейса (`ai_jobs`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "type")]
pub enum AnyJob {
    Knowledge(crate::ai::knowledge::KnowledgeJob),
    Research(crate::ai::research::ResearchJob),
}

impl AnyJob {
    fn running(&self) -> bool {
        match self {
            AnyJob::Knowledge(job) => job.state == JobState::Running,
            AnyJob::Research(job) => job.state == JobState::Running,
        }
    }
}

/// Все задания vault: идущие первыми, внутри каждой группы пополнения графа, потом
/// исследования.
pub fn all_jobs(vault_id: &str) -> Vec<AnyJob> {
    let mut jobs: Vec<AnyJob> = crate::ai::knowledge::jobs(vault_id)
        .into_iter()
        .map(AnyJob::Knowledge)
        .chain(
            crate::ai::research::research_jobs(vault_id)
                .into_iter()
                .map(AnyJob::Research),
        )
        .collect();
    jobs.sort_by_key(|job| !job.running());
    jobs
}

/// Бросает задание, если вызов, который его вёл, оборвали (закрыто соединение): идущее
/// задание становится отменённым и отметка отмены снимается.
pub struct JobGuard<T: Job + 'static> {
    registry: &'static Registry<T>,
    vault_id: String,
    id: String,
    armed: bool,
}

impl<T: Job + 'static> JobGuard<T> {
    pub fn new(registry: &'static Registry<T>, vault_id: &str, id: &str) -> Self {
        JobGuard {
            registry,
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

impl<T: Job + 'static> Drop for JobGuard<T> {
    fn drop(&mut self) {
        if self.armed
            && self
                .registry
                .get(&self.vault_id, &self.id)
                .is_some_and(|job| job.state() == JobState::Running)
        {
            self.registry.end(
                &self.vault_id,
                &self.id,
                JobState::Cancelled,
                Some(AiFailure::Cancelled),
                0,
            );
        }
    }
}
