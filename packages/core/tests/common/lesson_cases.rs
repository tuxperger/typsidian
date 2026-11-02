//! Сессия урока с ИИ-учителем (фича 051) — общая таблица для обоих таргетов. Сети нет:
//! поддельный транспорт отдаёт заготовленный поток и считает запросы.

#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::VecDeque;

use serde_json::{json, Value};
use typsidian_core::ai::cancel::cancel;
use typsidian_core::ai::client::Connection;
use typsidian_core::ai::journal::{journal_entries, JournalState};
use typsidian_core::ai::lesson::file::{parse, FORMAT};
use typsidian_core::ai::lesson::partial::{checkpoint, is_active, partial_path, progress, start};
use typsidian_core::ai::lesson::stream::StreamParser;
use typsidian_core::ai::lesson::turn::{begin_turn, finish_turn, send_turn};
use typsidian_core::ai::lesson::{
    create_lesson, list_lessons, open_lesson, set_language, set_state, Confidence, Language,
    LessonEvent, LessonState, LessonView, Next, NoteOutcome, TurnDone, TurnKind, UserAnswer,
    Waiting,
};
use typsidian_core::ai::settings::{save_settings, AiProvider, AiSettings};
use typsidian_core::ai::transport::{AiTransport, HttpRequest, HttpResponse};
use typsidian_core::ai::AiFailure;
use typsidian_core::vault::{classify, service_dir, FileKind};

use super::MemoryVault;

/// Тесты native идут параллельно, а отмена и буфер прогресса общие на процесс:
/// каждому циклу — свой vault_id.
fn unique(prefix: &str) -> String {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    format!(
        "{prefix}{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

const KEY: &str = "sk-ключ-урока";
const JOURNAL: &str = ".typsidian/ai-journal.jsonl";

enum Script {
    Reply {
        status: u16,
        chunks: Vec<Vec<u8>>,
    },
    /// Поток оборвался после частей.
    Cut {
        chunks: Vec<Vec<u8>>,
        failure: AiFailure,
    },
    /// Части пришли, дальше тишина.
    Hang {
        chunks: Vec<Vec<u8>>,
    },
}

struct StreamTransport {
    sent: RefCell<Vec<HttpRequest>>,
    scripts: RefCell<VecDeque<Script>>,
}

impl StreamTransport {
    fn with(scripts: Vec<Script>) -> Self {
        StreamTransport {
            sent: RefCell::default(),
            scripts: RefCell::new(scripts.into()),
        }
    }

    fn count(&self) -> usize {
        self.sent.borrow().len()
    }

    fn body(&self, index: usize) -> String {
        self.sent.borrow()[index].body.clone().unwrap_or_default()
    }
}

impl AiTransport for StreamTransport {
    async fn send(&self, _request: HttpRequest) -> Result<HttpResponse, AiFailure> {
        Err(AiFailure::Offline)
    }

    async fn send_stream(
        &self,
        request: HttpRequest,
        on_chunk: &mut (dyn FnMut(&[u8]) + Send),
    ) -> Result<u16, AiFailure> {
        self.sent.borrow_mut().push(request);
        let script = self.scripts.borrow_mut().pop_front();
        match script {
            None => Err(AiFailure::Offline),
            Some(Script::Reply { status, chunks }) => {
                if (200..300).contains(&status) {
                    for chunk in chunks {
                        on_chunk(&chunk);
                    }
                }
                Ok(status)
            }
            Some(Script::Cut { chunks, failure }) => {
                for chunk in chunks {
                    on_chunk(&chunk);
                }
                Err(failure)
            }
            Some(Script::Hang { chunks }) => {
                for chunk in chunks {
                    on_chunk(&chunk);
                }
                futures_util::future::pending::<()>().await;
                Ok(200)
            }
        }
    }
}

// ── Заготовки потоков ───────────────────────────────────────────────────────

fn event(name: &str, data: Value) -> Vec<u8> {
    format!("event: {name}\ndata: {data}\n\n").into_bytes()
}

/// Поток Anthropic: текст частями и, если нужно, вызов инструмента с аргументами частями.
fn anthropic(parts: &[&str], tool: Option<(&str, Value)>, input: u64, output: u64) -> Vec<Vec<u8>> {
    let mut chunks = vec![
        event(
            "message_start",
            json!({"type":"message_start","message":{"usage":{"input_tokens":input,"output_tokens":1}}}),
        ),
        event(
            "content_block_start",
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        ),
    ];
    for part in parts {
        chunks.push(event(
            "content_block_delta",
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":part}}),
        ));
    }
    chunks.push(event(
        "content_block_stop",
        json!({"type":"content_block_stop","index":0}),
    ));
    if let Some((name, arguments)) = tool {
        chunks.push(event(
            "content_block_start",
            json!({"type":"content_block_start","index":1,
                "content_block":{"type":"tool_use","id":"toolu_1","name":name,"input":{}}}),
        ));
        let raw = arguments.to_string();
        let middle = raw.len() / 2;
        // Границу по символам, а не по байтам: аргументы — русский текст.
        let middle = (0..=middle)
            .rev()
            .find(|i| raw.is_char_boundary(*i))
            .unwrap_or(0);
        for piece in [&raw[..middle], &raw[middle..]] {
            chunks.push(event(
                "content_block_delta",
                json!({"type":"content_block_delta","index":1,
                    "delta":{"type":"input_json_delta","partial_json":piece}}),
            ));
        }
        chunks.push(event(
            "content_block_stop",
            json!({"type":"content_block_stop","index":1}),
        ));
    }
    chunks.push(event(
        "message_delta",
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":output}}),
    ));
    chunks.push(event("message_stop", json!({"type":"message_stop"})));
    chunks
}

fn reply(chunks: Vec<Vec<u8>>) -> Script {
    Script::Reply {
        status: 200,
        chunks,
    }
}

fn say(text: &str) -> Script {
    reply(anthropic(&[text], None, 100, 10))
}

fn ask(text: &str, question: &str, choices: Option<Vec<&str>>) -> Script {
    let mut arguments = json!({"question": question});
    if let Some(choices) = choices {
        arguments["choices"] = json!(choices);
    }
    reply(anthropic(
        &[text],
        Some(("ask_question", arguments)),
        100,
        10,
    ))
}

fn read_note(text: &str, path: &str) -> Script {
    reply(anthropic(
        &[text],
        Some(("read_note", json!({"path": path}))),
        100,
        10,
    ))
}

/// Поток совместимого протокола с вызовом инструмента и объёмом в последнем куске.
fn compatible(parts: &[&str], tool: Option<(&str, &str)>, usage: (u64, u64)) -> Vec<Vec<u8>> {
    let data = |value: Value| format!("data: {value}\n\n").into_bytes();
    let mut chunks = Vec::new();
    for part in parts {
        chunks.push(data(
            json!({"choices":[{"index":0,"delta":{"content":part}}]}),
        ));
    }
    if let Some((name, arguments)) = tool {
        let middle = (0..=arguments.len() / 2)
            .rev()
            .find(|i| arguments.is_char_boundary(*i))
            .unwrap_or(0);
        chunks.push(data(json!({"choices":[{"index":0,"delta":{"tool_calls":[
            {"index":0,"id":"call_1","type":"function","function":{"name":name,"arguments":&arguments[..middle]}}]}}]})));
        chunks.push(data(json!({"choices":[{"index":0,"delta":{"tool_calls":[
            {"index":0,"function":{"arguments":&arguments[middle..]}}]}}]})));
    }
    chunks.push(data(
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}),
    ));
    chunks.push(data(
        json!({"choices":[],"usage":{"prompt_tokens":usage.0,"completion_tokens":usage.1}}),
    ));
    chunks.push(b"data: [DONE]\n\n".to_vec());
    chunks
}

// ── Окружение ───────────────────────────────────────────────────────────────

fn settings(provider: AiProvider, model: &str) -> AiSettings {
    AiSettings {
        enabled: true,
        provider,
        model: Some(model.to_owned()),
        ..AiSettings::default()
    }
}

async fn vault_with(settings: &AiSettings) -> MemoryVault {
    let mut vault = MemoryVault::new();
    save_settings(&mut vault, settings).await.unwrap();
    vault
}

async fn anthropic_vault() -> MemoryVault {
    vault_with(&settings(AiProvider::Anthropic, "claude-opus-5-5")).await
}

/// Снимок подключения по текущим настройкам vault, как его берёт оболочка.
async fn connection(vault: &MemoryVault) -> Connection {
    Connection::verify(
        &typsidian_core::ai::settings::load_settings(vault).await,
        true,
    )
    .unwrap()
}

/// Начало хода со свежим снимком подключения.
async fn begin_t(
    vault: &mut MemoryVault,
    path: &str,
    request_id: &str,
    answer: Option<UserAnswer>,
    expected: u32,
    now: i64,
) -> Result<typsidian_core::ai::lesson::PreparedTurn, AiFailure> {
    let verified = connection(vault).await;
    begin_turn(
        vault, path, request_id, answer, expected, now, true, &verified,
    )
    .await
}

/// Отправка со снимком, взятым по самому запросу.
async fn send_t(
    transport: &StreamTransport,
    vault_id: &str,
    prepared: &typsidian_core::ai::lesson::PreparedTurn,
    key: Option<&str>,
) -> Result<typsidian_core::ai::lesson::TurnReply, AiFailure> {
    let verified = Connection::of_request(prepared.provider, &prepared.request.url);
    send_turn(transport, vault_id, prepared, key, &verified).await
}

fn answer(text: &str) -> Option<UserAnswer> {
    Some(UserAnswer {
        text: text.to_owned(),
        choice: None,
        confidence: None,
        thinking: None,
    })
}

async fn new_lesson(vault: &mut MemoryVault) -> String {
    create_lesson(vault, "Понять замыкания", &[], Language::Ru, 10)
        .await
        .unwrap()
}

/// Цикл хода так, как его ведут host и клиент wasm: ядро решает, продолжать ли.
async fn cycle(
    vault: &mut MemoryVault,
    transport: &StreamTransport,
    path: &str,
    request_id: &str,
    mut answer: Option<UserAnswer>,
    mut expected: u32,
) -> Result<TurnDone, AiFailure> {
    let vault_id = unique("v");
    for _ in 0..20 {
        let prepared = begin_t(vault, path, request_id, answer.take(), expected, 100).await?;
        let outcome = send_t(transport, &vault_id, &prepared, Some(KEY)).await;
        let done = finish_turn(vault, &vault_id, &prepared, 200, outcome).await?;
        if done.next == Next::WaitUser {
            return Ok(done);
        }
        expected = done.view.events_len;
    }
    panic!("цикл не остановился");
}

fn teachers(view: &LessonView) -> Vec<&LessonEvent> {
    view.events
        .iter()
        .filter(|event| matches!(event, LessonEvent::Teacher { .. }))
        .collect()
}

fn users(view: &LessonView) -> usize {
    view.events
        .iter()
        .filter(|event| matches!(event, LessonEvent::User { .. }))
        .count()
}

fn text_of(view: &LessonView, index: usize) -> String {
    match &teachers(view)[index] {
        LessonEvent::Teacher { text, .. } => text.clone(),
        _ => unreachable!(),
    }
}

// ── T008: формат и разбор потока ────────────────────────────────────────────

pub async fn lesson_is_a_listed_kind_and_partial_stays_on_device() {
    let kind = classify(std::path::Path::new("Уроки/а.lesson"));
    assert_eq!(kind, FileKind::Lesson);
    assert!(kind.is_listed() && kind.is_openable() && kind.is_single_tab());
    assert!(service_dir::is_derived(".typsidian/lesson-partial/ab.txt"));
}

pub async fn file_skips_broken_lines_and_appends_after_torn_line() {
    let mut vault = anthropic_vault().await;
    let path = new_lesson(&mut vault).await;
    assert_eq!(path, "Уроки/Понять замыкания.lesson");
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(view.header.format, FORMAT);
    assert_eq!(view.header.goal, "Понять замыкания");
    assert_eq!(view.waiting, Waiting::Start);
    assert_eq!(view.events_len, 0);

    // Строка с мусором и оборванная последняя строка (сбой посреди записи).
    let mut text = String::from_utf8(vault.bytes_of(&path).unwrap()).unwrap();
    text.push_str("{ это не событие\n");
    text.push_str(r#"{"kind":"User","at":5,"text":"раз"}"#);
    text.push('\n');
    text.push_str(r#"{"kind":"Teacher","at":6,"te"#);
    vault.replace_externally(&path, text);
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(view.events_len, 1);
    assert_eq!(users(&view), 1);

    // Новая запись не слипается с оборванной строкой и не ломает прежние.
    set_language(&mut vault, &path, Language::En, 7)
        .await
        .unwrap();
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(view.language, Language::En);
    assert_eq!(view.events_len, 2);
    let raw = String::from_utf8(vault.bytes_of(&path).unwrap()).unwrap();
    assert!(raw.contains(r#"{"kind":"User","at":5,"text":"раз"}"#));

    // Нет шапки — урок не читается; формат новее — только чтение.
    vault.replace_externally(&path, "не json\n");
    assert_eq!(
        open_lesson(&mut vault, &path).await.unwrap_err(),
        AiFailure::LessonUnreadable
    );
    vault.replace_externally(
        &path,
        r#"{"format":99,"goal":"г","language":"ru","created_at":1}"#,
    );
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert!(view.read_only);
    assert!(parse(b"").is_err());
}

pub async fn goal_name_and_folder_rules() {
    let mut vault = anthropic_vault().await;
    assert_eq!(
        create_lesson(&mut vault, "   ", &[], Language::Ru, 1)
            .await
            .unwrap_err(),
        AiFailure::GoalEmpty
    );
    let first = create_lesson(
        &mut vault,
        "Что такое: Rust? / ссылки",
        &[],
        Language::Ru,
        1,
    )
    .await
    .unwrap();
    assert_eq!(first, "Уроки/Что такое Rust ссылки.lesson");
    let second = create_lesson(
        &mut vault,
        "Что такое: Rust? / ссылки",
        &[],
        Language::Ru,
        2,
    )
    .await
    .unwrap();
    assert_eq!(second, "Уроки/Что такое Rust ссылки 2.lesson");

    let mut changed = settings(AiProvider::Anthropic, "claude-opus-5-5");
    changed.lessons_folder = "Учёба/Языки".to_owned();
    save_settings(&mut vault, &changed).await.unwrap();
    let third = create_lesson(&mut vault, "Цель", &[], Language::En, 3)
        .await
        .unwrap();
    assert_eq!(third, "Учёба/Языки/Цель.lesson");
    // Служебное место не папка уроков.
    changed.lessons_folder = ".typsidian/x".to_owned();
    save_settings(&mut vault, &changed).await.unwrap();
    let fourth = create_lesson(&mut vault, "Цель", &[], Language::En, 3)
        .await
        .unwrap();
    assert_eq!(fourth, "Уроки/Цель.lesson");
}

pub async fn parser_assembles_anthropic_stream_from_arbitrary_chunks() {
    let stream: Vec<u8> = anthropic(
        &["Привет, ", "мир"],
        Some((
            "ask_question",
            json!({"question":"Что вернёт?","choices":["а","б"]}),
        )),
        42,
        7,
    )
    .concat();
    // Режем поток по байтам — в том числе посреди русской буквы.
    for step in [1usize, 3, 7, 50] {
        let mut parser = StreamParser::new(AiProvider::Anthropic);
        for piece in stream.chunks(step) {
            parser.feed(piece);
        }
        assert_eq!(parser.text(), "Привет, мир");
        let reply = parser.finish().unwrap();
        assert_eq!(reply.text, "Привет, мир");
        assert_eq!(reply.tool_calls.len(), 1);
        assert_eq!(reply.tool_calls[0].name, "ask_question");
        let arguments: Value = serde_json::from_str(&reply.tool_calls[0].arguments).unwrap();
        assert_eq!(arguments["choices"][1], "б");
        assert_eq!(
            (reply.input_tokens, reply.output_tokens),
            (Some(42), Some(7))
        );
    }
}

pub async fn parser_assembles_compatible_stream_with_usage() {
    let stream = compatible(
        &["Раз ", "два"],
        Some(("read_note", r#"{"path":"Заметки/а.typ"}"#)),
        (30, 5),
    )
    .concat();
    let mut parser = StreamParser::new(AiProvider::OpenAi);
    for piece in stream.chunks(5) {
        parser.feed(piece);
    }
    assert_eq!(parser.reading().as_deref(), Some("Заметки/а.typ"));
    let reply = parser.finish().unwrap();
    assert_eq!(reply.text, "Раз два");
    assert_eq!(reply.tool_calls[0].name, "read_note");
    assert_eq!(reply.tool_calls[0].arguments, r#"{"path":"Заметки/а.typ"}"#);
    assert_eq!(
        (reply.input_tokens, reply.output_tokens),
        (Some(30), Some(5))
    );
}

pub async fn parser_does_not_take_a_cut_stream_for_a_reply() {
    let mut chunks = anthropic(&["начало"], None, 1, 1);
    chunks.truncate(3);
    let mut parser = StreamParser::new(AiProvider::Anthropic);
    for chunk in &chunks {
        parser.feed(chunk);
    }
    assert_eq!(parser.text(), "начало");
    assert_eq!(parser.finish().unwrap_err(), AiFailure::Offline);

    // Ошибка внутри потока при коде 200 — отказ без текста сервера.
    let mut parser = StreamParser::new(AiProvider::Anthropic);
    parser.feed(
        "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"секрет сервера\"}}\n\n".as_bytes(),
    );
    assert_eq!(parser.finish().unwrap_err(), AiFailure::ProviderDown);
}

// ── T011: ход ───────────────────────────────────────────────────────────────

pub async fn answer_is_on_disk_before_the_network() {
    let mut vault = anthropic_vault().await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![say("ответ")]);
    let prepared = begin_t(&mut vault, &path, "r1", answer("МОЙ-ОТВЕТ"), 0, 100)
        .await
        .unwrap();

    // Сети ещё не было, а ответ уже в файле, и запрос уже в журнале как оборванный.
    assert_eq!(transport.count(), 0);
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(users(&view), 1);
    assert_eq!(view.waiting, Waiting::Retry);
    let entries = journal_entries(&vault).await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].state, JournalState::Interrupted);
    assert!(entries[0].request.contains("МОЙ-ОТВЕТ"));

    let outcome = send_t(&transport, "va", &prepared, Some(KEY)).await;
    let done = finish_turn(&mut vault, "va", &prepared, 200, outcome)
        .await
        .unwrap();
    assert_eq!(done.next, Next::WaitUser);
    assert_eq!(done.view.waiting, Waiting::User);
    assert_eq!(text_of(&done.view, 0), "ответ");
    assert_eq!(journal_entries(&vault).await[0].state, JournalState::Done);

    // Запрос потоковый, с инструментами; ключ ушёл заголовком, а не в журнал.
    let body: Value = serde_json::from_str(&transport.body(0)).unwrap();
    assert_eq!(body["stream"], true);
    assert_eq!(body["tools"].as_array().unwrap().len(), 2);
    assert!(body["system"]
        .as_str()
        .unwrap()
        .contains("Понять замыкания"));
    assert!(transport.sent.borrow()[0]
        .headers
        .iter()
        .any(|(n, v)| n == "x-api-key" && v == KEY));
    let raw = String::from_utf8(vault.bytes_of(JOURNAL).unwrap()).unwrap();
    assert!(!raw.contains(KEY));
}

pub async fn disabled_lesson_makes_no_requests() {
    let mut vault = MemoryVault::new();
    assert_eq!(
        create_lesson(&mut vault, "цель", &[], Language::Ru, 1)
            .await
            .unwrap_err(),
        AiFailure::Disabled
    );
    assert!(vault.paths_under("Уроки").is_empty());

    // Урок есть (включали раньше), а потом обучение выключили.
    let mut enabled = anthropic_vault().await;
    let path = new_lesson(&mut enabled).await;
    let before = enabled.bytes_of(&path).unwrap();
    save_settings(&mut enabled, &AiSettings::default())
        .await
        .unwrap();
    let transport = StreamTransport::with(vec![say("не должно прийти")]);
    assert_eq!(
        begin_t(&mut enabled, &path, "r", answer("текст"), 0, 1)
            .await
            .unwrap_err(),
        AiFailure::Disabled
    );
    assert_eq!(transport.count(), 0);
    assert_eq!(enabled.bytes_of(&path).unwrap(), before);
    assert!(enabled.bytes_of(JOURNAL).is_none());
    // Прочитать уже написанное можно.
    assert!(open_lesson(&mut enabled, &path).await.is_ok());
}

pub async fn question_with_choices_and_open_question() {
    let mut vault = anthropic_vault().await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![
        ask(
            "Начнём с предсказания.",
            "Что выведет код?",
            Some(vec!["1", "2", "3"]),
        ),
        ask("", "Почему так?", None),
    ]);
    let done = cycle(&mut vault, &transport, &path, "a", None, 0)
        .await
        .unwrap();
    assert_eq!(done.next, Next::WaitUser);
    assert_eq!(done.view.waiting, Waiting::User);
    let question = done.view.last_question.clone().unwrap();
    assert_eq!(question.question, "Что выведет код?");
    assert_eq!(question.choices.as_deref().unwrap().len(), 3);

    let done = cycle(
        &mut vault,
        &transport,
        &path,
        "b",
        Some(UserAnswer {
            text: String::new(),
            choice: Some(1),
            confidence: Some(Confidence::Guess),
            thinking: Some("наверное, замыкание помнит i".to_owned()),
        }),
        done.view.events_len,
    )
    .await
    .unwrap();
    let question = done.view.last_question.clone().unwrap();
    assert_eq!(question.question, "Почему так?");
    assert!(question.choices.is_none());

    // Второй запрос несёт вопрос учителя как его реплику, выбранный вариант, уверенность
    // и рассуждение; инструкции в нём — ru.
    let body: Value = serde_json::from_str(&transport.body(1)).unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[1]["role"], "assistant");
    assert!(messages[1]["content"].as_str().unwrap().contains("2) 2"));
    let last = messages[2]["content"].as_str().unwrap();
    assert_eq!(messages[2]["role"], "user");
    assert!(last.starts_with('2'), "выбранный вариант: {last}");
    assert!(last.contains("угадываю") && last.contains("замыкание помнит"));
}

pub async fn reply_without_tools_is_a_teacher_line() {
    let mut vault = anthropic_vault().await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![reply(anthropic(
        &["Сервер ", "без ", "инструментов?"],
        None,
        10,
        3,
    ))]);
    let done = cycle(&mut vault, &transport, &path, "a", None, 0)
        .await
        .unwrap();
    assert_eq!(done.next, Next::WaitUser);
    assert_eq!(text_of(&done.view, 0), "Сервер без инструментов?");
    assert!(done.view.last_question.is_none());
    // Урок без единого ответа: первое сообщение — просьба начать.
    let body: Value = serde_json::from_str(&transport.body(0)).unwrap();
    assert_eq!(body["messages"][0]["content"], "Начни урок.");
}

pub async fn changed_lesson_is_refused_and_nothing_is_written() {
    let mut vault = anthropic_vault().await;
    let path = new_lesson(&mut vault).await;
    let before = vault.bytes_of(&path).unwrap();
    assert_eq!(
        begin_t(&mut vault, &path, "r", answer("текст"), 3, 1)
            .await
            .unwrap_err(),
        AiFailure::LessonChanged
    );
    assert_eq!(vault.bytes_of(&path).unwrap(), before);
    assert!(vault.bytes_of(JOURNAL).is_none());
}

pub async fn journal_failure_keeps_the_answer_and_stops_the_request() {
    let mut vault = anthropic_vault().await.with_write_failure(JOURNAL);
    let path = new_lesson(&mut vault).await;
    assert_eq!(
        begin_t(&mut vault, &path, "r", answer("сохранить"), 0, 1)
            .await
            .unwrap_err(),
        AiFailure::JournalFailed
    );
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(users(&view), 1);
    assert_eq!(view.waiting, Waiting::Retry);
}

pub async fn status_failure_is_a_refusal_without_server_text() {
    let mut vault = anthropic_vault().await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![Script::Reply {
        status: 401,
        chunks: vec!["data: {\"секрет\":1}\n".as_bytes().to_vec()],
    }]);
    assert_eq!(
        cycle(&mut vault, &transport, &path, "a", answer("привет"), 0)
            .await
            .unwrap_err(),
        AiFailure::BadKey
    );
    let entries = journal_entries(&vault).await;
    assert_eq!(entries[0].state, JournalState::Failed);
    assert_eq!(entries[0].failure, Some(AiFailure::BadKey));
    // Ответ остался, а реплики учителя нет: можно запросить заново.
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!((users(&view), teachers(&view).len()), (1, 0));
    assert_eq!(view.waiting, Waiting::Retry);
}

pub async fn cancel_keeps_the_received_text_and_retry_needs_no_new_answer() {
    let mut vault = anthropic_vault().await;
    let path = create_lesson(&mut vault, "Отмена хода", &[], Language::Ru, 10)
        .await
        .unwrap();
    let hang = StreamTransport::with(vec![Script::Hang {
        chunks: anthropic(&["Часть ответа"], None, 1, 1)[..3].to_vec(),
    }]);
    let prepared = begin_t(&mut vault, &path, "stop", answer("мой ответ"), 0, 100)
        .await
        .unwrap();
    assert_eq!(prepared.kind, TurnKind::Teach);
    let sending = send_t(&hang, "vc", &prepared, Some(KEY));
    futures_util::pin_mut!(sending);
    assert!(futures_util::poll!(&mut sending).is_pending());
    // Опрос видит полученное, пока ход идёт; активный урок не «восстанавливается».
    assert_eq!(progress("vc", "stop").unwrap().text, "Часть ответа");
    assert!(is_active(&path));
    cancel("vc", "stop");
    let outcome = sending.await;
    assert_eq!(outcome, Err(AiFailure::Cancelled));
    assert_eq!(
        finish_turn(&mut vault, "vc", &prepared, 150, outcome)
            .await
            .unwrap_err(),
        AiFailure::Cancelled
    );
    assert!(progress("vc", "stop").is_none());
    assert!(!is_active(&path));

    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(view.waiting, Waiting::Retry);
    match teachers(&view)[0] {
        LessonEvent::Teacher {
            text,
            interrupted,
            failure,
            ..
        } => {
            assert_eq!(text, "Часть ответа");
            assert!(*interrupted);
            assert_eq!(*failure, Some(AiFailure::Cancelled));
        }
        _ => unreachable!(),
    }
    let entries = journal_entries(&vault).await;
    assert_eq!(entries[0].failure, Some(AiFailure::Cancelled));

    // Повтор без нового ответа: обрыв в запрос не идёт, нового ответа в файле нет.
    let transport = StreamTransport::with(vec![say("полный ответ")]);
    let done = cycle(
        &mut vault,
        &transport,
        &path,
        "again",
        None,
        view.events_len,
    )
    .await
    .unwrap();
    assert_eq!(users(&done.view), 1);
    assert_eq!(done.view.waiting, Waiting::User);
    assert!(!transport.body(0).contains("Часть ответа"));
    assert!(transport.body(0).contains("мой ответ"));
}

pub async fn network_break_saves_what_arrived() {
    let mut vault = anthropic_vault().await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![
        Script::Cut {
            chunks: anthropic(&["Пол", "ответа"], None, 1, 1)[..4].to_vec(),
            failure: AiFailure::Timeout,
        },
        // Поток закрылся без завершающего события — тоже обрыв.
        Script::Reply {
            status: 200,
            chunks: anthropic(&["Ещё"], None, 1, 1)[..3].to_vec(),
        },
    ]);
    assert_eq!(
        cycle(&mut vault, &transport, &path, "a", answer("ответ"), 0)
            .await
            .unwrap_err(),
        AiFailure::Timeout
    );
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(text_of(&view, 0), "Пол".to_owned() + "ответа");
    assert_eq!(
        cycle(&mut vault, &transport, &path, "b", None, view.events_len)
            .await
            .unwrap_err(),
        AiFailure::Offline
    );
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(text_of(&view, 1), "Ещё");
    assert_eq!(view.waiting, Waiting::Retry);
    assert_eq!(users(&view), 1);
}

pub async fn compatible_protocol_gets_stream_options() {
    let mut vault = vault_with(&settings(AiProvider::OpenAi, "gpt-x")).await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![reply(compatible(&["Привет"], None, (11, 4)))]);
    let done = cycle(&mut vault, &transport, &path, "a", None, 0)
        .await
        .unwrap();
    assert_eq!(text_of(&done.view, 0), "Привет");
    let body: Value = serde_json::from_str(&transport.body(0)).unwrap();
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["messages"][0]["role"], "system");
    assert!(transport.sent.borrow()[0]
        .url
        .ends_with("/chat/completions"));
    assert_eq!(body["tools"][0]["type"], "function");
    assert_eq!((done.view.input_tokens, done.view.output_tokens), (11, 4));
}

pub async fn language_and_state_are_events() {
    let mut vault = anthropic_vault().await;
    let path = new_lesson(&mut vault).await;
    let view = set_language(&mut vault, &path, Language::En, 20)
        .await
        .unwrap();
    assert_eq!(view.language, Language::En);
    assert_eq!(view.header.language, Language::Ru);
    let transport = StreamTransport::with(vec![say("hello")]);
    cycle(&mut vault, &transport, &path, "a", None, view.events_len)
        .await
        .unwrap();
    let body: Value = serde_json::from_str(&transport.body(0)).unwrap();
    assert!(body["system"].as_str().unwrap().contains("Talk in English"));
    assert_eq!(body["messages"][0]["content"], "Start the lesson.");
    let view = set_state(&mut vault, &path, LessonState::Finished, 30)
        .await
        .unwrap();
    assert_eq!(view.waiting, Waiting::Closed);
    let view = set_state(&mut vault, &path, LessonState::Active, 40)
        .await
        .unwrap();
    assert_eq!(view.waiting, Waiting::User);
}

// ── T021: восстановление, сжатие, список ────────────────────────────────────

pub async fn partial_file_restores_an_interrupted_reply() {
    let mut vault = anthropic_vault().await;
    let path = create_lesson(&mut vault, "Восстановление ответа", &[], Language::Ru, 10)
        .await
        .unwrap();
    begin_t(&mut vault, &path, "r", answer("вопрос"), 0, 100)
        .await
        .unwrap();
    // Идёт ответ: клиент раз в секунду просит записать накопленное.
    start("pv", "r", &path);
    typsidian_core::ai::lesson::partial::update("pv", "r", "Недописанн", None);
    checkpoint(&mut vault, "pv", &path, "r").await.unwrap();
    assert!(vault
        .bytes_of(&partial_path(&path).to_string_lossy())
        .is_some());

    // Пока ход идёт, урок не считается оборванным.
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert!(teachers(&view).is_empty());

    // Приложение закрыли: буфер памяти пропал, файл остался.
    typsidian_core::ai::lesson::partial::take("pv", "r");
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(text_of(&view, 0), "Недописанн");
    assert_eq!(view.waiting, Waiting::Retry);
    match teachers(&view)[0] {
        LessonEvent::Teacher {
            interrupted, model, ..
        } => {
            assert!(*interrupted);
            assert_eq!(model, "claude-opus-5-5");
        }
        _ => unreachable!(),
    }
    assert!(vault
        .bytes_of(&partial_path(&path).to_string_lossy())
        .is_none());
    // Повторное открытие ничего не дописывает.
    let again = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(again.events_len, view.events_len);
}

pub async fn compaction_summarises_and_keeps_the_full_file() {
    let mut config = settings(AiProvider::Anthropic, "claude-opus-5-5");
    config.context_budget = 8_000;
    let mut vault = vault_with(&config).await;
    let path = new_lesson(&mut vault).await;
    let long = |marker: &str| format!("{marker} {}", "а".repeat(20_000));
    let transport = StreamTransport::with(vec![
        say("первая реплика"),
        say("ИЗЛОЖЕНИЕ: цель, пройдено, путаница"),
        say("реплика после сжатия"),
    ]);

    let first = cycle(
        &mut vault,
        &transport,
        &path,
        "a",
        answer(&long("МАРКЕР-ПЕРВЫЙ")),
        0,
    )
    .await
    .unwrap();
    assert_eq!(transport.count(), 1);
    let done = cycle(
        &mut vault,
        &transport,
        &path,
        "b",
        answer(&long("МАРКЕР-ВТОРОЙ")),
        first.view.events_len,
    )
    .await
    .unwrap();

    // Запрос 2 — сжатие: вся переписка и просьба изложить, без инструментов.
    assert_eq!(transport.count(), 3);
    let compact: Value = serde_json::from_str(&transport.body(1)).unwrap();
    assert!(compact.get("tools").is_none());
    assert!(transport.body(1).contains("МАРКЕР-ПЕРВЫЙ"));
    assert!(transport.body(1).contains("Изложи пройденное"));
    // Запрос 3 — после изложения: оно есть, старых слов нет.
    assert!(transport.body(2).contains("ИЗЛОЖЕНИЕ"));
    assert!(!transport.body(2).contains("МАРКЕР-ПЕРВЫЙ"));
    assert!(!transport.body(2).contains("МАРКЕР-ВТОРОЙ"));

    // В файле осталась вся переписка и событие изложения; охват — число событий на тот момент.
    let kinds: Vec<&str> = done
        .view
        .events
        .iter()
        .map(|event| match event {
            LessonEvent::User { .. } => "U",
            LessonEvent::Teacher { .. } => "T",
            LessonEvent::Summary { .. } => "S",
            _ => "?",
        })
        .collect();
    assert_eq!(kinds, ["U", "T", "U", "S", "T"]);
    match &done.view.events[3] {
        LessonEvent::Summary { covers, .. } => assert_eq!(*covers, 3),
        _ => unreachable!(),
    }
    assert_eq!(done.view.waiting, Waiting::User);
    // Запись журнала у сжатия своя.
    assert_eq!(journal_entries(&vault).await.len(), 3);
}

pub async fn list_shows_all_lessons_with_states() {
    let mut vault = anthropic_vault().await;
    let first = new_lesson(&mut vault).await;
    let second = create_lesson(&mut vault, "Второй", &[], Language::En, 50)
        .await
        .unwrap();
    let transport = StreamTransport::with(vec![say("реплика")]);
    cycle(&mut vault, &transport, &second, "a", answer("привет"), 0)
        .await
        .unwrap();
    set_state(&mut vault, &first, LessonState::Finished, 900)
        .await
        .unwrap();
    // Перенесённый в другую папку, поломанный и чужой по расширению файлы.
    let moved = String::from_utf8(vault.bytes_of(&second).unwrap()).unwrap();
    let mut vault = vault
        .with_file("Архив/старый.lesson", moved.replace("Второй", "Старый"))
        .with_file("Архив/битый.lesson", "мусор")
        .with_file("Уроки/заметка.typ", "= не урок");
    let lessons = list_lessons(&vault).await;
    let goals: Vec<(&str, LessonState)> = lessons
        .iter()
        .map(|lesson| (lesson.goal.as_str(), lesson.state))
        .collect();
    assert_eq!(
        goals,
        [
            ("Понять замыкания", LessonState::Finished),
            ("Старый", LessonState::Active),
            ("Второй", LessonState::Active),
        ]
    );
    assert_eq!(lessons[0].last_at, 900);
    assert_eq!(lessons[2].steps, 2);
    let _ = &mut vault;
}

// ── T024: заметки ───────────────────────────────────────────────────────────

fn notes_vault_settings() -> AiSettings {
    let mut config = settings(AiProvider::Anthropic, "claude-opus-5-5");
    config.denied_folders = vec!["Личное".to_owned()];
    config
}

async fn vault_with_notes() -> MemoryVault {
    vault_with(&notes_vault_settings())
        .await
        .with_file("Заметки/а.typ", "= Замыкания\nТекст про захват переменных.")
        .with_file("Личное/секрет.typ", "ПАРОЛЬ-ОТ-БАНКА")
}

pub async fn attached_note_goes_into_request_and_log() {
    let mut vault = vault_with_notes().await;
    let path = create_lesson(
        &mut vault,
        "Замыкания",
        &["Заметки/а.typ".to_owned(), "Заметки/а.typ".to_owned()],
        Language::Ru,
        10,
    )
    .await
    .unwrap();
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(view.events_len, 1, "дубль прикрепления не пишется");
    assert!(matches!(
        &view.events[0],
        LessonEvent::NoteRead {
            by_user: true,
            outcome: NoteOutcome::Sent {
                truncated: false,
                ..
            },
            ..
        }
    ));
    // Текст заметки в урок не пишется (research.md §12).
    let raw = String::from_utf8(vault.bytes_of(&path).unwrap()).unwrap();
    assert!(!raw.contains("захват переменных"));

    let transport = StreamTransport::with(vec![say("ок"), say("ещё")]);
    let done = cycle(&mut vault, &transport, &path, "a", None, view.events_len)
        .await
        .unwrap();
    assert!(transport.body(0).contains("Текст про захват переменных"));
    let entries = journal_entries(&vault).await;
    assert!(entries[0].request.contains("Текст про захват переменных"));
    // Заметка остаётся у учителя весь урок.
    cycle(
        &mut vault,
        &transport,
        &path,
        "b",
        answer("дальше"),
        done.view.events_len,
    )
    .await
    .unwrap();
    assert!(transport.body(1).contains("Текст про захват переменных"));
}

pub async fn denied_missing_and_outside_notes_are_refused_without_text() {
    let mut vault = vault_with_notes().await;
    let notes = [
        "Личное/секрет.typ",
        "../за-пределами.typ",
        "/etc/passwd",
        ".typsidian/ai.json",
        "Заметки/нет.typ",
    ]
    .map(str::to_owned);
    let path = create_lesson(&mut vault, "Цель", &notes, Language::Ru, 10)
        .await
        .unwrap();
    let view = open_lesson(&mut vault, &path).await.unwrap();
    let outcomes: Vec<NoteOutcome> = view
        .events
        .iter()
        .map(|event| match event {
            LessonEvent::NoteRead { outcome, .. } => *outcome,
            _ => panic!(),
        })
        .collect();
    assert_eq!(
        outcomes,
        [
            NoteOutcome::Denied,
            NoteOutcome::Denied,
            NoteOutcome::Denied,
            NoteOutcome::Denied,
            NoteOutcome::Missing
        ]
    );
    let transport = StreamTransport::with(vec![say("ок")]);
    cycle(&mut vault, &transport, &path, "a", None, view.events_len)
        .await
        .unwrap();
    assert!(!transport.body(0).contains("ПАРОЛЬ-ОТ-БАНКА"));
    let journal = String::from_utf8(vault.bytes_of(JOURNAL).unwrap()).unwrap();
    assert!(!journal.contains("ПАРОЛЬ-ОТ-БАНКА"));
}

pub async fn read_note_tool_continues_and_result_goes_next() {
    let mut vault = vault_with_notes().await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![
        read_note("Загляну в заметку.", "Заметки/а.typ"),
        read_note("И в закрытую.", "Личное/секрет.typ"),
        read_note("И в несуществующую.", "Заметки/нет.typ"),
        ask("Теперь вопрос.", "Что захватывает замыкание?", None),
    ]);
    let done = cycle(&mut vault, &transport, &path, "a", answer("начнём"), 0)
        .await
        .unwrap();
    assert_eq!(transport.count(), 4);
    assert_eq!(done.next, Next::WaitUser);
    let outcomes: Vec<(bool, NoteOutcome)> = done
        .view
        .events
        .iter()
        .filter_map(|event| match event {
            LessonEvent::NoteRead {
                by_user, outcome, ..
            } => Some((*by_user, *outcome)),
            _ => None,
        })
        .collect();
    assert_eq!(outcomes.len(), 3);
    assert!(matches!(outcomes[0], (false, NoteOutcome::Sent { .. })));
    assert_eq!(outcomes[1].1, NoteOutcome::Denied);
    assert_eq!(outcomes[2].1, NoteOutcome::Missing);

    // Результат уходит следующим запросом: текст, отказ, «нет такой».
    assert!(transport.body(1).contains("Текст про захват переменных"));
    assert!(transport.body(2).contains("закрыт"));
    assert!(!transport.body(2).contains("Текст про захват переменных"));
    assert!(transport.body(3).contains("такой заметки нет"));
    for index in 0..4 {
        assert!(!transport.body(index).contains("ПАРОЛЬ-ОТ-БАНКА"));
    }
    // Прочитанное попало в журнал вместе с запросом (SC-003).
    let entries = journal_entries(&vault).await;
    assert!(
        entries[2].request.contains("Текст про захват переменных"),
        "журнал: новые первыми"
    );
    assert_eq!(done.view.waiting, Waiting::User);
}

pub async fn auto_continue_stops_after_four() {
    let mut vault = vault_with_notes().await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(
        (0..8)
            .map(|_| read_note("читаю", "Заметки/а.typ"))
            .collect(),
    );
    let done = cycle(&mut vault, &transport, &path, "a", answer("начнём"), 0)
        .await
        .unwrap();
    // Первый ход и четыре продолжения.
    assert_eq!(transport.count(), 5);
    assert_eq!(done.next, Next::WaitUser);
    assert_eq!(done.view.waiting, Waiting::User);
    assert_eq!(teachers(&done.view).len(), 5);
}

// ── T026: стоимость ─────────────────────────────────────────────────────────

pub async fn lesson_cost_equals_the_journal_sum() {
    let mut vault = anthropic_vault().await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![
        reply(anthropic(&["раз"], None, 500_000, 100_000)),
        reply(anthropic(&["два"], None, 1_000_000, 0)),
    ]);
    let first = cycle(&mut vault, &transport, &path, "a", None, 0)
        .await
        .unwrap();
    assert_eq!(first.view.cost_usd, Some(4.0));
    let done = cycle(
        &mut vault,
        &transport,
        &path,
        "b",
        answer("дальше"),
        first.view.events_len,
    )
    .await
    .unwrap();
    let journal: f64 = journal_entries(&vault)
        .await
        .iter()
        .map(|entry| entry.cost_usd.unwrap())
        .sum();
    assert_eq!(done.view.cost_usd, Some(journal));
    assert_eq!(done.view.cost_usd, Some(8.0));
    assert_eq!(
        (done.view.input_tokens, done.view.output_tokens),
        (1_500_000, 100_000)
    );
}

pub async fn unknown_price_makes_the_lesson_cost_unknown() {
    // Модель без цены в справочнике и совместимый протокол: «неизвестно», а не ноль.
    for config in [
        settings(AiProvider::Anthropic, "модель-без-цены"),
        settings(AiProvider::OpenAi, "gpt-x"),
    ] {
        let mut vault = vault_with(&config).await;
        let path = new_lesson(&mut vault).await;
        let script = if config.provider == AiProvider::OpenAi {
            reply(compatible(&["раз"], None, (10, 5)))
        } else {
            reply(anthropic(&["раз"], None, 10, 5))
        };
        let transport = StreamTransport::with(vec![script]);
        let done = cycle(&mut vault, &transport, &path, "a", None, 0)
            .await
            .unwrap();
        assert_eq!(done.view.cost_usd, None);
        assert_eq!((done.view.input_tokens, done.view.output_tokens), (10, 5));
    }
    // Урок без единого хода стоит ровно ноль: слагаемых нет.
    let mut vault = anthropic_vault().await;
    let path = new_lesson(&mut vault).await;
    assert_eq!(
        open_lesson(&mut vault, &path).await.unwrap().cost_usd,
        Some(0.0)
    );
}

// ── T032–T046: находки ревью безопасности (research.md §14) ─────────────────

fn custom_settings(endpoint: &str) -> AiSettings {
    AiSettings {
        enabled: true,
        provider: AiProvider::Custom,
        custom_endpoint: Some(endpoint.to_owned()),
        model: Some("своя".to_owned()),
        ..AiSettings::default()
    }
}

/// H1: настройки сменились после проверки оболочки — ни ход, ни отправка с ключом не идут.
pub async fn changed_connection_never_gets_the_old_key() {
    let mut vault = vault_with(&custom_settings("https://models.example.org/v1")).await;
    let path = new_lesson(&mut vault).await;
    let verified = connection(&vault).await;
    let transport = StreamTransport::with(vec![say("не должно прийти")]);

    // Ход, построенный до смены, уходит на проверенный адрес.
    let first = begin_turn(
        &mut vault,
        &path,
        "h1",
        answer("раз"),
        0,
        100,
        true,
        &verified,
    )
    .await
    .unwrap();
    assert!(first
        .request
        .url
        .starts_with("https://models.example.org/v1/"));

    // Синхронизация принесла другой адрес и другого провайдера.
    for changed in [
        custom_settings("https://evil.example.net/v1"),
        settings(AiProvider::OpenAi, "gpt-x"),
    ] {
        save_settings(&mut vault, &changed).await.unwrap();
        let before = vault.bytes_of(&path).unwrap();
        let journal = vault.bytes_of(JOURNAL);
        assert_eq!(
            begin_turn(
                &mut vault,
                &path,
                "h2",
                answer("два"),
                1,
                101,
                true,
                &verified
            )
            .await
            .unwrap_err(),
            AiFailure::EndpointNotApproved
        );
        // Отказ раньше любой записи: ни ответа в файле, ни строки в журнале.
        assert_eq!(vault.bytes_of(&path).unwrap(), before);
        assert_eq!(vault.bytes_of(JOURNAL), journal);

        // Ход, построенный уже по новым настройкам, со старым снимком не отправляется.
        let fresh = connection(&vault).await;
        let moved = begin_turn(&mut vault, &path, "h3", None, 1, 102, true, &fresh)
            .await
            .unwrap();
        assert_eq!(
            send_turn(&transport, "vh", &moved, Some(KEY), &verified)
                .await
                .unwrap_err(),
            AiFailure::EndpointNotApproved
        );
        // Начало адреса сверяется по границе: тот же хост с чужим путём не подходит.
        assert!(!verified.covers(AiProvider::Custom, "https://models.example.org/v1evil/x"));
    }
    assert_eq!(transport.count(), 0);
    assert!(verified.covers(AiProvider::Custom, &first.request.url));
}

/// M1: запись урока не идёт сквозь ссылки, свободное имя — по существованию.
pub async fn lesson_writes_never_go_through_symlinks() {
    let config = settings(AiProvider::Anthropic, "claude-opus-5-5");
    let initial = {
        let mut seed = vault_with(&config).await;
        let path = new_lesson(&mut seed).await;
        String::from_utf8(seed.bytes_of(&path).unwrap()).unwrap()
    };

    // Сам файл урока — ссылка: дописывание отказывает, файл не тронут.
    let mut vault = MemoryVault::new()
        .with_file("Уроки/ссылка.lesson", initial.clone())
        .with_symlink("Уроки/ссылка.lesson");
    save_settings(&mut vault, &config).await.unwrap();
    assert!(
        set_state(&mut vault, "Уроки/ссылка.lesson", LessonState::Finished, 5)
            .await
            .is_err()
    );
    assert_eq!(
        vault.bytes_of("Уроки/ссылка.lesson").unwrap(),
        initial.as_bytes()
    );
    assert_eq!(vault.write_count("Уроки/ссылка.lesson"), 0);

    // Папка по пути — ссылка: ни урок, ни дописывание.
    let mut vault = MemoryVault::new()
        .with_file("Уроки/а.lesson", initial.clone())
        .with_symlink("Уроки");
    save_settings(&mut vault, &config).await.unwrap();
    assert!(set_language(&mut vault, "Уроки/а.lesson", Language::En, 5)
        .await
        .is_err());
    assert_eq!(vault.write_count("Уроки/а.lesson"), 0);
    assert!(create_lesson(&mut vault, "Новый", &[], Language::Ru, 1)
        .await
        .is_err());
    assert!(vault.paths_under("Уроки/Новый").is_empty());

    // Имя занято всем, что существует: висячая ссылка и файл, который не читается.
    let mut vault = MemoryVault::new()
        .with_symlink("Уроки/Цель.lesson")
        .with_file("Уроки/Цель 2.lesson", "x")
        .with_failure(
            "Уроки/Цель 2.lesson",
            typsidian_core::vault::backend::ReadError::Io,
        );
    save_settings(&mut vault, &config).await.unwrap();
    let path = create_lesson(&mut vault, "Цель", &[], Language::Ru, 1)
        .await
        .unwrap();
    assert_eq!(path, "Уроки/Цель 3.lesson");
    assert_eq!(vault.write_count("Уроки/Цель.lesson"), 0);
}

/// M2: настройки приходят с других устройств — исправляются и при чтении.
pub async fn lesson_settings_are_sanitized_on_read() {
    use typsidian_core::ai::settings::{
        load_settings, CONTEXT_BUDGET_MAX, CONTEXT_BUDGET_MIN, DEFAULT_CONTEXT_BUDGET,
        DEFAULT_LESSONS_FOLDER,
    };
    let read = |folder: &str, budget: u64| {
        MemoryVault::new().with_file(
            ".typsidian/ai.json",
            json!({"enabled": true, "lessons_folder": folder, "context_budget": budget})
                .to_string(),
        )
    };
    for folder in [
        ".typsidian/x",
        ".Typsidian",
        ".typsidian./уроки",
        "../наружу",
        "/абсолютный",
        "   ",
        "",
    ] {
        let loaded = load_settings(&read(folder, 50_000)).await;
        assert_eq!(loaded.lessons_folder, DEFAULT_LESSONS_FOLDER, "{folder:?}");
    }
    assert_eq!(
        load_settings(&read("Учёба/./Языки", 50_000))
            .await
            .lessons_folder,
        "Учёба/Языки"
    );
    for (given, expected) in [
        (0, DEFAULT_CONTEXT_BUDGET),
        (1, CONTEXT_BUDGET_MIN),
        (u64::from(u32::MAX), CONTEXT_BUDGET_MAX),
        (50_000, 50_000),
    ] {
        assert_eq!(
            load_settings(&read("Уроки", given)).await.context_budget,
            expected
        );
    }
    // Служебная папка не принимает урок, даже если файл настроек обошёл сохранение.
    let mut vault = read(".typsidian/x", 50_000).with_file(
        ".typsidian/ai.json",
        json!({"enabled": true, "model": "m", "lessons_folder": ".typsidian/x"}).to_string(),
    );
    let path = create_lesson(&mut vault, "Цель", &[], Language::Ru, 1)
        .await
        .unwrap();
    assert_eq!(path, "Уроки/Цель.lesson");
}

/// M3: модели отдаются только заметки, и только небольшие; файл урока тоже с пределом.
pub async fn only_small_notes_are_read_and_lesson_files_have_a_limit() {
    use typsidian_core::vault::backend::ReadError;
    let mut vault = vault_with(&settings(AiProvider::Anthropic, "claude-opus-5-5"))
        .await
        .with_file("Заметки/а.typ", "= Тема")
        .with_file("Заметки/картинка.png", "СЕКРЕТНЫЕ-БАЙТЫ")
        .with_file("Заметки/данные.csv", "СЕКРЕТНЫЕ-ДАННЫЕ")
        .with_file("Заметки/чужой.lesson", "СЕКРЕТНЫЙ-УРОК")
        .with_file("Заметки/огромная.typ", "ОГРОМНАЯ")
        .with_failure("Заметки/огромная.typ", ReadError::TooLarge)
        .with_file("Уроки/большой.lesson", "x")
        .with_failure("Уроки/большой.lesson", ReadError::TooLarge);
    let notes = [
        "Заметки/а.typ",
        "Заметки/картинка.png",
        "Заметки/данные.csv",
        "Заметки/чужой.lesson",
        "Заметки/огромная.typ",
    ]
    .map(str::to_owned);
    let path = create_lesson(&mut vault, "Цель", &notes, Language::Ru, 10)
        .await
        .unwrap();
    let view = open_lesson(&mut vault, &path).await.unwrap();
    let outcomes: Vec<NoteOutcome> = view
        .events
        .iter()
        .map(|event| match event {
            LessonEvent::NoteRead { outcome, .. } => *outcome,
            _ => panic!(),
        })
        .collect();
    assert!(matches!(outcomes[0], NoteOutcome::Sent { .. }));
    assert!(outcomes[1..].iter().all(|o| *o == NoteOutcome::Denied));

    // Учитель просит прочитать то же самое инструментом: ответ тот же, текста нет.
    let transport = StreamTransport::with(vec![read_note("", "Заметки/картинка.png"), say("ок")]);
    cycle(&mut vault, &transport, &path, "a", None, view.events_len)
        .await
        .unwrap();
    assert!(!transport.body(1).contains("СЕКРЕТНЫЕ-БАЙТЫ"));

    // Слишком большой файл урока не открывается и в список не попадает.
    assert_eq!(
        open_lesson(&mut vault, "Уроки/большой.lesson")
            .await
            .unwrap_err(),
        AiFailure::LessonUnreadable
    );
    assert!(list_lessons(&vault)
        .await
        .iter()
        .all(|lesson| lesson.path != "Уроки/большой.lesson"));
}

/// M4: предел числа и объёма, нормализация путей, `by_user` только из начальных событий.
pub async fn attachments_are_limited_normalized_and_initial_only() {
    use typsidian_core::ai::lesson::file::{initial_text, LessonHeader};
    use typsidian_core::ai::lesson::turn::ATTACHED_TOTAL_CHARS;
    use typsidian_core::ai::lesson::MAX_ATTACHED_NOTES;

    let mut vault = vault_with(&settings(AiProvider::Anthropic, "claude-opus-5-5")).await;
    // Двенадцать заметок: прикрепляется десять; одна и та же в разных написаниях — одна.
    let mut notes: Vec<String> = Vec::new();
    for index in 0..12 {
        vault = vault.with_file(
            &format!("Н/{index}.typ"),
            format!("МАРКЕР-{index} {}", "а".repeat(100_000)),
        );
        notes.push(format!("Н/{index}.typ"));
    }
    notes.insert(1, "./Н/0.typ".to_owned());
    notes.insert(2, "Н//0.typ".to_owned());
    let path = create_lesson(&mut vault, "Цель", &notes, Language::Ru, 10)
        .await
        .unwrap();
    let view = open_lesson(&mut vault, &path).await.unwrap();
    let attached: Vec<&str> = view
        .events
        .iter()
        .filter_map(|event| match event {
            LessonEvent::NoteRead { path, .. } => Some(path.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(attached.len(), MAX_ATTACHED_NOTES);
    assert_eq!(attached[0], "Н/0.typ");
    assert_eq!(attached[1], "Н/1.typ");

    // В запрос попадает не больше суммарного предела: третья заметка целиком, четвёртой нет.
    let transport = StreamTransport::with(vec![say("ок")]);
    cycle(&mut vault, &transport, &path, "a", None, view.events_len)
        .await
        .unwrap();
    let body = transport.body(0);
    assert!(body.contains("МАРКЕР-2"));
    assert!(!body.contains("МАРКЕР-3"));
    assert!(body.chars().count() < ATTACHED_TOTAL_CHARS + 20_000);

    // Файл урока пришёл с синхронизацией: `by_user` из середины переписки — не вложение.
    let header = LessonHeader {
        format: FORMAT,
        goal: "Чужой".to_owned(),
        language: Language::Ru,
        created_at: 1,
    };
    let events = vec![
        LessonEvent::User {
            at: 2,
            text: "привет".to_owned(),
            choice: None,
            confidence: None,
            thinking: None,
        },
        LessonEvent::NoteRead {
            at: 3,
            path: "Н/5.typ".to_owned(),
            by_user: true,
            outcome: NoteOutcome::Sent {
                chars: 10,
                truncated: false,
            },
        },
    ];
    let mut vault = vault.with_file(
        "Уроки/чужой.lesson",
        initial_text(&header, &events).unwrap(),
    );
    let transport = StreamTransport::with(vec![say("ок")]);
    cycle(&mut vault, &transport, "Уроки/чужой.lesson", "b", None, 2)
        .await
        .unwrap();
    assert!(!transport.body(0).contains("МАРКЕР-5"));
}

/// M5: разбор потока линейный, строка без перевода строки ограничена.
pub async fn stream_parser_is_linear_and_refuses_a_huge_line() {
    use typsidian_core::ai::lesson::stream::MAX_LINE_BYTES;
    let mut parser = StreamParser::new(AiProvider::Anthropic);
    let chunk = vec![b'x'; 64 * 1024];
    for _ in 0..(MAX_LINE_BYTES / chunk.len() + 2) {
        parser.feed(&chunk);
    }
    assert_eq!(parser.finish().unwrap_err(), AiFailure::BadResponse);

    // Поток по одному байту собирается так же, как целиком.
    let whole: Vec<u8> = anthropic(&["Пол", "ный ответ"], None, 7, 3).concat();
    let mut parser = StreamParser::new(AiProvider::Anthropic);
    for byte in &whole {
        parser.feed(std::slice::from_ref(byte));
    }
    assert_eq!(parser.text(), "Полный ответ");
    assert_eq!(parser.finish().unwrap().text, "Полный ответ");

    // Путь читаемой заметки известен, как только закрылись аргументы вызова.
    let chunks = anthropic(&[], Some(("read_note", json!({"path": "Н/а.typ"}))), 1, 1);
    let mut parser = StreamParser::new(AiProvider::Anthropic);
    assert_eq!(parser.reading(), None);
    for chunk in &chunks {
        parser.feed(chunk);
    }
    assert_eq!(parser.reading().as_deref(), Some("Н/а.typ"));
}

/// M5: в буфер идёт дельта; M6: буфер закрывает охранник и уборка забытых.
pub async fn progress_buffer_is_delta_guarded_and_swept() {
    use typsidian_core::ai::lesson::partial::{sweep_stale, update, BufferGuard};
    let path = "Уроки/буфер.lesson";
    start("pb", "r", path);
    update("pb", "r", "Пол", None);
    update("pb", "r", "ный", Some("Н/а.typ".to_owned()));
    update("pb", "r", " ответ", None);
    let seen = progress("pb", "r").unwrap();
    assert_eq!(seen.text, "Полный ответ");
    assert_eq!(seen.reading.as_deref(), Some("Н/а.typ"));

    // Сброс охранника (оборвали соединение) снимает буфер: урок больше не «идёт».
    assert!(is_active(path));
    drop(BufferGuard::new("pb", "r"));
    assert!(progress("pb", "r").is_none());
    assert!(!is_active(path));

    // Забытый буфер убирается по сроку.
    start("pb", "forgotten", path);
    assert!(is_active(path));
    sweep_stale(typsidian_core::ai::lesson::partial::BUFFER_TTL_MS);
    assert!(progress("pb", "forgotten").is_some());
    sweep_stale(0);
    assert!(progress("pb", "forgotten").is_none());
}

/// M6, L1: брошенное ожидание освобождает слот отмены; повторный запрос с тем же id
/// не портит буфер идущего.
pub async fn dropped_request_frees_its_slot_and_a_duplicate_keeps_the_buffer() {
    use typsidian_core::ai::cancel::cancellable;
    let mut pending = Box::pin(cancellable("vs", "slot", async {
        futures_util::future::pending::<Result<u8, AiFailure>>().await
    }));
    assert!(futures_util::poll!(pending.as_mut()).is_pending());
    // Пока слот занят, второй запрос с тем же id отказывает, не выполняя работу.
    assert_eq!(
        cancellable("vs", "slot", async { Ok(1u8) }).await,
        Err(AiFailure::Cancelled)
    );
    drop(pending);
    assert_eq!(cancellable("vs", "slot", async { Ok(1u8) }).await, Ok(1));

    // L1: идущий ход держит буфер, дубликат с тем же request_id его не трогает.
    let mut vault = anthropic_vault().await;
    let path = new_lesson(&mut vault).await;
    let hang = StreamTransport::with(vec![Script::Hang {
        chunks: anthropic(&["Идёт"], None, 1, 1)[..3].to_vec(),
    }]);
    let prepared = begin_t(&mut vault, &path, "dup", answer("раз"), 0, 100)
        .await
        .unwrap();
    let first = send_t(&hang, "vd", &prepared, Some(KEY));
    futures_util::pin_mut!(first);
    assert!(futures_util::poll!(&mut first).is_pending());
    assert_eq!(progress("vd", "dup").unwrap().text, "Идёт");

    let second = StreamTransport::with(vec![say("чужой")]);
    assert_eq!(
        send_t(&second, "vd", &prepared, Some(KEY)).await,
        Err(AiFailure::Cancelled)
    );
    assert_eq!(second.count(), 0);
    assert_eq!(progress("vd", "dup").unwrap().text, "Идёт");
    cancel("vd", "dup");
    assert_eq!(first.await, Err(AiFailure::Cancelled));
}

/// L2: отмена между ходами цикла не теряется.
pub async fn cancel_between_turns_is_remembered_for_the_whole_cycle() {
    use typsidian_core::ai::cancel::Cycle;
    let cycle_guard = Cycle::open("vy", "c1").unwrap();
    // Второй цикл с тем же id не открывается.
    assert!(Cycle::open("vy", "c1").is_none());
    assert!(!cycle_guard.is_cancelled());
    // Сетевого запроса сейчас нет — отмена всё равно запоминается.
    cancel("vy", "c1");
    assert!(cycle_guard.is_cancelled());
    // Чужой цикл отмена не задевает.
    let other = Cycle::open("vy", "c2").unwrap();
    assert!(!other.is_cancelled());
    drop(cycle_guard);
    let again = Cycle::open("vy", "c1").unwrap();
    assert!(!again.is_cancelled());
}

/// L3: `checkpoint` пишет по пути из буфера хода, пути приводятся к одной форме.
pub async fn checkpoint_takes_the_path_from_the_turn_buffer() {
    use typsidian_core::ai::lesson::partial::update;
    let mut vault = anthropic_vault().await;
    let settings_before = vault.bytes_of(".typsidian/ai.json").unwrap();
    let mine = "Уроки/моё.lesson";
    let other = "Уроки/чужое.lesson";
    start("pk", "r", "Уроки/./моё.lesson");
    update("pk", "r", "Текст хода", None);

    // Клиент назвал чужой путь: ничего не пишется ни по нему, ни по настоящему.
    checkpoint(&mut vault, "pk", other, "r").await.unwrap();
    checkpoint(&mut vault, "pk", ".typsidian/ai.json", "r")
        .await
        .unwrap();
    assert!(vault
        .bytes_of(&partial_path(other).to_string_lossy())
        .is_none());
    assert!(vault
        .bytes_of(&partial_path(mine).to_string_lossy())
        .is_none());
    assert_eq!(
        vault.bytes_of(".typsidian/ai.json").unwrap(),
        settings_before
    );

    // Тот же урок в другом написании пути: файл пишется, и по обеим формам он один.
    checkpoint(&mut vault, "pk", "Уроки//моё.lesson", "r")
        .await
        .unwrap();
    assert_eq!(partial_path("Уроки/./моё.lesson"), partial_path(mine));
    assert_eq!(
        vault
            .bytes_of(&partial_path(mine).to_string_lossy())
            .unwrap(),
        "Текст хода".as_bytes()
    );
    assert!(is_active(mine) && is_active("Уроки/./моё.lesson"));
    typsidian_core::ai::lesson::partial::take("pk", "r");
}

/// L6: команды урока принимают только файлы вида `Lesson` вне служебного места.
pub async fn lesson_commands_accept_only_lesson_files() {
    let mut vault = vault_with(&settings(AiProvider::Anthropic, "claude-opus-5-5"))
        .await
        .with_file("Заметки/а.typ", "= Заметка")
        .with_file(".typsidian/x.lesson", "{}");
    let before = vault.bytes_of(".typsidian/ai.json").unwrap();
    for bad in [
        "Заметки/а.typ",
        ".typsidian/ai.json",
        ".typsidian/ai-journal.jsonl",
        ".typsidian/x.lesson",
        "../а.lesson",
        "/etc/passwd",
        "",
    ] {
        assert_eq!(
            open_lesson(&mut vault, bad).await.unwrap_err(),
            AiFailure::LessonUnreadable,
            "{bad}"
        );
        assert!(set_state(&mut vault, bad, LessonState::Finished, 1)
            .await
            .is_err());
        assert!(set_language(&mut vault, bad, Language::En, 1)
            .await
            .is_err());
        assert!(
            begin_t(&mut vault, bad, "r", answer("текст"), 0, 1)
                .await
                .is_err(),
            "{bad}"
        );
    }
    assert_eq!(vault.bytes_of(".typsidian/ai.json").unwrap(), before);
    assert_eq!(
        vault.bytes_of("Заметки/а.typ").unwrap(),
        "= Заметка".as_bytes()
    );
}

/// L7: имя из цели без зарезервированных имён Windows и символов форматирования.
pub async fn goal_name_avoids_reserved_names_and_format_characters() {
    let mut vault = anthropic_vault().await;
    let mut names: Vec<String> = Vec::new();
    for goal in [
        "CON",
        "nul",
        "Com1",
        "LPT9",
        "aux.",
        "PRN итоги",
        "\u{202E}gnp.рус",
        "\u{200B}\u{FEFF}Цель\u{2066}",
        "COM10",
    ] {
        let path = create_lesson(&mut vault, goal, &[], Language::Ru, 1)
            .await
            .unwrap();
        names.push(path.trim_start_matches("Уроки/").to_owned());
    }
    assert_eq!(
        names,
        [
            "_CON.lesson",
            "_nul.lesson",
            "_Com1.lesson",
            "_LPT9.lesson",
            "_aux.lesson",
            "PRN итоги.lesson",
            "gnp.рус.lesson",
            "Цель.lesson",
            "COM10.lesson",
        ]
    );
    assert!(names.iter().all(|name| !name.contains('\u{202E}')));
}

/// L8: у вопроса до пяти вариантов, длина вопроса и вариантов ограничена.
pub async fn question_is_limited_in_choices_and_length() {
    let mut vault = anthropic_vault().await;
    let path = new_lesson(&mut vault).await;
    let many: Vec<String> = (0..9).map(|i| format!("вариант {i}")).collect();
    let long_question = "в".repeat(5_000);
    let long_choice = "о".repeat(2_000);
    let mut choices: Vec<&str> = many.iter().map(String::as_str).collect();
    choices[0] = &long_choice;
    let transport = StreamTransport::with(vec![ask("", &long_question, Some(choices))]);
    let done = cycle(&mut vault, &transport, &path, "a", None, 0)
        .await
        .unwrap();
    let question = done.view.last_question.unwrap();
    assert_eq!(question.question.chars().count(), 1_000);
    let choices = question.choices.unwrap();
    assert_eq!(choices.len(), 5);
    assert_eq!(choices[0].chars().count(), 300);
    assert_eq!(choices[4], "вариант 4");
}

/// L9: история версий Syncthing закрыта для агента, как служебное место.
pub async fn versions_folder_is_closed_to_the_agent() {
    use typsidian_core::ai::access::check_access;
    let config = settings(AiProvider::Anthropic, "claude-opus-5-5");
    for path in [
        ".stversions/Личное/секрет~20260101-120000.typ",
        ".StVersions/а.typ",
        ".stversions./а.typ",
        ".typsidian/ai.json",
    ] {
        assert!(!check_access(path, &config), "{path}");
    }
    assert!(check_access("Заметки/.stversions-заметка.typ", &config));
}

/// L4 (T045): правило `.stignore` для промежуточных файлов урока есть в обязательных.
pub async fn stignore_report_includes_the_lesson_partial_folder() {
    use typsidian_core::sync::ignore::{report, IgnoreFile};
    for file in [IgnoreFile::StIgnore, IgnoreFile::GitIgnore] {
        let report = report(file, None);
        assert!(
            report
                .missing
                .iter()
                .any(|rule| rule == "/.typsidian/lesson-partial"),
            "{file:?}"
        );
        assert!(report.proposed_block.contains("/.typsidian/lesson-partial"));
    }
}

/// T046: в сводке списка видно, чего ждёт урок.
pub async fn summary_tells_what_the_lesson_waits_for() {
    let mut vault = anthropic_vault().await;
    let start_only = new_lesson(&mut vault).await;
    let answered = create_lesson(&mut vault, "Ждёт ответа", &[], Language::Ru, 20)
        .await
        .unwrap();
    let cut = create_lesson(&mut vault, "Оборван", &[], Language::Ru, 30)
        .await
        .unwrap();
    let finished = create_lesson(&mut vault, "Завершён", &[], Language::Ru, 40)
        .await
        .unwrap();
    let stopped = create_lesson(&mut vault, "Остановлен", &[], Language::Ru, 50)
        .await
        .unwrap();
    let transport = StreamTransport::with(vec![say("реплика")]);
    cycle(&mut vault, &transport, &answered, "a", answer("привет"), 0)
        .await
        .unwrap();
    // Ответ пользователя сохранён, ответа учителя нет: «ответ оборван».
    let bad = StreamTransport::with(vec![Script::Cut {
        chunks: vec![],
        failure: AiFailure::Offline,
    }]);
    assert!(cycle(&mut vault, &bad, &cut, "b", answer("вопрос"), 0)
        .await
        .is_err());
    set_state(&mut vault, &finished, LessonState::Finished, 60)
        .await
        .unwrap();
    set_state(&mut vault, &stopped, LessonState::Stopped, 70)
        .await
        .unwrap();

    let lessons = list_lessons(&vault).await;
    let waiting = |path: &str| lessons.iter().find(|l| l.path == path).unwrap().waiting;
    assert_eq!(waiting(&start_only), Waiting::Start);
    assert_eq!(waiting(&answered), Waiting::User);
    assert_eq!(waiting(&cut), Waiting::Retry);
    assert_eq!(waiting(&finished), Waiting::Closed);
    assert_eq!(waiting(&stopped), Waiting::Closed);
    let stopped_summary = lessons.iter().find(|l| l.path == stopped).unwrap();
    assert_eq!(stopped_summary.state, LessonState::Stopped);
}
