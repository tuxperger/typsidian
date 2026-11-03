//! Практика с ИИ (фича 053): сессия — тот же файл урока с `kind: Practice`. Здесь типы
//! событий задач, сбор материала, проверка вызовов инструментов агента практики и вид
//! задач, из которого ядро вырезает решение, пока у задачи нет итога (research.md §3).
//! Файл урока приходит и с синхронизацией, поэтому всё, что читается из него, проходит
//! линейно и с пределами длины, а решение скрывается независимо от `kind` шапки.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use super::file::{lesson_path, read_lesson, Language, LessonEvent, LessonKind};
use crate::ai::access::{check_access_in, folded};
use crate::ai::settings::AiSettings;
use crate::vault::backend::VaultBackend;

/// Пределы длины того, что пишет модель (как пределы вопроса учителя, 051 L8).
pub const STATEMENT_MAX_CHARS: usize = 4_000;
pub const SOLUTION_MAX_CHARS: usize = 4_000;
pub const HINT_MAX_CHARS: usize = 1_500;
pub const REVIEW_MAX_CHARS: usize = 3_000;
/// Название задачи в сводке: первая строка условия.
pub const TITLE_MAX_CHARS: usize = 120;
/// Материал: объём одного урока, всех уроков, их число, описание, путь и цель в записи.
pub const LESSON_MATERIAL_CHARS: usize = 30_000;
pub const MATERIAL_TOTAL_CHARS: usize = 100_000;
pub const MAX_MATERIAL_LESSONS: usize = 10;
pub const DESCRIPTION_MAX_CHARS: usize = 2_000;
pub const MATERIAL_PATH_CHARS: usize = 300;
pub const MATERIAL_GOAL_CHARS: usize = 500;

pub const POSE_PROBLEM: &str = "pose_problem";
pub const GIVE_HINT: &str = "give_hint";
pub const CONCLUDE: &str = "conclude";

fn clip(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Итог задачи.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ProblemOutcome {
    Correct,
    Partial,
    Wrong,
    Unsolved,
}

/// Урок в материале практики: цель и текст, собранный при создании.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LessonMaterial {
    pub path: String,
    pub goal: String,
    pub text: String,
    pub chars: u32,
}

/// Почему урок не попал в материал практики (L1: отказ виден, а не тихий).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum LessonDenyReason {
    /// Папка закрыта для агента (050) или путь идёт через ссылку.
    Denied,
    /// Файл не читается или не разбирается как урок.
    Unreadable,
    /// Это не урок: практика (в её файле решения) или иной вид.
    NotLesson,
    /// Путь не годится для урока.
    BadPath,
}

/// Урок, который не вошёл в материал, и причина.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DeniedLesson {
    pub path: String,
    pub reason: LessonDenyReason,
}

/// Задача в сводке сессии. `solution` есть только у задачи с итогом.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ProblemView {
    pub n: u32,
    pub title: String,
    pub outcome: Option<ProblemOutcome>,
    pub hints: u32,
    /// Номер события `Problem` в `events` вида: к нему прокручивается лента.
    pub anchor: u32,
    pub solution: Option<String>,
}

/// Производное практики для вида урока.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PracticeView {
    pub problems: Vec<ProblemView>,
    /// Номер задачи без итога.
    pub open_problem: Option<u32>,
    /// Разбор последнего итога: «трудности» агент пишет в него (research.md §5).
    pub last_review: Option<String>,
}

/// Сведения об одной задаче, собранные одним проходом по событиям.
struct Info {
    index: usize,
    n: u32,
    outcome: Option<ProblemOutcome>,
    hints: u32,
    /// Перекрыта более поздней задачей с тем же номером: считается закрытой, но без итога
    /// и без раскрытия решения (слияние файла с двух устройств, ревью 053 L2).
    superseded: bool,
}

impl Info {
    fn is_open(&self) -> bool {
        self.outcome.is_none() && !self.superseded
    }
}

/// Проход по событиям: задачи, их итоги и подсказки. Итог засчитывается только задаче с
/// тем же номером, поставленной раньше него и ещё без итога.
struct Scan {
    problems: Vec<Info>,
    last_review: Option<String>,
    /// Для каждого события `Summary` — была ли на тот момент задача без итога.
    summary_open: Vec<bool>,
}

fn scan(events: &[LessonEvent]) -> Scan {
    let mut problems: Vec<Info> = Vec::new();
    let mut latest: HashMap<u32, usize> = HashMap::new();
    let mut open = 0usize;
    let mut last_review = None;
    let mut summary_open = vec![false; events.len()];
    for (index, event) in events.iter().enumerate() {
        match event {
            LessonEvent::Problem { n, .. } => {
                if let Some(&p) = latest.get(n) {
                    if problems[p].is_open() {
                        problems[p].superseded = true;
                        open -= 1;
                    }
                }
                latest.insert(*n, problems.len());
                problems.push(Info {
                    index,
                    n: *n,
                    outcome: None,
                    hints: 0,
                    superseded: false,
                });
                open += 1;
            }
            LessonEvent::Hint { n, .. } => {
                if let Some(&p) = latest.get(n) {
                    problems[p].hints = problems[p].hints.saturating_add(1);
                }
            }
            LessonEvent::Verdict {
                n, outcome, review, ..
            } => {
                if let Some(&p) = latest.get(n) {
                    if problems[p].is_open() {
                        problems[p].outcome = Some(*outcome);
                        open -= 1;
                        last_review = Some(review.clone());
                    }
                }
            }
            LessonEvent::Summary { .. } => summary_open[index] = open > 0,
            _ => {}
        }
    }
    Scan {
        problems,
        last_review,
        summary_open,
    }
}

/// Вид события для интерфейса: решение задачи без итога вырезано, а изложение, написанное
/// при открытой задаче, пусто (модель могла вписать в него решение). Файл по-прежнему
/// хранит всё.
pub fn redact(events: Vec<LessonEvent>) -> Vec<LessonEvent> {
    let found = scan(&events);
    if found.problems.is_empty() {
        return events;
    }
    let mut concluded: HashMap<usize, bool> = HashMap::new();
    for info in &found.problems {
        concluded.insert(info.index, info.outcome.is_some());
    }
    events
        .into_iter()
        .enumerate()
        .map(|(index, event)| match event {
            LessonEvent::Problem {
                at,
                n,
                statement,
                solution,
            } => LessonEvent::Problem {
                at,
                n,
                statement,
                solution: solution.filter(|_| concluded.get(&index).copied().unwrap_or(false)),
            },
            LessonEvent::Summary {
                at,
                covers,
                model,
                input_tokens,
                output_tokens,
                cost_usd,
                text,
            } => LessonEvent::Summary {
                at,
                text: if found.summary_open[index] {
                    String::new()
                } else {
                    text
                },
                covers,
                model,
                input_tokens,
                output_tokens,
                cost_usd,
            },
            other => other,
        })
        .collect()
}

/// Сводка задач практики; решение — только у задач с итогом.
pub fn practice_view(events: &[LessonEvent]) -> PracticeView {
    let found = scan(events);
    let problems: Vec<ProblemView> = found
        .problems
        .iter()
        .map(|info| {
            let (title, solution) = match &events[info.index] {
                LessonEvent::Problem {
                    statement,
                    solution,
                    ..
                } => (
                    clip(
                        statement
                            .lines()
                            .find(|line| !line.trim().is_empty())
                            .unwrap_or_default()
                            .trim(),
                        TITLE_MAX_CHARS,
                    ),
                    solution.clone().filter(|_| info.outcome.is_some()),
                ),
                _ => (String::new(), None),
            };
            ProblemView {
                n: info.n,
                title,
                outcome: info.outcome,
                hints: info.hints,
                anchor: info.index as u32,
                solution,
            }
        })
        .collect();
    let open_problem = found
        .problems
        .iter()
        .rev()
        .find(|info| info.is_open())
        .map(|info| info.n);
    PracticeView {
        problems,
        open_problem,
        last_review: found.last_review,
    }
}

/// Задача без итога: последняя из поставленных, у которой итога ещё нет.
pub struct OpenProblem<'a> {
    pub index: usize,
    pub n: u32,
    pub statement: &'a str,
    pub solution: Option<&'a str>,
    pub hints: u32,
}

pub fn open_problem(events: &[LessonEvent]) -> Option<OpenProblem<'_>> {
    let found = scan(events);
    let info = found.problems.iter().rev().find(|info| info.is_open())?;
    match &events[info.index] {
        LessonEvent::Problem {
            statement,
            solution,
            ..
        } => Some(OpenProblem {
            index: info.index,
            n: info.n,
            statement,
            solution: solution.as_deref(),
            hints: info.hints,
        }),
        _ => None,
    }
}

/// Ждёт ли практика «Следующую задачу»: последнее из событий задач — итог.
pub fn awaiting_next(events: &[LessonEvent]) -> bool {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            LessonEvent::Verdict { .. } => Some(true),
            LessonEvent::NextProblem { .. } | LessonEvent::Problem { .. } => Some(false),
            _ => None,
        })
        .unwrap_or(false)
}

/// Последнее действие пользователя после события `from`: ответ, «Сдаюсь» или ничего.
fn last_action_after(events: &[LessonEvent], from: usize) -> Option<&LessonEvent> {
    events[from + 1..]
        .iter()
        .rev()
        .find(|event| matches!(event, LessonEvent::User { .. } | LessonEvent::GiveUp { .. }))
}

/// Пользователь сдался, а итога нет: ядро само вынесет `Unsolved`, если модель не вынесла.
pub fn give_up_pending(events: &[LessonEvent]) -> Option<u32> {
    let open = open_problem(events)?;
    match last_action_after(events, open.index) {
        Some(LessonEvent::GiveUp { .. }) => Some(open.n),
        _ => None,
    }
}

/// Вызов инструмента агента практики после разбора аргументов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PracticeCall {
    Pose {
        statement: String,
        solution: Option<String>,
    },
    Hint {
        text: String,
    },
    Conclude {
        outcome: Option<ProblemOutcome>,
        review: String,
    },
}

fn text_arg(input: &Value, key: &str, max_chars: usize) -> String {
    clip(
        input[key].as_str().map(str::trim).unwrap_or_default(),
        max_chars,
    )
}

/// Разбор аргументов инструмента `name`; пределы длины — здесь.
pub fn parse_call(name: &str, input: &Value) -> Option<PracticeCall> {
    match name {
        POSE_PROBLEM => {
            let solution = text_arg(input, "solution", SOLUTION_MAX_CHARS);
            Some(PracticeCall::Pose {
                statement: text_arg(input, "statement", STATEMENT_MAX_CHARS),
                solution: (!solution.is_empty()).then_some(solution),
            })
        }
        GIVE_HINT => Some(PracticeCall::Hint {
            text: text_arg(input, "text", HINT_MAX_CHARS),
        }),
        CONCLUDE => Some(PracticeCall::Conclude {
            outcome: match input["outcome"]
                .as_str()
                .map(|s| s.trim().to_lowercase())
                .as_deref()
            {
                Some("correct") => Some(ProblemOutcome::Correct),
                Some("partial") => Some(ProblemOutcome::Partial),
                Some("wrong") => Some(ProblemOutcome::Wrong),
                Some("unsolved") => Some(ProblemOutcome::Unsolved),
                _ => None,
            },
            review: text_arg(input, "review", REVIEW_MAX_CHARS),
        }),
        _ => None,
    }
}

/// Отказ ядра инструменту практики; причина уходит модели результатом инструмента.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PracticeError {
    NoStatement,
    NoSolution,
    ProblemOpen(u32),
    NextRequired,
    NoOpenProblem,
    NotAnswered,
    NoText,
    BadOutcome,
    /// Подсказка содержит решение открытой задачи.
    HintLeaksSolution,
}

/// Причина отказа на любом из языков: по ней считаются повторные отказы «без решения».
fn no_solution_reason(language: Language) -> String {
    super::prompt::practice_error(language, &PracticeError::NoSolution)
}

fn is_no_solution(reason: &str) -> bool {
    reason == no_solution_reason(Language::Ru) || reason == no_solution_reason(Language::En)
}

/// Сколько раз подряд поставленная задача отклонялась за отсутствие решения.
fn no_solution_streak(events: &[LessonEvent]) -> usize {
    events
        .iter()
        .rev()
        .take_while(|event| {
            !matches!(
                event,
                LessonEvent::Problem { .. }
                    | LessonEvent::Verdict { .. }
                    | LessonEvent::NextProblem { .. }
                    | LessonEvent::User { .. }
                    | LessonEvent::HintRequest { .. }
                    | LessonEvent::GiveUp { .. }
            )
        })
        .filter(|event| {
            matches!(event, LessonEvent::PlanRejected { tool, reason, .. }
                if tool == POSE_PROBLEM && is_no_solution(reason))
        })
        .count()
}

/// Проверка вызова против файла урока и событие, которое он порождает. Задача без
/// решения отклоняется; после второй неудачи подряд она принимается без решения
/// (Edge Case: «без проверяемого решения»).
pub fn resolve(
    events: &[LessonEvent],
    call: PracticeCall,
    now: i64,
) -> Result<LessonEvent, PracticeError> {
    let open = open_problem(events);
    match call {
        PracticeCall::Pose {
            statement,
            solution,
        } => {
            if statement.is_empty() {
                return Err(PracticeError::NoStatement);
            }
            if let Some(open) = &open {
                return Err(PracticeError::ProblemOpen(open.n));
            }
            if awaiting_next(events) {
                return Err(PracticeError::NextRequired);
            }
            if solution.is_none() && no_solution_streak(events) == 0 {
                return Err(PracticeError::NoSolution);
            }
            let n = scan(events)
                .problems
                .iter()
                .map(|info| info.n)
                .max()
                .map_or(1, |n| n.saturating_add(1));
            Ok(LessonEvent::Problem {
                at: now,
                n,
                statement,
                solution,
            })
        }
        PracticeCall::Hint { text } => {
            let open = open.ok_or(PracticeError::NoOpenProblem)?;
            if text.is_empty() {
                return Err(PracticeError::NoText);
            }
            if open
                .solution
                .is_some_and(|solution| contains_solution(&text, solution))
            {
                return Err(PracticeError::HintLeaksSolution);
            }
            Ok(LessonEvent::Hint {
                at: now,
                n: open.n,
                text,
            })
        }
        PracticeCall::Conclude { outcome, review } => {
            let open = open.ok_or(PracticeError::NoOpenProblem)?;
            let action = last_action_after(events, open.index).ok_or(PracticeError::NotAnswered)?;
            let outcome = outcome.ok_or(PracticeError::BadOutcome)?;
            // Сдавшемуся итог один: «не решено».
            let outcome = if matches!(action, LessonEvent::GiveUp { .. }) {
                ProblemOutcome::Unsolved
            } else {
                outcome
            };
            Ok(LessonEvent::Verdict {
                at: now,
                n: open.n,
                outcome,
                review,
            })
        }
    }
}

/// Решения короче этого в подсказке не ищутся: короткая строка встречается случайно.
pub const LEAK_MIN_CHARS: usize = 8;

/// Вид текста для сравнения: без регистра и без пробелов.
fn squeezed(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Содержит ли подсказка решение (нормализованная подстрока, решение не короче
/// `LEAK_MIN_CHARS`).
fn contains_solution(text: &str, solution: &str) -> bool {
    let solution = squeezed(solution);
    solution.chars().count() >= LEAK_MIN_CHARS && squeezed(text).contains(&solution)
}

/// Итог «не решено» за модель: пользователь сдался, а она итога не вынесла.
pub fn forced_verdict(events: &[LessonEvent], review: &str, now: i64) -> Option<LessonEvent> {
    let n = give_up_pending(events)?;
    Some(LessonEvent::Verdict {
        at: now,
        n,
        outcome: ProblemOutcome::Unsolved,
        review: clip(review.trim(), REVIEW_MAX_CHARS),
    })
}

// ── Материал ────────────────────────────────────────────────────────────────

/// Текст урока для материала: последнее изложение, иначе объяснения учителя. Практика как
/// материал не берётся: в её файле лежат решения.
fn lesson_text(events: &[LessonEvent]) -> String {
    if let Some(text) = events.iter().rev().find_map(|event| match event {
        LessonEvent::Summary { text, .. } if !text.trim().is_empty() => Some(text.trim()),
        _ => None,
    }) {
        return clip(text, LESSON_MATERIAL_CHARS);
    }
    let mut out = String::new();
    let mut count = 0usize;
    for event in events {
        if let LessonEvent::Teacher {
            text,
            interrupted: false,
            ..
        } = event
        {
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push_str("\n\n");
                count += 2;
            }
            out.push_str(text);
            count += text.chars().count();
            if count >= LESSON_MATERIAL_CHARS {
                break;
            }
        }
    }
    clip(&out, LESSON_MATERIAL_CHARS)
}

/// Собирает материал из уроков: путь в чистой форме, вид `Lesson`, проверка доступа 050
/// с пределами, как у заметок. Закрытый, нечитаемый урок и практика пропускаются, но
/// отказ записывается в `denied` (виден в ленте); повтор пути (в свёрнутой форме, как в
/// `check_access`) молча пропускается. Считаются попытки: список длиннее предела
/// обрезается до обработки, объём ограничен.
pub async fn collect_lessons<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    paths: &[String],
) -> (Vec<LessonMaterial>, Vec<DeniedLesson>) {
    let mut out: Vec<LessonMaterial> = Vec::new();
    let mut denied: Vec<DeniedLesson> = Vec::new();
    let mut seen: Vec<Vec<String>> = Vec::new();
    let mut room = MATERIAL_TOTAL_CHARS;
    for path in paths.iter().take(MAX_MATERIAL_LESSONS) {
        let deny = |reason| DeniedLesson {
            path: clip(path, MATERIAL_PATH_CHARS),
            reason,
        };
        let Ok(clean) = lesson_path(path) else {
            denied.push(deny(LessonDenyReason::BadPath));
            continue;
        };
        if let Some(form) = folded(Path::new(&clean)) {
            if seen.contains(&form) {
                continue;
            }
            seen.push(form);
        }
        if !check_access_in(backend, &clean, settings).await {
            denied.push(deny(LessonDenyReason::Denied));
            continue;
        }
        if room == 0 {
            continue;
        }
        let Ok(parsed) = read_lesson(backend, &clean).await else {
            denied.push(deny(LessonDenyReason::Unreadable));
            continue;
        };
        if parsed.header.kind != LessonKind::Lesson {
            denied.push(deny(LessonDenyReason::NotLesson));
            continue;
        }
        let text: String = lesson_text(&parsed.events).chars().take(room).collect();
        room -= text.chars().count();
        out.push(LessonMaterial {
            path: clip(&clean, MATERIAL_PATH_CHARS),
            goal: clip(&parsed.header.goal, MATERIAL_GOAL_CHARS),
            chars: text.chars().count() as u32,
            text,
        });
    }
    (out, denied)
}

impl PracticeCall {
    /// Имя инструмента, вызвавшего это.
    pub fn tool(&self) -> &'static str {
        match self {
            PracticeCall::Pose { .. } => POSE_PROBLEM,
            PracticeCall::Hint { .. } => GIVE_HINT,
            PracticeCall::Conclude { .. } => CONCLUDE,
        }
    }
}
