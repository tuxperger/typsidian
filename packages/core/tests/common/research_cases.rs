//! Фоновое исследование в уроке (фича 056) — общая таблица для обоих таргетов. Сети нет:
//! поддельный потоковый транспорт урока отдаёт заготовленные потоки провайдеров.

#![allow(dead_code)]

use serde_json::{json, Value};
use typsidian_core::ai::cancel::cancel;
use typsidian_core::ai::jobs::{all_jobs, AnyJob};
use typsidian_core::ai::journal::{journal_entries, JournalState};
use typsidian_core::ai::lesson::file::{initial_text, FORMAT};
use typsidian_core::ai::lesson::{
    open_lesson, Language, LessonEvent, LessonHeader, LessonKind, LessonPhase, LessonView, Next,
    Waiting,
};
use typsidian_core::ai::research::note::{build_note, escape_markup, NoteInput};
use typsidian_core::ai::research::reply::{clean_url, parse_submit, ResearchParser};
use typsidian_core::ai::research::{
    begin_research, finish_research, research_estimate, research_jobs, send_research,
    web_search_available, PreparedResearch, ResearchBegin, ResearchKind, ResearchSource,
    ResearchStatus, ResearchView,
};
use typsidian_core::ai::settings::{load_settings, save_settings, AiProvider, AiSettings};
use typsidian_core::ai::AiFailure;
use typsidian_core::preview::{compile_note_resolved, CompileOutcome, FontSet, PackageCache};

use super::lesson_cases::{
    answer, anthropic, compatible, connection, cycle, event, new_lesson, reply, say, settings,
    unique, vault_with, Script, StreamTransport, JOURNAL, KEY,
};
use super::MemoryVault;

const TEXT_PDF: &[u8] = include_bytes!("../fixtures/pdf/text.pdf");
const SCAN_PDF: &[u8] = include_bytes!("../fixtures/pdf/no-text.pdf");
const NOTES: &str = "Уроки/Исследования";
const QUESTION: &str = "Как работает сборка мусора в Rust?";

// ── Заготовки потоков ───────────────────────────────────────────────────────

/// Что отдаёт провайдер в одном ответе с поиском.
struct Web<'a> {
    results: Vec<(&'a str, &'a str)>,
    /// Код ошибки поиска вместо результатов (ответ при этом с кодом 200).
    error: Option<&'a str>,
    text: &'a str,
    citations: Vec<(&'a str, &'a str)>,
    submit: Option<Value>,
    stop: &'a str,
    searches: u64,
    input: u64,
    output: u64,
}

impl<'a> Web<'a> {
    fn new() -> Self {
        Web {
            results: Vec::new(),
            error: None,
            text: "",
            citations: Vec::new(),
            submit: None,
            stop: "end_turn",
            searches: 1,
            input: 1000,
            output: 500,
        }
    }
}

fn web_chunks(w: &Web<'_>) -> Vec<Vec<u8>> {
    let mut chunks = vec![event(
        "message_start",
        json!({"type":"message_start","message":{"usage":{"input_tokens":w.input,"output_tokens":1}}}),
    )];
    let mut index = 0u64;
    // Серверный вызов поиска и его результат.
    chunks.push(event(
        "content_block_start",
        json!({"type":"content_block_start","index":index,
            "content_block":{"type":"server_tool_use","id":"srvtoolu_1","name":"web_search","input":{}}}),
    ));
    chunks.push(event(
        "content_block_delta",
        json!({"type":"content_block_delta","index":index,
            "delta":{"type":"input_json_delta","partial_json":"{\"query\": \"сборка"}}),
    ));
    chunks.push(event(
        "content_block_delta",
        json!({"type":"content_block_delta","index":index,
            "delta":{"type":"input_json_delta","partial_json":" мусора\"}"}}),
    ));
    chunks.push(event(
        "content_block_stop",
        json!({"type":"content_block_stop","index":index}),
    ));
    index += 1;
    let content = match w.error {
        Some(code) => json!({"type":"web_search_tool_result_error","error_code":code}),
        None => Value::Array(
            w.results
                .iter()
                .map(|(title, url)| {
                    json!({"type":"web_search_result","url":url,"title":title,
                        "encrypted_content":"EiYK...","page_age":"May 2026"})
                })
                .collect(),
        ),
    };
    chunks.push(event(
        "content_block_start",
        json!({"type":"content_block_start","index":index,
            "content_block":{"type":"web_search_tool_result","tool_use_id":"srvtoolu_1","content":content}}),
    ));
    chunks.push(event(
        "content_block_stop",
        json!({"type":"content_block_stop","index":index}),
    ));
    index += 1;
    if !w.text.is_empty() {
        chunks.push(event(
            "content_block_start",
            json!({"type":"content_block_start","index":index,
                "content_block":{"type":"text","text":""}}),
        ));
        chunks.push(event(
            "content_block_delta",
            json!({"type":"content_block_delta","index":index,
                "delta":{"type":"text_delta","text":w.text}}),
        ));
        for (title, url) in &w.citations {
            chunks.push(event(
                "content_block_delta",
                json!({"type":"content_block_delta","index":index,
                    "delta":{"type":"citations_delta","citation":{
                        "type":"web_search_result_location","url":url,"title":title,
                        "encrypted_index":"Eo8B...","cited_text":"..."}}}),
            ));
        }
        chunks.push(event(
            "content_block_stop",
            json!({"type":"content_block_stop","index":index}),
        ));
        index += 1;
    }
    if let Some(arguments) = &w.submit {
        chunks.push(event(
            "content_block_start",
            json!({"type":"content_block_start","index":index,
                "content_block":{"type":"tool_use","id":"toolu_1","name":"submit_research","input":{}}}),
        ));
        let raw = arguments.to_string();
        let middle = (0..=raw.len() / 2)
            .rev()
            .find(|i| raw.is_char_boundary(*i))
            .unwrap_or(0);
        for piece in [&raw[..middle], &raw[middle..]] {
            chunks.push(event(
                "content_block_delta",
                json!({"type":"content_block_delta","index":index,
                    "delta":{"type":"input_json_delta","partial_json":piece}}),
            ));
        }
        chunks.push(event(
            "content_block_stop",
            json!({"type":"content_block_stop","index":index}),
        ));
    }
    chunks.push(event(
        "message_delta",
        json!({"type":"message_delta","delta":{"stop_reason":w.stop},
            "usage":{"output_tokens":w.output,"server_tool_use":{"web_search_requests":w.searches}}}),
    ));
    chunks.push(event("message_stop", json!({"type":"message_stop"})));
    chunks
}

fn web(w: Web<'_>) -> Script {
    reply(web_chunks(&w))
}

fn answer_of(paragraphs: &[&str], sources: Value) -> Value {
    json!({"paragraphs": paragraphs, "sources": sources})
}

/// Обычный успешный ответ: поиск, цитата и сданный результат.
fn good() -> Script {
    web(Web {
        results: vec![("Книга Rust", "https://doc.rust-lang.org/book/ch04-00.html")],
        text: "Память освобождается по владению.",
        citations: vec![("Книга Rust", "https://doc.rust-lang.org/book/ch04-00.html")],
        submit: Some(answer_of(
            &[
                "В Rust нет сборщика мусора.",
                "Память освобождается, когда владелец выходит из области.",
            ],
            json!([{"title": "Rust Book", "url": "https://doc.rust-lang.org/book/ch04-00.html"}]),
        )),
        ..Web::new()
    })
}

/// Ответ API ответов OpenAI: поиск, цитата и вызов `submit_research` в одном событии.
fn responses(text: &str, url: &str, submit: Option<Value>) -> Script {
    let mut output = vec![
        json!({"type":"web_search_call","id":"ws_1","status":"completed",
            "action":{"type":"search","queries":["rust gc"]}}),
        json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":text,
            "annotations":[{"type":"url_citation","url":url,"title":"Источник","start_index":0,"end_index":4}]}]}),
    ];
    if let Some(arguments) = submit {
        output.push(
            json!({"type":"function_call","call_id":"call_1","name":"submit_research",
            "arguments": arguments.to_string()}),
        );
    }
    let delta = format!(
        "data: {}\n\n",
        json!({"type":"response.output_text.delta","delta":text})
    );
    let done = format!(
        "data: {}\n\n",
        json!({"type":"response.completed","response":{"id":"resp_1","status":"completed",
            "output":output,"usage":{"input_tokens":900,"output_tokens":300}}})
    );
    reply(vec![delta.into_bytes(), done.into_bytes()])
}

// ── Окружение ───────────────────────────────────────────────────────────────

fn web_settings(provider: AiProvider, model: &str) -> AiSettings {
    AiSettings {
        web_research: true,
        ..settings(provider, model)
    }
}

async fn web_vault() -> MemoryVault {
    vault_with(&web_settings(AiProvider::Anthropic, "claude-opus-5-5")).await
}

fn custom_settings(web_research: bool) -> AiSettings {
    AiSettings {
        enabled: true,
        provider: AiProvider::Custom,
        custom_endpoint: Some("https://llm.example.org/v1".to_owned()),
        model: Some("своя".to_owned()),
        web_research,
        ..AiSettings::default()
    }
}

/// Цикл исследования так, как его ведут host и клиент wasm: ядро решает, продолжать ли.
async fn run(
    vault: &mut MemoryVault,
    transport: &StreamTransport,
    vault_id: &str,
    path: &str,
    id: &str,
    kind: ResearchKind,
    question: &str,
    pdf: Option<&str>,
) -> Result<ResearchView, AiFailure> {
    let verified = connection(vault).await;
    let mut current: Option<PreparedResearch> = None;
    for _ in 0..12 {
        let prepared = match current.take() {
            Some(prepared) => prepared,
            None => match begin_research(
                vault, vault_id, path, id, kind, question, pdf, 1_000, true, &verified,
            )
            .await?
            {
                ResearchBegin::Ready { prepared } => prepared,
                ResearchBegin::Queued => return Err(AiFailure::RateLimited),
                ResearchBegin::Finished { research } => return Ok(research),
            },
        };
        let outcome = send_research(transport, vault_id, &prepared, Some(KEY), &verified).await;
        let done = finish_research(vault, vault_id, &prepared, 2_000, outcome).await?;
        if done.next == Next::Continue {
            current = done.prepared;
            continue;
        }
        return Ok(done.research);
    }
    panic!("исследование не остановилось");
}

async fn run_web(
    vault: &mut MemoryVault,
    transport: &StreamTransport,
    path: &str,
    id: &str,
    question: &str,
) -> Result<ResearchView, AiFailure> {
    let vault_id = unique("rv");
    run(
        vault,
        transport,
        &vault_id,
        path,
        id,
        ResearchKind::Web,
        question,
        None,
    )
    .await
}

fn tool_names(body: &str) -> Vec<String> {
    let body: Value = serde_json::from_str(body).unwrap();
    body["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool["name"]
                        .as_str()
                        .or(tool["function"]["name"].as_str())
                        .or(tool["type"].as_str())
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn note_text(vault: &MemoryVault, path: &str) -> String {
    String::from_utf8(vault.bytes_of(path).expect("заметка записана")).unwrap()
}

async fn view_of(vault: &mut MemoryVault, path: &str) -> LessonView {
    open_lesson(vault, path).await.unwrap()
}

fn events_of(view: &LessonView, name: &str) -> usize {
    view.events
        .iter()
        .filter(|event| match (name, event) {
            ("started", LessonEvent::ResearchStarted { .. }) => true,
            ("done", LessonEvent::ResearchDone { .. }) => true,
            ("failed", LessonEvent::ResearchFailed { .. }) => true,
            _ => false,
        })
        .count()
}

fn put_lesson(vault: &MemoryVault, path: &str, events: &[LessonEvent]) {
    let header = LessonHeader {
        format: FORMAT,
        goal: "Понять замыкания".to_owned(),
        language: Language::Ru,
        created_at: 1,
        start: LessonPhase::Teach,
        kind: LessonKind::Lesson,
    };
    vault.replace_externally(path, initial_text(&header, events).unwrap());
}

fn started(id: &str, by_user: bool) -> LessonEvent {
    LessonEvent::ResearchStarted {
        at: 5,
        id: id.to_owned(),
        research: ResearchKind::Web,
        question: "Вопрос".to_owned(),
        path: None,
        by_user,
    }
}

// ── T005: SC-003, поиск только у провайдера ─────────────────────────────────

/// Выключено по умолчанию: ни исследование, ни ход учителя не получают инструмента поиска;
/// свой адрес поиска не имеет и при включённой настройке.
pub async fn search_is_off_by_default_and_a_custom_endpoint_has_none() {
    // Настройка по умолчанию — выключено.
    let defaults = AiSettings::default();
    assert!(!defaults.web_research);
    assert_eq!(defaults.research_parallel, 2);
    assert!(!defaults.research_agent_pdf);

    let mut vault = vault_with(&settings(AiProvider::Anthropic, "claude-opus-5-5")).await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![]);
    let verified = connection(&vault).await;
    let failure = begin_research(
        &mut vault,
        &unique("rv"),
        &path,
        "r1",
        ResearchKind::Web,
        QUESTION,
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap_err();
    assert_eq!(failure, AiFailure::SearchDisabled);
    // Ни сети, ни журнала, ни записи в урок.
    assert_eq!(transport.count(), 0);
    assert!(journal_entries(&vault).await.is_empty());
    let view = view_of(&mut vault, &path).await;
    assert!(view.research.is_empty());
    assert_eq!(events_of(&view, "started"), 0);

    // Ход учителя при выключенном поиске: инструмента `research` нет, поиска в теле нет.
    let teacher = StreamTransport::with(vec![say("Начнём.")]);
    cycle(&mut vault, &teacher, &path, "t1", answer("привет"), 0)
        .await
        .unwrap();
    assert!(!tool_names(&teacher.body(0)).contains(&"research".to_owned()));
    assert!(!teacher.body(0).contains("web_search"));

    // Включили: учитель видит `research`, но поиск в его запрос всё равно не входит —
    // он живёт только в запросе исследования.
    let mut on = web_settings(AiProvider::Anthropic, "claude-opus-5-5");
    save_settings(&mut vault, &on).await.unwrap();
    assert!(web_search_available(&load_settings(&vault).await));
    let teacher = StreamTransport::with(vec![say("Дальше.")]);
    let current = view_of(&mut vault, &path).await.events_len;
    cycle(&mut vault, &teacher, &path, "t2", answer("ещё"), current)
        .await
        .unwrap();
    assert!(tool_names(&teacher.body(0)).contains(&"research".to_owned()));
    assert!(!teacher.body(0).contains("web_search"));

    // Выключили снова — инструмент исчез.
    on.web_research = false;
    save_settings(&mut vault, &on).await.unwrap();
    assert!(!web_search_available(&load_settings(&vault).await));
    let teacher = StreamTransport::with(vec![say("И так.")]);
    let current = view_of(&mut vault, &path).await.events_len;
    cycle(&mut vault, &teacher, &path, "t3", answer("и ещё"), current)
        .await
        .unwrap();
    assert!(!tool_names(&teacher.body(0)).contains(&"research".to_owned()));

    // Свой адрес: настройка включена, а поиска нет — ни исследования, ни инструмента.
    let mut custom = vault_with(&custom_settings(true)).await;
    assert!(!web_search_available(&load_settings(&custom).await));
    let path = new_lesson(&mut custom).await;
    let verified = connection(&custom).await;
    let failure = begin_research(
        &mut custom,
        &unique("rv"),
        &path,
        "c1",
        ResearchKind::Web,
        QUESTION,
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap_err();
    assert_eq!(failure, AiFailure::SearchUnavailable);
    assert!(journal_entries(&custom).await.is_empty());
    let teacher = StreamTransport::with(vec![reply(compatible(&["Привет"], None, (5, 5)))]);
    cycle(&mut custom, &teacher, &path, "t4", answer("привет"), 0)
        .await
        .unwrap();
    assert!(!tool_names(&teacher.body(0)).contains(&"research".to_owned()));
    assert!(!teacher.body(0).contains("web_search"));
}

/// Ни один запрос без включённого поиска не несёт инструмента поиска, даже если слово
/// «web_search» стоит в тексте пользователя; запрос, подделанный клиентом, не отправляется.
pub async fn a_request_without_search_never_carries_the_search_tool() {
    let mut vault = vault_with(&settings(AiProvider::Anthropic, "claude-opus-5-5"))
        .await
        .with_file("Книги/Текст.pdf", TEXT_PDF);
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![reply(anthropic(
        &["ok"],
        Some((
            "submit_research",
            answer_of(
                &["Ответ про web_search_20250305"],
                json!([{"title":"Текст","pages":[1]}]),
            ),
        )),
        100,
        50,
    ))]);
    let vault_id = unique("rv");
    let view = run(
        &mut vault,
        &transport,
        &vault_id,
        &path,
        "p1",
        ResearchKind::Pdf,
        "Что такое web_search?",
        Some("Книги/Текст.pdf"),
    )
    .await
    .unwrap();
    assert_eq!(view.status, ResearchStatus::Done);
    let body: Value = serde_json::from_str(&transport.body(0)).unwrap();
    assert!(body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|tool| !tool["type"]
            .as_str()
            .unwrap_or_default()
            .starts_with("web_search")
            && tool["name"] != "web_search"));
    assert_eq!(body["tool_choice"]["name"], "submit_research");

    // Подделанный запрос: поиск в теле при `web: false` не уходит в сеть.
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let verified = connection(&vault).await;
    let ResearchBegin::Ready { mut prepared } = begin_research(
        &mut vault,
        &unique("rv"),
        &path,
        "f1",
        ResearchKind::Web,
        QUESTION,
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap() else {
        panic!("запрос готов");
    };
    prepared.web = false;
    let silent = StreamTransport::with(vec![good()]);
    let outcome = send_research(&silent, &unique("rv"), &prepared, Some(KEY), &verified).await;
    assert_eq!(outcome.unwrap_err(), AiFailure::SearchUnavailable);
    assert_eq!(silent.count(), 0);
}

/// Поиск, выключенный посреди исследования, продолжения после `pause_turn` не получает.
pub async fn turning_search_off_mid_research_stops_the_continuation() {
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![web(Web {
        results: vec![("А", "https://a.example/")],
        text: "Промежуточно.",
        stop: "pause_turn",
        ..Web::new()
    })]);
    let vault_id = unique("rv");
    let verified = connection(&vault).await;
    let ResearchBegin::Ready { prepared } = begin_research(
        &mut vault,
        &vault_id,
        &path,
        "m1",
        ResearchKind::Web,
        QUESTION,
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap() else {
        panic!("запрос готов");
    };
    let outcome = send_research(&transport, &vault_id, &prepared, Some(KEY), &verified).await;
    save_settings(
        &mut vault,
        &settings(AiProvider::Anthropic, "claude-opus-5-5"),
    )
    .await
    .unwrap();
    let done = finish_research(&mut vault, &vault_id, &prepared, 2_000, outcome)
        .await
        .unwrap();
    assert_eq!(done.next, Next::WaitUser);
    assert_eq!(done.research.status, ResearchStatus::Failed);
    assert_eq!(done.research.failure, Some(AiFailure::SearchDisabled));
    // Больше ни одного запроса, в журнале только первый.
    assert_eq!(transport.count(), 1);
    assert_eq!(journal_entries(&vault).await.len(), 1);
}

// ── T003: веб-исследование ──────────────────────────────────────────────────

/// Полный цикл: запрос с поиском и `submit_research` в журнале до сети, заметка, события,
/// источник, стоимость с ценой поисков.
pub async fn web_research_ends_with_a_note_sources_and_cost() {
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![good()]);
    let vault_id = unique("rv");
    let verified = connection(&vault).await;

    let ResearchBegin::Ready { prepared } = begin_research(
        &mut vault,
        &vault_id,
        &path,
        "r1",
        ResearchKind::Web,
        QUESTION,
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap() else {
        panic!("запрос готов");
    };
    // Запрос уже в журнале, сети ещё не было.
    assert_eq!(transport.count(), 0);
    let entries = journal_entries(&vault).await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "r1#r1");
    assert_eq!(entries[0].state, JournalState::Interrupted);
    assert!(entries[0].request.contains(QUESTION));
    assert!(!entries[0].request.contains(KEY));
    // Форма запроса: поиск версии 20250305 с пределом и клиентский инструмент сдачи.
    let body: Value = serde_json::from_str(prepared.request.body.as_deref().unwrap()).unwrap();
    assert_eq!(body["tools"][0]["type"], "web_search_20250305");
    assert_eq!(body["tools"][0]["name"], "web_search");
    assert_eq!(body["tools"][0]["max_uses"], 5);
    assert_eq!(body["tools"][1]["name"], "submit_research");
    assert!(body.get("tool_choice").is_none());
    assert_eq!(body["stream"], true);
    assert!(prepared.request.url.ends_with("/messages"));
    // Вопрос уже записан в урок событием пользователя; идёт, а не прервано.
    let view = view_of(&mut vault, &path).await;
    assert_eq!(view.research.len(), 1);
    assert_eq!(view.research[0].status, ResearchStatus::Running);
    assert!(view.research[0].by_user);
    assert!(!view.research[0].launch_due);
    assert!(all_jobs(&vault_id)
        .iter()
        .any(|job| matches!(job, AnyJob::Research(job) if job.id == "r1")));

    let outcome = send_research(&transport, &vault_id, &prepared, Some(KEY), &verified).await;
    let done = finish_research(&mut vault, &vault_id, &prepared, 2_000, outcome)
        .await
        .unwrap();
    assert_eq!(done.next, Next::WaitUser);
    let research = done.research;
    assert_eq!(research.status, ResearchStatus::Done);
    // Источник один: цитата и названный моделью совпали по адресу.
    assert_eq!(research.sources.len(), 1);
    assert_eq!(
        research.sources[0].url.as_deref(),
        Some("https://doc.rust-lang.org/book/ch04-00.html")
    );
    assert_eq!(research.sources[0].title, "Книга Rust");
    let note = research.note.clone().unwrap();
    assert!(note.starts_with(&format!("{NOTES}/")), "{note}");
    assert!(note.ends_with(".typ"));
    let text = note_text(&vault, &note);
    assert!(text.contains("Найдено моделью"), "{text}");
    assert!(text.contains("#link(\"https://doc.rust-lang.org/book/ch04-00.html\")"));
    assert!(text.contains("В Rust нет сборщика мусора."));
    assert!(research.summary.unwrap().contains("владелец"));

    // Журнал: итог с токенами; цена = токены по справочнику + поиск по 0,01.
    let entries = journal_entries(&vault).await;
    assert_eq!(entries[0].state, JournalState::Done);
    let cost = entries[0].cost_usd.expect("цена известна");
    assert!((cost - (0.004 + 0.010 + 0.01)).abs() < 1e-9, "{cost}");
    // Урок: события в файле, стоимость урока включает исследование.
    let view = view_of(&mut vault, &path).await;
    assert_eq!(events_of(&view, "started"), 1);
    assert_eq!(events_of(&view, "done"), 1);
    assert!((view.cost_usd.unwrap() - cost).abs() < 1e-9);
    assert_eq!(view.research[0].status, ResearchStatus::Done);
    assert_eq!(view.waiting, Waiting::Start, "исследование урок не двигает");
    // Задание закончено, место свободно.
    let jobs = research_jobs(&vault_id);
    assert_eq!(jobs[0].note.as_deref(), Some(note.as_str()));
    assert!(jobs[0].searches == 1);
}

/// Текст модели не доверенный: разметка, код и метки в нём экранируются, заметка собирается
/// Typst без ошибок, а ссылки идут только через `#link("…")` по проверенным адресам.
pub async fn hostile_model_text_is_escaped_and_the_note_compiles() {
    let hostile = [
        "#panic(\"из абзаца\") и #import \"@preview/evil:0.1.0\": *",
        "$ x^2 $ и *жирный* и _курсив_ и `код` и <метка> и @ссылка",
        "= заголовок\n- пункт\n+ нумерация\n1. номер\n/ термин: определение",
        "обратная \\ косая, [скобки] и ]] [[ и ~ // комментарий /* блок */",
        "#read(\"/etc/passwd\") #system(\"rm\") #link(\"javascript:alert(1)\")[x]",
        "строка\u{202e}с управлением\u{0}и\u{2028}разделителем",
    ];
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![web(Web {
        results: vec![("Хост", "https://ok.example/путь?q=1&b=2")],
        text: "Текст.",
        citations: vec![(
            "#panic(\"title\") *заголовок* ]",
            "https://ok.example/путь?q=1&b=2",
        )],
        submit: Some(answer_of(
            &hostile,
            json!([
                {"title": "#panic(\"в названии\")\n= не заголовок", "url": "https://ok.example/a#panic"},
                {"title": "кавычка", "url": "https://ok.example/\"quote"},
                {"title": "javascript", "url": "javascript:alert(1)"},
                {"title": "пробел", "url": "https://ok.example/a b"},
                {"title": "без адреса"},
                {"title": "скобка", "url": "https://ok.example/<a>"},
                {"title": "слэш", "url": "https://ok.example/back\\slash"},
            ]),
        )),
        ..Web::new()
    })]);
    let question = "Что значит #panic(\"в вопросе\") и $ и * в Typst?";
    let research = run_web(&mut vault, &transport, &path, "h1", question)
        .await
        .unwrap();
    assert_eq!(research.status, ResearchStatus::Done);
    // Годные адреса остались, негодные отброшены.
    let urls: Vec<&str> = research
        .sources
        .iter()
        .filter_map(|source| source.url.as_deref())
        .collect();
    assert_eq!(
        urls,
        [
            "https://ok.example/путь?q=1&b=2",
            "https://ok.example/a#panic"
        ]
    );
    let note = research.note.unwrap();
    let text = note_text(&vault, &note);
    // Каждая решётка в заметке либо экранирована, либо открывает наш `#link(`.
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if *c != '#' {
            continue;
        }
        let escaped = i > 0 && chars[i - 1] == '\\';
        let rest: String = chars[i..].iter().take(6).collect();
        // Адрес внутри строки в кавычках — часть строки, а не разметка.
        let in_string = text[..text.char_indices().nth(i).unwrap().0]
            .rsplit('\n')
            .next()
            .unwrap()
            .contains("#link(\"");
        assert!(escaped || rest == "#link(" || in_string, "{rest} в {text}");
    }
    assert!(text.contains("\\#panic"));
    assert!(!text.contains('\u{202e}') && !text.contains('\u{0}') && !text.contains('\u{2028}'));
    // Typst собирает заметку без единого сообщения об ошибке.
    let doc = compile_note_resolved(
        &vault,
        &PackageCache::new(),
        "заметка.typ",
        &text,
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap();
    assert_eq!(
        doc.outcome,
        CompileOutcome::Succeeded,
        "{:?}\n{text}",
        doc.diagnostics
    );
    assert!(doc.diagnostics.is_empty(), "{:?}\n{text}", doc.diagnostics);
}

/// Экранирование — по таблице: спецсимволы получают косую черту, знаки списка — только в
/// начале строки, пробельные символы становятся пробелами.
pub async fn escaping_covers_markup_specials_and_line_starts() {
    assert_eq!(
        escape_markup("a#b$c*d_e`f<g>h@i[j]k~l/m\\n"),
        "a\\#b\\$c\\*d\\_e\\`f\\<g\\>h\\@i\\[j\\]k\\~l\\/m\\\\n"
    );
    assert_eq!(
        escape_markup("- a\n+ b\n= c\n1. d\n12.5"),
        "\\- a\n\\+ b\n\\= c\n1\\. d\n12\\.5"
    );
    assert_eq!(escape_markup("кое-что + 1 = 2"), "кое-что + 1 = 2");
    assert_eq!(escape_markup("a\tb\u{a0}c\r\nd"), "a b c\nd");
    assert_eq!(escape_markup("x\u{0}y\u{202e}z"), "xyz");
    assert_eq!(
        clean_url("https://a.example/x"),
        Some("https://a.example/x".into())
    );
    for bad in [
        "javascript:alert(1)",
        "ftp://a.example",
        "https://",
        "https:///etc",
        "http://a b",
        "https://a.example/\"",
        "https://a.example/<",
        "https://a.example/a\nb",
        &format!("https://a.example/{}", "a".repeat(600)),
    ] {
        assert_eq!(clean_url(bad), None, "{bad}");
    }
}

/// SC-002: у результата есть источник либо явная пометка, что источников нет.
pub async fn a_result_has_a_source_or_an_explicit_no_sources_mark() {
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    // Модель назвала только негодное, поиск ничего не процитировал.
    let transport = StreamTransport::with(vec![web(Web {
        results: Vec::new(),
        submit: Some(answer_of(
            &["Из памяти модели."],
            json!([{"title": "Без адреса"}]),
        )),
        searches: 0,
        ..Web::new()
    })]);
    let research = run_web(&mut vault, &transport, &path, "s1", "Вопрос без источников")
        .await
        .unwrap();
    assert_eq!(research.status, ResearchStatus::Done);
    assert!(research.sources.is_empty());
    let text = note_text(&vault, &research.note.unwrap());
    assert!(text.contains("Источников нет"), "{text}");
    assert!(text.contains("Найдено моделью"));
    // Данные для учителя тоже говорят «источников нет».
    let teacher = StreamTransport::with(vec![say("Принял.")]);
    let done = cycle(&mut vault, &teacher, &path, "t1", answer("ну?"), 2)
        .await
        .unwrap();
    assert_eq!(events_of(&done.view, "done"), 1);
    assert!(
        teacher.body(0).contains("Источников нет."),
        "{}",
        teacher.body(0)
    );

    // Поиск нашёл страницы, но модель ничего не процитировала и не назвала: берутся найденные.
    let transport = StreamTransport::with(vec![web(Web {
        results: vec![
            ("Первая", "https://one.example/"),
            ("Вторая", "https://two.example/"),
        ],
        text: "Ответ без цитат.",
        ..Web::new()
    })]);
    let research = run_web(&mut vault, &transport, &path, "s2", "Другой вопрос")
        .await
        .unwrap();
    // Свободный текст вместо `submit_research` — запасной путь: абзацы из текста.
    assert_eq!(research.status, ResearchStatus::Done);
    assert_eq!(research.sources.len(), 2);
    assert_eq!(research.sources[0].title, "Первая");
}

/// Ошибка поиска приходит внутри ответа с кодом 200: без результата это отказ с причиной,
/// с результатом — исследование идёт дальше.
pub async fn a_search_error_inside_a_200_reply_is_handled() {
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    for (code, expected) in [
        ("too_many_requests", AiFailure::RateLimited),
        ("unavailable", AiFailure::ProviderDown),
        ("max_uses_exceeded", AiFailure::BadResponse),
        ("query_too_long", AiFailure::BadResponse),
    ] {
        let transport = StreamTransport::with(vec![web(Web {
            error: Some(code),
            searches: 0,
            ..Web::new()
        })]);
        let research = run_web(
            &mut vault,
            &transport,
            &path,
            &format!("e-{code}"),
            &format!("в {code}"),
        )
        .await
        .unwrap();
        assert_eq!(research.status, ResearchStatus::Failed, "{code}");
        assert_eq!(research.failure, Some(expected), "{code}");
        assert!(research.note.is_none());
    }
    // Часть поисков упала, но модель всё же сдала результат.
    let transport = StreamTransport::with(vec![web(Web {
        error: Some("unavailable"),
        searches: 0,
        submit: Some(answer_of(&["Из памяти."], json!([]))),
        ..Web::new()
    })]);
    let research = run_web(
        &mut vault,
        &transport,
        &path,
        "e-ok",
        "Вопрос с ошибкой поиска",
    )
    .await
    .unwrap();
    assert_eq!(research.status, ResearchStatus::Done);
    let view = view_of(&mut vault, &path).await;
    assert_eq!(events_of(&view, "failed"), 4);
    // Отказ записан событием и журналом; текста сервера нет.
    let failed = view
        .events
        .iter()
        .filter_map(|event| match event {
            LessonEvent::ResearchFailed {
                failure, cancelled, ..
            } => Some((*failure, *cancelled)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(failed[0], (AiFailure::RateLimited, false));
}

/// `pause_turn`: ответ ассистента возвращается провайдеру, ограниченное число раз.
pub async fn pause_turn_continues_with_the_assistant_reply_and_is_bounded() {
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let pause = || {
        web(Web {
            results: vec![("А", "https://a.example/")],
            text: "Промежуточный текст.",
            citations: vec![("А", "https://a.example/")],
            stop: "pause_turn",
            ..Web::new()
        })
    };
    let transport = StreamTransport::with(vec![
        pause(),
        web(Web {
            results: vec![("Б", "https://b.example/")],
            submit: Some(answer_of(
                &["Итог после продолжения."],
                json!([{"title": "Б", "url": "https://b.example/"}]),
            )),
            ..Web::new()
        }),
    ]);
    let research = run_web(&mut vault, &transport, &path, "pt", QUESTION)
        .await
        .unwrap();
    assert_eq!(research.status, ResearchStatus::Done);
    assert_eq!(transport.count(), 2);
    // Второй запрос несёт первый ответ ассистента как есть.
    let second: Value = serde_json::from_str(&transport.body(1)).unwrap();
    let messages = second["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1]["role"], "assistant");
    let blocks = messages[1]["content"].as_array().unwrap();
    let kinds: Vec<&str> = blocks.iter().filter_map(|b| b["type"].as_str()).collect();
    assert_eq!(kinds, ["server_tool_use", "web_search_tool_result", "text"]);
    assert_eq!(blocks[0]["input"]["query"], "сборка мусора");
    assert_eq!(blocks[2]["citations"][0]["url"], "https://a.example/");
    assert_eq!(second["tools"][0]["type"], "web_search_20250305");
    // Оба запроса в журнале до сети; цена — сумма двух (токены и по поиску).
    let entries = journal_entries(&vault).await;
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|e| e.id == "pt#r1") && entries.iter().any(|e| e.id == "pt#r2"));
    let view = view_of(&mut vault, &path).await;
    let sum: f64 = entries.iter().map(|e| e.cost_usd.unwrap()).sum();
    assert!((view.cost_usd.unwrap() - sum).abs() < 1e-9);
    // Источники: цитата первого ответа и названный во втором.
    let urls: Vec<_> = research
        .sources
        .iter()
        .filter_map(|s| s.url.clone())
        .collect();
    assert_eq!(urls, ["https://b.example/"]);

    // Бесконечные паузы: ровно MAX_ROUNDS запросов, дальше разбирается то, что есть.
    let endless = StreamTransport::with((0..10).map(|_| pause()).collect());
    let research = run_web(&mut vault, &endless, &path, "pt2", "Бесконечный вопрос")
        .await
        .unwrap();
    assert_eq!(endless.count(), 4, "предел запросов держит ядро");
    assert_eq!(research.status, ResearchStatus::Done);
    assert_eq!(
        research.sources[0].url.as_deref(),
        Some("https://a.example/")
    );
}

/// Продолжение не раздувается: тело сверх предела — отказ.
pub async fn a_continuation_body_is_limited() {
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let huge = "я".repeat(1_100_000);
    let transport = StreamTransport::with(vec![web(Web {
        results: vec![("А", "https://a.example/")],
        text: &huge,
        stop: "pause_turn",
        ..Web::new()
    })]);
    let research = run_web(&mut vault, &transport, &path, "big", QUESTION)
        .await
        .unwrap();
    assert_eq!(research.status, ResearchStatus::Failed);
    assert_eq!(research.failure, Some(AiFailure::BadResponse));
    assert_eq!(transport.count(), 1);
}

/// OpenAI: только API ответов, `web_search_call` и `url_citation`; цена поиска неизвестна.
pub async fn openai_research_uses_the_responses_api() {
    let mut vault = vault_with(&web_settings(AiProvider::OpenAi, "gpt-5")).await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![responses(
        "Ответ с цитатой.",
        "https://openai.example/source",
        Some(answer_of(&["Итог по ответам."], json!([]))),
    )]);
    let vault_id = unique("rv");
    let verified = connection(&vault).await;
    let ResearchBegin::Ready { prepared } = begin_research(
        &mut vault,
        &vault_id,
        &path,
        "o1",
        ResearchKind::Web,
        QUESTION,
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap() else {
        panic!("запрос готов");
    };
    assert!(
        prepared.request.url.ends_with("/responses"),
        "{}",
        prepared.request.url
    );
    let body: Value = serde_json::from_str(prepared.request.body.as_deref().unwrap()).unwrap();
    assert_eq!(body["tools"][0]["type"], "web_search");
    assert_eq!(body["tools"][1]["type"], "function");
    assert_eq!(body["tools"][1]["name"], "submit_research");
    assert!(body.get("messages").is_none());
    assert_eq!(
        body["input"][0]["content"]
            .as_str()
            .unwrap()
            .contains(QUESTION),
        true
    );
    assert_eq!(body["store"], false);
    let outcome = send_research(&transport, &vault_id, &prepared, Some(KEY), &verified).await;
    let done = finish_research(&mut vault, &vault_id, &prepared, 2_000, outcome)
        .await
        .unwrap();
    let research = done.research;
    assert_eq!(research.status, ResearchStatus::Done);
    assert_eq!(
        research.sources[0].url.as_deref(),
        Some("https://openai.example/source")
    );
    // Цена поиска неизвестна — стоимость урока неизвестна, а не ноль.
    assert_eq!(research.cost_usd, None);
    let entries = journal_entries(&vault).await;
    assert_eq!(entries[0].cost_usd, None);
    let view = view_of(&mut vault, &path).await;
    assert_eq!(view.cost_usd, None);
}

/// Разбор потока не зависит от нарезки частей, линеен и отказывает на строке-бомбе.
pub async fn the_stream_parser_is_chunk_independent_and_bounded() {
    let whole: Vec<u8> = web_chunks(&Web {
        results: vec![("А", "https://a.example/")],
        text: "Привет, мир.",
        citations: vec![("А", "https://a.example/")],
        submit: Some(answer_of(&["Абзац."], json!([]))),
        ..Web::new()
    })
    .concat();
    let parse = |size: usize| {
        let mut parser = ResearchParser::new(AiProvider::Anthropic);
        for chunk in whole.chunks(size) {
            parser.feed(chunk);
        }
        parser.finish().unwrap()
    };
    let reference = parse(whole.len());
    assert_eq!(reference.text, "Привет, мир.");
    assert_eq!(reference.searches, 1);
    assert_eq!(reference.cited.len(), 1);
    assert!(reference.submit.is_some());
    for size in [1, 3, 7, 64] {
        assert_eq!(parse(size), reference, "нарезка по {size}");
    }
    // Поток, оборванный до конца, — не ответ.
    let mut cut = ResearchParser::new(AiProvider::Anthropic);
    cut.feed(&whole[..whole.len() / 2]);
    assert_eq!(cut.finish().unwrap_err(), AiFailure::Offline);
    // Ошибка внутри потока.
    let mut broken = ResearchParser::new(AiProvider::Anthropic);
    broken.feed(b"data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\"}}\n\n");
    assert_eq!(broken.finish().unwrap_err(), AiFailure::ProviderDown);
    // Строка без перевода строки длиннее предела.
    let mut bomb = ResearchParser::new(AiProvider::Anthropic);
    let piece = vec![b'x'; 64 * 1024];
    for _ in 0..20 {
        bomb.feed(&piece);
    }
    assert_eq!(bomb.finish().unwrap_err(), AiFailure::BadResponse);
}

/// Линейность на враждебном потоке: длинная строка по одному байту против той же строки
/// крупными частями (время сравнивается между собой, а не с часами) и тысячи блоков.
pub async fn the_stream_parser_is_linear_on_a_hostile_stream(kilobytes: usize) {
    use typsidian_core::properties::now_ms;
    let line = vec![b'a'; kilobytes * 1024];
    let started = now_ms();
    let mut parser = ResearchParser::new(AiProvider::Anthropic);
    for byte in &line {
        parser.feed(std::slice::from_ref(byte));
    }
    parser.feed(b"\n");
    let by_byte = now_ms() - started;
    let started = now_ms();
    let mut parser = ResearchParser::new(AiProvider::Anthropic);
    for chunk in line.chunks(4096) {
        parser.feed(chunk);
    }
    parser.feed(b"\n");
    let by_chunk = now_ms() - started;
    assert!(
        by_byte <= by_chunk * 300 + 5_000,
        "{by_byte} мс по байту против {by_chunk} мс частями на {kilobytes} КБ"
    );
    // Много блоков: лишние отбрасываются, память не растёт без предела.
    let mut blocks = ResearchParser::new(AiProvider::Anthropic);
    for index in 0..5_000u64 {
        blocks.feed(&event(
            "content_block_start",
            json!({"type":"content_block_start","index":index,"content_block":{"type":"text","text":""}}),
        ));
    }
    blocks.feed(&event("message_stop", json!({"type":"message_stop"})));
    let reply = blocks.finish().unwrap();
    assert!(reply.text.is_empty());
}

/// Пределы длины всего, что приходит от модели: абзацы, источники, изложение, размер заметки.
pub async fn model_output_is_limited_everywhere() {
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let paragraphs: Vec<String> = (0..100).map(|_| "абзац ".repeat(2_000)).collect();
    let refs: Vec<&str> = paragraphs.iter().map(String::as_str).collect();
    let sources: Vec<Value> = (0..500)
        .map(|i| json!({"title": "т".repeat(1_000), "url": format!("https://s{i}.example/")}))
        .collect();
    let transport = StreamTransport::with(vec![web(Web {
        submit: Some(answer_of(&refs, Value::Array(sources))),
        ..Web::new()
    })]);
    let research = run_web(&mut vault, &transport, &path, "lim", &"в".repeat(5_000))
        .await
        .unwrap();
    assert_eq!(research.status, ResearchStatus::Done);
    assert!(research.sources.len() <= 30);
    assert!(research
        .sources
        .iter()
        .all(|s| s.title.chars().count() <= 200));
    assert!(research.summary.unwrap().chars().count() <= 1_500);
    assert!(research.question.chars().count() <= 500);
    let text = note_text(&vault, &research.note.unwrap());
    assert!(text.len() < 100_000, "{} байт", text.len());
    // Ссылки на страницы PDF: несуществующие страницы и мусор отбрасываются.
    let parsed = parse_submit(
        &answer_of(
            &["а"],
            json!([
                {"title": "d", "pages": [1, 2, 99, 0, "2", "x", 1]},
                {"title": "e", "pages": [500]},
                {"title": "f"}
            ]),
        )
        .to_string(),
        ResearchKind::Pdf,
        3,
    )
    .unwrap();
    assert_eq!(parsed.sources.len(), 1);
    assert_eq!(parsed.sources[0].pages, Some(vec![1, 2]));
    assert!(parse_submit("не json", ResearchKind::Web, 0).is_none());
    assert!(parse_submit(&json!({"paragraphs": []}).to_string(), ResearchKind::Web, 0).is_none());
}

/// Заметка пишется под свободным именем, не через символические ссылки и атомарно.
pub async fn the_note_takes_a_free_name_and_never_goes_through_a_symlink() {
    let mut vault = web_vault().await.with_file(
        &format!("{NOTES}/Как работает сборка мусора в Rust.typ"),
        "чужая заметка",
    );
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![good()]);
    let research = run_web(&mut vault, &transport, &path, "n1", QUESTION)
        .await
        .unwrap();
    assert_eq!(
        research.note.as_deref(),
        Some(format!("{NOTES}/Как работает сборка мусора в Rust 2.typ").as_str())
    );
    assert_eq!(
        note_text(
            &vault,
            &format!("{NOTES}/Как работает сборка мусора в Rust.typ")
        ),
        "чужая заметка"
    );

    // Папка — ссылка: заметка не пишется, отказ записан в урок.
    let mut linked = vault_with(&web_settings(AiProvider::Anthropic, "claude-opus-5-5"))
        .await
        .with_symlink(NOTES);
    let path = new_lesson(&mut linked).await;
    let transport = StreamTransport::with(vec![good()]);
    let research = run_web(&mut linked, &transport, &path, "n2", QUESTION)
        .await
        .unwrap();
    assert_eq!(research.status, ResearchStatus::Failed);
    assert_eq!(research.failure, Some(AiFailure::JournalFailed));
    assert!(linked.paths_under(NOTES).is_empty());
}

/// Отмена: сеть прекращается, карточка «отменено», дальше запросов нет.
pub async fn cancel_ends_the_research_and_sends_nothing_more() {
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let vault_id = unique("rv");
    let verified = connection(&vault).await;
    let ResearchBegin::Ready { prepared } = begin_research(
        &mut vault,
        &vault_id,
        &path,
        "x1",
        ResearchKind::Web,
        QUESTION,
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap() else {
        panic!("запрос готов");
    };
    let hang = StreamTransport::with(vec![Script::Hang {
        chunks: web_chunks(&Web::new())[..3].to_vec(),
    }]);
    let sending = send_research(&hang, &vault_id, &prepared, Some(KEY), &verified);
    futures_util::pin_mut!(sending);
    assert!(futures_util::poll!(&mut sending).is_pending());
    cancel(&vault_id, "x1");
    let outcome = sending.await;
    assert_eq!(outcome.clone().unwrap_err(), AiFailure::Cancelled);
    let done = finish_research(&mut vault, &vault_id, &prepared, 2_000, outcome)
        .await
        .unwrap();
    assert_eq!(done.research.status, ResearchStatus::Cancelled);
    assert_eq!(done.next, Next::WaitUser);
    let view = view_of(&mut vault, &path).await;
    assert_eq!(view.research[0].status, ResearchStatus::Cancelled);
    assert!(view.events.iter().any(|event| matches!(
        event,
        LessonEvent::ResearchFailed {
            cancelled: true,
            ..
        }
    )));
    assert_eq!(hang.count(), 1);
    // Запоздавший ответ уже закончившегося задания ничего не пишет.
    let late = finish_research(
        &mut vault,
        &vault_id,
        &prepared,
        3_000,
        Ok(reply_for_late()),
    )
    .await
    .unwrap_err();
    assert_eq!(late, AiFailure::Cancelled);
    assert_eq!(events_of(&view_of(&mut vault, &path).await, "done"), 0);
    assert!(vault.paths_under(NOTES).is_empty());
}

fn reply_for_late() -> typsidian_core::ai::research::ResearchReply {
    let mut parser = ResearchParser::new(AiProvider::Anthropic);
    for chunk in web_chunks(&Web {
        submit: Some(answer_of(&["Поздно."], json!([]))),
        ..Web::new()
    }) {
        parser.feed(&chunk);
    }
    parser.finish().unwrap()
}

/// Отмена до отправки (в очереди) и отмена между запросами заканчивают исследование без сети.
pub async fn cancel_while_queued_and_before_sending_needs_no_network() {
    let mut vault = vault_with(&AiSettings {
        research_parallel: 1,
        ..web_settings(AiProvider::Anthropic, "claude-opus-5-5")
    })
    .await;
    let path = new_lesson(&mut vault).await;
    let vault_id = unique("rv");
    let verified = connection(&vault).await;
    let ResearchBegin::Ready { prepared: first } =
        begin_q(&mut vault, &vault_id, &path, &verified, "q1", "Первый")
            .await
            .unwrap()
    else {
        panic!("первый идёт");
    };
    assert!(matches!(
        begin_q(&mut vault, &vault_id, &path, &verified, "q2", "Второй")
            .await
            .unwrap(),
        ResearchBegin::Queued
    ));
    // Очередь видна в карточках и заданиях.
    let view = view_of(&mut vault, &path).await;
    let statuses: Vec<_> = view.research.iter().map(|r| r.status).collect();
    assert_eq!(statuses, [ResearchStatus::Running, ResearchStatus::Queued]);
    assert!(research_jobs(&vault_id)
        .iter()
        .any(|j| j.id == "q2" && j.queued));
    // Повторный вызов того же ждущего задания остаётся в очереди.
    assert!(matches!(
        begin_q(&mut vault, &vault_id, &path, &verified, "q2", "Второй")
            .await
            .unwrap(),
        ResearchBegin::Queued
    ));
    // Отмена ждущего: следующий вызов заканчивает его без запроса.
    cancel(&vault_id, "q2");
    let ResearchBegin::Finished { research } =
        begin_q(&mut vault, &vault_id, &path, &verified, "q2", "Второй")
            .await
            .unwrap()
    else {
        panic!("отменено в очереди");
    };
    assert_eq!(research.status, ResearchStatus::Cancelled);
    // Первое идёт своим чередом и отдаёт место следующему по очереди.
    assert!(matches!(
        begin_q(&mut vault, &vault_id, &path, &verified, "q3", "Третий")
            .await
            .unwrap(),
        ResearchBegin::Queued
    ));
    let transport = StreamTransport::with(vec![good()]);
    let outcome = send_research(&transport, &vault_id, &first, Some(KEY), &verified).await;
    finish_research(&mut vault, &vault_id, &first, 2_000, outcome)
        .await
        .unwrap();
    assert!(matches!(
        begin_q(&mut vault, &vault_id, &path, &verified, "q3", "Третий")
            .await
            .unwrap(),
        ResearchBegin::Ready { .. }
    ));
    assert_eq!(transport.count(), 1);
}

/// Очередь по `research_parallel`: не больше заданного идут вместе, остальные по порядку.
pub async fn the_queue_follows_research_parallel_in_order() {
    let mut vault = vault_with(&AiSettings {
        research_parallel: 2,
        ..web_settings(AiProvider::Anthropic, "claude-opus-5-5")
    })
    .await;
    let path = new_lesson(&mut vault).await;
    let vault_id = unique("rv");
    let verified = connection(&vault).await;
    let mut results = Vec::new();
    for (id, question) in [("a", "Один"), ("b", "Два"), ("c", "Три"), ("d", "Четыре")]
    {
        let step = begin_research(
            &mut vault,
            &vault_id,
            &path,
            id,
            ResearchKind::Web,
            question,
            None,
            1_000,
            true,
            &verified,
        )
        .await
        .unwrap();
        results.push(match step {
            ResearchBegin::Ready { prepared } => Some(prepared),
            ResearchBegin::Queued => None,
            ResearchBegin::Finished { .. } => panic!("не должно кончиться"),
        });
    }
    assert_eq!(
        results.iter().map(Option::is_some).collect::<Vec<_>>(),
        [true, true, false, false]
    );
    // Освободилось одно место: «четвёртый» не обгоняет «третьего».
    let first = results[0].take().unwrap();
    let transport = StreamTransport::with(vec![good()]);
    let outcome = send_research(&transport, &vault_id, &first, Some(KEY), &verified).await;
    finish_research(&mut vault, &vault_id, &first, 2_000, outcome)
        .await
        .unwrap();
    let again = |id: &'static str, question: &'static str| {
        let (vault_id, path, verified) = (vault_id.clone(), path.clone(), verified.clone());
        (id, question, vault_id, path, verified)
    };
    let (id, question, vid, p, v) = again("d", "Четыре");
    assert!(matches!(
        begin_research(
            &mut vault,
            &vid,
            &p,
            id,
            ResearchKind::Web,
            question,
            None,
            1_000,
            true,
            &v
        )
        .await
        .unwrap(),
        ResearchBegin::Queued
    ));
    let (id, question, vid, p, v) = again("c", "Три");
    assert!(matches!(
        begin_research(
            &mut vault,
            &vid,
            &p,
            id,
            ResearchKind::Web,
            question,
            None,
            1_000,
            true,
            &v
        )
        .await
        .unwrap(),
        ResearchBegin::Ready { .. }
    ));
    // Те же вопросы вторым заданием не запускаются, пока идут.
    let duplicate = begin_research(
        &mut vault,
        &vault_id,
        &path,
        "dup",
        ResearchKind::Web,
        "три",
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap_err();
    assert_eq!(duplicate, AiFailure::Cancelled);
}

/// Приложение закрыли посреди исследования: незаконченное при открытии — «прервано», итоги
/// и повторы читаются из файла; поддельные и повторные события игнорируются.
pub async fn unfinished_research_is_interrupted_and_events_fold_into_cards() {
    let mut vault = web_vault().await;
    let path = "Уроки/Закрыли.lesson";
    put_lesson(
        &vault,
        path,
        &[
            started("a", true),
            started("b", true),
            started("c", true),
            started("d", false),
            // Повторный старт с тем же id и событие без старта не создают карточек.
            started("a", false),
            LessonEvent::ResearchDone {
                at: 6,
                id: "b".into(),
                note: "Уроки/Исследования/Б.typ".into(),
                summary: "итог".into(),
                sources: vec![ResearchSource {
                    title: "Т".into(),
                    url: Some("https://t.example/".into()),
                    pages: None,
                }],
                input_tokens: None,
                output_tokens: None,
                cost_usd: None,
            },
            LessonEvent::ResearchFailed {
                at: 7,
                id: "c".into(),
                failure: AiFailure::Cancelled,
                cancelled: true,
            },
            LessonEvent::ResearchFailed {
                at: 7,
                id: "нет-такого".into(),
                failure: AiFailure::Offline,
                cancelled: false,
            },
            // Второй итог для закончившегося исследования не меняет карточку.
            LessonEvent::ResearchFailed {
                at: 8,
                id: "b".into(),
                failure: AiFailure::Offline,
                cancelled: false,
            },
        ],
    );
    let view = open_lesson(&mut vault, path).await.unwrap();
    let cards: Vec<(&str, ResearchStatus)> = view
        .research
        .iter()
        .map(|card| (card.id.as_str(), card.status))
        .collect();
    assert_eq!(
        cards,
        [
            ("a", ResearchStatus::Interrupted),
            ("b", ResearchStatus::Done),
            ("c", ResearchStatus::Cancelled),
            ("d", ResearchStatus::Interrupted),
        ]
    );
    assert!(view.research.iter().all(|card| !card.launch_due));
    assert_eq!(
        view.research[1].note.as_deref(),
        Some("Уроки/Исследования/Б.typ")
    );
    assert_eq!(view.research[1].found, 1);
    // Повтор — новое исследование с тем же вопросом: прерванное остаётся прерванным.
    let transport = StreamTransport::with(vec![good()]);
    let research = run_web(&mut vault, &transport, path, "again", "Вопрос")
        .await
        .unwrap();
    assert_eq!(research.status, ResearchStatus::Done);
    let view = open_lesson(&mut vault, path).await.unwrap();
    assert_eq!(view.research.len(), 5);
    assert_eq!(view.research[0].status, ResearchStatus::Interrupted);
    // Старые файлы без событий исследования читаются как раньше.
    put_lesson(&vault, "Уроки/Старый.lesson", &[]);
    let old = open_lesson(&mut vault, "Уроки/Старый.lesson")
        .await
        .unwrap();
    assert!(old.research.is_empty());
}

/// Задание, оборванное вместе с вызовом (закрыто соединение), не остаётся идущим.
pub async fn a_dropped_call_does_not_leave_a_running_job() {
    use typsidian_core::ai::research::{ResearchGuard, RESEARCH_JOBS};
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let vault_id = unique("rv");
    let verified = connection(&vault).await;
    begin_research(
        &mut vault,
        &vault_id,
        &path,
        "g1",
        ResearchKind::Web,
        QUESTION,
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap();
    {
        let _guard = ResearchGuard::new(&vault_id, "g1");
        assert!(research_jobs(&vault_id)[0].queued || !research_jobs(&vault_id).is_empty());
    }
    let job = RESEARCH_JOBS.get(&vault_id, "g1").unwrap();
    assert_eq!(job.state, typsidian_core::ai::jobs::JobState::Cancelled);
    // В файле нет итога: при открытии это «прервано».
    let view = view_of(&mut vault, &path).await;
    assert_eq!(view.research[0].status, ResearchStatus::Interrupted);
    // Охрана с `disarm` задание не трогает.
    begin_research(
        &mut vault,
        &vault_id,
        &path,
        "g2",
        ResearchKind::Web,
        "Другой",
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap();
    {
        let mut guard = ResearchGuard::new(&vault_id, "g2");
        guard.disarm();
    }
    assert_eq!(
        RESEARCH_JOBS.get(&vault_id, "g2").unwrap().state,
        typsidian_core::ai::jobs::JobState::Running
    );
}

/// Поддельный или запоздавший ответ ничего не пишет; снимок подключения проверяется.
pub async fn forged_steps_and_a_changed_connection_are_refused() {
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let vault_id = unique("rv");
    let verified = connection(&vault).await;
    let ResearchBegin::Ready { prepared } = begin_research(
        &mut vault,
        &vault_id,
        &path,
        "z1",
        ResearchKind::Web,
        QUESTION,
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap() else {
        panic!("запрос готов");
    };
    let transport = StreamTransport::with(vec![good(), good()]);
    let outcome = send_research(&transport, &vault_id, &prepared, Some(KEY), &verified).await;
    // Чужой id, чужой номер запроса.
    let mut stranger = prepared.clone();
    stranger.request_id = "нет".into();
    assert_eq!(
        finish_research(&mut vault, &vault_id, &stranger, 2_000, outcome.clone())
            .await
            .unwrap_err(),
        AiFailure::Cancelled
    );
    let mut second = prepared.clone();
    second.round = 2;
    assert_eq!(
        finish_research(&mut vault, &vault_id, &second, 2_000, outcome.clone())
            .await
            .unwrap_err(),
        AiFailure::Cancelled
    );
    // Чужой vault_id тоже не подходит.
    assert_eq!(
        finish_research(&mut vault, "чужой", &prepared, 2_000, outcome.clone())
            .await
            .unwrap_err(),
        AiFailure::Cancelled
    );
    assert_eq!(events_of(&view_of(&mut vault, &path).await, "done"), 0);
    // Подключение сменилось (синхронизация подменила провайдера): ключ не уходит.
    let other = Connection::of_request(AiProvider::OpenAi, "https://api.openai.com/v1/responses");
    assert_eq!(
        send_research(&transport, &vault_id, &prepared, Some(KEY), &other)
            .await
            .unwrap_err(),
        AiFailure::EndpointNotApproved
    );
    assert_eq!(transport.count(), 1);
    // begin со снимком прежнего подключения после смены настроек отказывает до записи.
    save_settings(&mut vault, &web_settings(AiProvider::OpenAi, "gpt-5"))
        .await
        .unwrap();
    let before = journal_entries(&vault).await.len();
    let failure = begin_research(
        &mut vault,
        &unique("rv"),
        &path,
        "z2",
        ResearchKind::Web,
        "Новый вопрос",
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap_err();
    assert_eq!(failure, AiFailure::EndpointNotApproved);
    assert_eq!(journal_entries(&vault).await.len(), before);
    let view = view_of(&mut vault, &path).await;
    assert_eq!(view.research.len(), 1, "второе исследование не записано");
}

use typsidian_core::ai::client::Connection;

/// Урок из закрытой папки и выключенное обучение исследований не принимают.
pub async fn research_respects_denied_lessons_and_disabled_learning() {
    let mut vault = vault_with(&AiSettings {
        denied_folders: vec!["Уроки".to_owned()],
        ..web_settings(AiProvider::Anthropic, "claude-opus-5-5")
    })
    .await;
    let path = new_lesson(&mut vault).await;
    let verified = connection(&vault).await;
    let failure = begin_research(
        &mut vault,
        &unique("rv"),
        &path,
        "d1",
        ResearchKind::Web,
        QUESTION,
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap_err();
    assert_eq!(failure, AiFailure::Forbidden);
    assert!(journal_entries(&vault).await.is_empty());

    let mut off = vault_with(&AiSettings {
        enabled: false,
        ..web_settings(AiProvider::Anthropic, "claude-opus-5-5")
    })
    .await;
    let verified = connection(&off).await;
    assert_eq!(
        begin_research(
            &mut off,
            &unique("rv"),
            "Уроки/А.lesson",
            "d2",
            ResearchKind::Web,
            QUESTION,
            None,
            1_000,
            true,
            &verified,
        )
        .await
        .unwrap_err(),
        AiFailure::Disabled
    );
    assert_eq!(
        research_estimate(&off, "Книги/А.pdf").await.unwrap_err(),
        AiFailure::Disabled
    );
    // Пустой вопрос.
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let verified = connection(&vault).await;
    assert_eq!(
        begin_research(
            &mut vault,
            &unique("rv"),
            &path,
            "d3",
            ResearchKind::Web,
            "   ",
            None,
            1_000,
            true,
            &verified,
        )
        .await
        .unwrap_err(),
        AiFailure::GoalEmpty
    );
    // Не урок.
    assert_eq!(
        begin_research(
            &mut vault,
            &unique("rv"),
            "Заметка.typ",
            "d4",
            ResearchKind::Web,
            QUESTION,
            None,
            1_000,
            true,
            &verified,
        )
        .await
        .unwrap_err(),
        AiFailure::LessonUnreadable
    );
}

// ── T004: учитель ───────────────────────────────────────────────────────────

fn tool_script(text: &str, name: &str, arguments: Value) -> Script {
    reply(anthropic(&[text], Some((name, arguments)), 100, 10))
}

/// Учитель запрашивает исследование: ход не ждёт («запущено»), запуск делает клиент по
/// признаку, готовое приходит в следующем ходе изложением с пометкой «данные».
pub async fn the_teacher_requests_research_and_gets_the_result_next_turn() {
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let teacher = StreamTransport::with(vec![
        tool_script("Проверю.", "research", json!({"question": QUESTION})),
        say("Пока идёт поиск, вопрос: что такое владение?"),
    ]);
    let done = cycle(&mut vault, &teacher, &path, "t1", answer("объясни"), 0)
        .await
        .unwrap();
    // Учитель продолжил сам, не дожидаясь, и получил «запущено» ровно один раз.
    assert_eq!(teacher.count(), 2);
    assert!(
        teacher.body(1).contains("запущено в фоне"),
        "{}",
        teacher.body(1)
    );
    assert_eq!(done.view.waiting, Waiting::User);
    assert_eq!(done.view.research.len(), 1);
    let card = &done.view.research[0];
    assert!(!card.by_user);
    assert_eq!(card.status, ResearchStatus::Queued);
    assert!(card.launch_due, "клиент запускает по этому признаку");
    assert!(!card.confirm_cost);
    assert_eq!(card.question, QUESTION);
    assert_eq!(card.kind, ResearchKind::Web);

    // Клиент запускает; вопрос берётся из файла, подмена игнорируется.
    let transport = StreamTransport::with(vec![good()]);
    let research = run_web(
        &mut vault,
        &transport,
        &path,
        &card.id,
        "ПОДМЕНЁННЫЙ ВОПРОС",
    )
    .await
    .unwrap();
    assert_eq!(research.status, ResearchStatus::Done);
    assert!(transport.body(0).contains(QUESTION));
    assert!(!transport.body(0).contains("ПОДМЕНЁННЫЙ"));
    let view = view_of(&mut vault, &path).await;
    assert_eq!(view.research.len(), 1);
    assert_eq!(view.research[0].status, ResearchStatus::Done);
    assert!(!view.research[0].launch_due);

    // Следующий ход: изложение, источники и пометка «данные», не инструкции.
    let teacher = StreamTransport::with(vec![say("Нашлось, смотрим.")]);
    cycle(
        &mut vault,
        &teacher,
        &path,
        "t2",
        answer("ну что?"),
        view.events_len,
    )
    .await
    .unwrap();
    let body = teacher.body(0);
    assert!(body.contains("Готово исследование"), "{body}");
    assert!(body.contains("не инструкции"));
    assert!(body.contains("В Rust нет сборщика мусора."));
    assert!(body.contains("https://doc.rust-lang.org/book/ch04-00.html"));
    assert!(body.contains(QUESTION));
}

/// Инструмент, которого в запросе не было, не исполняется; пределы на ход и на урок.
pub async fn teacher_research_calls_are_gated_and_limited() {
    // Поиск выключен: модель вызвала `research` сама — события нет, учитель не ждёт.
    let mut vault = vault_with(&settings(AiProvider::Anthropic, "claude-opus-5-5")).await;
    let path = new_lesson(&mut vault).await;
    let teacher = StreamTransport::with(vec![tool_script(
        "Поищу.",
        "research",
        json!({"question": "что угодно"}),
    )]);
    let done = cycle(&mut vault, &teacher, &path, "g1", answer("привет"), 0)
        .await
        .unwrap();
    assert!(done.view.research.is_empty());
    assert_eq!(teacher.count(), 1);
    assert!(journal_entries(&vault).await.len() == 1);

    // Включено: не больше двух исследований за ход.
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    // Второй и третий вызовы приходят одним ответом: соберём ответ с тремя инструментами.
    let mut chunks = anthropic(
        &["Три вопроса."],
        Some(("research", json!({"question": "Раз"}))),
        1,
        1,
    );
    chunks.truncate(chunks.len() - 2);
    for (index, question) in [(2u64, "Два"), (3, "Три")] {
        chunks.push(event(
            "content_block_start",
            json!({"type":"content_block_start","index":index,
                "content_block":{"type":"tool_use","id":format!("t{index}"),"name":"research","input":{}}}),
        ));
        chunks.push(event(
            "content_block_delta",
            json!({"type":"content_block_delta","index":index,
                "delta":{"type":"input_json_delta","partial_json":json!({"question":question}).to_string()}}),
        ));
        chunks.push(event(
            "content_block_stop",
            json!({"type":"content_block_stop","index":index}),
        ));
    }
    chunks.push(event(
        "message_delta",
        json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":1}}),
    ));
    chunks.push(event("message_stop", json!({"type":"message_stop"})));
    let teacher = StreamTransport::with(vec![reply(chunks), say("Дальше.")]);
    let done = cycle(&mut vault, &teacher, &path, "g2", answer("привет"), 0)
        .await
        .unwrap();
    assert_eq!(done.view.research.len(), 2);

    // Предел на урок: сверх него отказ инструменту, события нет.
    let mut vault = web_vault().await;
    let path = "Уроки/Много.lesson";
    let many: Vec<LessonEvent> = (0..30).map(|i| started(&format!("old{i}"), true)).collect();
    put_lesson(&vault, path, &many);
    let teacher = StreamTransport::with(vec![
        tool_script("Ещё.", "research", json!({"question": "Тридцать первый"})),
        say("Хорошо."),
    ]);
    let current = view_of(&mut vault, path).await.events_len;
    let done = cycle(&mut vault, &teacher, path, "g3", answer("да"), current)
        .await
        .unwrap();
    assert_eq!(done.view.research.len(), 30);
    assert!(done.view.events.iter().any(|event| matches!(
        event,
        LessonEvent::PlanRejected { tool, .. } if tool == "research"
    )));
    assert!(teacher.body(1).contains("отказ"));
}

/// `study_pdf`: инструмент есть только при настройке, закрытый и не-PDF файл — отказ учителю,
/// допустимый — карточка с подтверждением стоимости.
pub async fn the_teacher_reads_pdf_only_when_allowed_and_through_access_check() {
    let build = |agent: bool| async move {
        vault_with(&AiSettings {
            research_agent_pdf: agent,
            denied_folders: vec!["Личное".to_owned()],
            ..settings(AiProvider::Anthropic, "claude-opus-5-5")
        })
        .await
        .with_file("Книги/Текст.pdf", TEXT_PDF)
        .with_file("Личное/Секрет.pdf", TEXT_PDF)
        .with_file("Книги/Заметка.typ", "= т")
    };
    // Настройка выключена: инструмента нет, вызов не исполняется.
    let mut vault = build(false).await;
    let path = new_lesson(&mut vault).await;
    let teacher = StreamTransport::with(vec![tool_script(
        "Прочту.",
        "study_pdf",
        json!({"path": "Книги/Текст.pdf", "question": "О чём?"}),
    )]);
    let done = cycle(&mut vault, &teacher, &path, "p1", answer("привет"), 0)
        .await
        .unwrap();
    assert!(!tool_names(&teacher.body(0)).contains(&"study_pdf".to_owned()));
    assert!(done.view.research.is_empty());

    let mut vault = build(true).await;
    let path = new_lesson(&mut vault).await;
    let teacher = StreamTransport::with(vec![
        tool_script(
            "Прочту.",
            "study_pdf",
            json!({"path": "Личное/Секрет.pdf", "question": "О чём?"}),
        ),
        tool_script(
            "Тогда эту.",
            "study_pdf",
            json!({"path": "Книги/Заметка.typ", "question": "О чём?"}),
        ),
        tool_script(
            "Тогда эту.",
            "study_pdf",
            json!({"path": "Книги/Текст.pdf", "question": "О чём?"}),
        ),
        say("Читаю."),
    ]);
    let done = cycle(&mut vault, &teacher, &path, "p2", answer("привет"), 0)
        .await
        .unwrap();
    assert!(tool_names(&teacher.body(0)).contains(&"study_pdf".to_owned()));
    // Закрытый файл и не-PDF: отказ результатом инструмента, карточки нет.
    assert!(
        teacher.body(1).contains("доступ к файлу закрыт"),
        "{}",
        teacher.body(1)
    );
    assert!(!teacher.body(1).contains("СЕКРЕТ"));
    assert!(teacher.body(2).contains("доступ к файлу закрыт"));
    // Допустимый файл: карточка PDF, запуск требует подтверждения стоимости.
    assert_eq!(done.view.research.len(), 1);
    let card = &done.view.research[0];
    assert_eq!(card.kind, ResearchKind::Pdf);
    assert_eq!(card.path.as_deref(), Some("Книги/Текст.pdf"));
    assert!(card.launch_due && card.confirm_cost);
}

// ── T008: PDF ───────────────────────────────────────────────────────────────

async fn pdf_vault() -> MemoryVault {
    vault_with(&AiSettings {
        denied_folders: vec!["Личное".to_owned()],
        ..settings(AiProvider::Anthropic, "claude-opus-5-5")
    })
    .await
    .with_file("Книги/Текст.pdf", TEXT_PDF)
    .with_file("Книги/Скан.pdf", SCAN_PDF)
    .with_file("Книги/Битая.pdf", "это не PDF")
    .with_file("Книги/Заметка.typ", "= Заметка")
    .with_file("Личное/Секрет.pdf", TEXT_PDF)
}

/// Оценка: объём, страницы, стоимость; закрытый и неподходящий путь — отказ или причина.
pub async fn the_estimate_shows_volume_pages_and_cost_and_respects_access() {
    let vault = pdf_vault().await;
    let estimate = research_estimate(&vault, "Книги/Текст.pdf").await.unwrap();
    assert_eq!(estimate.path, "Книги/Текст.pdf");
    assert_eq!(estimate.pages, 2);
    assert_eq!(estimate.pages_read, 2);
    assert!(estimate.chars > 20);
    assert!(estimate.units >= 1_500);
    assert!(!estimate.truncated);
    assert_eq!(estimate.problem, None);
    // Цена по справочнику: токены на 4 / 20 долларов за миллион.
    let tokens = f64::from(estimate.units);
    let expected = (tokens * 4.0 + 1_500.0 * 20.0) / 1_000_000.0;
    assert!((estimate.cost_usd.unwrap() - expected).abs() < 1e-9);
    // Цена неизвестна у модели вне справочника — не ноль.
    let mut unknown = pdf_vault().await;
    save_settings(
        &mut unknown,
        &settings(AiProvider::Anthropic, "claude-неизвестная"),
    )
    .await
    .unwrap();
    assert_eq!(
        research_estimate(&unknown, "Книги/Текст.pdf")
            .await
            .unwrap()
            .cost_usd,
        None
    );

    // Закрытая папка, не-PDF, выход за vault — отказ проверкой доступа.
    for path in [
        "Личное/Секрет.pdf",
        "Книги/Заметка.typ",
        "../Текст.pdf",
        "/etc/passwd.pdf",
    ] {
        assert_eq!(
            research_estimate(&vault, path).await.unwrap_err(),
            AiFailure::Forbidden,
            "{path}"
        );
    }
    // Нечитаемое: оценка с причиной, запуск отклонит.
    use typsidian_core::ai::knowledge::ImportSkipReason as Reason;
    for (path, reason) in [
        ("Книги/Скан.pdf", Reason::NoText),
        ("Книги/Битая.pdf", Reason::Unsupported),
        ("Книги/Нет.pdf", Reason::Missing),
    ] {
        assert_eq!(
            research_estimate(&vault, path).await.unwrap().problem,
            Some(reason),
            "{path}"
        );
    }
}

/// PDF в разрешённой папке: итог со страницами, текст страниц в запросе, поиска нет;
/// запрещённая — отказ без записи, журнала и сети.
pub async fn a_pdf_is_read_with_page_sources_and_a_denied_one_is_refused() {
    let mut vault = pdf_vault().await;
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![reply(anthropic(
        &["Готово."],
        Some((
            "submit_research",
            answer_of(
                &["Замыкание хранит окружение.", "Счётчик прячет состояние."],
                json!([
                    {"title": "Текст", "pages": [1, 2, 77]},
                    {"title": "Выдумка", "pages": [500]},
                ]),
            ),
        )),
        2_000,
        300,
    ))]);
    let vault_id = unique("rv");
    let research = run(
        &mut vault,
        &transport,
        &vault_id,
        &path,
        "pdf1",
        ResearchKind::Pdf,
        "Как замыкание хранит состояние?",
        Some("Книги/Текст.pdf"),
    )
    .await
    .unwrap();
    assert_eq!(research.status, ResearchStatus::Done);
    assert_eq!(research.kind, ResearchKind::Pdf);
    assert_eq!(research.path.as_deref(), Some("Книги/Текст.pdf"));
    // Источники — страницы документа; несуществующие отброшены вместе с выдумкой.
    assert_eq!(research.sources.len(), 1);
    assert_eq!(research.sources[0].pages, Some(vec![1, 2]));
    assert_eq!(research.sources[0].url, None);
    let body = transport.body(0);
    assert!(
        body.contains("[стр. 1]") && body.contains("[стр. 2]"),
        "{body}"
    );
    assert!(body.contains("Замыкание хранит окружение, в котором создано."));
    assert!(body.contains("Как замыкание хранит состояние?"));
    assert!(!body.contains("web_search"));
    // Файл PDF в журнале — тот же дословный запрос, ключа в нём нет.
    let entries = journal_entries(&vault).await;
    assert!(entries[0].request.contains("[стр. 1]"));
    assert!(!entries[0].request.contains(KEY));
    // Заметка: пометка, путь PDF, страницы.
    let text = note_text(&vault, &research.note.unwrap());
    assert!(text.contains("Составлено моделью по тексту PDF"), "{text}");
    assert!(text.contains("PDF: Книги\\/Текст.pdf"), "{text}");
    assert!(text.contains("стр. 1, 2"));
    let doc = compile_note_resolved(
        &vault,
        &PackageCache::new(),
        "заметка.typ",
        &text,
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap();
    assert_eq!(
        doc.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        doc.diagnostics
    );
    // Цена без поиска: только токены.
    assert!((entries[0].cost_usd.unwrap() - (0.008 + 0.006)).abs() < 1e-9);

    // Запрещённая папка: отказ до записи, журнала и сети.
    let silent = StreamTransport::with(vec![]);
    let before_events = view_of(&mut vault, &path).await.events_len;
    let before_journal = journal_entries(&vault).await.len();
    let failure = run(
        &mut vault,
        &silent,
        &unique("rv"),
        &path,
        "pdf2",
        ResearchKind::Pdf,
        "Что там?",
        Some("Личное/Секрет.pdf"),
    )
    .await
    .unwrap_err();
    assert_eq!(failure, AiFailure::Forbidden);
    assert_eq!(silent.count(), 0);
    assert_eq!(journal_entries(&vault).await.len(), before_journal);
    assert_eq!(view_of(&mut vault, &path).await.events_len, before_events);
    // Не-PDF и PDF без пути; скан и битый файл отклоняются как нечитаемые.
    for (pdf, expected) in [
        (Some("Книги/Заметка.typ"), AiFailure::Forbidden),
        (None, AiFailure::Forbidden),
        (Some("Книги/Скан.pdf"), AiFailure::LessonUnreadable),
        (Some("Книги/Битая.pdf"), AiFailure::LessonUnreadable),
        (Some("Книги/Нет.pdf"), AiFailure::LessonUnreadable),
    ] {
        let failure = run(
            &mut vault,
            &silent,
            &unique("rv"),
            &path,
            &format!("bad-{pdf:?}"),
            ResearchKind::Pdf,
            "Что там?",
            pdf,
        )
        .await
        .unwrap_err();
        assert_eq!(failure, expected, "{pdf:?}");
    }
    assert_eq!(silent.count(), 0);
}

/// Большой PDF читается не целиком, и оценка об этом говорит; текст в запросе ограничен.
pub async fn a_long_pdf_is_truncated_in_the_request_and_the_estimate_says_so() {
    use typsidian_core::ai::research::PDF_REQUEST_MAX_CHARS;
    // Мелкий шрифт на странице с узкими полями: сотни тысяч символов в немногих страницах.
    let words = PDF_REQUEST_MAX_CHARS / 6 + 12_000;
    let pdf = pdf_of(&format!(
        "#set page(margin: 5pt)\n#set text(size: 2pt)\n#(\"слово \" * {words})"
    ))
    .await;
    let mut vault = vault_with(&settings(AiProvider::Anthropic, "claude-opus-5-5"))
        .await
        .with_file("Книги/Длинная.pdf", pdf);
    let estimate = research_estimate(&vault, "Книги/Длинная.pdf")
        .await
        .unwrap();
    assert_eq!(estimate.problem, None, "{estimate:?}");
    assert!(estimate.truncated, "{estimate:?}");
    assert!(estimate.pages_read < estimate.pages, "{estimate:?}");
    assert!(estimate.chars as usize <= PDF_REQUEST_MAX_CHARS);
    // Запрос несёт ровно столько же, сколько обещала оценка.
    let path = new_lesson(&mut vault).await;
    let transport = StreamTransport::with(vec![reply(anthropic(
        &["ok"],
        Some((
            "submit_research",
            answer_of(&["Прочитано начало."], json!([{"title":"Д","pages":[1]}])),
        )),
        1,
        1,
    ))]);
    let research = run(
        &mut vault,
        &transport,
        &unique("rv"),
        &path,
        "long",
        ResearchKind::Pdf,
        "Что в начале?",
        Some("Книги/Длинная.pdf"),
    )
    .await
    .unwrap();
    assert_eq!(research.status, ResearchStatus::Done);
    let body = transport.body(0);
    assert!(
        body.len() < PDF_REQUEST_MAX_CHARS * 4,
        "{} байт",
        body.len()
    );
    let text = note_text(&vault, &research.note.unwrap());
    assert!(text.contains("Прочитана только начальная часть"), "{text}");
}

async fn pdf_of(note: &str) -> Vec<u8> {
    use typsidian_core::export;
    use typsidian_core::vault::vault_settings::VaultOverrides;
    let mut vault = MemoryVault::new().with_file("Н.typ", note);
    let overrides = VaultOverrides {
        auto_download_templates: Some(false),
        ..VaultOverrides::default()
    };
    let report = export::export_note_pdf(
        &mut vault,
        &PackageCache::new(),
        "Н.typ",
        None,
        FontSet::builtin(),
        &overrides,
    )
    .await;
    vault
        .bytes_of(&report.pdf_path)
        .unwrap_or_else(|| panic!("PDF не записан: {:?}", report.diagnostics))
}

// ── Прочее ──────────────────────────────────────────────────────────────────

/// Настройки: значения по умолчанию, границы и старые файлы без новых полей.
pub async fn research_settings_have_defaults_and_bounds() {
    let mut vault = vault_with(&AiSettings {
        research_parallel: 9,
        ..settings(AiProvider::Anthropic, "claude-opus-5-5")
    })
    .await;
    assert_eq!(load_settings(&vault).await.research_parallel, 4);
    save_settings(
        &mut vault,
        &AiSettings {
            research_parallel: 0,
            ..settings(AiProvider::Anthropic, "claude-opus-5-5")
        },
    )
    .await
    .unwrap();
    assert_eq!(load_settings(&vault).await.research_parallel, 2);
    // Файл настроек прежних версий читается: поиск выключен.
    let old = MemoryVault::new().with_file(
        ".typsidian/ai.json",
        r#"{"enabled":true,"provider":"Anthropic","model":"claude-opus-5-5","knowledge_auto":false}"#,
    );
    let loaded = load_settings(&old).await;
    assert!(loaded.enabled);
    assert!(!loaded.web_research);
    assert_eq!(loaded.research_parallel, 2);
    assert!(!loaded.research_agent_pdf);
    assert!(!web_search_available(&loaded));
    // Повреждённые настройки не включают поиск.
    let broken = MemoryVault::new().with_file(".typsidian/ai.json", "{ нет");
    assert!(!web_search_available(&load_settings(&broken).await));
    let _ = JOURNAL;
}

/// Общий список заданий несёт задания обоих видов; формат событий устойчив.
pub async fn the_shared_job_list_has_both_kinds_and_events_round_trip() {
    let mut vault = web_vault().await;
    let path = new_lesson(&mut vault).await;
    let vault_id = unique("rv");
    let verified = connection(&vault).await;
    begin_research(
        &mut vault,
        &vault_id,
        &path,
        "j1",
        ResearchKind::Web,
        QUESTION,
        None,
        1_000,
        true,
        &verified,
    )
    .await
    .unwrap();
    let jobs = all_jobs(&vault_id);
    assert_eq!(jobs.len(), 1);
    let json = serde_json::to_value(&jobs[0]).unwrap();
    assert_eq!(json["type"], "Research");
    assert_eq!(json["id"], "j1");
    assert_eq!(json["kind"], "Web");
    assert_eq!(json["state"], "Running");
    // События пишутся в файл с тегом `kind`, вид исследования — в поле `research`.
    let line = serde_json::to_value(started("e1", false)).unwrap();
    assert_eq!(line["kind"], "ResearchStarted");
    assert_eq!(line["research"], "Web");
    let back: LessonEvent = serde_json::from_value(line).unwrap();
    assert_eq!(back, started("e1", false));
    let done = LessonEvent::ResearchFailed {
        at: 1,
        id: "e1".into(),
        failure: AiFailure::RateLimited,
        cancelled: false,
    };
    let back: LessonEvent = serde_json::from_str(&serde_json::to_string(&done).unwrap()).unwrap();
    assert_eq!(back, done);
    // Заметка собирается и без модели: пометка языка и страниц.
    let text = build_note(&NoteInput {
        language: Language::En,
        kind: ResearchKind::Pdf,
        question: "Q?",
        pdf: Some("a.pdf"),
        model: "m",
        paragraphs: &["p".to_owned()],
        sources: &[ResearchSource {
            title: "t".into(),
            url: None,
            pages: Some(vec![3, 4]),
        }],
        truncated: true,
    });
    assert!(text.contains("Written by the model"));
    assert!(text.contains("p. 3, 4"));
    assert!(text.contains("Only the beginning"));
}

async fn begin_q(
    vault: &mut MemoryVault,
    vault_id: &str,
    path: &str,
    verified: &Connection,
    id: &str,
    question: &str,
) -> Result<ResearchBegin, AiFailure> {
    begin_research(
        vault,
        vault_id,
        path,
        id,
        ResearchKind::Web,
        question,
        None,
        1_000,
        true,
        verified,
    )
    .await
}
