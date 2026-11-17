//! Задание пополнения графа знаний (фича 054). Общий реестр заданий — `ai/jobs.rs`
//! (с фичи 056); здесь тип задания, его реестр и охрана с прежней сигнатурой.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

pub use crate::ai::jobs::{Job, JobState, Registry, MAX_JOBS_PER_VAULT, STALE_RUNNING_MS};
use crate::ai::AiFailure;

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

    fn release(vault_id: &str, id: &str) {
        super::import::drop_run(vault_id, id);
    }
}

/// Реестр заданий графа знаний на процесс.
pub static KNOWLEDGE_JOBS: LazyLock<Registry<KnowledgeJob>> = LazyLock::new(Registry::new);

/// Задания vault для интерфейса.
pub fn jobs(vault_id: &str) -> Vec<KnowledgeJob> {
    KNOWLEDGE_JOBS.list(vault_id, crate::properties::now_ms())
}

/// Охрана задания пополнения: см. [`crate::ai::jobs::JobGuard`].
pub struct JobGuard(crate::ai::jobs::JobGuard<KnowledgeJob>);

impl JobGuard {
    pub fn new(vault_id: &str, id: &str) -> Self {
        JobGuard(crate::ai::jobs::JobGuard::new(
            &KNOWLEDGE_JOBS,
            vault_id,
            id,
        ))
    }

    /// Снимает охрану: задание с этим id не наше (отказ при начале), трогать его нельзя.
    pub fn disarm(&mut self) {
        self.0.disarm();
    }
}
