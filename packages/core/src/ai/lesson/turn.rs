//! Ход учителя в три шага, как пробный запрос 050 (фича 051, research.md §2, §5, §6):
//! [`begin_turn`] пишет ответ пользователя в файл и запрос в журнал,
//! [`send_turn`] ходит в сеть, [`finish_turn`] дописывает реплику учителя. Сеть
//! вынесена отдельно: на её время не держатся ни блокировка vault, ни очередь вызовов
//! wasm. Решение «продолжать самому или ждать пользователя» принимает ядро ([`Next`]).

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::file::{
    append_events, current_language, lesson_path, read_lesson, teacher_run, view_of, Confidence,
    Language, LessonEvent, LessonQuestion, LessonView, NoteOutcome, ParsedLesson, FORMAT,
    MAX_AUTO_CONTINUES,
};
use super::partial;
use super::prompt::{self, NoteResult};
use super::stream::{StreamParser, ToolCall, TurnReply};
use super::MAX_ATTACHED_NOTES;
use crate::ai::access::{check_access_in, normalize_folder};
use crate::ai::cancel::cancellable;
use crate::ai::client::Connection;
use crate::ai::endpoint::base_url;
use crate::ai::journal::{record_done, record_sent};
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
/// Суммарный объём прикреплённых заметок в запросе, символов.
pub const ATTACHED_TOTAL_CHARS: usize = 300_000;
/// Вопрос учителя: число вариантов и пределы длины (L8).
const MAX_CHOICES: usize = 5;
const QUESTION_MAX_CHARS: usize = 1_000;
const CHOICE_MAX_CHARS: usize = 300;
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
    previous: Option<&LessonQuestion>,
) -> String {
    let mut out = text.trim().to_owned();
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
        out.push_str(&format!(
            "{}: {}",
            prompt::question_label(language),
            question.question
        ));
        for (index, choice) in question.choices.iter().flatten().enumerate() {
            out.push_str(&format!("\n{}) {choice}", index + 1));
        }
    }
    out
}

/// Собирает запрос из файла урока: инструкции с прикреплёнными заметками, изложение (если
/// было) и события после него. Оборванные реплики модели не показываются.
async fn build_context<B: VaultBackend>(
    backend: &B,
    settings: &AiSettings,
    parsed: &ParsedLesson,
) -> Context {
    let language = current_language(parsed);
    let mut system = prompt::system(language, &parsed.header.goal);

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
                text: prompt::start_message(language).to_owned(),
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

fn build_request(
    provider: AiProvider,
    base: &str,
    model: &str,
    system: &str,
    messages: &[Message],
    tools: bool,
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
            if tools {
                body["tools"] = prompt::tools(provider);
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
            if tools {
                body["tools"] = prompt::tools(provider);
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
        let event = LessonEvent::User {
            at: now,
            text: answer.text,
            choice: answer.choice,
            confidence: answer.confidence,
            thinking: answer
                .thinking
                .filter(|thinking| !thinking.trim().is_empty()),
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
            true,
            MAX_TOKENS_TEACH,
        ),
        TurnKind::Compact => {
            let mut messages = context.messages;
            push(
                &mut messages,
                Role::User,
                prompt::compact_request(language).to_owned(),
            );
            build_request(
                settings.provider,
                &base,
                &model,
                &context.system,
                &messages,
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

struct Calls {
    question: Option<LessonQuestion>,
    reads: Vec<String>,
}

fn clip(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn parse_calls(calls: &[ToolCall]) -> Calls {
    let mut parsed = Calls {
        question: None,
        reads: Vec::new(),
    };
    for call in calls {
        let input: Value = if call.arguments.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&call.arguments).unwrap_or(Value::Null)
        };
        match call.name.as_str() {
            prompt::ASK_QUESTION if parsed.question.is_none() => {
                let question = input["question"]
                    .as_str()
                    .map(str::trim)
                    .unwrap_or_default();
                if question.is_empty() {
                    continue;
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
                parsed.question = Some(LessonQuestion {
                    question: clip(question, QUESTION_MAX_CHARS),
                    choices: (choices.len() >= 2).then_some(choices),
                });
            }
            prompt::READ_NOTE if parsed.reads.len() < MAX_NOTE_READS_PER_TURN => {
                if let Some(path) = input["path"].as_str().map(str::trim) {
                    // Путь в чистом виде; негодный остаётся как есть и получит отказ.
                    let path = normalize_folder(Path::new(path)).unwrap_or_else(|| path.to_owned());
                    if !path.is_empty() && !parsed.reads.contains(&path) {
                        parsed.reads.push(path);
                    }
                }
            }
            _ => {}
        }
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
            let mut did_read = false;
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
                    let calls = parse_calls(&reply.tool_calls);
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
                        did_read = true;
                        events.push(LessonEvent::NoteRead {
                            at: now,
                            path,
                            by_user: false,
                            outcome,
                        });
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
                    if did_read && !asked && teacher_run(&parsed.events) <= MAX_AUTO_CONTINUES =>
                {
                    Next::Continue
                }
                TurnKind::Teach => Next::WaitUser,
            };
            Ok(TurnDone {
                view: view_of(&prepared.path, parsed),
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
