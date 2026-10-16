//! Свойства в индексе vault и операции над ним (research.md §4–§5, FR-009, FR-010).
//!
//! Источник истины о свойствах — текст заметок. Индекс — производное, целиком
//! восстановимое состояние на устройстве: [`IndexCache`] хранит по заметке
//! список [`PropertyEntry`] (native — таблица `properties` в `cache.sqlite`,
//! web — память, пересчитываемая при открытии вместе со ссылками). Backend
//! отдаёт его одним вопросом [`VaultBackend::indexed_notes`]; умолчание этого
//! вопроса читает все заметки и разбирает их заново, поэтому двойники в
//! тестах и любой будущий backend работают без дополнительного кода.
//!
//! [`IndexCache`]: crate::vault::cache::IndexCache

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::query::{run_selection, Selection, SelectionResult};
use super::{entries_of, now_ms, PropertyEntry, ValueKind};
use crate::vault::backend::VaultBackend;
use crate::vault::relocate::MoveControl;
use crate::vault::FileKind;

/// Что отбор знает о заметке: путь, встроенные сведения и свойства.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct IndexedNote {
    /// Относительно корня vault, через `/`.
    pub path: String,
    /// Имя файла.
    pub name: String,
    /// Время изменения, мс от эпохи; `None` — неизвестно.
    pub modified_ms: Option<i64>,
    /// Время создания; пока нигде не хранится — всегда `None` (research.md §6).
    pub created_ms: Option<i64>,
    pub properties: Vec<PropertyEntry>,
}

impl IndexedNote {
    pub fn new(path: &Path, name: &str, modified_ms: Option<i64>) -> Self {
        Self {
            path: path.to_string_lossy().replace('\\', "/"),
            name: name.to_string(),
            modified_ms: modified_ms.filter(|ms| *ms > 0),
            created_ms: None,
            properties: Vec::new(),
        }
    }
}

/// Умолчание [`VaultBackend::indexed_notes`]: читает все заметки и разбирает
/// их. Медленно, зато верно для любого backend.
pub async fn scan_notes<B: VaultBackend>(backend: &B) -> Vec<IndexedNote> {
    let mut out = Vec::new();
    for note in backend.list().await {
        if note.kind != FileKind::Note {
            continue;
        }
        let mut indexed = IndexedNote::new(&note.path, &note.name, Some(note.stamp.mtime_millis));
        if let Ok(text) = backend.read(&note.path).await {
            indexed.properties = entries_of(&text);
        }
        out.push(indexed);
    }
    out
}

/// Отбор заметок vault по выборке (`VaultSession::select` из контракта).
pub async fn select_notes<B: VaultBackend>(backend: &B, selection: &Selection) -> SelectionResult {
    run_selection(&backend.indexed_notes().await, selection, now_ms())
}

/// Ключ свойства vault и виды его значений — для подсказок в форме и фишках.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PropertyKeyInfo {
    pub key: String,
    /// Какие виды значений встречаются под этим ключом, по порядку вида.
    pub kinds: Vec<ValueKind>,
    /// В скольких заметках ключ задан.
    pub count: u32,
}

/// Ключи свойств всего vault: чаще используемые раньше, затем по алфавиту.
pub async fn property_keys<B: VaultBackend>(backend: &B) -> Vec<PropertyKeyInfo> {
    let mut found: std::collections::BTreeMap<
        String,
        (std::collections::BTreeSet<ValueKind>, u32),
    > = std::collections::BTreeMap::new();
    for note in backend.indexed_notes().await {
        for entry in &note.properties {
            let slot = found.entry(entry.key.clone()).or_default();
            slot.0.insert(entry.value.kind());
            slot.1 += 1;
        }
    }
    let mut keys: Vec<PropertyKeyInfo> = found
        .into_iter()
        .map(|(key, (kinds, count))| PropertyKeyInfo {
            key,
            kinds: kinds.into_iter().collect(),
            count,
        })
        .collect();
    keys.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
    keys
}

// ── Пересчёт всего vault (принцип X) ───────────────────────────────────────

/// Сколько заметок пересчитывается за один шаг: между шагами интерфейс (или
/// оболочка) успевает отдать управление и проверить отмену.
pub const REINDEX_BATCH: u32 = 50;

/// Итог одного шага пересчёта.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ReindexStep {
    pub done: u32,
    pub total: u32,
    pub finished: bool,
}

/// Итог пакетного пересчёта (data-model.md «ReindexProgress»).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ReindexOutcome {
    pub done: u32,
    pub total: u32,
    pub cancelled: bool,
}

/// Один шаг пересчёта: заметки `[from, from + count)` по порядку путей.
///
/// Запись идёт по заметке, поэтому остановка между шагами оставляет кэш
/// согласованным: пересчитанные заметки новые, остальные прежние. Отметка
/// «сведения актуальны» ставится только после последнего шага.
pub async fn reindex_step<B: VaultBackend>(backend: &mut B, from: u32, count: u32) -> ReindexStep {
    let mut notes: Vec<_> = backend
        .list()
        .await
        .into_iter()
        .filter(|note| note.kind == FileKind::Note)
        .collect();
    notes.sort_by(|a, b| a.path.cmp(&b.path));
    let total = notes.len() as u32;
    let end = from.saturating_add(count).min(total);
    for note in notes
        .iter()
        .skip(from as usize)
        .take((end - from.min(end)) as usize)
    {
        let entries = match backend.read(&note.path).await {
            Ok(text) => entries_of(&text),
            // Нечитаемая заметка остаётся без свойств: пересчёт не отказывает.
            Err(_) => Vec::new(),
        };
        backend.set_indexed_properties(&note.path, entries).await;
    }
    let finished = end >= total;
    if finished {
        backend.mark_properties_current().await;
    }
    ReindexStep {
        done: end,
        total,
        finished,
    }
}

/// Пакетный пересчёт свойств всего vault с прогрессом и отменой.
///
/// Ход и отмена — через тот же [`MoveControl`], что у переноса (фича 023):
/// интерфейс держит флаг у себя, ядро читает его между шагами.
pub async fn reindex_properties<B: VaultBackend>(
    backend: &mut B,
    control: &dyn MoveControl,
) -> ReindexOutcome {
    let mut from = 0;
    loop {
        if control.cancelled() {
            let total = backend
                .list()
                .await
                .iter()
                .filter(|note| note.kind == FileKind::Note)
                .count() as u32;
            return ReindexOutcome {
                done: from,
                total,
                cancelled: true,
            };
        }
        let step = reindex_step(backend, from, REINDEX_BATCH).await;
        control.progress(step.done, step.total);
        if step.finished {
            return ReindexOutcome {
                done: step.done,
                total: step.total,
                cancelled: false,
            };
        }
        from = step.done;
    }
}

/// Собирает сведения об заметках из записей индекса и свойств кэша. Доски и
/// прочие не-заметки пропускаются. `modified` отдаёт время изменения: оно
/// на разных таргетах берётся из разных мест, а правило одно.
pub fn join_notes(
    notes: Vec<crate::vault::index::Note>,
    mut properties: std::collections::HashMap<std::path::PathBuf, Vec<PropertyEntry>>,
    modified: impl Fn(&crate::vault::index::Note) -> Option<i64>,
) -> Vec<IndexedNote> {
    notes
        .into_iter()
        .filter(|note| note.kind == FileKind::Note)
        .map(|note| {
            let mut indexed = IndexedNote::new(&note.path, &note.name, modified(&note));
            indexed.properties = properties.remove(&note.path).unwrap_or_default();
            indexed
        })
        .collect()
}
