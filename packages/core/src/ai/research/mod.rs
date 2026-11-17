//! Фоновое исследование в уроке (фича 056): веб-поиск встроенным инструментом подключённого
//! провайдера или чтение PDF из vault. Три шага, как у хода учителя 051 и пополнения графа
//! 054: [`begin_research`] пишет запрос в журнал, [`send_research`] ходит в сеть,
//! [`finish_research`] собирает заметку Typst и пишет итог в файл урока. На время сети не
//! держатся ни vault, ни очередь вызовов wasm; урок за это время идёт своим чередом.
//!
//! Что уходит модели: вопрос и, для PDF, текст страниц файла, прошедшего проверку доступа
//! 050. Инструмент поиска попадает в запрос только при включённой настройке `web_research`
//! и подключении, у которого поиск есть (Anthropic, OpenAI); иначе он не строится вовсе
//! (SC-003) — и это проверяется заново перед каждым запросом, в том числе перед
//! продолжением после `pause_turn`.
//!
//! Результат — заметка Typst в `<папка уроков>/Исследования/`. Текст модели экранируется
//! ([`note`]), ссылки идут через `#link("…")`, заметка помечена «найдено моделью».

pub mod note;
pub mod reply;
pub mod texts;
#[cfg(target_arch = "wasm32")]
pub mod web;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};

pub use reply::{ResearchParser, ResearchReply};

use super::access::{check_access_in, normalize_folder};
use super::cancel::{cancellable, flag_cancelled, flag_close};
use super::client::Connection;
use super::endpoint::base_url;
use super::jobs::{now_ms, Job, JobState, Registry};
use super::journal::{record_done, record_sent};
use super::knowledge::import::ImportSkipReason;
use super::knowledge::pdf_text::{pdf_text, PdfError, MAX_PDF_BYTES};
use super::lesson::file::{
    append_events, current_language, lesson_path, read_lesson, Language, LessonEvent, FORMAT,
};
use super::lesson::stream::StreamParser;
use super::lesson::{file_stem, LessonView, Next};
use super::price::estimate_usd;
use super::protocol::{
    authorize, check_status, request_has_search_tool, research_pdf_request, research_web_request,
    search_cost_usd, web_search_supported,
};
use super::settings::{load_settings, AiProvider, AiSettings};
use super::transport::{AiTransport, HttpMethod, HttpRequest};
use super::AiFailure;
use crate::vault::backend::{ReadError, VaultBackend};

/// Вопрос исследования, символов.
pub const QUESTION_MAX_CHARS: usize = 500;
/// Запросов в одном исследовании: первый и до трёх продолжений после `pause_turn`.
pub const MAX_ROUNDS: u32 = 4;
/// Исследований в одном уроке: страховка от бесконечного платного запуска.
pub const MAX_RESEARCH_PER_LESSON: usize = 30;
/// Исследований, которые учитель запускает за один ход.
pub const MAX_RESEARCH_PER_TURN: usize = 2;
/// Идущих и ожидающих исследований в vault.
pub const MAX_ACTIVE_PER_VAULT: usize = 8;
/// Тело запроса, байт: продолжение после `pause_turn` несёт результаты поиска целиком.
pub const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;
/// Символов текста PDF в одном запросе; остальные страницы не читаются, и оценка об этом
/// говорит.
pub const PDF_REQUEST_MAX_CHARS: usize = 300_000;
/// Изложение в событии `ResearchDone`, символов.
pub const SUMMARY_MAX_CHARS: usize = 1_500;
const MAX_TOKENS_WEB: u32 = 6_000;
const MAX_TOKENS_PDF: u32 = 4_096;
/// Токены на служебную часть запроса и на ответ при оценке PDF.
const OVERHEAD_TOKENS: u64 = 1_500;
const OUTPUT_TOKENS: u64 = 1_500;
/// Как долго запрошенное учителем исследование ждёт запуска клиентом, прежде чем стать
/// «прерванным».
const PENDING_TTL_MS: u64 = 120_000;

/// Вид исследования.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ResearchKind {
    Web,
    Pdf,
}

/// Источник результата: ссылка для веба, страницы для PDF.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ResearchSource {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<Vec<u32>>,
}

/// Состояние исследования для карточки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ResearchStatus {
    /// Ждёт места среди одновременных исследований (или запуска клиентом).
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
    /// Приложение закрыли посреди исследования: итога нет, можно повторить.
    Interrupted,
}

/// Исследование в `LessonView.research`: производное от событий файла урока и реестра
/// заданий процесса.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ResearchView {
    pub id: String,
    pub kind: ResearchKind,
    pub question: String,
    /// Путь PDF у исследования документа.
    pub path: Option<String>,
    pub by_user: bool,
    pub status: ResearchStatus,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub sources: Vec<ResearchSource>,
    /// Путь заметки с результатом.
    pub note: Option<String>,
    pub summary: Option<String>,
    /// Найдено источников (у идущего — по уже полученным ответам).
    pub found: u32,
    pub failure: Option<AiFailure>,
    /// Учитель запросил исследование, а клиент его ещё не запустил: клиент запускает его
    /// `ai_research_start` с этим `id`, один раз.
    pub launch_due: bool,
    /// Запуск требует подтверждения пользователем с оценкой стоимости: чтение PDF, которое
    /// запросил учитель.
    pub confirm_cost: bool,
    pub cost_usd: Option<f64>,
}

/// Задание исследования в реестре процесса.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ResearchJob {
    /// Он же `request_id` отмены.
    pub id: String,
    /// Путь урока.
    pub source: String,
    pub kind: ResearchKind,
    pub question: String,
    pub pdf: Option<String>,
    pub state: JobState,
    /// Ждёт места среди одновременных исследований (`research_parallel`).
    pub queued: bool,
    /// Законченных запросов к модели.
    pub rounds: u32,
    pub searches: u32,
    pub found: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Сумма цен запросов; `None`, если цена хоть одного неизвестна.
    pub cost_usd: Option<f64>,
    pub failure: Option<AiFailure>,
    pub note: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    #[serde(skip)]
    order: u64,
}

impl Job for ResearchJob {
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
        self.queued = false;
    }
}

/// Реестр исследований на процесс.
pub static RESEARCH_JOBS: LazyLock<Registry<ResearchJob>> = LazyLock::new(Registry::new);
static ORDER: AtomicU64 = AtomicU64::new(0);
/// Выдача мест среди одновременных исследований идёт под одним замком: считать занятые и
/// занимать место нужно как одно действие.
static CLAIM: Mutex<()> = Mutex::new(());

/// Задания исследований vault: идущие первыми.
pub fn research_jobs(vault_id: &str) -> Vec<ResearchJob> {
    RESEARCH_JOBS.list(vault_id, crate::properties::now_ms())
}

/// Охрана задания исследования: см. [`super::jobs::JobGuard`].
pub struct ResearchGuard(super::jobs::JobGuard<ResearchJob>);

impl ResearchGuard {
    pub fn new(vault_id: &str, id: &str) -> Self {
        ResearchGuard(super::jobs::JobGuard::new(&RESEARCH_JOBS, vault_id, id))
    }

    pub fn disarm(&mut self) {
        self.0.disarm();
    }
}

// ── Доступность поиска ──────────────────────────────────────────────────────

/// Доступен ли веб-поиск: обучение включено, исследование включено в настройках и у
/// подключения есть встроенный поиск. Единственное место, от которого зависит появление
/// инструмента поиска в запросе и инструмента `research` у учителя (SC-003).
pub fn web_search_available(settings: &AiSettings) -> bool {
    settings.enabled && settings.web_research && web_search_supported(settings.provider)
}

fn ensure_web(settings: &AiSettings) -> Result<(), AiFailure> {
    if !settings.web_research {
        Err(AiFailure::SearchDisabled)
    } else if !web_search_supported(settings.provider) {
        Err(AiFailure::SearchUnavailable)
    } else {
        Ok(())
    }
}

// ── Запрошенное учителем, но ещё не запущенное ──────────────────────────────

static PENDING: LazyLock<Mutex<HashMap<String, u64>>> = LazyLock::new(Mutex::default);

fn pending_key(path: &str, id: &str) -> String {
    format!("{}\0{id}", super::lesson::partial::canonical(path))
}

/// Запоминает исследование, запрошенное учителем: до запуска клиентом оно «в очереди», а не
/// «прервано».
pub(crate) fn mark_pending(path: &str, id: &str) {
    if let Ok(mut pending) = PENDING.lock() {
        let now = now_ms();
        pending.retain(|_, at| now.saturating_sub(*at) < PENDING_TTL_MS);
        pending.insert(pending_key(path, id), now);
    }
}

fn clear_pending(path: &str, id: &str) {
    if let Ok(mut pending) = PENDING.lock() {
        pending.remove(&pending_key(path, id));
    }
}

fn is_pending(path: &str, id: &str) -> bool {
    PENDING.lock().is_ok_and(|pending| {
        pending
            .get(&pending_key(path, id))
            .is_some_and(|at| now_ms().saturating_sub(*at) < PENDING_TTL_MS)
    })
}

/// Идентификатор исследования, которое запускает учитель: неповторим в пределах урока и,
/// практически, vault.
pub(crate) fn new_id(at: i64, index: usize, path: &str) -> String {
    let hash = blake3::hash(path.as_bytes()).to_hex();
    format!("research-{at}-{index}-{}", &hash.as_str()[..8])
}

// ── Вид ─────────────────────────────────────────────────────────────────────

/// Исследования из событий файла. Не законченное итогом — `Interrupted`: настоящее
/// состояние идущих добавляет [`annotate`]. События с неизвестным или повторным `id`
/// игнорируются, разбор линейный.
pub fn views_of(events: &[LessonEvent]) -> Vec<ResearchView> {
    let mut views: Vec<ResearchView> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for event in events {
        match event {
            LessonEvent::ResearchStarted {
                at,
                id,
                research,
                question,
                path,
                by_user,
            } => {
                if index.contains_key(id.as_str()) {
                    continue;
                }
                index.insert(id, views.len());
                views.push(ResearchView {
                    id: id.clone(),
                    kind: *research,
                    question: question.clone(),
                    path: path.clone(),
                    by_user: *by_user,
                    status: ResearchStatus::Interrupted,
                    started_at: *at,
                    finished_at: None,
                    sources: Vec::new(),
                    note: None,
                    summary: None,
                    found: 0,
                    failure: None,
                    launch_due: false,
                    confirm_cost: false,
                    cost_usd: None,
                });
            }
            LessonEvent::ResearchDone {
                at,
                id,
                note,
                summary,
                sources,
                cost_usd,
                ..
            } => {
                if let Some(view) = index.get(id.as_str()).map(|i| &mut views[*i]) {
                    if view.status == ResearchStatus::Interrupted {
                        view.status = ResearchStatus::Done;
                        view.finished_at = Some(*at);
                        view.note = Some(note.clone());
                        view.summary = Some(summary.clone());
                        view.found = sources.len() as u32;
                        view.sources = sources.clone();
                        view.cost_usd = *cost_usd;
                    }
                }
            }
            LessonEvent::ResearchFailed {
                at,
                id,
                failure,
                cancelled,
            } => {
                if let Some(view) = index.get(id.as_str()).map(|i| &mut views[*i]) {
                    if view.status == ResearchStatus::Interrupted {
                        view.status = if *cancelled {
                            ResearchStatus::Cancelled
                        } else {
                            ResearchStatus::Failed
                        };
                        view.finished_at = Some(*at);
                        view.failure = Some(*failure);
                    }
                }
            }
            _ => {}
        }
    }
    views
}

/// Дополняет вид урока тем, чего нет в файле: идущее исследование — `Running` или `Queued`
/// по реестру процесса, запрошенное учителем и ещё не запущенное — `Queued` с признаком
/// запуска. Остальное незаконченное остаётся `Interrupted`.
pub fn annotate(mut view: LessonView) -> LessonView {
    let wanted = super::lesson::partial::canonical(&view.path);
    for item in &mut view.research {
        if item.status != ResearchStatus::Interrupted {
            continue;
        }
        let running = RESEARCH_JOBS.running_where(None, |job| {
            job.id == item.id && super::lesson::partial::canonical(&job.source) == wanted
        });
        if let Some((_, job)) = running.first() {
            item.status = if job.queued {
                ResearchStatus::Queued
            } else {
                ResearchStatus::Running
            };
            item.found = job.found;
        } else if !item.by_user && is_pending(&view.path, &item.id) {
            item.status = ResearchStatus::Queued;
            item.launch_due = true;
            item.confirm_cost = item.kind == ResearchKind::Pdf;
        }
    }
    view
}

// ── PDF ─────────────────────────────────────────────────────────────────────

/// Оценка чтения PDF до отправки (FR-003).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ResearchEstimate {
    pub path: String,
    /// Страниц в документе и страниц, которые уйдут модели.
    pub pages: u32,
    pub pages_read: u32,
    /// Символов текста, которые уйдут модели.
    pub chars: u32,
    /// Оценка входных токенов (символы / 4 и служебная часть).
    pub units: u32,
    /// Оценка стоимости; `None` — цена неизвестна (не ноль, 050 FR-014).
    pub cost_usd: Option<f64>,
    /// Документ длиннее предела запроса: часть страниц не читается.
    pub truncated: bool,
    pub model: Option<String>,
    /// Почему читать нельзя; тогда запуск откажет.
    pub problem: Option<ImportSkipReason>,
}

struct PdfDoc {
    pages: u32,
    pages_read: u32,
    chars: usize,
    text: String,
    truncated: bool,
}

fn is_pdf(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
}

/// Читает PDF для запроса: доступ 050, предел размера до разбора, текст страниц со
/// ссылками на номера. Чужой разборщик и пределы — как у импорта (055): `pdf_text` как есть.
async fn read_pdf<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    language: Language,
    path: &str,
) -> Result<PdfDoc, ImportSkipReason> {
    let Some(path) = normalize_folder(Path::new(path)) else {
        return Err(ImportSkipReason::Denied);
    };
    if !check_access_in(backend, &path, settings).await {
        return Err(ImportSkipReason::Denied);
    }
    let bytes = match backend
        .read_bytes_limited(Path::new(&path), MAX_PDF_BYTES)
        .await
    {
        Ok(bytes) => bytes,
        Err(ReadError::NotFound) => return Err(ImportSkipReason::Missing),
        Err(ReadError::TooLarge) => return Err(ImportSkipReason::TooLarge),
        Err(_) => return Err(ImportSkipReason::Unsupported),
    };
    let text = match pdf_text(&bytes) {
        Ok(text) if text.has_text() => text,
        Ok(_) => return Err(ImportSkipReason::NoText),
        Err(PdfError::TooLarge) => return Err(ImportSkipReason::TooLarge),
        Err(PdfError::Unreadable) => return Err(ImportSkipReason::Unsupported),
    };
    let mut out = String::new();
    let mut chars = 0usize;
    let mut read = 0u32;
    let mut truncated = false;
    for (index, page) in text.pages.iter().enumerate() {
        let page = page.trim();
        if page.is_empty() {
            read = index as u32 + 1;
            continue;
        }
        let room = PDF_REQUEST_MAX_CHARS.saturating_sub(chars);
        let size = page.chars().count();
        if room == 0 || (size > room && read > 0 && chars > 0) {
            truncated = true;
            break;
        }
        let page: String = page.chars().take(room).collect();
        let taken = page.chars().count();
        out.push_str(&texts::page_marker(language, index + 1));
        out.push('\n');
        out.push_str(&page);
        out.push_str("\n\n");
        chars += taken;
        read = index as u32 + 1;
        if taken < size {
            truncated = true;
            break;
        }
    }
    Ok(PdfDoc {
        pages: text.pages.len() as u32,
        pages_read: read,
        chars,
        text: out,
        truncated,
    })
}

fn problem_failure(problem: ImportSkipReason) -> AiFailure {
    match problem {
        ImportSkipReason::Denied => AiFailure::Forbidden,
        _ => AiFailure::LessonUnreadable,
    }
}

/// Оценка чтения PDF: объём, страницы и стоимость; ничего не пишет и модели не отправляет.
/// Закрытый для ИИ путь и не-PDF — отказ `Forbidden`; нечитаемый, слишком большой и PDF без
/// текста — оценка с `problem`.
pub async fn research_estimate<B: VaultBackend>(
    backend: &B,
    pdf: &str,
) -> Result<ResearchEstimate, AiFailure> {
    let settings = load_settings(backend).await;
    if !settings.enabled {
        return Err(AiFailure::Disabled);
    }
    let Some(path) = normalize_folder(Path::new(pdf)) else {
        return Err(AiFailure::Forbidden);
    };
    if !is_pdf(&path) || !check_access_in(backend, &path, &settings).await {
        return Err(AiFailure::Forbidden);
    }
    let mut estimate = ResearchEstimate {
        path: path.clone(),
        pages: 0,
        pages_read: 0,
        chars: 0,
        units: 0,
        cost_usd: None,
        truncated: false,
        model: settings.model.clone(),
        problem: None,
    };
    match read_pdf(backend, &settings, Language::Ru, &path).await {
        Ok(doc) => {
            let tokens = doc.chars as u64 / 4 + OVERHEAD_TOKENS;
            estimate.pages = doc.pages;
            estimate.pages_read = doc.pages_read;
            estimate.chars = doc.chars.min(u32::MAX as usize) as u32;
            estimate.units = tokens.min(u64::from(u32::MAX)) as u32;
            estimate.truncated = doc.truncated;
            estimate.cost_usd = match (&settings.model, settings.provider) {
                (Some(model), AiProvider::Anthropic) => estimate_usd(model, tokens, OUTPUT_TOKENS),
                _ => None,
            };
        }
        Err(ImportSkipReason::Denied) => return Err(AiFailure::Forbidden),
        Err(problem) => estimate.problem = Some(problem),
    }
    Ok(estimate)
}

// ── Шаги ────────────────────────────────────────────────────────────────────

/// Запрос исследования, записанный в журнал. Ключа не содержит.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PreparedResearch {
    /// Запись журнала: `<request_id>#r<номер запроса>`.
    pub id: String,
    /// Идентификатор задания и отмены.
    pub request_id: String,
    /// Путь урока.
    pub path: String,
    pub kind: ResearchKind,
    pub provider: AiProvider,
    pub model: String,
    /// Номер запроса в исследовании, от 1.
    pub round: u32,
    /// Страниц в PDF (для проверки ссылок на страницы); у веба 0.
    pub pages: u32,
    /// Есть ли в запросе инструмент поиска.
    pub web: bool,
    /// Прочитана не вся книга.
    pub truncated: bool,
    pub request: HttpRequest,
}

/// Итог первого шага.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "step")]
pub enum ResearchBegin {
    /// Запрос в журнале, можно отправлять.
    Ready { prepared: PreparedResearch },
    /// Нет места среди одновременных исследований: повторить `begin` позже с тем же id.
    Queued,
    /// Делать больше нечего: исследование отменено до отправки.
    Finished { research: ResearchView },
}

/// Итог третьего шага.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ResearchFinish {
    pub research: ResearchView,
    /// `Continue` — отправить `prepared` (продолжение после `pause_turn`), `WaitUser` —
    /// исследование кончилось.
    pub next: Next,
    pub prepared: Option<PreparedResearch>,
}

fn fold(path: &str) -> String {
    super::lesson::partial::canonical(path).to_lowercase()
}

fn started_of(events: &[LessonEvent], id: &str) -> Option<(ResearchKind, String, Option<String>)> {
    events.iter().find_map(|event| match event {
        LessonEvent::ResearchStarted {
            id: known,
            research,
            question,
            path,
            ..
        } if known == id => Some((*research, question.clone(), path.clone())),
        _ => None,
    })
}

fn ended(events: &[LessonEvent], id: &str) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            LessonEvent::ResearchDone { id: known, .. }
                | LessonEvent::ResearchFailed { id: known, .. } if known == id
        )
    })
}

/// Вид исследования после изменений: событие файла плюс то, что знает реестр.
async fn view_for<B: VaultBackend>(
    backend: &B,
    vault_id: &str,
    path: &str,
    id: &str,
) -> Option<ResearchView> {
    let parsed = read_lesson(backend, path).await.ok()?;
    let mut view = views_of(&parsed.events)
        .into_iter()
        .find(|view| view.id == id)?;
    if let Some(job) = RESEARCH_JOBS.get(vault_id, id) {
        if job.state == JobState::Running && view.status == ResearchStatus::Interrupted {
            view.status = if job.queued {
                ResearchStatus::Queued
            } else {
                ResearchStatus::Running
            };
            view.found = job.found;
        }
    }
    Some(view)
}

/// Заканчивает исследование отказом: событие в файл урока (если оно там начато и не
/// закончено) и конечное состояние задания. Отмена — не отказ: `cancelled`.
async fn fail_job<B: VaultBackend>(
    backend: &mut B,
    vault_id: &str,
    path: &str,
    id: &str,
    failure: AiFailure,
    now: i64,
) -> Option<ResearchView> {
    let cancelled = failure == AiFailure::Cancelled;
    if let Ok(parsed) = read_lesson(backend, path).await {
        if parsed.header.format <= FORMAT
            && started_of(&parsed.events, id).is_some()
            && !ended(&parsed.events, id)
        {
            let event = LessonEvent::ResearchFailed {
                at: now,
                id: id.to_owned(),
                failure,
                cancelled,
            };
            let _ = append_events(backend, path, &[event]).await;
        }
    }
    RESEARCH_JOBS.end(
        vault_id,
        id,
        if cancelled {
            JobState::Cancelled
        } else {
            JobState::Failed
        },
        Some(failure),
        now,
    );
    clear_pending(path, id);
    view_for(backend, vault_id, path, id).await
}

/// Занимает место среди одновременных исследований: по очереди поступления и не больше
/// `limit` идущих. `true` — задание больше не в очереди.
fn claim_slot(vault_id: &str, id: &str, limit: usize) -> bool {
    let Ok(_guard) = CLAIM.lock() else {
        return false;
    };
    let Some(me) = RESEARCH_JOBS.get(vault_id, id) else {
        return false;
    };
    if me.state != JobState::Running {
        return false;
    }
    if !me.queued {
        return true;
    }
    let active = RESEARCH_JOBS
        .running_where(Some(vault_id), |job| !job.queued)
        .len();
    if active >= limit.max(1) {
        return false;
    }
    let first = RESEARCH_JOBS
        .running_where(Some(vault_id), |job| job.queued)
        .into_iter()
        .map(|(_, job)| job)
        .min_by_key(|job| job.order);
    if first.is_some_and(|job| job.id == id) {
        RESEARCH_JOBS.update(vault_id, id, |job| job.queued = false);
        true
    } else {
        false
    }
}

/// Шаг 1. Выключенное обучение отказывает до любой записи и сети. Веб-исследование при
/// выключенной настройке или у подключения без поиска — `SearchDisabled` /
/// `SearchUnavailable` до записи и запроса. Первый вызов с `request_id` заводит задание и,
/// если исследование запустил пользователь, событие `ResearchStarted` в файле урока;
/// исследование, запрошенное учителем, уже записано в файле, и вопрос берётся оттуда.
/// Следующие вызовы с тем же id пока `Queued` пробуют занять место. Ключ и одобрение адреса
/// оболочка проверила до этого вызова; `verified` — снимок подключения.
#[allow(clippy::too_many_arguments)]
pub async fn begin_research<B: VaultBackend>(
    backend: &mut B,
    vault_id: &str,
    path: &str,
    request_id: &str,
    kind: ResearchKind,
    question: &str,
    pdf: Option<&str>,
    now: i64,
    allow_custom: bool,
    verified: &Connection,
) -> Result<ResearchBegin, AiFailure> {
    let known = RESEARCH_JOBS.get(vault_id, request_id);
    let mut created = false;
    let result = begin_inner(
        backend,
        vault_id,
        path,
        request_id,
        (kind, question, pdf),
        now,
        allow_custom,
        verified,
        known.is_some(),
        &mut created,
    )
    .await;
    if let Err(failure) = &result {
        // Заканчивается отказом задание, которое идёт или заведено этим вызовом; чужое с тем
        // же id (второй вызов получил `Cancelled`) не трогаем.
        let path = lesson_path(path).unwrap_or_else(|_| path.to_owned());
        if known.is_some() || created {
            fail_job(backend, vault_id, &path, request_id, *failure, now).await;
        } else if *failure != AiFailure::Cancelled {
            // Исследование, которое запросил учитель, уже записано в файле: отказ до заведения
            // задания закрывает его, а не оставляет «прерванным».
            if let Ok(lesson) = read_lesson(backend, &path).await {
                if started_of(&lesson.events, request_id).is_some()
                    && !ended(&lesson.events, request_id)
                {
                    fail_job(backend, vault_id, &path, request_id, *failure, now).await;
                }
            }
        }
    }
    result
}

#[allow(clippy::too_many_arguments)]
async fn begin_inner<B: VaultBackend>(
    backend: &mut B,
    vault_id: &str,
    path: &str,
    request_id: &str,
    (kind, question, pdf): (ResearchKind, &str, Option<&str>),
    now: i64,
    allow_custom: bool,
    verified: &Connection,
    continuing: bool,
    created: &mut bool,
) -> Result<ResearchBegin, AiFailure> {
    let settings = load_settings(backend).await;
    if !settings.enabled {
        return Err(AiFailure::Disabled);
    }
    let path = lesson_path(path)?;
    if !verified.is_current(&settings, allow_custom) {
        return Err(AiFailure::EndpointNotApproved);
    }
    // Урок из закрытой папки модели не отдаётся ни кусочком (050, FR-015).
    if !check_access_in(backend, &path, &settings).await {
        return Err(AiFailure::Forbidden);
    }
    let lesson = read_lesson(backend, &path).await?;
    if lesson.header.format > FORMAT {
        return Err(AiFailure::LessonUnreadable);
    }
    let language = current_language(&lesson);
    // Вопрос исследования, запрошенного учителем, берётся из файла: клиент не подменит его.
    let from_file = started_of(&lesson.events, request_id);
    if from_file.is_some() && ended(&lesson.events, request_id) {
        return Err(AiFailure::Cancelled);
    }
    let (kind, question, pdf_path) = match from_file.clone() {
        Some(found) => found,
        None => {
            let question = question.trim();
            if question.is_empty() {
                return Err(AiFailure::GoalEmpty);
            }
            let pdf_path = match kind {
                ResearchKind::Web => None,
                ResearchKind::Pdf => Some(
                    pdf.and_then(|pdf| normalize_folder(Path::new(pdf)))
                        .ok_or(AiFailure::Forbidden)?,
                ),
            };
            (
                kind,
                question
                    .chars()
                    .take(QUESTION_MAX_CHARS)
                    .collect::<String>(),
                pdf_path,
            )
        }
    };
    if kind == ResearchKind::Web {
        ensure_web(&settings)?;
    } else {
        // Закрытый для ИИ документ отказывает до любой записи: ни событий, ни заданий.
        let pdf = pdf_path
            .as_deref()
            .filter(|pdf| is_pdf(pdf))
            .ok_or(AiFailure::Forbidden)?;
        if !check_access_in(backend, pdf, &settings).await {
            return Err(AiFailure::Forbidden);
        }
    }

    if continuing {
        match RESEARCH_JOBS.get(vault_id, request_id) {
            Some(job)
                if job.state == JobState::Running
                    && fold(&job.source) == fold(&path)
                    && job.kind == kind
                    && job.rounds == 0 => {}
            _ => return Err(AiFailure::Cancelled),
        }
        if flag_cancelled(vault_id, request_id) {
            let research = fail_job(
                backend,
                vault_id,
                &path,
                request_id,
                AiFailure::Cancelled,
                now,
            )
            .await
            .ok_or(AiFailure::Cancelled)?;
            return Ok(ResearchBegin::Finished { research });
        }
    } else {
        let started = lesson
            .events
            .iter()
            .filter(|event| matches!(event, LessonEvent::ResearchStarted { .. }))
            .count();
        if from_file.is_none() && started >= MAX_RESEARCH_PER_LESSON {
            return Err(AiFailure::RateLimited);
        }
        if RESEARCH_JOBS.running_where(Some(vault_id), |_| true).len() >= MAX_ACTIVE_PER_VAULT {
            return Err(AiFailure::RateLimited);
        }
        if !RESEARCH_JOBS.open_flag(vault_id, request_id) {
            return Err(AiFailure::Cancelled);
        }
        let job = ResearchJob {
            id: request_id.to_owned(),
            source: path.clone(),
            kind,
            question: question.clone(),
            pdf: pdf_path.clone(),
            state: JobState::Running,
            queued: true,
            rounds: 0,
            searches: 0,
            found: 0,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd: Some(0.0),
            failure: None,
            note: None,
            started_at: now,
            finished_at: None,
            order: ORDER.fetch_add(1, Ordering::Relaxed),
        };
        // То же исследование (вопрос и документ) второй раз не запускается, пока идёт первое.
        let (wanted_source, wanted_question, wanted_pdf) = (
            fold(&path),
            question.to_lowercase(),
            pdf_path.as_deref().map(fold),
        );
        *created = true;
        if let Err(failure) = RESEARCH_JOBS.start(vault_id, job, now, move |other| {
            other.kind == kind
                && fold(&other.source) == wanted_source
                && other.question.to_lowercase() == wanted_question
                && other.pdf.as_deref().map(fold) == wanted_pdf
        }) {
            *created = false;
            flag_close(vault_id, request_id);
            return Err(failure);
        }
        if from_file.is_none() {
            let event = LessonEvent::ResearchStarted {
                at: now,
                id: request_id.to_owned(),
                research: kind,
                question: question.clone(),
                path: pdf_path.clone(),
                by_user: true,
            };
            append_events(backend, &path, &[event]).await?;
        }
    }
    clear_pending(&path, request_id);

    if !claim_slot(vault_id, request_id, settings.research_parallel as usize) {
        return Ok(ResearchBegin::Queued);
    }

    let base = base_url(&settings, allow_custom)?;
    let model = settings.model.clone().ok_or(AiFailure::ModelMissing)?;
    let system = texts::system(language, kind);
    let (request, pages, truncated) = match kind {
        ResearchKind::Web => {
            let request = research_web_request(
                settings.provider,
                &base,
                &model,
                system,
                &texts::web_message(language, &question),
                MAX_TOKENS_WEB,
            )
            .ok_or(AiFailure::SearchUnavailable)?;
            (request, 0, false)
        }
        ResearchKind::Pdf => {
            let path = pdf_path.as_deref().unwrap_or_default();
            let doc = read_pdf(backend, &settings, language, path)
                .await
                .map_err(problem_failure)?;
            let request = research_pdf_request(
                settings.provider,
                &base,
                &model,
                system,
                &texts::pdf_message(language, &question, &doc.text),
                MAX_TOKENS_PDF,
            );
            (request, doc.pages, doc.truncated)
        }
    };
    let id = format!("{request_id}#r1");
    record_sent(
        backend,
        &id,
        now,
        settings.provider,
        &model,
        &request.url,
        request.body.as_deref().unwrap_or_default(),
    )
    .await?;
    Ok(ResearchBegin::Ready {
        prepared: PreparedResearch {
            id,
            request_id: request_id.to_owned(),
            path,
            kind,
            provider: settings.provider,
            model,
            round: 1,
            pages,
            web: kind == ResearchKind::Web,
            truncated,
            request,
        },
    })
}

/// Шаг 2. Только сеть: vault здесь не нужен. Ключ уходит только на тот адрес и тому
/// провайдеру, что в `verified`. Запрос без поиска, в теле которого всё же есть инструмент
/// поиска, не отправляется.
pub async fn send_research<T: AiTransport>(
    transport: &T,
    vault_id: &str,
    prepared: &PreparedResearch,
    key: Option<&str>,
    verified: &Connection,
) -> Result<ResearchReply, AiFailure> {
    if !verified.covers(prepared.provider, &prepared.request.url) {
        return Err(AiFailure::EndpointNotApproved);
    }
    let body = prepared.request.body.as_deref().unwrap_or_default();
    let has_search = request_has_search_tool(body);
    if has_search && (!prepared.web || !web_search_supported(prepared.provider)) {
        return Err(AiFailure::SearchUnavailable);
    }
    let mut request = prepared.request.clone();
    authorize(&mut request, prepared.provider, key);
    let work = async {
        if prepared.web {
            let mut parser = ResearchParser::new(prepared.provider);
            let status = transport
                .send_stream(request, &mut |chunk| parser.feed(chunk))
                .await?;
            check_status(status)?;
            parser.finish()
        } else {
            let mut parser = StreamParser::new(prepared.provider);
            let status = transport
                .send_stream(request, &mut |chunk| parser.feed(chunk))
                .await?;
            check_status(status)?;
            parser.finish().map(ResearchReply::from_turn)
        }
    };
    cancellable(vault_id, &prepared.request_id, work).await
}

fn failure_of_search_errors(errors: &[String]) -> AiFailure {
    if errors.iter().any(|code| code == "too_many_requests") {
        AiFailure::RateLimited
    } else if errors.iter().any(|code| code == "unavailable") {
        AiFailure::ProviderDown
    } else {
        AiFailure::BadResponse
    }
}

fn same_url(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

/// Источники результата. Веб: сперва цитаты (провайдер сам привязал их к тексту), потом
/// названные моделью; страницы, найденные поиском, но не процитированные, — только если
/// больше ничего нет. PDF: названные моделью со страницами, существующими в документе.
fn merge_sources(
    kind: ResearchKind,
    submitted: Vec<ResearchSource>,
    reply: &ResearchReply,
    pdf_name: &str,
) -> Vec<ResearchSource> {
    let mut out: Vec<ResearchSource> = Vec::new();
    let push = |out: &mut Vec<ResearchSource>, source: ResearchSource| {
        let duplicate = source.url.as_deref().is_some_and(|url| {
            out.iter()
                .any(|known| known.url.as_deref().is_some_and(|u| same_url(u, url)))
        });
        if !duplicate && out.len() < reply::MAX_SOURCES {
            out.push(source);
        }
    };
    match kind {
        ResearchKind::Web => {
            for source in reply.cited.iter().cloned().chain(submitted) {
                push(&mut out, source);
            }
            if out.is_empty() {
                for source in reply.found.iter().take(5).cloned() {
                    push(&mut out, source);
                }
            }
        }
        ResearchKind::Pdf => {
            for mut source in submitted {
                if source.title.trim().is_empty() {
                    source.title = pdf_name.to_owned();
                }
                out.push(source);
            }
        }
    }
    out
}

fn summary_of(paragraphs: &[String]) -> String {
    let joined = paragraphs.join("\n\n");
    if joined.chars().count() <= SUMMARY_MAX_CHARS {
        return joined;
    }
    let mut clipped: String = joined.chars().take(SUMMARY_MAX_CHARS - 1).collect();
    clipped.push('…');
    clipped
}

/// Записывает заметку под свободным именем в `<папка уроков>/Исследования/`. Запись не идёт
/// через символические ссылки; существующий файл (в том числе ссылка и файл без прав) имя
/// не освобождает.
async fn write_note<B: VaultBackend>(
    backend: &mut B,
    settings: &AiSettings,
    question: &str,
    text: &str,
) -> Result<String, AiFailure> {
    let folder = Path::new(&settings.lessons_folder).join("Исследования");
    if backend.crosses_symlink(&folder.join("_")).await || backend.is_symlink(&folder).await {
        return Err(AiFailure::JournalFailed);
    }
    backend
        .ensure_folder(&folder)
        .await
        .map_err(|_| AiFailure::JournalFailed)?;
    let stem = file_stem(question);
    let mut number = 1u32;
    let path = loop {
        let name = if number == 1 {
            format!("{stem}.typ")
        } else {
            format!("{stem} {number}.typ")
        };
        let candidate = folder.join(name);
        let free = !backend.is_symlink(&candidate).await
            && matches!(
                backend.read_bytes_any_size(&candidate).await,
                Err(ReadError::NotFound)
            );
        if free {
            break candidate;
        }
        number += 1;
        if number > 10_000 {
            return Err(AiFailure::JournalFailed);
        }
    };
    backend
        .write(&path, text, None)
        .await
        .map_err(|_| AiFailure::JournalFailed)?;
    Ok(path.to_string_lossy().replace('\\', "/"))
}

/// Продолжение после `pause_turn`: тот же запрос с ответом ассистента, возвращённым
/// провайдеру как есть. Поиск, выключенный с начала исследования, продолжение не получает.
async fn continue_request<B: VaultBackend>(
    backend: &mut B,
    prepared: &PreparedResearch,
    assistant: &str,
    now: i64,
) -> Result<PreparedResearch, AiFailure> {
    let settings = load_settings(backend).await;
    if !settings.enabled {
        return Err(AiFailure::Disabled);
    }
    if prepared.web && !web_search_available(&settings) {
        return Err(AiFailure::SearchDisabled);
    }
    // Настройки едут с синхронизацией: подключение должно остаться тем, что было.
    let current = Connection::verify(&settings, true)?;
    if !current.covers(prepared.provider, &prepared.request.url) {
        return Err(AiFailure::EndpointNotApproved);
    }
    let mut body: serde_json::Value =
        serde_json::from_str(prepared.request.body.as_deref().unwrap_or_default())
            .map_err(|_| AiFailure::BadResponse)?;
    let content: serde_json::Value =
        serde_json::from_str(assistant).map_err(|_| AiFailure::BadResponse)?;
    body["messages"]
        .as_array_mut()
        .ok_or(AiFailure::BadResponse)?
        .push(serde_json::json!({"role": "assistant", "content": content}));
    let body = body.to_string();
    if body.len() > MAX_REQUEST_BYTES {
        return Err(AiFailure::BadResponse);
    }
    let round = prepared.round + 1;
    let id = format!("{}#r{round}", prepared.request_id);
    let request = HttpRequest {
        method: HttpMethod::Post,
        url: prepared.request.url.clone(),
        headers: prepared.request.headers.clone(),
        body: Some(body),
    };
    record_sent(
        backend,
        &id,
        now,
        prepared.provider,
        &prepared.model,
        &request.url,
        request.body.as_deref().unwrap_or_default(),
    )
    .await?;
    Ok(PreparedResearch {
        id,
        round,
        request,
        ..prepared.clone()
    })
}

fn request_cost(prepared: &PreparedResearch, reply: &ResearchReply) -> Option<f64> {
    if prepared.provider != AiProvider::Anthropic {
        return None;
    }
    let tokens = estimate_usd(&prepared.model, reply.input_tokens?, reply.output_tokens?)?;
    Some(tokens + search_cost_usd(prepared.provider, u64::from(reply.searches))?)
}

/// Шаг 3. Ответ принимается, только пока задание идёт и это то же исследование и тот же по
/// счёту запрос: поддельный или запоздавший (задание отменено, урок удалён) файл не пишет.
/// Отказ не возвращается ошибкой: он остаётся в файле урока событием `ResearchFailed`.
pub async fn finish_research<B: VaultBackend>(
    backend: &mut B,
    vault_id: &str,
    prepared: &PreparedResearch,
    now: i64,
    outcome: Result<ResearchReply, AiFailure>,
) -> Result<ResearchFinish, AiFailure> {
    match RESEARCH_JOBS.get(vault_id, &prepared.request_id) {
        Some(job)
            if job.state == JobState::Running
                && !job.queued
                && fold(&job.source) == fold(&prepared.path)
                && job.kind == prepared.kind
                && job.rounds + 1 == prepared.round => {}
        _ => return Err(AiFailure::Cancelled),
    }
    let stop = |view: Option<ResearchView>| -> Result<ResearchFinish, AiFailure> {
        view.map(|research| ResearchFinish {
            research,
            next: Next::WaitUser,
            prepared: None,
        })
        .ok_or(AiFailure::Cancelled)
    };
    let reply = match outcome {
        Ok(reply) => reply,
        Err(failure) => {
            let _ = record_done(backend, &prepared.id, now, Some(failure), None, None, None).await;
            let view = fail_job(
                backend,
                vault_id,
                &prepared.path,
                &prepared.request_id,
                failure,
                now,
            )
            .await;
            return stop(view);
        }
    };
    let cost = request_cost(prepared, &reply);
    let _ = record_done(
        backend,
        &prepared.id,
        now,
        None,
        reply.input_tokens,
        reply.output_tokens,
        cost,
    )
    .await;
    let found = (reply.cited.len().max(reply.found.len())) as u32;
    let job = RESEARCH_JOBS.update(vault_id, &prepared.request_id, |job| {
        job.rounds += 1;
        job.searches = job.searches.saturating_add(reply.searches);
        job.found = job.found.max(found);
        job.input_tokens = job
            .input_tokens
            .saturating_add(reply.input_tokens.unwrap_or(0));
        job.output_tokens = job
            .output_tokens
            .saturating_add(reply.output_tokens.unwrap_or(0));
        job.cost_usd = match (job.cost_usd, cost) {
            (Some(sum), Some(price)) => Some(sum + price),
            _ => None,
        };
    });
    let Some(job) = job else {
        return Err(AiFailure::Cancelled);
    };

    // Модель просит продолжить: отправить её ответ обратно, ограниченное число раз.
    if reply.pause && job.rounds < MAX_ROUNDS && reply.submit.is_none() {
        if flag_cancelled(vault_id, &prepared.request_id) {
            let view = fail_job(
                backend,
                vault_id,
                &prepared.path,
                &prepared.request_id,
                AiFailure::Cancelled,
                now,
            )
            .await;
            return stop(view);
        }
        let next = match &reply.assistant {
            Some(assistant) => continue_request(backend, prepared, assistant, now).await,
            None => Err(AiFailure::BadResponse),
        };
        return match next {
            Ok(next) => {
                let view = view_for(backend, vault_id, &prepared.path, &prepared.request_id)
                    .await
                    .ok_or(AiFailure::Cancelled)?;
                Ok(ResearchFinish {
                    research: view,
                    next: Next::Continue,
                    prepared: Some(next),
                })
            }
            Err(failure) => {
                let view = fail_job(
                    backend,
                    vault_id,
                    &prepared.path,
                    &prepared.request_id,
                    failure,
                    now,
                )
                .await;
                stop(view)
            }
        };
    }

    // Результат: сданный инструментом или, если модель его не вызвала, её текст.
    let submission = reply
        .submit
        .as_deref()
        .and_then(|arguments| reply::parse_submit(arguments, prepared.kind, prepared.pages))
        .or_else(|| {
            let paragraphs = reply::paragraphs_of(&reply.text);
            (!paragraphs.is_empty()).then_some(reply::Submission {
                paragraphs,
                sources: Vec::new(),
            })
        });
    let Some(submission) = submission else {
        let failure = if reply.text.trim().is_empty() && !reply.search_errors.is_empty() {
            failure_of_search_errors(&reply.search_errors)
        } else {
            AiFailure::BadResponse
        };
        let view = fail_job(
            backend,
            vault_id,
            &prepared.path,
            &prepared.request_id,
            failure,
            now,
        )
        .await;
        return stop(view);
    };
    let done = write_result(backend, vault_id, prepared, now, &job, reply, submission).await;
    match done {
        Ok(view) => Ok(ResearchFinish {
            research: view,
            next: Next::WaitUser,
            prepared: None,
        }),
        Err(failure) => {
            let view = fail_job(
                backend,
                vault_id,
                &prepared.path,
                &prepared.request_id,
                failure,
                now,
            )
            .await;
            stop(view)
        }
    }
}

/// Заметка в vault, событие `ResearchDone` в файл урока, задание закончено.
async fn write_result<B: VaultBackend>(
    backend: &mut B,
    vault_id: &str,
    prepared: &PreparedResearch,
    now: i64,
    job: &ResearchJob,
    reply: ResearchReply,
    submission: reply::Submission,
) -> Result<ResearchView, AiFailure> {
    let settings = load_settings(backend).await;
    let lesson = read_lesson(backend, &prepared.path).await?;
    let pdf_name = job
        .pdf
        .as_deref()
        .and_then(|pdf| Path::new(pdf).file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let sources = merge_sources(prepared.kind, submission.sources, &reply, &pdf_name);
    let text = note::build_note(&note::NoteInput {
        language: current_language(&lesson),
        kind: prepared.kind,
        question: &job.question,
        pdf: job.pdf.as_deref(),
        model: &prepared.model,
        paragraphs: &submission.paragraphs,
        sources: &sources,
        truncated: prepared.truncated,
    });
    let note_path = write_note(backend, &settings, &job.question, &text).await?;
    let summary = summary_of(&submission.paragraphs);
    let event = LessonEvent::ResearchDone {
        at: now,
        id: prepared.request_id.clone(),
        note: note_path.clone(),
        summary,
        sources,
        input_tokens: Some(job.input_tokens),
        output_tokens: Some(job.output_tokens),
        cost_usd: job.cost_usd,
    };
    append_events(backend, &prepared.path, &[event]).await?;
    RESEARCH_JOBS.update(vault_id, &prepared.request_id, |job| {
        job.note = Some(note_path.clone());
    });
    RESEARCH_JOBS.end(vault_id, &prepared.request_id, JobState::Done, None, now);
    view_for(backend, vault_id, &prepared.path, &prepared.request_id)
        .await
        .ok_or(AiFailure::Cancelled)
}
