//! Ход учителя в три шага, как пробный запрос 050 (фича 051, research.md §2, §5, §6):
//! [`begin_turn`] пишет ответ пользователя в файл и запрос в журнал,
//! [`send_turn`] ходит в сеть, [`finish_turn`] дописывает реплику учителя. Сеть
//! вынесена отдельно: на её время не держатся ни блокировка vault, ни очередь вызовов
//! wasm. Решение «продолжать самому или ждать пользователя» принимает ядро ([`Next`]).

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::file::{
    append_events, current_language, current_phase, lesson_path, probe_count, probe_result,
    read_lesson, teacher_run, view_of, Confidence, Language, LessonEvent, LessonKind, LessonPhase,
    LessonQuestion, LessonView, NoteOutcome, ParsedLesson, ProbePart, ProbeState, FORMAT,
    GOAL_MAX_CHARS, MAX_AUTO_CONTINUES,
};
use super::partial;
use super::plan::{self, PlanError, PlanProgressArgs, PlanSet};
use super::practice::{self, PracticeCall};
use super::prompt::{self, NoteResult, ToolSet, PROBE_QUESTIONS_LIMIT};
use super::stream::{StreamParser, ToolCall, TurnReply};
use super::MAX_ATTACHED_NOTES;
use crate::ai::access::{check_access_in, normalize_folder};
use crate::ai::cancel::cancellable;
use crate::ai::client::Connection;
use crate::ai::endpoint::base_url;
use crate::ai::journal::{record_done, record_sent};
use crate::ai::knowledge::{agent_results, graph_readable, visible_material, KnowledgeMaterial};
use crate::ai::price::estimate_usd;
use crate::ai::protocol::{authorize, check_status};
use crate::ai::settings::{load_settings, AiProvider, AiSettings};
use crate::ai::transport::{AiTransport, HttpMethod, HttpRequest};
use crate::ai::AiFailure;
use crate::vault::backend::{ReadError, VaultBackend};
use crate::vault::{classify, FileKind};

/// Текст заметки, который уходит модели, — не длиннее (research.md §12).
pub const NOTE_LIMIT_CHARS: usize = 100_000;
/// Сколько вызовов `read_note` за один ход исполняется.
const MAX_NOTE_READS_PER_TURN: usize = 3;
/// Сколько вызовов `read_knowledge` за один ход исполняется (фича 054).
const MAX_KNOWLEDGE_READS_PER_TURN: usize = 2;
/// Суммарный объём прикреплённых заметок в запросе, символов.
pub const ATTACHED_TOTAL_CHARS: usize = 300_000;
/// Вопрос учителя: число вариантов и пределы длины (L8).
const MAX_CHOICES: usize = 5;
const QUESTION_MAX_CHARS: usize = 1_000;
const CHOICE_MAX_CHARS: usize = 300;
/// Материал практики ищется среди первых событий файла: он пишется при создании.
const MATERIAL_SCAN: usize = 8;
const MAX_TOKENS_TEACH: u32 = 4096;
const MAX_TOKENS_COMPACT: u32 = 2048;

/// Ответ пользователя на вопрос учителя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct UserAnswer {
    pub text: String,
    #[serde(default)]
    pub choice: Option<u32>,
    #[serde(default)]
    pub confidence: Option<Confidence>,
    #[serde(default)]
    pub thinking: Option<String>,
    /// «Не знаю» (кнопка замера): ответ-сведение; текст может быть пустым.
    #[serde(default)]
    pub dont_know: bool,
    /// Что делает пользователь (053); по умолчанию — отвечает, как в уроках 051 и 052.
    #[serde(default)]
    pub action: AnswerAction,
}

/// Действие пользователя в сессии. Кроме `Answer` действуют только в практике.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum AnswerAction {
    #[default]
    Answer,
    HintRequest,
    GiveUp,
    NextProblem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum TurnKind {
    /// Обычный ход учителя.
    Teach,
    /// Сжатие переписки: модель излагает пройденное, результат — событие `Summary`.
    Compact,
}

/// Ход, записанный в журнал. Ключа не содержит.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PreparedTurn {
    /// Идентификатор записи журнала: `<request_id>#<число событий>`, уникален в цикле.
    pub id: String,
    /// Идентификатор, по которому интерфейс отменяет ход и опрашивает прогресс.
    pub request_id: String,
    pub path: String,
    pub provider: AiProvider,
    pub model: String,
    pub kind: TurnKind,
    /// Число событий в файле на момент начала хода (с ответом пользователя).
    pub events_len: u32,
    pub request: HttpRequest,
}

/// Что делать после хода.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Next {
    /// Начать следующий ход сразу, без ответа пользователя (`answer: null`).
    Continue,
    /// Ждать пользователя.
    WaitUser,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TurnDone {
    pub view: LessonView,
    pub next: Next,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    User,
    Assistant,
}

struct Message {
    role: Role,
    text: String,
}

struct Context {
    system: String,
    messages: Vec<Message>,
    /// Реплики после последнего изложения: сжимать, когда сжимать нечего, незачем.
    steps_since_summary: usize,
    /// Инструменты стадии урока.
    tools: ToolSet,
}

/// Читает заметку для модели: проверка доступа 050 одна для прикрепления и
/// инструмента (research.md §12). Читаются только заметки (`.typ`): иначе модель
/// получила бы любой файл vault, включая посторонние, вложения и чужие уроки. Предел
/// размера проверяется по метаданным до чтения (ревью безопасности 051, M3); слишком
/// большая и неподходящая заметка отвечает так же, как закрытая, — «недоступна».
async fn read_note_text<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    path: &str,
) -> (NoteOutcome, Option<String>) {
    let Some(path) = normalize_folder(Path::new(path)) else {
        return (NoteOutcome::Denied, None);
    };
    if classify(Path::new(&path)) != FileKind::Note
        || !check_access_in(backend, &path, settings).await
    {
        return (NoteOutcome::Denied, None);
    }
    let bytes = match backend.read_bytes(Path::new(&path)).await {
        Ok(bytes) => bytes,
        Err(ReadError::TooLarge) => return (NoteOutcome::Denied, None),
        Err(_) => return (NoteOutcome::Missing, None),
    };
    let text = String::from_utf8_lossy(&bytes);
    let total = text.chars().count();
    let truncated = total > NOTE_LIMIT_CHARS;
    let text: String = if truncated {
        text.chars().take(NOTE_LIMIT_CHARS).collect()
    } else {
        text.into_owned()
    };
    (
        NoteOutcome::Sent {
            chars: total.min(NOTE_LIMIT_CHARS) as u32,
            truncated,
        },
        Some(text),
    )
}

fn push(messages: &mut Vec<Message>, role: Role, text: String) {
    if text.trim().is_empty() {
        return;
    }
    match messages.last_mut() {
        Some(last) if last.role == role => {
            last.text.push_str("\n\n");
            last.text.push_str(&text);
        }
        _ => messages.push(Message { role, text }),
    }
}

fn user_text(
    language: Language,
    text: &str,
    choice: Option<u32>,
    confidence: Option<Confidence>,
    thinking: Option<&str>,
    dont_know: bool,
    previous: Option<&LessonQuestion>,
) -> String {
    let mut out = text.trim().to_owned();
    if dont_know {
        let label = prompt::dont_know_label(language);
        out = if out.is_empty() {
            label.to_owned()
        } else {
            format!("{label}\n{out}")
        };
    }
    if out.is_empty() {
        if let (Some(index), Some(choices)) = (choice, previous.and_then(|q| q.choices.as_ref())) {
            out = choices.get(index as usize).cloned().unwrap_or_default();
        }
    }
    if let Some(confidence) = confidence {
        out.push_str(&format!(
            "\n({})",
            prompt::confidence_label(language, confidence)
        ));
    }
    if let Some(thinking) = thinking.map(str::trim).filter(|t| !t.is_empty()) {
        out.push_str(&format!(
            "\n{}: {thinking}",
            prompt::thinking_label(language)
        ));
    }
    out
}

fn teacher_text(language: Language, text: &str, question: Option<&LessonQuestion>) -> String {
    let mut out = text.trim().to_owned();
    if let Some(question) = question {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        match &question.part {
            Some(part) => out.push_str(&format!(
                "{} ({} {part}): {}",
                prompt::question_label(language),
                prompt::part_label(language),
                question.question
            )),
            None => out.push_str(&format!(
                "{}: {}",
                prompt::question_label(language),
                question.question
            )),
        }
        for (index, choice) in question.choices.iter().flatten().enumerate() {
            out.push_str(&format!("\n{}) {choice}", index + 1));
        }
    }
    out
}

/// Цель урока для инструкций: файл приходит и с синхронизацией, поэтому длина ограничена.
fn goal_of(parsed: &ParsedLesson) -> String {
    parsed.header.goal.chars().take(GOAL_MAX_CHARS).collect()
}

/// Инструменты, которые действуют в следующем ходе урока.
fn tool_set_of(parsed: &ParsedLesson) -> ToolSet {
    if parsed.header.kind == LessonKind::Practice {
        return ToolSet::Practice;
    }
    if current_phase(&parsed.header, &parsed.events) == LessonPhase::Teach {
        ToolSet::Teach
    } else if probe_count(&parsed.events) >= PROBE_QUESTIONS_LIMIT {
        ToolSet::ProbeEnd
    } else {
        ToolSet::Probe
    }
}

/// Знания графа, выбранные пользователем при создании сессии (событие `Material`, фича 054).
/// Это снимок, но запрет папки графа действует и на него: закрытая сейчас папка — блока нет.
async fn push_knowledge<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    language: Language,
    parsed: &ParsedLesson,
    system: &mut String,
) {
    let material: Option<&KnowledgeMaterial> =
        parsed
            .events
            .iter()
            .take(MATERIAL_SCAN)
            .find_map(|event| match event {
                LessonEvent::Material { knowledge, .. } => knowledge.as_ref(),
                _ => None,
            });
    if let Some(material) = material {
        if graph_readable(backend, settings).await {
            // Утверждения, у которых источник закрыли после выбора, снимком не уходят (M1).
            if let Some(visible) = visible_material(backend, settings, material).await {
                system.push_str("\n\n");
                system.push_str(&prompt::knowledge_block(language, &visible));
            }
        }
    }
}

/// Инструкции стадии: замер (с требованием завершить после предела вопросов) или обучение
/// с правилами плана, итогом замера (или пометкой о пропуске) и текущим планом. Всё это
/// берётся из событий файла, поэтому переживает перезапуск и сжатие переписки.
async fn stage_system<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    language: Language,
    parsed: &ParsedLesson,
) -> (String, ToolSet) {
    let goal = goal_of(parsed);
    let tools = tool_set_of(parsed);
    if tools == ToolSet::Practice {
        return (
            practice_system(backend, settings, language, &goal, parsed).await,
            tools,
        );
    }
    if tools != ToolSet::Teach {
        let mut system = prompt::system_probe(language, &goal, tools == ToolSet::ProbeEnd);
        push_knowledge(backend, settings, language, parsed, &mut system).await;
        return (system, tools);
    }
    let mut system = prompt::system(language, &goal);
    push_knowledge(backend, settings, language, parsed, &mut system).await;
    system.push_str("\n\n");
    system.push_str(prompt::plan_rules(language));
    if let Some(summary) = probe_result(&parsed.events) {
        system.push_str("\n\n");
        system.push_str(&prompt::probe_block(language, &summary));
    } else if parsed
        .events
        .iter()
        .any(|event| matches!(event, LessonEvent::Phase { skipped: true, .. }))
    {
        system.push_str("\n\n");
        system.push_str(prompt::probe_skipped_block(language));
    }
    if let Some(view) = plan::plan_view(&parsed.events, None) {
        system.push_str("\n\n");
        system.push_str(&prompt::plan_block(language, &view));
    }
    (system, tools)
}

/// Инструкции практики: материал, собранный при создании, и открытая задача с решением
/// (его знает только модель). Берётся из событий файла, поэтому переживает перезапуск и
/// сжатие переписки.
///
/// Материал урока — снимок, но запрет папки действует и на него (ревью 053 M1): урок с
/// путём, закрытым `check_access`, из инструкций выкидывается; исчезнувший урок остаётся
/// снимком.
async fn practice_system<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    language: Language,
    goal: &str,
    parsed: &ParsedLesson,
) -> String {
    let mut system = prompt::system_practice(language, goal);
    for event in parsed.events.iter().take(MATERIAL_SCAN) {
        if let LessonEvent::Material {
            lessons,
            description,
            ..
        } = event
        {
            let mut open = Vec::new();
            for lesson in lessons.iter().take(practice::MAX_MATERIAL_LESSONS) {
                if check_access_in(backend, &lesson.path, settings).await {
                    open.push(lesson.clone());
                }
            }
            system.push_str("\n\n");
            system.push_str(&prompt::material_block(
                language,
                &open,
                description.as_deref(),
            ));
            break;
        }
    }
    push_knowledge(backend, settings, language, parsed, &mut system).await;
    if let Some(open) = practice::open_problem(&parsed.events) {
        system.push_str("\n\n");
        system.push_str(&prompt::open_problem_block(
            language,
            open.n,
            open.statement,
            open.solution,
            open.hints,
        ));
    }
    system
}

/// Собирает запрос из файла урока: инструкции с прикреплёнными заметками, изложение (если
/// было) и события после него. Оборванные реплики модели не показываются.
async fn build_context<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    parsed: &ParsedLesson,
) -> Context {
    let language = current_language(parsed);
    let (mut system, tools) = stage_system(backend, settings, language, parsed).await;

    // Прикреплённые заметки читаются заново на каждом ходу: так учитель помнит их весь
    // урок, а запрещённая позже папка перестаёт отдавать текст. Прикреплением считаются
    // только начальные события, до первой реплики: файл урока приходит и с
    // синхронизацией, и событие `by_user` из середины переписки не должно подставить
    // модели заметку от имени пользователя (ревью безопасности 051, M4). Заметок не
    // больше предела, объём в сумме тоже ограничен.
    let first_turn = parsed
        .events
        .iter()
        .position(|event| {
            matches!(
                event,
                LessonEvent::User { .. } | LessonEvent::Teacher { .. }
            )
        })
        .unwrap_or(parsed.events.len());
    let mut attached: Vec<String> = Vec::new();
    let mut block = String::new();
    let mut room = ATTACHED_TOTAL_CHARS;
    for event in &parsed.events[..first_turn] {
        if let LessonEvent::NoteRead {
            path,
            by_user: true,
            outcome: NoteOutcome::Sent { .. },
            ..
        } = event
        {
            let Some(path) = normalize_folder(Path::new(path)) else {
                continue;
            };
            if attached.contains(&path) {
                continue;
            }
            if attached.len() >= MAX_ATTACHED_NOTES || room == 0 {
                break;
            }
            attached.push(path.clone());
            if let (_, Some(text)) = read_note_text(backend, settings, &path).await {
                let text: String = text.chars().take(room).collect();
                room -= text.chars().count();
                block.push_str(&format!("\n\n### {path}\n{text}"));
            }
        }
    }
    if !block.is_empty() {
        system.push_str(&format!(
            "\n\n## {}{block}",
            prompt::attached_heading(language)
        ));
    }

    let summary = parsed
        .events
        .iter()
        .rposition(|event| matches!(event, LessonEvent::Summary { .. }));
    let start = match summary {
        Some(index) => match &parsed.events[index] {
            LessonEvent::Summary { covers, .. } => (*covers as usize).min(parsed.events.len()),
            _ => 0,
        },
        None => 0,
    };
    let last_teacher = parsed
        .events
        .iter()
        .rposition(|event| matches!(event, LessonEvent::Teacher { .. }));

    let mut messages: Vec<Message> = Vec::new();
    if let Some(index) = summary {
        if let LessonEvent::Summary { text, .. } = &parsed.events[index] {
            push(
                &mut messages,
                Role::User,
                format!("{}:\n{text}", prompt::summary_heading(language)),
            );
        }
    }
    let mut previous_question: Option<&LessonQuestion> = None;
    let mut steps = 0usize;
    for (index, event) in parsed.events.iter().enumerate().skip(start) {
        match event {
            LessonEvent::User {
                text,
                choice,
                confidence,
                thinking,
                dont_know,
                ..
            } => {
                steps += 1;
                push(
                    &mut messages,
                    Role::User,
                    user_text(
                        language,
                        text,
                        *choice,
                        *confidence,
                        thinking.as_deref(),
                        *dont_know,
                        previous_question,
                    ),
                );
            }
            LessonEvent::Teacher {
                text,
                question,
                interrupted: false,
                ..
            } => {
                steps += 1;
                previous_question = question.as_ref();
                push(
                    &mut messages,
                    Role::Assistant,
                    teacher_text(language, text, question.as_ref()),
                );
            }
            // Результат инструмента уходит модели один раз — в запросе сразу за ходом, где
            // она его попросила. Дальше о нём помнит её собственный ответ.
            LessonEvent::NoteRead {
                path,
                by_user: false,
                outcome,
                ..
            } if last_teacher.is_none_or(|teacher| index > teacher) => {
                let text = match outcome {
                    NoteOutcome::Sent { .. } => match read_note_text(backend, settings, path).await
                    {
                        (_, Some(text)) => prompt::note_result(
                            language,
                            path,
                            NoteResult::Text {
                                text: &text,
                                truncated: matches!(
                                    outcome,
                                    NoteOutcome::Sent {
                                        truncated: true,
                                        ..
                                    }
                                ),
                            },
                        ),
                        (NoteOutcome::Denied, None) => {
                            prompt::note_result(language, path, NoteResult::Denied)
                        }
                        _ => prompt::note_result(language, path, NoteResult::Missing),
                    },
                    NoteOutcome::Denied => prompt::note_result(language, path, NoteResult::Denied),
                    NoteOutcome::Missing => {
                        prompt::note_result(language, path, NoteResult::Missing)
                    }
                };
                push(&mut messages, Role::User, text);
            }
            // Чтение графа знаний: результат собирается заново по запросу (фича 054).
            LessonEvent::KnowledgeRead { query, .. }
                if last_teacher.is_none_or(|teacher| index > teacher) =>
            {
                let (_, found) = agent_results(backend, settings, query).await;
                push(
                    &mut messages,
                    Role::User,
                    prompt::knowledge_result(language, query, found.as_ref()),
                );
            }
            // Отказ инструменту плана и принятый итог замера уходят модели так же, как
            // результат чтения заметки: один раз, следующим запросом.
            LessonEvent::PlanRejected { tool, reason, .. }
                if last_teacher.is_none_or(|teacher| index > teacher) =>
            {
                push(
                    &mut messages,
                    Role::User,
                    prompt::tool_rejected(language, tool, reason),
                );
            }
            LessonEvent::ProbeResult { .. }
                if last_teacher.is_none_or(|teacher| index > teacher) =>
            {
                push(
                    &mut messages,
                    Role::User,
                    prompt::probe_done_result(language).to_owned(),
                );
            }
            LessonEvent::Problem { n, statement, .. } => push(
                &mut messages,
                Role::Assistant,
                prompt::problem_text(language, *n, statement),
            ),
            LessonEvent::Hint { text, .. } => push(
                &mut messages,
                Role::Assistant,
                prompt::hint_text(language, text),
            ),
            LessonEvent::Verdict {
                n, outcome, review, ..
            } => push(
                &mut messages,
                Role::Assistant,
                prompt::verdict_text(language, *n, *outcome, review),
            ),
            LessonEvent::HintRequest { .. } => push(
                &mut messages,
                Role::User,
                prompt::hint_request_text(language).to_owned(),
            ),
            LessonEvent::GiveUp { .. } => push(
                &mut messages,
                Role::User,
                prompt::give_up_text(language).to_owned(),
            ),
            LessonEvent::NextProblem { .. } => push(
                &mut messages,
                Role::User,
                prompt::next_problem_text(language).to_owned(),
            ),
            _ => {}
        }
    }

    // Протоколы требуют, чтобы разговор начинался и кончался репликой пользователя.
    if messages
        .first()
        .is_none_or(|first| first.role == Role::Assistant)
    {
        messages.insert(
            0,
            Message {
                role: Role::User,
                text: if tools == ToolSet::Practice {
                    prompt::practice_start_message(language).to_owned()
                } else {
                    prompt::start_message(language).to_owned()
                },
            },
        );
    }
    if messages
        .last()
        .is_some_and(|last| last.role == Role::Assistant)
    {
        messages.push(Message {
            role: Role::User,
            text: prompt::continue_message(language).to_owned(),
        });
    }
    Context {
        system,
        messages,
        steps_since_summary: steps,
        tools,
    }
}

/// Оценка объёма в «единицах» настройки `context_budget`: символы / 4.
fn estimated_size(context: &Context) -> usize {
    let chars = context.system.chars().count()
        + context
            .messages
            .iter()
            .map(|message| message.text.chars().count())
            .sum::<usize>();
    chars / 4
}

#[allow(clippy::too_many_arguments)]
fn build_request(
    provider: AiProvider,
    base: &str,
    model: &str,
    system: &str,
    messages: &[Message],
    tools: Option<ToolSet>,
    knowledge: bool,
    max_tokens: u32,
) -> HttpRequest {
    let role = |role: Role| match role {
        Role::User => "user",
        Role::Assistant => "assistant",
    };
    let list: Vec<Value> = messages
        .iter()
        .map(|message| json!({"role": role(message.role), "content": message.text}))
        .collect();
    let (path, body) = match provider {
        AiProvider::Anthropic => {
            let mut body = json!({
                "model": model,
                "max_tokens": max_tokens,
                "stream": true,
                "system": system,
                "messages": list,
            });
            if let Some(set) = tools {
                body["tools"] = prompt::tools_with(provider, set, knowledge);
            }
            ("messages", body)
        }
        AiProvider::OpenAi | AiProvider::Custom => {
            let mut all = vec![json!({"role": "system", "content": system})];
            all.extend(list);
            let mut body = json!({
                "model": model,
                "stream": true,
                "stream_options": {"include_usage": true},
                "messages": all,
            });
            if let Some(set) = tools {
                body["tools"] = prompt::tools_with(provider, set, knowledge);
            }
            ("chat/completions", body)
        }
    };
    HttpRequest {
        method: HttpMethod::Post,
        url: format!("{base}/{path}"),
        headers: vec![("content-type".to_owned(), "application/json".to_owned())],
        body: Some(body.to_string()),
    }
}

/// Событие действия пользователя в практике. Подсказка и «Сдаюсь» нужны открытой задаче,
/// «Следующая задача» — после итога; иначе вид интерфейса устарел (`LessonChanged`), и в
/// файл ничего не пишется.
fn action_event(
    parsed: &ParsedLesson,
    action: AnswerAction,
    now: i64,
) -> Result<LessonEvent, AiFailure> {
    if parsed.header.kind != LessonKind::Practice {
        return Err(AiFailure::LessonChanged);
    }
    let open = practice::open_problem(&parsed.events).map(|open| open.n);
    match (action, open) {
        (AnswerAction::HintRequest, Some(n)) => Ok(LessonEvent::HintRequest { at: now, n }),
        (AnswerAction::GiveUp, Some(n)) => Ok(LessonEvent::GiveUp { at: now, n }),
        (AnswerAction::NextProblem, None) if practice::awaiting_next(&parsed.events) => {
            Ok(LessonEvent::NextProblem { at: now })
        }
        _ => Err(AiFailure::LessonChanged),
    }
}

/// Шаг 1. Выключенная функция отказывает до любой записи и сети (FR-001). Ответ
/// пользователя ложится в файл урока до журнала и сети (FR-007), журнал получает запрос
/// до отправки (FR-012). `expected` — число событий, которое видит интерфейс.
///
/// `verified` — подключение, для которого оболочка проверила одобрение адреса и взяла
/// ключ. Настройки читаются здесь заново; если провайдер или адрес с тех пор сменились
/// (синхронизация посреди цикла), ход отказывает `EndpointNotApproved` до любой записи
/// (ревью безопасности 051, H1).
#[allow(clippy::too_many_arguments)]
pub async fn begin_turn<B: VaultBackend>(
    backend: &mut B,
    path: &str,
    request_id: &str,
    answer: Option<UserAnswer>,
    expected: u32,
    now: i64,
    allow_custom: bool,
    verified: &Connection,
) -> Result<PreparedTurn, AiFailure> {
    let settings = load_settings(backend).await;
    if !settings.enabled {
        return Err(AiFailure::Disabled);
    }
    let path = lesson_path(path)?;
    let path = path.as_str();
    if !verified.is_current(&settings, allow_custom) {
        return Err(AiFailure::EndpointNotApproved);
    }
    let mut parsed = read_lesson(backend, path).await?;
    if parsed.header.format > FORMAT {
        return Err(AiFailure::LessonUnreadable);
    }
    if parsed.events.len() as u32 != expected {
        return Err(AiFailure::LessonChanged);
    }
    let base = base_url(&settings, allow_custom)?;
    let model = settings.model.clone().ok_or(AiFailure::ModelMissing)?;

    if let Some(answer) = answer {
        let event = match answer.action {
            AnswerAction::Answer => LessonEvent::User {
                at: now,
                text: answer.text,
                choice: answer.choice,
                confidence: answer.confidence,
                thinking: answer
                    .thinking
                    .filter(|thinking| !thinking.trim().is_empty()),
                dont_know: answer.dont_know,
            },
            action => action_event(&parsed, action, now)?,
        };
        append_events(backend, path, std::slice::from_ref(&event)).await?;
        parsed.events.push(event);
    }

    let language = current_language(&parsed);
    let context = build_context(backend, &settings, &parsed).await;
    let kind = if estimated_size(&context) > settings.context_budget as usize
        && context.steps_since_summary >= 2
    {
        TurnKind::Compact
    } else {
        TurnKind::Teach
    };
    let request = match kind {
        TurnKind::Teach => build_request(
            settings.provider,
            &base,
            &model,
            &context.system,
            &context.messages,
            Some(context.tools),
            settings.knowledge_agent_reads,
            MAX_TOKENS_TEACH,
        ),
        TurnKind::Compact => {
            let mut messages = context.messages;
            push(
                &mut messages,
                Role::User,
                if context.tools == ToolSet::Practice {
                    prompt::compact_request_practice(language).to_owned()
                } else {
                    prompt::compact_request(language).to_owned()
                },
            );
            build_request(
                settings.provider,
                &base,
                &model,
                &context.system,
                &messages,
                None,
                false,
                MAX_TOKENS_COMPACT,
            )
        }
    };
    let events_len = parsed.events.len() as u32;
    let id = format!("{request_id}#{events_len}");
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
    Ok(PreparedTurn {
        id,
        request_id: request_id.to_owned(),
        path: path.to_owned(),
        provider: settings.provider,
        model,
        kind,
        events_len,
        request,
    })
}

/// Шаг 2. Только сеть: vault здесь не нужен. Текст копится в буфере прогресса по
/// `vault_id/request_id`; отмена прекращает ожидание сразу, а полученное остаётся в
/// буфере для шага 3.
///
/// Ключ уходит только на тот адрес и тому провайдеру, что в `verified` (ревью
/// безопасности 051, H1): иначе ход, построенный по сменившимся настройкам, унёс бы ключ
/// прежнего подключения на новый адрес. Буфер открывается после того, как занят слот
/// отмены: повторный запрос с тем же id не портит буфер идущего (L1).
pub async fn send_turn<T: AiTransport>(
    transport: &T,
    vault_id: &str,
    prepared: &PreparedTurn,
    key: Option<&str>,
    verified: &Connection,
) -> Result<TurnReply, AiFailure> {
    if !verified.covers(prepared.provider, &prepared.request.url) {
        return Err(AiFailure::EndpointNotApproved);
    }
    let mut request = prepared.request.clone();
    authorize(&mut request, prepared.provider, key);
    let work = async {
        partial::start(vault_id, &prepared.request_id, &prepared.path);
        let mut parser = StreamParser::new(prepared.provider);
        let status = transport
            .send_stream(request, &mut |chunk| {
                let before = parser.text().len();
                parser.feed(chunk);
                partial::update(
                    vault_id,
                    &prepared.request_id,
                    &parser.text()[before..],
                    parser.reading(),
                );
            })
            .await?;
        check_status(status)?;
        parser.finish()
    };
    cancellable(vault_id, &prepared.request_id, work).await
}

/// Пределы итога замера (как у вопроса учителя, 051 L8).
const MAX_PROBE_PARTS: usize = 20;
const PART_NAME_MAX_CHARS: usize = 100;
const PROBE_NOTE_MAX_CHARS: usize = 300;
const PROBE_START_MAX_CHARS: usize = 300;

struct Calls {
    question: Option<LessonQuestion>,
    reads: Vec<String>,
    probe_done: Option<(Vec<ProbePart>, String)>,
    plan_set: Option<Result<PlanSet, PlanError>>,
    plan_progress: Option<PlanProgressArgs>,
    practice: Option<PracticeCall>,
    knowledge_reads: Vec<String>,
}

fn clip(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Вопрос модели с пределами: не больше пяти вариантов и длины (L8).
fn question_of(input: &Value, part: Option<String>) -> Option<LessonQuestion> {
    let question = input["question"]
        .as_str()
        .map(str::trim)
        .unwrap_or_default();
    if question.is_empty() {
        return None;
    }
    // Модель не хозяин интерфейса: не больше пяти вариантов и пределы длины
    // (ревью безопасности 051, L8).
    let choices: Vec<String> = input["choices"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|choice| choice.as_str())
                .map(|choice| clip(choice.trim(), CHOICE_MAX_CHARS))
                .filter(|choice| !choice.is_empty())
                .take(MAX_CHOICES)
                .collect()
        })
        .unwrap_or_default();
    Some(LessonQuestion {
        question: clip(question, QUESTION_MAX_CHARS),
        choices: (choices.len() >= 2).then_some(choices),
        part,
    })
}

fn probe_parts_of(input: &Value) -> (Vec<ProbePart>, String) {
    let parts = input["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let name = clip(item["name"].as_str()?.trim(), PART_NAME_MAX_CHARS);
            if name.is_empty() {
                return None;
            }
            let state = match item["state"].as_str().map(str::to_lowercase).as_deref() {
                Some("known") => ProbeState::Known,
                Some("partial") => ProbeState::Partial,
                _ => ProbeState::Unknown,
            };
            Some(ProbePart {
                name,
                state,
                note: clip(
                    item["note"].as_str().map(str::trim).unwrap_or_default(),
                    PROBE_NOTE_MAX_CHARS,
                ),
            })
        })
        .take(MAX_PROBE_PARTS)
        .collect();
    let start = clip(
        input["start"].as_str().map(str::trim).unwrap_or_default(),
        PROBE_START_MAX_CHARS,
    );
    (parts, start)
}

/// Разбор вызовов инструментов. Действуют только инструменты стадии: вызов замера в
/// обучении и наоборот игнорируется; на ход — не больше одного вопроса, одного
/// `plan_set`, одного `plan_progress`.
fn parse_calls(calls: &[ToolCall], set: ToolSet) -> Calls {
    let mut parsed = Calls {
        question: None,
        reads: Vec::new(),
        probe_done: None,
        plan_set: None,
        plan_progress: None,
        practice: None,
        knowledge_reads: Vec::new(),
    };
    for call in calls {
        let input: Value = if call.arguments.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&call.arguments).unwrap_or(Value::Null)
        };
        match call.name.as_str() {
            prompt::ASK_QUESTION if set == ToolSet::Teach && parsed.question.is_none() => {
                parsed.question = question_of(&input, None);
            }
            prompt::PROBE_QUESTION if set == ToolSet::Probe && parsed.question.is_none() => {
                // Название части нужно ленте и подсчёту; без него — прочерк.
                let part = clip(
                    input["part"].as_str().map(str::trim).unwrap_or_default(),
                    PART_NAME_MAX_CHARS,
                );
                let part = if part.is_empty() {
                    "—".to_owned()
                } else {
                    part
                };
                parsed.question = question_of(&input, Some(part));
            }
            prompt::PROBE_DONE
                if matches!(set, ToolSet::Probe | ToolSet::ProbeEnd)
                    && parsed.probe_done.is_none() =>
            {
                parsed.probe_done = Some(probe_parts_of(&input));
            }
            prompt::READ_NOTE
                if matches!(set, ToolSet::Teach | ToolSet::Probe)
                    && parsed.reads.len() < MAX_NOTE_READS_PER_TURN =>
            {
                if let Some(path) = input["path"].as_str().map(str::trim) {
                    // Путь в чистом виде; негодный остаётся как есть и получит отказ.
                    let path = normalize_folder(Path::new(path)).unwrap_or_else(|| path.to_owned());
                    if !path.is_empty() && !parsed.reads.contains(&path) {
                        parsed.reads.push(path);
                    }
                }
            }
            prompt::READ_KNOWLEDGE
                if matches!(set, ToolSet::Teach | ToolSet::Probe | ToolSet::Practice)
                    && parsed.knowledge_reads.len() < MAX_KNOWLEDGE_READS_PER_TURN =>
            {
                let query = clip(
                    input["query"].as_str().map(str::trim).unwrap_or_default(),
                    crate::ai::knowledge::QUERY_MAX_CHARS,
                );
                if !query.is_empty() && !parsed.knowledge_reads.contains(&query) {
                    parsed.knowledge_reads.push(query);
                }
            }
            prompt::PLAN_SET if set == ToolSet::Teach && parsed.plan_set.is_none() => {
                parsed.plan_set = Some(plan::parse_plan_set(&input));
            }
            prompt::PLAN_PROGRESS if set == ToolSet::Teach && parsed.plan_progress.is_none() => {
                parsed.plan_progress = Some(plan::parse_plan_progress(&input));
            }
            // Агент практики: первый вызов хода, остальные игнорируются.
            practice::POSE_PROBLEM | practice::GIVE_HINT | practice::CONCLUDE
                if set == ToolSet::Practice && parsed.practice.is_none() =>
            {
                parsed.practice = practice::parse_call(call.name.as_str(), &input);
            }
            _ => {}
        }
    }
    // Итог замера закрывает стадию: вопрос в том же ходе уже ни к чему.
    if parsed.probe_done.is_some() {
        parsed.question = None;
    }
    parsed
}

/// Шаг 3. Итог в файл урока и журнал. Оборванный ход дописывается оборванной репликой с
/// тем, что успело прийти. Отказ возвращается после записи; урок интерфейс перечитывает.
pub async fn finish_turn<B: VaultBackend>(
    backend: &mut B,
    vault_id: &str,
    prepared: &PreparedTurn,
    now: i64,
    outcome: Result<TurnReply, AiFailure>,
) -> Result<TurnDone, AiFailure> {
    let buffered = partial::take(vault_id, &prepared.request_id).unwrap_or_default();
    let reply = match outcome {
        Ok(reply) if prepared.kind == TurnKind::Compact && reply.text.trim().is_empty() => {
            Err(AiFailure::BadResponse)
        }
        other => other,
    };
    match reply {
        Err(failure) => {
            if prepared.kind == TurnKind::Teach && !buffered.trim().is_empty() {
                let event = LessonEvent::Teacher {
                    at: now,
                    text: buffered,
                    question: None,
                    interrupted: true,
                    failure: Some(failure),
                    model: prepared.model.clone(),
                    input_tokens: None,
                    output_tokens: None,
                    cost_usd: None,
                };
                append_events(backend, &prepared.path, &[event]).await?;
            }
            let _ = record_done(backend, &prepared.id, now, Some(failure), None, None, None).await;
            partial::clear_partial(backend, &prepared.path).await;
            Err(failure)
        }
        Ok(reply) => {
            let cost_usd = match (reply.input_tokens, reply.output_tokens) {
                (Some(input), Some(output)) if prepared.provider == AiProvider::Anthropic => {
                    estimate_usd(&prepared.model, input, output)
                }
                _ => None,
            };
            let mut events: Vec<LessonEvent> = Vec::new();
            let mut did_tool = false;
            let mut asked = false;
            match prepared.kind {
                TurnKind::Compact => events.push(LessonEvent::Summary {
                    at: now,
                    text: reply.text.trim().to_owned(),
                    covers: prepared.events_len,
                    model: Some(prepared.model.clone()),
                    input_tokens: reply.input_tokens,
                    output_tokens: reply.output_tokens,
                    cost_usd,
                }),
                TurnKind::Teach => {
                    let before = read_lesson(backend, &prepared.path).await?;
                    let language = current_language(&before);
                    let calls = parse_calls(&reply.tool_calls, tool_set_of(&before));
                    asked = calls.question.is_some();
                    events.push(LessonEvent::Teacher {
                        at: now,
                        text: reply.text.trim().to_owned(),
                        question: calls.question,
                        interrupted: false,
                        failure: None,
                        model: prepared.model.clone(),
                        input_tokens: reply.input_tokens,
                        output_tokens: reply.output_tokens,
                        cost_usd,
                    });
                    let settings = load_settings(backend).await;
                    for path in calls.reads {
                        let (outcome, _) = read_note_text(backend, &settings, &path).await;
                        did_tool = true;
                        events.push(LessonEvent::NoteRead {
                            at: now,
                            path,
                            by_user: false,
                            outcome,
                        });
                    }
                    // Чтение графа знаний исполняется, только если оно разрешено: модель могла
                    // вызвать инструмент, которого в запросе не было.
                    if settings.knowledge_agent_reads {
                        for query in calls.knowledge_reads {
                            let (outcome, _) = agent_results(backend, &settings, &query).await;
                            did_tool = true;
                            events.push(LessonEvent::KnowledgeRead {
                                at: now,
                                query,
                                outcome,
                            });
                        }
                    }
                    // Итог замера и план: проверяются ядром против файла урока.
                    let mut seen: Vec<LessonEvent> = Vec::new();
                    if calls.plan_set.is_some() || calls.plan_progress.is_some() {
                        seen = before.events.clone();
                        seen.extend(events.iter().cloned());
                    }
                    if let Some((parts, start)) = calls.probe_done {
                        did_tool = true;
                        // После итога замера урок ждёт: пользователь смотрит итог и план и
                        // сам нажимает «Начать обучение» (макет 052, артборд C).
                        asked = true;
                        events.push(LessonEvent::ProbeResult {
                            at: now,
                            parts,
                            start,
                        });
                    }
                    if let Some(result) = calls.plan_set {
                        match result {
                            Ok(set) => {
                                let event = LessonEvent::Plan {
                                    at: now,
                                    version: plan::plan_history(&seen).len() as u32 + 1,
                                    steps: set.steps,
                                    goal: set.goal,
                                    current: set.current,
                                    reason: set.reason,
                                };
                                seen.push(event.clone());
                                events.push(event);
                            }
                            Err(error) => {
                                did_tool = true;
                                events.push(rejected(language, now, prompt::PLAN_SET, &error));
                            }
                        }
                    }
                    if let Some(mut args) = calls.plan_progress {
                        let verdict = match plan::plan_view(&seen, None) {
                            None => Err(PlanError::NoPlan),
                            Some(view) => {
                                let current = args.current.take().unwrap_or(view.current);
                                let checked = PlanProgressArgs {
                                    current: Some(current.clone()),
                                    done: args.done,
                                };
                                plan::check_progress(&seen, &checked)
                                    .map(|()| (current, checked.done))
                            }
                        };
                        match verdict {
                            Ok((current, done)) => events.push(LessonEvent::PlanProgress {
                                at: now,
                                current,
                                done,
                            }),
                            Err(error) => {
                                did_tool = true;
                                events.push(rejected(language, now, prompt::PLAN_PROGRESS, &error));
                            }
                        }
                    }
                    if before.header.kind == LessonKind::Practice {
                        let mut concluded = false;
                        if let Some(call) = calls.practice {
                            let tool = call.tool();
                            match practice::resolve(&before.events, call, now) {
                                Ok(event) => {
                                    concluded = matches!(event, LessonEvent::Verdict { .. });
                                    asked = true;
                                    events.push(event);
                                }
                                Err(error) => {
                                    did_tool = true;
                                    events.push(LessonEvent::PlanRejected {
                                        at: now,
                                        tool: tool.to_owned(),
                                        reason: prompt::practice_error(language, &error),
                                    });
                                }
                            }
                        }
                        // «Сдаюсь» без итога от модели: решение показывает ядро.
                        if !concluded {
                            if let Some(event) =
                                practice::forced_verdict(&before.events, &reply.text, now)
                            {
                                asked = true;
                                events.push(event);
                            }
                        }
                    }
                }
            }
            append_events(backend, &prepared.path, &events).await?;
            let _ = record_done(
                backend,
                &prepared.id,
                now,
                None,
                reply.input_tokens,
                reply.output_tokens,
                cost_usd,
            )
            .await;
            partial::clear_partial(backend, &prepared.path).await;

            let parsed = read_lesson(backend, &prepared.path).await?;
            let next = match prepared.kind {
                TurnKind::Compact => Next::Continue,
                TurnKind::Teach
                    if did_tool && !asked && teacher_run(&parsed.events) <= MAX_AUTO_CONTINUES =>
                {
                    Next::Continue
                }
                TurnKind::Teach => Next::WaitUser,
            };
            Ok(TurnDone {
                view: crate::ai::knowledge::annotate(backend, view_of(&prepared.path, parsed))
                    .await,
                next,
            })
        }
    }
}

/// Прикрепление заметки при создании урока: та же проверка и то же чтение, что у
/// инструмента `read_note`.
pub(super) async fn read_attached<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    path: &str,
) -> (NoteOutcome, Option<String>) {
    read_note_text(backend, settings, path).await
}

/// Событие отказа инструменту плана: причина — на языке урока.
fn rejected(language: Language, at: i64, tool: &str, error: &PlanError) -> LessonEvent {
    LessonEvent::PlanRejected {
        at,
        tool: tool.to_owned(),
        reason: prompt::plan_error(language, error),
    }
}
