//! Граф знаний из уроков (фича 054): утверждения и цепочки рассуждений, которые ядро
//! выделяет из объяснений учителя, объединяет без дублей и хранит обычными файлами vault
//! (`<папка уроков>/Граф знаний/*.jsonl`, только дописывание). Модуль строится на запросах
//! 050 и уроках 051–053: журнал отправленного, снимок подключения, проверка доступа,
//! отмена. Отказы — `AiFailure` без текста.
//!
//! Что уходит модели: при извлечении — только объяснения учителя и до 50 найденных
//! локально известных утверждений ([`extract`]); дальше граф уходит ей лишь по явному
//! выбору пользователя при создании урока или практики ([`select_material`]) и, если это
//! разрешено настройкой `knowledge_agent_reads`, инструментом `read_knowledge`
//! ([`agent_results`]). Папка графа в запрещённых закрывает оба пути.

pub mod extract;
pub mod jobs;
pub mod merge;
pub mod search;
pub mod store;
pub mod view;
#[cfg(target_arch = "wasm32")]
pub mod web;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

pub use extract::{
    begin_extract, finish_extract, send_extract, ExtractDone, PreparedExtract, AUTO_MIN_CHARS,
};
pub use jobs::{jobs, JobGuard, JobState, KnowledgeJob, KNOWLEDGE_JOBS};
pub use view::{Chain, KnowledgeGraph, KnowledgeGroup, Proposition, SourceRef, StatementRef};

use super::access::{check_access_in, normalize_folder};
use super::lesson::file::{lesson_path, LessonKind};
use super::lesson::{partial, LessonView};
use super::settings::{load_settings, AiSettings};
use super::AiFailure;
use crate::vault::backend::{DeleteError, VaultBackend};
use store::{clip, Projection, PropositionLine, SourceLine, TEXT_MAX_CHARS};

/// Сколько утверждений пользователь выбирает для урока или практики.
pub const MAX_SELECTED: usize = 50;
/// Сколько утверждений `read_knowledge` отдаёт модели за раз.
pub const AGENT_READ_MAX: usize = 30;
/// Запрос `read_knowledge`, символов.
pub const QUERY_MAX_CHARS: usize = 200;

/// Выбранное утверждение в материале сессии.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MaterialStatement {
    pub id: String,
    pub text: String,
}

/// Выбранная цепочка: тексты посылок и вывода.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MaterialChain {
    pub premises: Vec<String>,
    pub conclusion: String,
}

/// Знания из графа, выбранные пользователем при создании сессии. Снимок: текст лежит в
/// событии `Material`, как текст уроков в материале практики.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct KnowledgeMaterial {
    pub statements: Vec<MaterialStatement>,
    pub chains: Vec<MaterialChain>,
}

/// Чем кончилось чтение графа агентом: текст в урок не пишется, он есть в графе и в
/// журнале 050 (как у `NoteOutcome`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum KnowledgeOutcome {
    /// Модели отдано `count` утверждений.
    Sent { count: u32 },
    /// Чтение выключено или папка графа закрыта.
    Denied,
}

/// Утверждения для модели и цепочки между ними.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentResult {
    pub statements: Vec<MaterialStatement>,
    pub chains: Vec<MaterialChain>,
}

fn graph_open_check_path(settings: &AiSettings) -> String {
    store::graph_folder_string(settings)
}

/// Открыт ли граф для агента: функция включена и папка графа не закрыта.
pub async fn graph_readable<B: VaultBackend>(backend: &B, settings: &AiSettings) -> bool {
    settings.enabled && check_access_in(backend, &graph_open_check_path(settings), settings).await
}

/// Источники утверждений `ids`, открытые агенту (`check_access_in`). Закрытая папка урока
/// закрывает и его вклад в граф: утверждение уходит модели, только если хотя бы у одной
/// формулировки источник открыт (ревью 054, M1; так же, как 053 M1).
pub(crate) async fn open_sources<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    projection: &Projection,
    ids: &[String],
) -> HashSet<String> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut open: HashSet<String> = HashSet::new();
    for id in ids {
        let Some(entry) = projection.propositions.get(id) else {
            continue;
        };
        for phrasing in &entry.phrasings {
            if seen.insert(phrasing.source.as_str())
                && check_access_in(backend, &phrasing.source, settings).await
            {
                open.insert(phrasing.source.clone());
            }
        }
    }
    open
}

/// Формулировка утверждения из открытого источника; `None` — модели его отдавать нельзя.
/// Берётся первая такая: формулировка из закрытой папки не уходит, даже если она первая.
pub(crate) fn visible_text(
    projection: &Projection,
    id: &str,
    open: &HashSet<String>,
) -> Option<String> {
    projection
        .propositions
        .get(id)?
        .phrasings
        .iter()
        .find(|phrasing| open.contains(&phrasing.source))
        .map(|phrasing| clip(&phrasing.text, TEXT_MAX_CHARS))
}

fn statements_of(
    projection: &Projection,
    ids: &[String],
    open: &HashSet<String>,
) -> Vec<MaterialStatement> {
    ids.iter()
        .filter_map(|id| {
            Some(MaterialStatement {
                id: id.clone(),
                text: visible_text(projection, id, open)?,
            })
        })
        .collect()
}

/// Цепочки, у которых вывод и все оставшиеся посылки среди `ids`, и все они видны модели.
fn chains_among(
    projection: &Projection,
    ids: &[String],
    limit: usize,
    open: &HashSet<String>,
) -> Vec<MaterialChain> {
    let set: HashSet<&str> = ids
        .iter()
        .filter(|id| visible_text(projection, id, open).is_some())
        .map(String::as_str)
        .collect();
    let mut keys: Vec<&String> = projection.chains.keys().collect();
    keys.sort();
    let mut out = Vec::new();
    for key in keys {
        let chain = &projection.chains[key];
        if !set.contains(chain.conclusion.as_str()) {
            continue;
        }
        let Some(conclusion) = visible_text(projection, &chain.conclusion, open) else {
            continue;
        };
        let present: Vec<&String> = chain
            .premises
            .iter()
            .filter(|premise| projection.propositions.contains_key(*premise))
            .collect();
        if present.is_empty()
            || present
                .iter()
                .any(|premise| !set.contains(premise.as_str()))
        {
            continue;
        }
        out.push(MaterialChain {
            premises: present
                .iter()
                .filter_map(|premise| visible_text(projection, premise, open))
                .collect(),
            conclusion,
        });
        if out.len() >= limit {
            break;
        }
    }
    out
}

/// Знания, выбранные пользователем по id, для события `Material`. Папка графа закрыта
/// или функция выключена — отказ `Forbidden`, а не молчаливый пропуск: выбор не должен
/// стать пустым материалом. Неизвестные id и утверждения только из закрытых папок
/// пропускаются.
pub async fn select_material<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    ids: &[String],
) -> Result<Option<KnowledgeMaterial>, AiFailure> {
    if ids.is_empty() {
        return Ok(None);
    }
    if !graph_readable(backend, settings).await {
        return Err(AiFailure::Forbidden);
    }
    let projection = store::load(backend, settings).await;
    let mut candidates: Vec<String> = Vec::new();
    for id in ids.iter().take(MAX_SELECTED * 4) {
        if projection.propositions.contains_key(id) && !candidates.contains(id) {
            candidates.push(id.clone());
        }
    }
    let open = open_sources(backend, settings, &projection, &candidates).await;
    let chosen: Vec<String> = candidates
        .into_iter()
        .filter(|id| visible_text(&projection, id, &open).is_some())
        .take(MAX_SELECTED)
        .collect();
    let statements = statements_of(&projection, &chosen, &open);
    if statements.is_empty() {
        return Ok(None);
    }
    Ok(Some(KnowledgeMaterial {
        chains: chains_among(&projection, &chosen, MAX_SELECTED, &open),
        statements,
    }))
}

/// Снимок `Material.knowledge` для запроса: утверждения, у которых больше нет открытого
/// источника (папку урока закрыли после выбора) или которых нет в графе, и цепочки с ними
/// не уходят модели (ревью 054, M1). Текст берётся из открытой формулировки.
pub async fn visible_material<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    material: &KnowledgeMaterial,
) -> Option<KnowledgeMaterial> {
    let projection = store::load(backend, settings).await;
    let ids: Vec<String> = material
        .statements
        .iter()
        .map(|statement| statement.id.clone())
        .collect();
    let open = open_sources(backend, settings, &projection, &ids).await;
    // Прежний текст снимка -> текст, который можно отдать.
    let mut texts: HashMap<&str, String> = HashMap::new();
    let mut statements = Vec::new();
    for statement in &material.statements {
        if let Some(text) = visible_text(&projection, &statement.id, &open) {
            texts.insert(statement.text.as_str(), text.clone());
            statements.push(MaterialStatement {
                id: statement.id.clone(),
                text,
            });
        }
    }
    if statements.is_empty() {
        return None;
    }
    let chains = material
        .chains
        .iter()
        .filter_map(|chain| {
            Some(MaterialChain {
                premises: chain
                    .premises
                    .iter()
                    .map(|premise| texts.get(premise.as_str()).cloned())
                    .collect::<Option<Vec<_>>>()?,
                conclusion: texts.get(chain.conclusion.as_str()).cloned()?,
            })
        })
        .collect();
    Some(KnowledgeMaterial { statements, chains })
}

/// Ответ инструмента `read_knowledge`: до [`AGENT_READ_MAX`] утверждений по словам
/// запроса, поиск локальный. Чтение выключено настройкой или папка графа закрыта —
/// `Denied` без содержимого. Утверждения из закрытых папок уроков не отдаются.
pub async fn agent_results<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    query: &str,
) -> (KnowledgeOutcome, Option<AgentResult>) {
    if !settings.knowledge_agent_reads || !graph_readable(backend, settings).await {
        return (KnowledgeOutcome::Denied, None);
    }
    let projection = store::load(backend, settings).await;
    let query = clip(query.trim(), QUERY_MAX_CHARS);
    let candidates = search::related(&projection, &query, AGENT_READ_MAX * 4, |_, _| true);
    let open = open_sources(backend, settings, &projection, &candidates).await;
    let ids: Vec<String> = candidates
        .into_iter()
        .filter(|id| visible_text(&projection, id, &open).is_some())
        .take(AGENT_READ_MAX)
        .collect();
    let result = AgentResult {
        statements: statements_of(&projection, &ids, &open),
        chains: chains_among(&projection, &ids, AGENT_READ_MAX, &open),
    };
    (
        KnowledgeOutcome::Sent {
            count: result.statements.len() as u32,
        },
        Some(result),
    )
}

// ── Урок ────────────────────────────────────────────────────────────────────

/// Дополняет вид урока признаками для графа: сколько объяснений ещё не в графе и пора ли
/// пополнять сам. Читается только `sources.jsonl`. «Пора» — функция включена, автозапуск
/// включён, урок открыт агенту и непокрытого текста больше [`AUTO_MIN_CHARS`]. Запуск
/// остаётся за клиентом: ядро не держит vault на время сети.
pub async fn annotate<B: VaultBackend>(backend: &B, mut view: LessonView) -> LessonView {
    if view.header.kind != LessonKind::Lesson || view.read_only {
        return view;
    }
    let Ok(path) = lesson_path(&view.path) else {
        return view;
    };
    let settings = load_settings(backend).await;
    let projection = store::load_sources(backend, &settings).await;
    let covered = projection.covered_of(&path);
    let chars = extract::uncovered_chars(&view.header, &view.events, covered);
    view.knowledge_pending_chars = chars;
    view.knowledge_auto_due = settings.enabled
        && settings.knowledge_auto
        && !projection.damaged
        && chars as usize > AUTO_MIN_CHARS
        && check_access_in(backend, &path, &settings).await;
    view
}

// ── Вид ─────────────────────────────────────────────────────────────────────

pub async fn graph_view<B: VaultBackend>(backend: &B, query: &str) -> KnowledgeGraph {
    let settings = load_settings(backend).await;
    view::graph(&store::load(backend, &settings).await, query)
}

pub async fn chains_view<B: VaultBackend>(backend: &B, query: &str) -> Vec<Chain> {
    let settings = load_settings(backend).await;
    view::chains(&store::load(backend, &settings).await, query)
}

pub async fn proposition_view<B: VaultBackend>(backend: &B, id: &str) -> Option<Proposition> {
    let settings = load_settings(backend).await;
    view::proposition(&store::load(backend, &settings).await, id)
}

// ── Удаление ────────────────────────────────────────────────────────────────
/// Удаляет утверждение: надгробие в `propositions.jsonl`. Цепочки с ним как посылкой
/// показываются неполными, с ним как выводом — скрываются. Неизвестный id — ничего.
/// Надгробие позже всех строк этого id (L1), даже если часы устройства отстают.
pub async fn remove_proposition<B: VaultBackend>(
    backend: &mut B,
    id: &str,
    now: i64,
) -> Result<(), AiFailure> {
    let settings = load_settings(backend).await;
    let projection = store::load(backend, &settings).await;
    if !projection.propositions.contains_key(id) {
        return Ok(());
    }
    let at = store::stamp_after(now, projection.latest_by_id.get(id).copied());
    store::append_propositions(
        backend,
        &settings,
        &[PropositionLine::Removed {
            id: id.to_owned(),
            at,
        }],
    )
    .await
}

/// Убирает вклад источника (`keep: false`) или помечает источник удалённым, оставив
/// вклад (`keep: true`). Покрытие сохраняется: убранный вклад существующего урока заново
/// не извлекается, пока в уроке нет нового текста. Строка позже всех строк источника (L1).
pub async fn remove_source<B: VaultBackend>(
    backend: &mut B,
    source: &str,
    keep: bool,
    now: i64,
) -> Result<(), AiFailure> {
    let Some(source) = normalize_folder(Path::new(source)) else {
        return Ok(());
    };
    let settings = load_settings(backend).await;
    let projection = store::load(backend, &settings).await;
    if !projection.has_coverage(&source) && !projection.source_paths().contains(&source) {
        return Ok(());
    }
    let wanted = store::folded_key(&source);
    let floor = projection
        .latest_by_source
        .iter()
        .filter(|(known, _)| store::folded_key(known) == wanted)
        .map(|(_, at)| *at)
        .max();
    store::append_sources(
        backend,
        &settings,
        &[SourceLine::SourceRemoved {
            source,
            keep,
            at: store::stamp_after(now, floor),
        }],
    )
    .await
}

/// Удаляет файл урока обычным удалением vault (корзина ОС) и решает судьбу вклада в граф:
/// `keep_knowledge` — вклад остаётся, источник помечен удалённым; иначе вклад убирается.
/// Идущий ход урока удалению мешает (`LessonChanged`): он ищется среди ходов этого vault,
/// чужой vault на том же пути (на сервере — чужой аккаунт) не виден (ревью 054, L2).
/// Идущее задание пополнения графа по этому уроку отменяется. Путь через символическую
/// ссылку не удаляется. После удаления в очередь событий бэкенда ставится `NoteDeleted`.
pub async fn delete_lesson<B: VaultBackend>(
    backend: &mut B,
    vault_id: Option<&str>,
    path: &str,
    keep_knowledge: bool,
    now: i64,
) -> Result<(), AiFailure> {
    let path = lesson_path(path)?;
    if partial::is_active_in(vault_id, &path) {
        return Err(AiFailure::LessonChanged);
    }
    if backend.crosses_symlink(Path::new(&path)).await || backend.is_symlink(Path::new(&path)).await
    {
        return Err(AiFailure::Forbidden);
    }
    let wanted = store::folded_key(&path);
    for (vault, job) in
        KNOWLEDGE_JOBS.running_where(vault_id, |job| store::folded_key(&job.source) == wanted)
    {
        crate::ai::cancel::cancel(&vault, &job.id);
        KNOWLEDGE_JOBS.end(
            &vault,
            &job.id,
            JobState::Cancelled,
            Some(AiFailure::Cancelled),
            now,
        );
    }
    backend
        .delete(Path::new(&path))
        .await
        .map_err(|error| match error {
            DeleteError::NotFound => AiFailure::LessonUnreadable,
            _ => AiFailure::JournalFailed,
        })?;
    partial::clear_partial(backend, &path).await;
    // Урок не заметка: ни наблюдатель, ни опрос диска о его удалении не скажут. Событие то же,
    // что о любом удалённом файле vault, и идёт через очередь событий бэкенда на обоих
    // таргетах: дерево перечитывается, вкладка закрывается без обходов в интерфейсе (T030).
    backend.publish_event(crate::vault::events::VaultEvent::NoteDeleted {
        path: std::path::PathBuf::from(&path),
    });
    remove_source(backend, &path, keep_knowledge, now).await
}
