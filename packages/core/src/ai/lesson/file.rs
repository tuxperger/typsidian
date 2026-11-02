//! Файл урока (фича 051, research.md §1, data-model.md): JSON Lines, только
//! дописывание. Первая строка — шапка, дальше события. Нечитаемая строка
//! пропускается, как в журнале 050: сбой посреди записи портит не больше последней.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::plan::{self, PlanStep, PlanVersionInfo, PlanView};
use crate::ai::access::normalize_folder;
use crate::ai::AiFailure;
use crate::vault::backend::VaultBackend;
use crate::vault::{classify, FileKind};

/// Версия формата, которую понимает это ядро. Файл новее открывается только для
/// чтения (`LessonView::read_only`), ход в нём не начинается.
pub const FORMAT: u32 = 1;
/// Предел цели урока в символах.
pub const GOAL_MAX_CHARS: usize = 500;

/// Язык общения учителя.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Ru,
    En,
}

/// Уверенность пользователя в ответе: учитель разбирает «угадал» иначе, чем «уверен».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Confidence {
    Guess,
    Likely,
    Sure,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum LessonState {
    #[default]
    Active,
    Finished,
    Stopped,
}

/// Стадия урока (фича 052, research.md §1): замер понимания или обучение.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum LessonPhase {
    Probe,
    /// По умолчанию: уроки 051 без поля `start` читаются как раньше.
    #[default]
    Teach,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LessonHeader {
    pub format: u32,
    pub goal: String,
    pub language: Language,
    pub created_at: i64,
    /// С какой стадии начался урок.
    #[serde(default)]
    pub start: LessonPhase,
}

/// Вопрос учителя (инструмент `ask_question`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LessonQuestion {
    pub question: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,
    /// Часть цели, о которой вопрос замера (`probe_question`); у вопросов учителя нет.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
}

/// Знание части цели по итогам замера.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ProbeState {
    Known,
    Partial,
    Unknown,
}

/// Часть цели в итоге замера.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ProbePart {
    pub name: String,
    pub state: ProbeState,
    pub note: String,
}

/// Итог замера для вида урока.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ProbeSummary {
    pub parts: Vec<ProbePart>,
    /// С чего начать урок.
    pub start: String,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Чем кончилось чтение заметки: текст в урок не пишется, он есть в заметке и в
/// журнале 050 (research.md §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum NoteOutcome {
    Sent { chars: u32, truncated: bool },
    Denied,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum LessonEvent {
    /// Ответ пользователя. Пишется до журнала и сети (FR-007).
    User {
        at: i64,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        choice: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        confidence: Option<Confidence>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thinking: Option<String>,
        /// «Не знаю»: ответ-сведение, а не ошибка (052, research.md §2).
        #[serde(default, skip_serializing_if = "is_false")]
        dont_know: bool,
    },
    /// Итог хода учителя: целиком или оборванный.
    Teacher {
        at: i64,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        question: Option<LessonQuestion>,
        #[serde(default)]
        interrupted: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failure: Option<AiFailure>,
        model: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input_tokens: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output_tokens: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cost_usd: Option<f64>,
    },
    NoteRead {
        at: i64,
        path: String,
        by_user: bool,
        outcome: NoteOutcome,
    },
    /// Изложение пройденного вместо `covers` первых событий (research.md §6).
    Summary {
        at: i64,
        text: String,
        covers: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input_tokens: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output_tokens: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cost_usd: Option<f64>,
    },
    Language {
        at: i64,
        language: Language,
    },
    State {
        at: i64,
        state: LessonState,
    },
    /// Смена стадии. `skipped` — пользователь пропустил замер.
    Phase {
        at: i64,
        phase: LessonPhase,
        #[serde(default, skip_serializing_if = "is_false")]
        skipped: bool,
    },
    /// Итог замера (инструмент `probe_done`); стадия становится `Teach`.
    ProbeResult {
        at: i64,
        parts: Vec<ProbePart>,
        start: String,
    },
    /// Принятая версия плана (инструмент `plan_set`).
    Plan {
        at: i64,
        version: u32,
        steps: Vec<PlanStep>,
        goal: String,
        current: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// Отметки хода по плану (инструмент `plan_progress`).
    PlanProgress {
        at: i64,
        current: String,
        done: Vec<String>,
    },
    /// Отказ ядра инструменту плана: причина уходит модели результатом инструмента, урок
    /// не прерывается. В вид плана такой отказ не попадает.
    PlanRejected {
        at: i64,
        tool: String,
        reason: String,
    },
}

impl LessonEvent {
    pub fn at(&self) -> i64 {
        match self {
            LessonEvent::User { at, .. }
            | LessonEvent::Teacher { at, .. }
            | LessonEvent::NoteRead { at, .. }
            | LessonEvent::Summary { at, .. }
            | LessonEvent::Language { at, .. }
            | LessonEvent::State { at, .. }
            | LessonEvent::Phase { at, .. }
            | LessonEvent::ProbeResult { at, .. }
            | LessonEvent::Plan { at, .. }
            | LessonEvent::PlanProgress { at, .. }
            | LessonEvent::PlanRejected { at, .. } => *at,
        }
    }
}

/// Прочитанный файл урока.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedLesson {
    pub header: LessonHeader,
    pub events: Vec<LessonEvent>,
}

/// Чего ждёт урок.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Waiting {
    /// Урок только создан: учитель ещё не говорил.
    Start,
    /// Учитель ответил: ждёт ответа пользователя.
    User,
    /// Ответ пользователя сохранён, а ответа учителя нет или он оборван:
    /// «Запросить ответ снова» (`answer: null`).
    Retry,
    /// Урок завершён или остановлен.
    Closed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LessonView {
    pub path: String,
    pub header: LessonHeader,
    /// Действующий язык: шапка или последняя смена.
    pub language: Language,
    pub state: LessonState,
    pub events: Vec<LessonEvent>,
    pub waiting: Waiting,
    /// Вопрос последней реплики учителя, пока урок ждёт ответа.
    pub last_question: Option<LessonQuestion>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Сумма цен; `None`, если цена хоть одного хода неизвестна (050 FR-014).
    pub cost_usd: Option<f64>,
    /// Число событий для проверки «урок изменился» (research.md §10).
    pub events_len: u32,
    /// Формат файла новее известного: смотреть можно, вести урок нельзя.
    pub read_only: bool,
    /// Действующая стадия: `start` шапки, итог замера или пропуск.
    pub phase: LessonPhase,
    /// Сколько вопросов замера задано.
    pub probe_count: u32,
    pub probe_result: Option<ProbeSummary>,
    /// Последняя принятая версия плана с раскладкой.
    pub plan: Option<PlanView>,
    /// Версии плана, от первой.
    pub plan_history: Vec<PlanVersionInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LessonSummary {
    pub path: String,
    pub goal: String,
    /// Время последнего события (мс); у урока без событий — создания.
    pub last_at: i64,
    /// Число шагов: ответы пользователя и реплики учителя.
    pub steps: u32,
    pub state: LessonState,
    /// Чего ждёт урок: «ждёт ответа», «ответ оборван», «завершён», «остановлен».
    pub waiting: Waiting,
}

/// Разбирает файл: шапка обязательна, нечитаемые строки событий пропускаются.
pub fn parse(bytes: &[u8]) -> Result<ParsedLesson, AiFailure> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let header: LessonHeader = lines
        .next()
        .and_then(|line| serde_json::from_str(line).ok())
        .ok_or(AiFailure::LessonUnreadable)?;
    let events = lines
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    Ok(ParsedLesson { header, events })
}

fn line_of<T: Serialize>(value: &T) -> Result<String, AiFailure> {
    serde_json::to_string(value).map_err(|_| AiFailure::JournalFailed)
}

/// Первое содержимое нового файла.
pub fn initial_text(header: &LessonHeader, events: &[LessonEvent]) -> Result<String, AiFailure> {
    let mut text = line_of(header)?;
    text.push('\n');
    for event in events {
        text.push_str(&line_of(event)?);
        text.push('\n');
    }
    Ok(text)
}

/// Путь урока в чистой форме (без `.`, без `..`) или отказ: команды урока принимают
/// только файлы вида `Lesson` вне служебного места (ревью безопасности 051, L6) —
/// иначе через них читалось бы и дописывалось что угодно в vault.
pub fn lesson_path(path: &str) -> Result<String, AiFailure> {
    let clean = normalize_folder(Path::new(path)).ok_or(AiFailure::LessonUnreadable)?;
    if classify(Path::new(&clean)) == FileKind::Lesson {
        Ok(clean)
    } else {
        Err(AiFailure::LessonUnreadable)
    }
}

/// Читает и разбирает урок. Предел размера — предел заметки, он проверяется по
/// метаданным до чтения (M3): огромный файл с расширением `.lesson` памяти не исчерпает.
pub async fn read_lesson<B: VaultBackend>(
    backend: &B,
    path: &str,
) -> Result<ParsedLesson, AiFailure> {
    let path = lesson_path(path)?;
    let bytes = backend
        .read_bytes(Path::new(&path))
        .await
        .map_err(|_| AiFailure::LessonUnreadable)?;
    parse(&bytes)
}

/// Дописывает события. Файл читается заново и переписывается целиком с новыми
/// строками в конце: прежние строки не меняются, а запись самого файла атомарна.
/// Оборванная последняя строка (сбой посреди прежней записи) отделяется переводом
/// строки, чтобы новая не слиплась с ней. Запись не идёт через символические ссылки:
/// они увели бы её за пределы vault (ревью безопасности 051, M1).
pub async fn append_events<B: VaultBackend>(
    backend: &mut B,
    path: &str,
    events: &[LessonEvent],
) -> Result<(), AiFailure> {
    if events.is_empty() {
        return Ok(());
    }
    let path = lesson_path(path)?;
    let path = Path::new(&path);
    if backend.crosses_symlink(path).await || backend.is_symlink(path).await {
        return Err(AiFailure::LessonUnreadable);
    }
    let current = backend
        .read_bytes(path)
        .await
        .map_err(|_| AiFailure::LessonUnreadable)?;
    let mut text = String::from_utf8_lossy(&current).into_owned();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    for event in events {
        text.push_str(&line_of(event)?);
        text.push('\n');
    }
    backend
        .write(path, &text, None)
        .await
        .map_err(|_| AiFailure::JournalFailed)
}

/// Число подряд идущих реплик учителя после последнего ответа пользователя
/// (оборванные не в счёт): по нему считается предел автопродолжений.
pub fn teacher_run(events: &[LessonEvent]) -> usize {
    events
        .iter()
        .rev()
        .take_while(|event| !matches!(event, LessonEvent::User { .. }))
        .filter(|event| {
            matches!(
                event,
                LessonEvent::Teacher {
                    interrupted: false,
                    ..
                }
            )
        })
        .count()
}

/// Сколько раз учитель может продолжить сам после одного ответа пользователя.
pub const MAX_AUTO_CONTINUES: usize = 4;

pub fn current_language(parsed: &ParsedLesson) -> Language {
    parsed
        .events
        .iter()
        .rev()
        .find_map(|event| match event {
            LessonEvent::Language { language, .. } => Some(*language),
            _ => None,
        })
        .unwrap_or(parsed.header.language)
}

pub fn current_state(events: &[LessonEvent]) -> LessonState {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            LessonEvent::State { state, .. } => Some(*state),
            _ => None,
        })
        .unwrap_or_default()
}

fn waiting_of(events: &[LessonEvent], state: LessonState) -> Waiting {
    if state != LessonState::Active {
        return Waiting::Closed;
    }
    let Some(last) = events.iter().rposition(|event| {
        matches!(
            event,
            LessonEvent::User { .. } | LessonEvent::Teacher { .. }
        )
    }) else {
        return Waiting::Start;
    };
    match &events[last] {
        LessonEvent::User { .. } => Waiting::Retry,
        LessonEvent::Teacher {
            interrupted: true, ..
        } => Waiting::Retry,
        LessonEvent::Teacher { question, .. } => {
            // Учитель прочитал заметки и должен продолжить сам, но приложение закрыли
            // раньше. Если предел автопродолжений исчерпан, ход за пользователем.
            let pending_tool = events[last + 1..].iter().any(|event| {
                matches!(
                    event,
                    LessonEvent::NoteRead { by_user: false, .. } | LessonEvent::PlanRejected { .. }
                )
            });
            // Пропуск замера требует продолжения учителя; итог замера — пауза: урок ждёт
            // «Начать обучение» (research.md §7).
            let must_go_on = events[last + 1..]
                .iter()
                .any(|event| matches!(event, LessonEvent::Phase { skipped: true, .. }));
            let probe_done = events[last + 1..]
                .iter()
                .any(|event| matches!(event, LessonEvent::ProbeResult { .. }));
            if !probe_done
                && (must_go_on
                    || (question.is_none()
                        && pending_tool
                        && teacher_run(events) <= MAX_AUTO_CONTINUES))
            {
                Waiting::Retry
            } else {
                Waiting::User
            }
        }
        _ => Waiting::Start,
    }
}

fn last_question(events: &[LessonEvent], waiting: Waiting) -> Option<LessonQuestion> {
    if waiting != Waiting::User {
        return None;
    }
    events.iter().rev().find_map(|event| match event {
        LessonEvent::Teacher {
            question,
            interrupted: false,
            ..
        } => Some(question.clone()),
        _ => None,
    })?
}

/// Сводка по ценам: сумма известна, только если известна цена каждого хода.
fn totals(events: &[LessonEvent]) -> (u64, u64, Option<f64>) {
    let (mut input, mut output, mut cost) = (0u64, 0u64, Some(0.0f64));
    for event in events {
        let (i, o, c) = match event {
            LessonEvent::Teacher {
                input_tokens,
                output_tokens,
                cost_usd,
                ..
            }
            | LessonEvent::Summary {
                input_tokens,
                output_tokens,
                cost_usd,
                ..
            } => (input_tokens, output_tokens, cost_usd),
            _ => continue,
        };
        input += i.unwrap_or(0);
        output += o.unwrap_or(0);
        cost = match (cost, c) {
            (Some(sum), Some(price)) => Some(sum + price),
            _ => None,
        };
    }
    (input, output, cost)
}

pub fn view_of(path: &str, parsed: ParsedLesson) -> LessonView {
    let language = current_language(&parsed);
    let state = current_state(&parsed.events);
    let waiting = waiting_of(&parsed.events, state);
    let (input_tokens, output_tokens, cost_usd) = totals(&parsed.events);
    LessonView {
        path: path.to_owned(),
        language,
        state,
        waiting,
        last_question: last_question(&parsed.events, waiting),
        input_tokens,
        output_tokens,
        cost_usd,
        events_len: parsed.events.len() as u32,
        read_only: parsed.header.format > FORMAT,
        phase: current_phase(&parsed.header, &parsed.events),
        probe_count: probe_count(&parsed.events),
        probe_result: probe_result(&parsed.events),
        plan: plan::plan_view(&parsed.events, None),
        plan_history: plan::plan_history(&parsed.events),
        header: parsed.header,
        events: parsed.events,
    }
}

pub fn summary_of(path: &str, parsed: &ParsedLesson) -> LessonSummary {
    let state = current_state(&parsed.events);
    LessonSummary {
        path: path.to_owned(),
        goal: parsed.header.goal.clone(),
        last_at: parsed
            .events
            .iter()
            .map(LessonEvent::at)
            .max()
            .unwrap_or(parsed.header.created_at),
        steps: parsed
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    LessonEvent::User { .. } | LessonEvent::Teacher { .. }
                )
            })
            .count() as u32,
        state,
        waiting: waiting_of(&parsed.events, state),
    }
}

/// Действующая стадия: итог замера и пропуск переводят в `Teach`, иначе — `start` шапки.
pub fn current_phase(header: &LessonHeader, events: &[LessonEvent]) -> LessonPhase {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            LessonEvent::Phase { phase, .. } => Some(*phase),
            LessonEvent::ProbeResult { .. } => Some(LessonPhase::Teach),
            _ => None,
        })
        .unwrap_or(header.start)
}

/// Вопросы замера: реплики учителя с вопросом, заданные, пока шла стадия `Probe`.
pub fn probe_count(events: &[LessonEvent]) -> u32 {
    events
        .iter()
        .filter(|event| {
            matches!(
                event,
                LessonEvent::Teacher {
                    question: Some(LessonQuestion { part: Some(_), .. }),
                    interrupted: false,
                    ..
                }
            )
        })
        .count() as u32
}

pub fn probe_result(events: &[LessonEvent]) -> Option<ProbeSummary> {
    events.iter().rev().find_map(|event| match event {
        LessonEvent::ProbeResult { parts, start, .. } => Some(ProbeSummary {
            parts: parts.clone(),
            start: start.clone(),
        }),
        _ => None,
    })
}
