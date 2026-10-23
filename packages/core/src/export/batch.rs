//! Пакетный экспорт: состояние очереди и переходы (009 FR-015 … FR-024,
//! data-model.md «BatchState», research.md §5).
//!
//! Чистые функции «состояние на входе → состояние на выходе». Пакет не держит
//! ни vault, ни компилятор: цикл в UI берёт у [`next`] следующую заметку,
//! экспортирует её обычным [`super::export_note_pdf`] и отдаёт итог в
//! [`record`]. Так правила очереди — порядок, пропуск, отмена, «остановить при
//! ошибке», оценка времени — живут в ядре и проверяются тестами на обоих
//! таргетах (принцип I), а оболочкам не нужно хранить объект между вызовами.
//!
//! Отмена действует между заметками: текущая доводится до конца, и её запись
//! атомарна (research.md §4), поэтому полузаписанных файлов не остаётся
//! (FR-019).

use serde::{Deserialize, Serialize};

use super::folder::export_folder;
use super::{ExportOutcome, ExportReport};
use crate::vault::vault_settings::VaultOverrides;

/// Состояние пакета.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BatchState {
    /// Задания в порядке списка (FR-015).
    pub jobs: Vec<BatchJob>,
    pub stop_on_error: bool,
    pub phase: BatchPhase,
    /// Куда пишутся файлы — для итога (FR-023).
    pub export_folder: String,
    /// Формат пакета (фича 034): PDF или страницы HTML.
    #[serde(default)]
    pub format: ExportFormat,
}

/// Формат экспорта (фича 034).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ExportFormat {
    #[default]
    Pdf,
    Html,
}

/// Тот же пакет в другом формате (фича 034).
pub fn with_format(mut state: BatchState, format: ExportFormat) -> BatchState {
    state.format = format;
    state
}

/// Одна заметка пакета.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BatchJob {
    pub note_path: String,
    pub status: JobStatus,
    /// Время экспорта этой заметки, мс. Меряет UI: часов в ядре под wasm нет,
    /// а это время представления, а не правило (research.md §10).
    pub elapsed_ms: Option<u32>,
    pub report: Option<ExportReport>,
}

/// Состояние строки (FR-015): ровно одно в каждый момент.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum JobStatus {
    Queued,
    Exporting,
    Done,
    DoneWithWarnings,
    Failed,
    Skipped,
    Cancelled,
}

/// Фаза пакета.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum BatchPhase {
    Running,
    Cancelled,
    /// Остановлен первой ошибкой при включённой опции (FR-021).
    StoppedOnError {
        note_path: String,
    },
    Finished,
}

/// Состояние вместе с производными значениями, которые показывает UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BatchView {
    pub state: BatchState,
    /// Обработано заданий: всё, что уже не в очереди и не в работе (FR-017).
    pub processed: u32,
    pub total: u32,
    /// Оценка оставшегося времени, мс. `None`, пока не готово ни одной
    /// заметки — оценивать не по чему (FR-017).
    pub remaining_estimate_ms: Option<u64>,
    /// Пакет больше ничего делать не будет, и отмена которого-то задания
    /// невозможна.
    pub is_over: bool,
    pub summary: BatchSummary,
}

/// Итог пакета (FR-023).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BatchSummary {
    pub done: u32,
    pub failed: u32,
    /// Пропущенные пользователем и не начатые из-за отмены.
    pub not_exported: u32,
}

/// Новый пакет: все заметки в очереди в заданном порядке.
///
/// Повторы одного пути сворачиваются в одно задание: экспортировать заметку
/// дважды в одном пакете — лишняя работа с тем же результатом.
pub fn start(paths: Vec<String>, stop_on_error: bool, overrides: &VaultOverrides) -> BatchState {
    let mut jobs: Vec<BatchJob> = Vec::with_capacity(paths.len());
    for note_path in paths {
        if jobs.iter().any(|job| job.note_path == note_path) {
            continue;
        }
        jobs.push(BatchJob {
            note_path,
            status: JobStatus::Queued,
            elapsed_ms: None,
            report: None,
        });
    }
    let phase = if jobs.is_empty() {
        BatchPhase::Finished
    } else {
        BatchPhase::Running
    };
    BatchState {
        jobs,
        stop_on_error,
        phase,
        export_folder: export_folder(overrides),
        format: ExportFormat::Pdf,
    }
}

/// Следующая заметка к экспорту.
///
/// Возвращает `None`, если пакет отменён, остановлен или очередь пуста; в
/// последнем случае пакет становится `Finished`. Пока одно задание в работе,
/// второго не выдаётся: пакет обрабатывает заметки строго по одной (FR-015).
pub fn next(mut state: BatchState) -> (BatchState, Option<String>) {
    if state.phase != BatchPhase::Running {
        return (state, None);
    }
    if state
        .jobs
        .iter()
        .any(|job| job.status == JobStatus::Exporting)
    {
        return (state, None);
    }
    match state
        .jobs
        .iter_mut()
        .find(|job| job.status == JobStatus::Queued)
    {
        Some(job) => {
            job.status = JobStatus::Exporting;
            let path = job.note_path.clone();
            (state, Some(path))
        }
        None => {
            state.phase = BatchPhase::Finished;
            (state, None)
        }
    }
}

/// Записывает итог экспорта заметки, которая сейчас в работе.
///
/// Итог для заметки не в работе игнорируется: это рассогласование цикла с
/// очередью, и менять из-за него состояние нельзя.
pub fn record(mut state: BatchState, report: ExportReport, elapsed_ms: u32) -> BatchState {
    let Some(index) = state
        .jobs
        .iter()
        .position(|job| job.note_path == report.note_path && job.status == JobStatus::Exporting)
    else {
        return state;
    };
    let status = match report.outcome {
        ExportOutcome::Written { .. } if report.has_warnings() => JobStatus::DoneWithWarnings,
        ExportOutcome::Written { .. } => JobStatus::Done,
        _ => JobStatus::Failed,
    };
    let job = &mut state.jobs[index];
    job.status = status;
    job.elapsed_ms = Some(elapsed_ms);
    job.report = Some(report);

    if status == JobStatus::Failed && state.stop_on_error && state.phase == BatchPhase::Running {
        let note_path = state.jobs[index].note_path.clone();
        cancel_queued(&mut state);
        state.phase = BatchPhase::StoppedOnError { note_path };
    }
    state
}

/// Пропускает заметку, которая ещё ждёт очереди (FR-020). Начатую пропустить
/// нельзя — она уже пишется, и прервать её значило бы нарушить FR-019.
pub fn skip(mut state: BatchState, note_path: &str) -> BatchState {
    if let Some(job) = state
        .jobs
        .iter_mut()
        .find(|job| job.note_path == note_path && job.status == JobStatus::Queued)
    {
        job.status = JobStatus::Skipped;
    }
    state
}

/// Отменяет пакет (FR-019): не начатые заметки больше не начнутся. Заметка в
/// работе остаётся в работе до своего [`record`] — её запись атомарна.
pub fn cancel(mut state: BatchState) -> BatchState {
    if state.phase == BatchPhase::Running {
        cancel_queued(&mut state);
        state.phase = BatchPhase::Cancelled;
    }
    state
}

fn cancel_queued(state: &mut BatchState) {
    for job in &mut state.jobs {
        if job.status == JobStatus::Queued {
            job.status = JobStatus::Cancelled;
        }
    }
}

/// Состояние с производными значениями для показа.
pub fn view(state: BatchState) -> BatchView {
    let total = state.jobs.len() as u32;
    let is_pending = |status: JobStatus| matches!(status, JobStatus::Queued | JobStatus::Exporting);
    let processed = state
        .jobs
        .iter()
        .filter(|job| !is_pending(job.status))
        .count() as u32;

    let timed: Vec<u64> = state
        .jobs
        .iter()
        .filter(|job| {
            matches!(
                job.status,
                JobStatus::Done | JobStatus::DoneWithWarnings | JobStatus::Failed
            )
        })
        .filter_map(|job| job.elapsed_ms.map(u64::from))
        .collect();
    let left = state
        .jobs
        .iter()
        .filter(|job| is_pending(job.status))
        .count() as u64;
    let remaining_estimate_ms = (!timed.is_empty() && state.phase == BatchPhase::Running)
        .then(|| timed.iter().sum::<u64>() / timed.len() as u64 * left);

    let mut summary = BatchSummary::default();
    for job in &state.jobs {
        match job.status {
            JobStatus::Done | JobStatus::DoneWithWarnings => summary.done += 1,
            JobStatus::Failed => summary.failed += 1,
            JobStatus::Skipped | JobStatus::Cancelled => summary.not_exported += 1,
            JobStatus::Queued | JobStatus::Exporting => {}
        }
    }

    let exporting = state
        .jobs
        .iter()
        .any(|job| job.status == JobStatus::Exporting);
    let is_over = !exporting
        && (state.phase != BatchPhase::Running
            || !state.jobs.iter().any(|job| job.status == JobStatus::Queued));

    BatchView {
        state,
        processed,
        total,
        remaining_estimate_ms,
        is_over,
        summary,
    }
}

/// Результат [`next`] одним значением: состояние и заметка, которую брать в
/// работу. Отдельный тип, а не пара, — чтобы граница wasm и команда Tauri
/// отдавали интерфейсу одну и ту же форму (принцип IV).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BatchStep {
    pub state: BatchState,
    pub next: Option<String>,
}

/// [`next`] в форме для границы.
pub fn step(state: BatchState) -> BatchStep {
    let (state, next) = next(state);
    BatchStep { state, next }
}

/// Строки Центра экспорта для открытого vault: те же записи, что у дерева.
pub async fn candidates_of<B: crate::vault::backend::VaultBackend>(
    backend: &B,
    overrides: &VaultOverrides,
) -> Vec<super::ExportCandidate> {
    super::candidates(&backend.list().await, overrides)
}
