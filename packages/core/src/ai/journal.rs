//! Журнал отправленного (FR-011–FR-013): `.typsidian/ai-journal.jsonl`.
//!
//! Две строки на запрос: `Sent` — до отправки, `Done` — после. `Sent` без `Done`
//! показывается как оборванный запрос. Данные устройства: в синхронизацию не
//! попадают (`service_dir::derived_paths`). Заголовков здесь нет, поэтому нет и ключа.

use serde::{Deserialize, Serialize};

use super::settings::AiProvider;
use super::AiFailure;
use crate::vault::backend::VaultBackend;
use crate::vault::service_dir;

/// Сколько последних запросов хранится.
pub const MAX_ENTRIES: usize = 500;
/// Предел журнала в байтах (ревью 056, M2): тела исследований с текстом PDF и продолжениями
/// весят мегабайты. Старые записи вытесняются целиком, тела остаются дословными (050 FR-011).
pub const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "line")]
enum JournalLine {
    Sent {
        id: String,
        at: i64,
        provider: AiProvider,
        model: String,
        endpoint: String,
        request: String,
    },
    Done {
        id: String,
        at: i64,
        failure: Option<AiFailure>,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cost_usd: Option<f64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum JournalState {
    Done,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct JournalEntry {
    pub id: String,
    pub at: i64,
    pub provider: AiProvider,
    pub model: String,
    pub endpoint: String,
    /// Дословно отправленное тело запроса.
    pub request: String,
    pub state: JournalState,
    pub failure: Option<AiFailure>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
}

/// Строка без `Done` и строка, которую не удалось разобрать, файл не ломают.
async fn read_lines<B: VaultBackend>(backend: &B) -> Vec<JournalLine> {
    let Ok(bytes) = backend
        .read_bytes_any_size(&service_dir::ai_journal_path())
        .await
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&bytes)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

async fn write_lines<B: VaultBackend>(
    backend: &mut B,
    lines: &[JournalLine],
) -> Result<(), AiFailure> {
    let mut text = String::new();
    for line in lines {
        text.push_str(&serde_json::to_string(line).map_err(|_| AiFailure::JournalFailed)?);
        text.push('\n');
    }
    backend
        .write_bytes(&service_dir::ai_journal_path(), text.as_bytes())
        .await
        .map_err(|_| AiFailure::JournalFailed)
}

/// Оставляет строки последних [`MAX_ENTRIES`] запросов и не больше [`MAX_BYTES`] байт: старые
/// запросы уходят целиком (строка `Sent` и её `Done`), последний остаётся всегда.
fn trimmed(lines: Vec<JournalLine>) -> Vec<JournalLine> {
    let id_of = |line: &JournalLine| match line {
        JournalLine::Sent { id, .. } | JournalLine::Done { id, .. } => id.clone(),
    };
    let sent: Vec<String> = lines
        .iter()
        .filter(|line| matches!(line, JournalLine::Sent { .. }))
        .map(id_of)
        .collect();
    let mut dropped: std::collections::HashSet<String> = std::collections::HashSet::new();
    if sent.len() > MAX_ENTRIES {
        dropped.extend(sent[..sent.len() - MAX_ENTRIES].iter().cloned());
    }
    // Размер считается по сериализации, как строки и пишутся в файл.
    let size_of = |line: &JournalLine| serde_json::to_string(line).map_or(0, |text| text.len() + 1);
    let mut total: usize = lines
        .iter()
        .filter(|line| !dropped.contains(&id_of(line)))
        .map(size_of)
        .sum();
    if total > MAX_BYTES {
        let kept: Vec<&String> = sent.iter().filter(|id| !dropped.contains(*id)).collect();
        for id in kept.iter().take(kept.len().saturating_sub(1)) {
            if total <= MAX_BYTES {
                break;
            }
            let freed: usize = lines
                .iter()
                .filter(|line| id_of(line) == **id)
                .map(size_of)
                .sum();
            total = total.saturating_sub(freed);
            dropped.insert((*id).clone());
        }
    }
    if dropped.is_empty() {
        return lines;
    }
    lines
        .into_iter()
        .filter(|line| !dropped.contains(&id_of(line)))
        .collect()
}

/// Записывает запрос до отправки: отказ здесь останавливает запрос (FR-012).
pub(super) async fn record_sent<B: VaultBackend>(
    backend: &mut B,
    id: &str,
    at: i64,
    provider: AiProvider,
    model: &str,
    endpoint: &str,
    request: &str,
) -> Result<(), AiFailure> {
    let mut lines = read_lines(backend).await;
    lines.push(JournalLine::Sent {
        id: id.to_owned(),
        at,
        provider,
        model: model.to_owned(),
        endpoint: endpoint.to_owned(),
        request: request.to_owned(),
    });
    write_lines(backend, &trimmed(lines)).await
}

pub(super) async fn record_done<B: VaultBackend>(
    backend: &mut B,
    id: &str,
    at: i64,
    failure: Option<AiFailure>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cost_usd: Option<f64>,
) -> Result<(), AiFailure> {
    let mut lines = read_lines(backend).await;
    lines.push(JournalLine::Done {
        id: id.to_owned(),
        at,
        failure,
        input_tokens,
        output_tokens,
        cost_usd,
    });
    write_lines(backend, &lines).await
}

/// Записи журнала, новые первыми.
pub async fn journal_entries<B: VaultBackend>(backend: &B) -> Vec<JournalEntry> {
    let mut entries: Vec<JournalEntry> = Vec::new();
    for line in read_lines(backend).await {
        match line {
            JournalLine::Sent {
                id,
                at,
                provider,
                model,
                endpoint,
                request,
            } => entries.push(JournalEntry {
                id,
                at,
                provider,
                model,
                endpoint,
                request,
                state: JournalState::Interrupted,
                failure: None,
                input_tokens: None,
                output_tokens: None,
                cost_usd: None,
            }),
            JournalLine::Done {
                id,
                failure,
                input_tokens,
                output_tokens,
                cost_usd,
                ..
            } => {
                if let Some(entry) = entries.iter_mut().rev().find(|entry| entry.id == id) {
                    entry.state = if failure.is_some() {
                        JournalState::Failed
                    } else {
                        JournalState::Done
                    };
                    entry.failure = failure;
                    entry.input_tokens = input_tokens;
                    entry.output_tokens = output_tokens;
                    entry.cost_usd = cost_usd;
                }
            }
        }
    }
    entries.reverse();
    entries
}

/// Очищает журнал; возвращает число удалённых записей.
pub async fn journal_clear<B: VaultBackend>(backend: &mut B) -> Result<u32, AiFailure> {
    let count = journal_entries(backend).await.len();
    write_lines(backend, &[]).await?;
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}
