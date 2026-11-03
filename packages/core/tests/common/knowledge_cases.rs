//! Граф знаний из уроков (фича 054) — общая таблица для обоих таргетов. Сети нет:
//! поддельный потоковый транспорт урока отдаёт заготовленные вызовы `submit_findings`.

#![allow(dead_code)]

use std::collections::HashSet;

use serde_json::{json, Value};
use typsidian_core::ai::cancel::cancel;
use typsidian_core::ai::client::Connection;
use typsidian_core::ai::journal::{journal_entries, JournalState};
use typsidian_core::ai::knowledge::extract::{
    explanations, plan_batches, BATCH_MAX_CHARS, RELATED_MAX,
};
use typsidian_core::ai::knowledge::jobs::{jobs, JobState, KnowledgeJob};
use typsidian_core::ai::knowledge::merge::{merge, parse_findings, Findings};
use typsidian_core::ai::knowledge::search::related;
use typsidian_core::ai::knowledge::store::{
    self, chain_id, normalize_text, project, proposition_id, Projection, PropositionLine,
    SourceKind, SourceLine, CHAINS_FILE, MAX_FILE_BYTES, PROPOSITIONS_FILE, SOURCES_FILE,
};
use typsidian_core::ai::knowledge::{
    agent_results, begin_extract, chains_view, delete_lesson, finish_extract, graph_view,
    proposition_view, remove_proposition, remove_source, select_material, send_extract,
    KnowledgeOutcome,
};
use typsidian_core::ai::lesson::file::{initial_text, FORMAT};
use typsidian_core::ai::lesson::{
    create_lesson, create_lesson_with_knowledge, create_practice_with_knowledge, open_lesson,
    Language, LessonEvent, LessonHeader, LessonKind, LessonPhase, Next, Waiting,
};
use typsidian_core::ai::settings::{load_settings, save_settings, AiProvider, AiSettings};
use typsidian_core::ai::AiFailure;
use typsidian_core::vault::{classify, FileKind};

use super::lesson_cases::{
    anthropic, anthropic_vault, connection, cycle, reply, say, unique, vault_with, Script,
    StreamTransport, JOURNAL, KEY,
};
use super::MemoryVault;

const LESSON: &str = "Уроки/Замыкания.lesson";
const GRAPH: &str = "Уроки/Граф знаний";

fn teacher(at: i64, text: &str) -> LessonEvent {
    LessonEvent::Teacher {
        at,
        text: text.to_owned(),
        question: None,
        interrupted: false,
        failure: None,
        model: "claude-opus-5-5".to_owned(),
        input_tokens: None,
        output_tokens: None,
        cost_usd: None,
    }
}

fn user(at: i64, text: &str) -> LessonEvent {
    LessonEvent::User {
        at,
        text: text.to_owned(),
        choice: None,
        confidence: None,
        thinking: Some("МОИ-РАССУЖДЕНИЯ".to_owned()),
        dont_know: false,
    }
}

fn put_lesson(
    vault: &MemoryVault,
    path: &str,
    kind: LessonKind,
    start: LessonPhase,
    events: &[LessonEvent],
) {
    let header = LessonHeader {
        format: FORMAT,
        goal: "Понять замыкания".to_owned(),
        language: Language::Ru,
        created_at: 1,
        start,
        kind,
    };
    vault.replace_externally(path, initial_text(&header, events).unwrap());
}

fn put_teach(vault: &MemoryVault, path: &str, events: &[LessonEvent]) {
    put_lesson(vault, path, LessonKind::Lesson, LessonPhase::Teach, events);
}

fn findings(propositions: Value, chains: Value) -> Script {
    reply(anthropic(
        &["ok"],
        Some((
            "submit_findings",
            json!({"propositions": propositions, "chains": chains}),
        )),
        120,
        30,
    ))
}

/// Одно утверждение и больше ничего.
fn one(key: &str, text: &str, event: u32) -> Script {
    findings(
        json!([{"key": key, "text": text, "event": event}]),
        json!([]),
    )
}

/// Цикл пачек так, как его ведут host и клиент wasm: ядро решает, продолжать ли.
async fn run(
    vault: &mut MemoryVault,
    transport: &StreamTransport,
    vault_id: &str,
    source: &str,
    request_id: &str,
    now: i64,
) -> Result<KnowledgeJob, AiFailure> {
    let verified = connection(vault).await;
    for _ in 0..50 {
        let Some(prepared) =
            begin_extract(vault, vault_id, source, request_id, now, true, &verified).await?
        else {
            break;
        };
        let outcome = send_extract(transport, vault_id, &prepared, Some(KEY), &verified).await;
        let done = finish_extract(vault, vault_id, &prepared, now + 1, outcome).await?;
        if done.next == Next::WaitUser {
            return Ok(done.job);
        }
    }
    Ok(jobs(vault_id)
        .into_iter()
        .find(|job| job.id == request_id)
        .expect("задание в реестре"))
}

async fn graph_of(vault: &MemoryVault) -> Projection {
    store::load(vault, &load_settings(vault).await).await
}

fn texts(projection: &Projection) -> Vec<String> {
    let mut texts: Vec<String> = projection
        .propositions
        .values()
        .map(|entry| entry.phrasings[0].text.clone())
        .collect();
    texts.sort();
    texts
}

fn text(s: &str) -> String {
    s.to_owned()
}

fn long(sentence: &str, chars: usize) -> String {
    let mut out = String::new();
    while out.chars().count() < chars {
        out.push_str(sentence);
        out.push(' ');
    }
    out
}

// ── T001: хранилище ─────────────────────────────────────────────────────────

pub async fn graph_files_are_other_kind_and_projection_applies_tombstones() {
    for file in [PROPOSITIONS_FILE, CHAINS_FILE, SOURCES_FILE] {
        let path = format!("{GRAPH}/{file}");
        assert_eq!(classify(std::path::Path::new(&path)), FileKind::Other);
    }
    let a = proposition_id("Замыкание хранит окружение");
    let b = proposition_id("Функция — значение");
    let line = |value: Value| format!("{value}\n");
    let phrasing = |id: &str, text: &str, source: &str, at: i64| {
        line(json!({"line":"Phrasing","id":id,"text":text,"source":source,"event":1,"at":at}))
    };
    let props = [
        phrasing(&a, "Замыкание хранит окружение", "Уроки/А.lesson", 10),
        phrasing(&b, "Функция — значение", "Уроки/А.lesson", 10),
        phrasing(&a, "Замыкание помнит окружение", "Уроки/Б.lesson", 20),
        line(json!({"line":"Removed","id":b,"at":30})),
    ]
    .concat();
    let chains = line(json!({"line":"Chain","id":chain_id(&[a.clone()], &b),
        "premises":[a],"conclusion":b,"source":"Уроки/А.lesson","event":2,"at":10}));
    let sources = [
        line(json!({"line":"Covered","source":"Уроки/А.lesson","kind":"Lesson","upto":7,"at":11})),
        line(json!({"line":"Covered","source":"Уроки/А.lesson","kind":"Lesson","upto":3,"at":12})),
        line(json!({"line":"SourceRemoved","source":"Уроки/Б.lesson","keep":false,"at":25})),
    ]
    .concat();
    let projection = project(props.as_bytes(), chains.as_bytes(), sources.as_bytes());
    // Надгробие скрыло `b`; вклад Б убран (строка раньше надгробия источника), А остался.
    assert!(!projection.propositions.contains_key(&b));
    let entry = &projection.propositions[&a];
    assert_eq!(entry.phrasings.len(), 1);
    assert_eq!(entry.phrasings[0].source, "Уроки/А.lesson");
    // Покрытие — строка с наибольшим `at`, а не максимум: пересозданный короче урок станет
    // покрытым (ревью 054, H1).
    assert_eq!(projection.covered["Уроки/А.lesson"], 3);

    // Строка позже надгробия оживляет утверждение.
    let revived = format!(
        "{props}{}",
        phrasing(&b, "Функция — значение", "Уроки/А.lesson", 40)
    );
    let projection = project(revived.as_bytes(), chains.as_bytes(), sources.as_bytes());
    assert!(projection.propositions.contains_key(&b));
}

pub async fn moved_lines_make_the_source_follow_a_rename() {
    let a = proposition_id("Утверждение раз");
    let c = proposition_id("Утверждение два");
    let phrasing = |id: &str, source: &str, at: i64| {
        format!(
            "{}\n",
            json!({"line":"Phrasing","id":id,"text":"т","source":source,"at":at})
        )
    };
    let props = [
        phrasing(&a, "Уроки/Старая/А.lesson", 10),
        phrasing(&c, "Уроки/Старая/А.lesson", 60),
    ]
    .concat();
    let sources = [
        format!(
            "{}\n",
            json!({"line":"Covered","source":"Уроки/Старая/А.lesson","kind":"Lesson","upto":4,"at":11})
        ),
        // Папка переехала целиком в момент 50: всё записанное до — на новом месте.
        format!(
            "{}\n",
            json!({"line":"Moved","from":"Уроки/Старая","to":"Уроки/Новая","at":50})
        ),
    ]
    .concat();
    let projection = project(props.as_bytes(), b"", sources.as_bytes());
    assert_eq!(
        projection.propositions[&a].phrasings[0].source,
        "Уроки/Новая/А.lesson"
    );
    // Строка после переноса пишет уже новое имя урока: она не переносится.
    assert_eq!(
        projection.propositions[&c].phrasings[0].source,
        "Уроки/Старая/А.lesson"
    );
    assert_eq!(projection.covered["Уроки/Новая/А.lesson"], 4);

    // Через vault: `move_source` пишет строку `Moved`, когда граф уже есть.
    let mut vault = anthropic_vault().await;
    let settings = load_settings(&vault).await;
    store::append_sources(
        &mut vault,
        &settings,
        &[SourceLine::Covered {
            source: text("Уроки/А.lesson"),
            kind: SourceKind::Lesson,
            upto: 3,
            at: 5,
        }],
    )
    .await
    .unwrap();
    store::move_source(
        &mut vault,
        std::path::Path::new("Уроки/А.lesson"),
        std::path::Path::new("Уроки/В.lesson"),
        9,
    )
    .await;
    assert_eq!(graph_of(&vault).await.covered["Уроки/В.lesson"], 3);
    // Графа нет — переносить нечего, файлов не появляется.
    let mut empty = anthropic_vault().await;
    store::move_source(
        &mut empty,
        std::path::Path::new("Уроки/А.lesson"),
        std::path::Path::new("Уроки/В.lesson"),
        9,
    )
    .await;
    assert!(empty.paths_under(GRAPH).is_empty());
}

pub async fn hostile_graph_files_are_read_with_limits() {
    let a = proposition_id("Нормальное утверждение");
    let mut props = String::new();
    props.push_str("{ не json\n");
    props.push_str("\u{0}\u{1}\n");
    props.push_str(&format!(
        "{}\n",
        json!({"line":"Phrasing","id":"p_нет","text":"плохой id","source":"х","at":1})
    ));
    props.push_str(&format!(
        "{}\n",
        json!({"line":"Phrasing","id":"c_0123456789abcdef","text":"не тот префикс","source":"х","at":1})
    ));
    props.push_str(&format!(
        "{}\n",
        json!({"line":"Phrasing","id":a,"text":"x".repeat(100_000),"source":"s","at":1})
    ));
    props.push_str(&format!(
        "{}\n",
        json!({"line":"Phrasing","id":a,"text":"Нормальное утверждение","source":"Уроки/А.lesson","at":2})
    ));
    // Огромная строка пропускается, не разбираясь, и следующие за ней живут.
    props.push_str(&"я".repeat(1_000_000));
    props.push('\n');
    props.push_str(&format!(
        "{}\n",
        json!({"line":"Phrasing","id":a,"text":"После огромной","source":"Уроки/Б.lesson","at":3})
    ));
    let projection = project(props.as_bytes(), b"", b"");
    assert_eq!(projection.propositions.len(), 1);
    let phrasings = &projection.propositions[&a].phrasings;
    // Длинная формулировка обрезана до предела, а не отвергнута и не растянута.
    assert!(phrasings.iter().all(|p| p.text.chars().count() <= 300));
    assert!(phrasings.iter().any(|p| p.text == "После огромной"));

    // Формулировок на утверждение — не больше предела; посылок в цепочке — не больше.
    let many: String = (0..100)
        .map(|i| {
            format!(
                "{}\n",
                json!({"line":"Phrasing","id":a,"text":format!("вариант {i}"),"source":"s","at":i})
            )
        })
        .collect();
    let projection = project(many.as_bytes(), b"", b"");
    assert!(projection.propositions[&a].phrasings.len() <= store::MAX_PHRASINGS);
    let wide = format!(
        "{}\n",
        json!({"line":"Chain","id":chain_id(&[a.clone()], &a),"premises":vec![a.clone(); 50],
            "conclusion":a,"source":"s","at":1})
    );
    assert!(project(many.as_bytes(), wide.as_bytes(), b"")
        .chains
        .is_empty());

    // Файл больше предела не читается и не растёт: проекция «повреждена», запись отказывает.
    let mut vault = anthropic_vault().await;
    let settings = load_settings(&vault).await;
    let huge = vec![b'\n'; MAX_FILE_BYTES + 1];
    vault.replace_externally(&format!("{GRAPH}/{PROPOSITIONS_FILE}"), huge.clone());
    let projection = store::load(&vault, &settings).await;
    assert!(projection.damaged && projection.propositions.is_empty());
    let line = PropositionLine::Removed { id: a, at: 1 };
    assert_eq!(
        store::append_propositions(&mut vault, &settings, &[line])
            .await
            .unwrap_err(),
        AiFailure::JournalFailed
    );
    assert_eq!(
        vault
            .bytes_of(&format!("{GRAPH}/{PROPOSITIONS_FILE}"))
            .unwrap(),
        huge
    );
}

pub async fn graph_parsing_is_linear_on_a_hostile_file() {
    // 150 тыс. строк мусора и 50 тыс. настоящих читаются, не растя время квадратично.
    let mut props = String::new();
    for i in 0..150_000 {
        props.push_str(if i % 3 == 0 {
            "{мусор\n"
        } else {
            "[1,2\n"
        });
    }
    for i in 0..50_000 {
        let id = proposition_id(&format!("утверждение номер {i}"));
        props.push_str(&format!(
            "{}\n",
            json!({"line":"Phrasing","id":id,"text":format!("утверждение номер {i}"),"source":"s","at":1})
        ));
    }
    let started = typsidian_core::properties::now_ms();
    let projection = project(props.as_bytes(), b"", b"");
    let elapsed = typsidian_core::properties::now_ms() - started;
    assert!(projection.propositions.len() <= store::MAX_PROPOSITIONS);
    assert!(projection.propositions.len() > 40_000);
    // Запас на медленные машины и wasm в браузере.
    assert!(elapsed < 20_000, "{elapsed} мс");
}

pub async fn append_is_atomic_and_never_goes_through_a_symlink() {
    let mut vault = anthropic_vault().await;
    let settings = load_settings(&vault).await;
    let line = SourceLine::Covered {
        source: text(LESSON),
        kind: SourceKind::Lesson,
        upto: 1,
        at: 1,
    };
    // Оборванная последняя строка отделяется, прежнее не меняется.
    vault.replace_externally(&format!("{GRAPH}/{SOURCES_FILE}"), "{\"line\":\"Cov");
    store::append_sources(&mut vault, &settings, &[line.clone()])
        .await
        .unwrap();
    let raw =
        String::from_utf8(vault.bytes_of(&format!("{GRAPH}/{SOURCES_FILE}")).unwrap()).unwrap();
    assert!(raw.starts_with("{\"line\":\"Cov\n{\"line\":\"Covered\""));
    assert_eq!(graph_of(&vault).await.covered[LESSON], 1);

    // Ссылка на файл, на папку графа и на папку уроков: записи нет.
    for link in [
        format!("{GRAPH}/{SOURCES_FILE}"),
        GRAPH.to_owned(),
        "Уроки".to_owned(),
    ] {
        let mut vault = anthropic_vault().await.with_symlink(&link);
        let before = vault.paths_under("");
        assert_eq!(
            store::append_sources(&mut vault, &settings, &[line.clone()])
                .await
                .unwrap_err(),
            AiFailure::JournalFailed,
            "{link}"
        );
        assert_eq!(vault.paths_under(""), before, "{link}");
        // Читать через ссылку тоже нельзя: граф «повреждён», а не чужой.
        assert!(store::load(&vault, &settings).await.damaged, "{link}");
    }

    // Сбой записи не оставляет полуфайла.
    let mut failing = anthropic_vault()
        .await
        .with_write_failure(&format!("{GRAPH}/{SOURCES_FILE}"));
    assert_eq!(
        store::append_sources(&mut failing, &settings, &[line])
            .await
            .unwrap_err(),
        AiFailure::JournalFailed
    );
    assert!(failing
        .bytes_of(&format!("{GRAPH}/{SOURCES_FILE}"))
        .is_none());
}

// ── T002: настройки ─────────────────────────────────────────────────────────

pub async fn knowledge_settings_have_defaults_and_old_files_still_read() {
    let defaults = AiSettings::default();
    assert!(defaults.knowledge_auto && defaults.knowledge_map);
    assert!(!defaults.knowledge_agent_reads);
    // ai.json без новых полей (написан до 054) читается с умолчаниями.
    let vault = MemoryVault::new().with_file(
        ".typsidian/ai.json",
        r#"{"enabled":true,"provider":"Anthropic","model":"m","lessons_folder":"Уроки"}"#,
    );
    let loaded = load_settings(&vault).await;
    assert!(loaded.enabled && loaded.knowledge_auto && loaded.knowledge_map);
    assert!(!loaded.knowledge_agent_reads);
    // Значения переживают запись.
    let mut vault = anthropic_vault().await;
    let mut settings = load_settings(&vault).await;
    settings.knowledge_auto = false;
    settings.knowledge_agent_reads = true;
    settings.knowledge_map = false;
    save_settings(&mut vault, &settings).await.unwrap();
    let loaded = load_settings(&vault).await;
    assert!(!loaded.knowledge_auto && loaded.knowledge_agent_reads && !loaded.knowledge_map);
}

// ── T003: пачки и запрос ────────────────────────────────────────────────────

fn header(kind: LessonKind, start: LessonPhase) -> LessonHeader {
    LessonHeader {
        format: FORMAT,
        goal: text("г"),
        language: Language::Ru,
        created_at: 1,
        start,
        kind,
    }
}

pub async fn batches_take_only_complete_teach_phase_explanations() {
    let mut interrupted = teacher(0, "оборванное");
    if let LessonEvent::Teacher {
        interrupted: flag, ..
    } = &mut interrupted
    {
        *flag = true;
    }
    let events = vec![
        teacher(1, "замер: вопрос про ваш ответ"), // 0 — стадия замера
        user(2, "мой ответ"),                      // 1
        LessonEvent::ProbeResult {
            at: 3,
            parts: vec![],
            start: text("с"),
        }, // 2
        teacher(4, "объяснение раз"),              // 3
        interrupted,                               // 4
        teacher(5, "   "),                         // 5
        LessonEvent::Summary {
            at: 6,
            text: text("пересказ с ответом"),
            covers: 4,
            model: None,
            input_tokens: None,
            output_tokens: None,
            cost_usd: None,
        }, // 6
        teacher(7, "объяснение два"),              // 7
    ];
    let items = explanations(&header(LessonKind::Lesson, LessonPhase::Probe), &events, 0);
    let got: Vec<(u32, &str)> = items.iter().map(|i| (i.index, i.text.as_str())).collect();
    assert_eq!(got, vec![(3, "объяснение раз"), (7, "объяснение два")]);
    // Покрытие пропускает уже разобранное; покрытие больше файла — «другой файл», берётся всё.
    assert_eq!(
        explanations(&header(LessonKind::Lesson, LessonPhase::Probe), &events, 4).len(),
        1
    );
    assert_eq!(
        explanations(&header(LessonKind::Lesson, LessonPhase::Probe), &events, 99).len(),
        2
    );
    // Практика не извлекается вовсе.
    assert!(explanations(
        &header(LessonKind::Practice, LessonPhase::Teach),
        &events,
        0
    )
    .is_empty());

    // Пачки до 20 тыс. символов; хвост файла покрывает последняя; громадное объяснение режется.
    let big: Vec<LessonEvent> = (0..5).map(|i| teacher(i, &"а".repeat(9_000))).collect();
    let batches = plan_batches(
        explanations(&header(LessonKind::Lesson, LessonPhase::Teach), &big, 0),
        big.len(),
    );
    assert_eq!(batches.len(), 3);
    assert!(batches.iter().all(|b| b
        .items
        .iter()
        .map(|i| i.text.chars().count())
        .sum::<usize>()
        <= BATCH_MAX_CHARS));
    assert_eq!(batches[0].upto, 2);
    assert_eq!(batches.last().unwrap().upto, 5);
    let giant = vec![teacher(0, &"б".repeat(50_000))];
    let batches = plan_batches(
        explanations(&header(LessonKind::Lesson, LessonPhase::Teach), &giant, 0),
        1,
    );
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].items[0].text.chars().count(), BATCH_MAX_CHARS);
}

pub async fn request_has_only_teacher_explanations_and_is_journaled_before_the_network() {
    let mut vault = anthropic_vault().await;
    put_teach(
        &vault,
        LESSON,
        &[
            user(1, "СЕКРЕТНЫЙ-ОТВЕТ-ПОЛЬЗОВАТЕЛЯ"),
            teacher(2, "Замыкание — это функция вместе с окружением."),
            user(3, "МОЙ-ВТОРОЙ-ОТВЕТ"),
            teacher(4, "Окружение хранит переменные, видимые при создании."),
            LessonEvent::Summary {
                at: 5,
                text: text("ПЕРЕСКАЗ-С-ОТВЕТАМИ"),
                covers: 4,
                model: None,
                input_tokens: None,
                output_tokens: None,
                cost_usd: None,
            },
        ],
    );
    let transport = StreamTransport::with(vec![one("a", "Замыкание — функция с окружением", 1)]);
    let vault_id = unique("kv");
    let verified = connection(&vault).await;
    let request_id = unique("r");
    let prepared = begin_extract(
        &mut vault,
        &vault_id,
        LESSON,
        &request_id,
        100,
        true,
        &verified,
    )
    .await
    .unwrap()
    .unwrap();
    // Журнал получил запрос до сети (сети ещё не было), ключа в нём нет.
    assert_eq!(transport.count(), 0);
    let entries = journal_entries(&vault).await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].state, JournalState::Interrupted);
    assert_eq!(
        Some(entries[0].request.as_str()),
        prepared.request.body.as_deref()
    );
    assert!(!vault
        .bytes_of(JOURNAL)
        .map(|b| String::from_utf8_lossy(&b).contains(KEY))
        .unwrap_or(false));

    let outcome = send_extract(&transport, &vault_id, &prepared, Some(KEY), &verified).await;
    finish_extract(&mut vault, &vault_id, &prepared, 101, outcome)
        .await
        .unwrap();
    let body = transport.body(0);
    // SC-004: объяснения учителя есть, ответов и рассуждений пользователя, пересказа — нет.
    assert!(body.contains("Замыкание — это функция вместе с окружением."));
    assert!(body.contains("Окружение хранит переменные"));
    assert!(body.contains("submit_findings"));
    for forbidden in [
        "СЕКРЕТНЫЙ-ОТВЕТ-ПОЛЬЗОВАТЕЛЯ",
        "МОЙ-ВТОРОЙ-ОТВЕТ",
        "МОИ-РАССУЖДЕНИЯ",
        "ПЕРЕСКАЗ-С-ОТВЕТАМИ",
    ] {
        assert!(!body.contains(forbidden), "{forbidden}");
    }
    // Итог — в журнал.
    assert_eq!(journal_entries(&vault).await[0].state, JournalState::Done);
}

pub async fn extraction_of_a_practice_or_probe_sends_nothing() {
    let mut vault = anthropic_vault().await;
    let practice = "Уроки/Практика А.lesson";
    put_lesson(
        &vault,
        practice,
        LessonKind::Practice,
        LessonPhase::Teach,
        &[
            LessonEvent::Problem {
                at: 1,
                n: 1,
                statement: text("ЗАДАЧА"),
                solution: Some(text("РЕШЕНИЕ-ЗАДАЧИ")),
            },
            LessonEvent::Hint {
                at: 2,
                n: 1,
                text: text("ПОДСКАЗКА"),
            },
            teacher(3, "Разбор вашего ответа и решение"),
        ],
    );
    put_lesson(
        &vault,
        LESSON,
        LessonKind::Lesson,
        LessonPhase::Probe,
        &[teacher(1, "Верно ли, что вы знаете про замыкания?")],
    );
    let transport = StreamTransport::with(vec![]);
    for source in [practice, LESSON] {
        let job = run(
            &mut vault,
            &transport,
            &unique("kv"),
            source,
            &unique("r"),
            10,
        )
        .await
        .unwrap();
        assert_eq!(job.state, JobState::Done);
        assert_eq!(job.batches_total, 0, "{source}");
    }
    assert_eq!(transport.count(), 0);
    assert!(journal_entries(&vault).await.is_empty());
    assert!(vault.paths_under(GRAPH).is_empty());
}

// ── T004: объединение ───────────────────────────────────────────────────────

pub async fn findings_parse_has_limits_and_accepts_both_spellings() {
    assert!(parse_findings("не json").is_none());
    assert!(parse_findings("[1]").is_none());
    assert_eq!(parse_findings("").unwrap(), Findings::default());
    let props: Vec<Value> = (0..100)
        .map(|i| json!({"key": format!("k{i}"), "text": "т".repeat(1_000)}))
        .collect();
    let chains: Vec<Value> = (0..100)
        .map(|_| json!({"premises": ["a","b","c","d","e","f","g","h","i"], "conclusion": "z"}))
        .collect();
    let parsed =
        parse_findings(&json!({"propositions": props, "chains": chains}).to_string()).unwrap();
    assert_eq!(parsed.propositions.len(), 40);
    assert!(parsed
        .propositions
        .iter()
        .all(|p| p.text.chars().count() == 300));
    assert_eq!(parsed.chains.len(), 30);
    assert!(parsed.chains.iter().all(|c| c.premises.len() <= 7));
    // `conclude` — синоним `conclusion`; пустые тексты пропускаются.
    let parsed = parse_findings(
        &json!({"propositions":[{"key":"a","text":"  "},{"key":"b","text":"Б"}],
            "chains":[{"premises":["b"],"conclude":"b2","event":3}]})
        .to_string(),
    )
    .unwrap();
    assert_eq!(parsed.propositions.len(), 1);
    assert_eq!(parsed.chains[0].conclusion, "b2");
    assert_eq!(parsed.chains[0].event, Some(3));
}

pub async fn ids_come_from_normalized_text() {
    assert_eq!(
        proposition_id("  Замыкание   помнит окружение. "),
        proposition_id("замыкание помнит окружение")
    );
    assert_eq!(normalize_text("«Да!»"), "да");
    assert_ne!(proposition_id("раз"), proposition_id("два"));
    let (a, b, c) = (
        proposition_id("а"),
        proposition_id("б"),
        proposition_id("в"),
    );
    assert_eq!(
        chain_id(&[a.clone(), b.clone()], &c),
        chain_id(&[b.clone(), a.clone()], &c)
    );
    assert_ne!(chain_id(&[a.clone()], &c), chain_id(&[b], &c));
    assert!(proposition_id("х").starts_with("p_") && chain_id(&[a], &c).starts_with("c_"));
}

pub async fn merge_gives_no_duplicates_same_as_and_rejects_dangling_chains() {
    let found = parse_findings(
        &json!({"propositions":[
            {"key":"a","text":"Замыкание хранит окружение","event":2},
            {"key":"b","text":"Окружение — набор переменных","event":2},
            {"key":"c","text":"Замыкание видит переменные","event":99}],
          "chains":[
            {"premises":["a","b"],"conclusion":"c","event":2},
            {"premises":["a"],"conclusion":"нет-такого"},
            {"premises":["p_0123456789abcdef"],"conclusion":"c"},
            {"premises":["c"],"conclusion":"c"},
            {"premises":[],"conclusion":"c"}]})
        .to_string(),
    )
    .unwrap();
    let plan = merge(
        &Projection::default(),
        &found,
        LESSON,
        &[2, 4],
        100,
        &HashSet::new(),
    );
    assert_eq!((plan.propositions_added, plan.propositions_merged), (3, 0));
    assert_eq!(plan.chains_added, 1);
    assert_eq!(plan.chains_rejected, 4);
    assert_eq!((plan.propositions.len(), plan.chains.len()), (3, 1));
    // Событие вне пачки заменено первым событием пачки.
    assert!(plan.propositions.iter().any(|line| matches!(line,
        PropositionLine::Phrasing { event: Some(2), text, .. } if text.starts_with("Замыкание видит"))));

    // Применили; повтор той же пачки — 0 новых утверждений, строк и цепочек (SC-002).
    let mut vault = anthropic_vault().await;
    let settings = load_settings(&vault).await;
    store::append_propositions(&mut vault, &settings, &plan.propositions)
        .await
        .unwrap();
    store::append_chains(&mut vault, &settings, &plan.chains)
        .await
        .unwrap();
    let projection = graph_of(&vault).await;
    let again = merge(&projection, &found, LESSON, &[2, 4], 200, &HashSet::new());
    assert_eq!((again.propositions_added, again.chains_added), (0, 0));
    assert_eq!(again.propositions_merged, 3);
    assert!(again.propositions.is_empty() && again.chains.is_empty());

    // То же утверждение из другого урока: дописывается источник, а не новое утверждение.
    let other = merge(
        &projection,
        &found,
        "Уроки/Другой.lesson",
        &[2],
        300,
        &HashSet::new(),
    );
    assert_eq!(other.propositions_added, 0);
    assert_eq!(other.propositions.len(), 3);
    assert_eq!(other.chains.len(), 1);
    assert_eq!(other.chains_added, 0);

    // `same_as`: другая формулировка того же смысла идёт к существующему id.
    let existing = proposition_id("Замыкание хранит окружение");
    let renamed = parse_findings(
        &json!({"propositions":[{"key":"x","text":"Замыкание помнит окружение","same_as":existing},
            {"key":"y","text":"Новое","same_as":"p_ffffffffffffffff"}]})
        .to_string(),
    )
    .unwrap();
    let plan = merge(
        &projection,
        &renamed,
        "Уроки/Другой.lesson",
        &[1],
        400,
        &HashSet::from([existing.clone()]),
    );
    assert_eq!((plan.propositions_added, plan.propositions_merged), (1, 1));
    assert!(plan.propositions.iter().any(|line| matches!(line,
        PropositionLine::Phrasing { id, text, .. } if *id == existing && text == "Замыкание помнит окружение")));
    // Ссылка в пустоту не теряет утверждение: оно становится новым по своему хэшу.
    assert!(plan.propositions.iter().any(|line| matches!(line,
        PropositionLine::Phrasing { id, .. } if *id == proposition_id("Новое"))));
}

// ── T005/T006: задания и извлечение целиком ─────────────────────────────────

pub async fn three_explanations_become_propositions_with_sources_and_repeat_adds_nothing() {
    let mut vault = anthropic_vault().await;
    put_teach(
        &vault,
        LESSON,
        &[
            teacher(1, "Замыкание — это функция вместе с окружением."),
            user(2, "ага"),
            teacher(3, "Окружение — переменные, видимые при создании функции."),
            user(4, "понял"),
            teacher(5, "Поэтому замыкание помнит значения переменных."),
        ],
    );
    let transport = StreamTransport::with(vec![findings(
        json!([
            {"key":"a","text":"Замыкание — функция вместе с окружением","event":0},
            {"key":"b","text":"Окружение — переменные, видимые при создании функции","event":2},
            {"key":"c","text":"Замыкание помнит значения переменных","event":4}]),
        json!([{"premises":["a","b"],"conclusion":"c","event":4}]),
    )]);
    let vault_id = unique("kv");
    let request = unique("r");
    let job = run(&mut vault, &transport, &vault_id, LESSON, &request, 100)
        .await
        .unwrap();
    assert_eq!(job.state, JobState::Done);
    assert_eq!((job.batches_total, job.batches_done), (1, 1));
    assert_eq!(
        (
            job.propositions_added,
            job.chains_added,
            job.chains_rejected
        ),
        (3, 1, 0)
    );
    assert_eq!(job.failure, None);

    let projection = graph_of(&vault).await;
    assert_eq!(projection.propositions.len(), 3);
    assert_eq!(projection.chains.len(), 1);
    assert_eq!(projection.covered[LESSON], 5);
    let a = proposition_id("Замыкание — функция вместе с окружением");
    let phrasing = &projection.propositions[&a].phrasings[0];
    assert_eq!(
        (phrasing.source.as_str(), phrasing.event),
        (LESSON, Some(0))
    );
    // Файлы графа лежат рядом с уроками и читаются как обычный JSON Lines.
    for file in [PROPOSITIONS_FILE, CHAINS_FILE, SOURCES_FILE] {
        let raw = String::from_utf8(vault.bytes_of(&format!("{GRAPH}/{file}")).unwrap()).unwrap();
        assert!(raw
            .lines()
            .all(|line| serde_json::from_str::<Value>(line).is_ok()));
    }
    // Задание видно в реестре, у другого vault — нет.
    assert_eq!(jobs(&vault_id).len(), 1);
    assert!(jobs("другой-vault").is_empty());

    // Повтор: всё покрыто — запроса нет.
    let second = run(&mut vault, &transport, &vault_id, LESSON, &unique("r"), 200)
        .await
        .unwrap();
    assert_eq!(second.batches_total, 0);
    assert_eq!(transport.count(), 1);

    // Покрытие потеряно (файл sources.jsonl стёрт), модель отвечает тем же: 0 дублей.
    vault.remove(std::path::Path::new(&format!("{GRAPH}/{SOURCES_FILE}")));
    let transport = StreamTransport::with(vec![findings(
        json!([
            {"key":"a","text":"Замыкание — функция вместе с окружением.","event":0},
            {"key":"b","text":"окружение — переменные, видимые при создании функции","event":2},
            {"key":"c","text":"Замыкание помнит значения переменных","event":4}]),
        json!([{"premises":["b","a"],"conclusion":"c","event":4}]),
    )]);
    let third = run(&mut vault, &transport, &vault_id, LESSON, &unique("r"), 300)
        .await
        .unwrap();
    assert_eq!((third.propositions_added, third.chains_added), (0, 0));
    assert_eq!(third.propositions_merged, 3);
    let after = graph_of(&vault).await;
    assert_eq!(after.propositions.len(), 3);
    assert_eq!(after.chains.len(), 1);
    assert!(after
        .propositions
        .values()
        .all(|entry| entry.phrasings.len() == 1));
}

pub async fn existing_statements_are_found_locally_and_same_as_merges_them() {
    let mut vault = anthropic_vault().await;
    put_teach(
        &vault,
        LESSON,
        &[teacher(1, "Замыкание хранит окружение функции.")],
    );
    let first = StreamTransport::with(vec![one("a", "Замыкание хранит окружение функции", 0)]);
    run(&mut vault, &first, &unique("kv"), LESSON, &unique("r"), 10)
        .await
        .unwrap();
    // Много посторонних утверждений: в запрос попадают только найденные по словам, до 50.
    let mut unrelated = String::new();
    for i in 0..80 {
        let text = format!("Фотосинтез номер {i} идёт в хлоропластах");
        unrelated.push_str(&format!(
            "{}\n",
            json!({"line":"Phrasing","id":proposition_id(&text),"text":text,"source":"Уроки/Био.lesson","at":1})
        ));
    }
    let mut related_many = String::new();
    for i in 0..80 {
        let text = format!("Замыкание вариант {i} связано с окружением");
        related_many.push_str(&format!(
            "{}\n",
            json!({"line":"Phrasing","id":proposition_id(&text),"text":text,"source":"Уроки/Био.lesson","at":1})
        ));
    }
    let settings = load_settings(&vault).await;
    let current = String::from_utf8(
        vault
            .bytes_of(&format!("{GRAPH}/{PROPOSITIONS_FILE}"))
            .unwrap(),
    )
    .unwrap();
    vault.replace_externally(
        &format!("{GRAPH}/{PROPOSITIONS_FILE}"),
        format!("{current}{unrelated}{related_many}"),
    );
    let _ = settings;

    let second_lesson = "Уроки/Второй.lesson";
    put_teach(
        &vault,
        second_lesson,
        &[teacher(1, "Замыкание помнит окружение, в котором создано.")],
    );
    let existing = proposition_id("Замыкание хранит окружение функции");
    let transport = StreamTransport::with(vec![findings(
        json!([{"key":"a","text":"Замыкание помнит окружение, где создано","same_as":existing,"event":0}]),
        json!([]),
    )]);
    let job = run(
        &mut vault,
        &transport,
        &unique("kv"),
        second_lesson,
        &unique("r"),
        20,
    )
    .await
    .unwrap();
    assert_eq!((job.propositions_added, job.propositions_merged), (0, 1));
    let body = transport.body(0);
    assert!(body.contains(&existing));
    assert!(!body.contains("Фотосинтез"));
    assert!(
        body.matches("- p_").count() <= RELATED_MAX,
        "{}",
        body.matches("- p_").count()
    );
    // Формулировка дописана к существующему, источников два, нового утверждения нет.
    let projection = graph_of(&vault).await;
    let entry = &projection.propositions[&existing];
    assert_eq!(entry.phrasings.len(), 2);
    let sources: Vec<&str> = entry.phrasings.iter().map(|p| p.source.as_str()).collect();
    assert!(sources.contains(&LESSON) && sources.contains(&second_lesson));
    assert!(!projection
        .propositions
        .contains_key(&proposition_id("Замыкание помнит окружение, где создано")));
}

pub async fn dangling_chain_is_rejected_and_counted() {
    let mut vault = anthropic_vault().await;
    put_teach(&vault, LESSON, &[teacher(1, "Любое объяснение.")]);
    let transport = StreamTransport::with(vec![findings(
        json!([{"key":"a","text":"Единственное утверждение"}]),
        json!([{"premises":["a"],"conclusion":"b"},{"premises":["ghost"],"conclusion":"a"}]),
    )]);
    let job = run(
        &mut vault,
        &transport,
        &unique("kv"),
        LESSON,
        &unique("r"),
        10,
    )
    .await
    .unwrap();
    assert_eq!(
        (
            job.propositions_added,
            job.chains_added,
            job.chains_rejected
        ),
        (1, 0, 2)
    );
    let projection = graph_of(&vault).await;
    assert!(projection.chains.is_empty());
    assert!(!vault
        .paths_under(GRAPH)
        .iter()
        .any(|p| p.ends_with(CHAINS_FILE)));
}

pub async fn a_cut_between_batches_keeps_what_is_done_and_a_retry_takes_the_rest() {
    let mut vault = anthropic_vault().await;
    let events: Vec<LessonEvent> = (0..3)
        .map(|i| teacher(i, &long(&format!("Объяснение {i} о замыканиях."), 12_000)))
        .collect();
    put_teach(&vault, LESSON, &events);
    // Три пачки по одному объяснению; вторая обрывается.
    let transport = StreamTransport::with(vec![
        one("a", "Утверждение первой пачки", 0),
        Script::Cut {
            chunks: anthropic(&["час"], None, 1, 1)[..3].to_vec(),
            failure: AiFailure::Offline,
        },
    ]);
    let vault_id = unique("kv");
    let job = run(&mut vault, &transport, &vault_id, LESSON, &unique("r"), 10)
        .await
        .unwrap();
    assert_eq!(job.state, JobState::Failed);
    assert_eq!(job.failure, Some(AiFailure::Offline));
    assert_eq!((job.batches_total, job.batches_done), (3, 1));
    let projection = graph_of(&vault).await;
    assert_eq!(
        texts(&projection),
        vec!["Утверждение первой пачки".to_owned()]
    );
    assert_eq!(projection.covered[LESSON], 1);
    // Журнал: первая пачка выполнена, вторая записана как неудачная.
    let entries = journal_entries(&vault).await;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].failure, Some(AiFailure::Offline));
    assert_eq!(entries[1].state, JournalState::Done);

    // Повтор берёт две оставшиеся пачки и ничего не дублирует.
    let retry = StreamTransport::with(vec![
        findings(
            json!([{"key":"a","text":"Утверждение первой пачки"},{"key":"b","text":"Вторая"}]),
            json!([]),
        ),
        one("c", "Третья", 2),
    ]);
    let job = run(&mut vault, &retry, &vault_id, LESSON, &unique("r"), 20)
        .await
        .unwrap();
    assert_eq!(job.state, JobState::Done);
    assert_eq!((job.batches_total, job.batches_done), (2, 2));
    assert_eq!(retry.count(), 2);
    assert!(!retry.body(0).contains("Объяснение 0 о замыканиях"));
    assert_eq!(graph_of(&vault).await.propositions.len(), 3);
    assert_eq!(graph_of(&vault).await.covered[LESSON], 3);
}

pub async fn a_bad_reply_leaves_the_batch_uncovered() {
    let mut vault = anthropic_vault().await;
    put_teach(&vault, LESSON, &[teacher(1, "Объяснение.")]);
    // Ни вызова инструмента, ни JSON: покрытие не двигается.
    for script in [
        say("просто текст"),
        reply(anthropic(
            &["x"],
            Some(("submit_findings", json!("не объект"))),
            1,
            1,
        )),
    ] {
        let transport = StreamTransport::with(vec![script]);
        let job = run(
            &mut vault,
            &transport,
            &unique("kv"),
            LESSON,
            &unique("r"),
            10,
        )
        .await
        .unwrap();
        assert_eq!(
            (job.state, job.failure),
            (JobState::Failed, Some(AiFailure::BadResponse))
        );
    }
    assert!(graph_of(&vault).await.covered.is_empty());
    // Статус отказа провайдера — без текста сервера, как у хода учителя.
    let transport = StreamTransport::with(vec![Script::Reply {
        status: 401,
        chunks: vec![],
    }]);
    let job = run(
        &mut vault,
        &transport,
        &unique("kv"),
        LESSON,
        &unique("r"),
        10,
    )
    .await
    .unwrap();
    assert_eq!(job.failure, Some(AiFailure::BadKey));
}

pub async fn cancel_stops_the_request_and_the_remaining_batches() {
    let mut vault = anthropic_vault().await;
    let events: Vec<LessonEvent> = (0..3)
        .map(|i| teacher(i, &long(&format!("Объяснение {i}."), 12_000)))
        .collect();
    put_teach(&vault, LESSON, &events);
    let verified = connection(&vault).await;
    let vault_id = unique("kv");
    let request = unique("r");

    // Отмена посреди сетевого запроса.
    let hang = StreamTransport::with(vec![Script::Hang {
        chunks: anthropic(&["ч"], None, 1, 1)[..3].to_vec(),
    }]);
    let prepared = begin_extract(&mut vault, &vault_id, LESSON, &request, 10, true, &verified)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(prepared.remaining, 2);
    let sending = send_extract(&hang, &vault_id, &prepared, Some(KEY), &verified);
    futures_util::pin_mut!(sending);
    assert!(futures_util::poll!(&mut sending).is_pending());
    assert_eq!(jobs(&vault_id)[0].state, JobState::Running);
    cancel(&vault_id, &request);
    let outcome = sending.await;
    assert_eq!(outcome, Err(AiFailure::Cancelled));
    let done = finish_extract(&mut vault, &vault_id, &prepared, 11, outcome)
        .await
        .unwrap();
    assert_eq!(
        (done.job.state, done.next),
        (JobState::Cancelled, Next::WaitUser)
    );
    assert!(graph_of(&vault).await.covered.is_empty());
    // Закончившееся задание отметку снимает: тот же id снова не занят отменой.
    assert!(
        begin_extract(&mut vault, &vault_id, LESSON, &request, 12, true, &verified)
            .await
            .is_err()
    );

    // Отмена между пачками: следующая пачка не начинается.
    let request = unique("r");
    let transport = StreamTransport::with(vec![one("a", "Первая", 0), one("b", "Вторая", 1)]);
    let prepared = begin_extract(&mut vault, &vault_id, LESSON, &request, 20, true, &verified)
        .await
        .unwrap()
        .unwrap();
    let outcome = send_extract(&transport, &vault_id, &prepared, Some(KEY), &verified).await;
    cancel(&vault_id, &request);
    let done = finish_extract(&mut vault, &vault_id, &prepared, 21, outcome)
        .await
        .unwrap();
    assert_eq!(
        (done.job.state, done.next),
        (JobState::Cancelled, Next::WaitUser)
    );
    // Готовое сохранено, а вторая пачка в сеть не ходила.
    assert_eq!(graph_of(&vault).await.propositions.len(), 1);
    assert_eq!(transport.count(), 1);

    // Отмена, пришедшая до `begin` следующей пачки (клиент не успел вызвать finish).
    let request = unique("r");
    let transport = StreamTransport::with(vec![one("a", "Третья", 1), one("b", "Четвёртая", 2)]);
    let prepared = begin_extract(&mut vault, &vault_id, LESSON, &request, 30, true, &verified)
        .await
        .unwrap()
        .unwrap();
    let outcome = send_extract(&transport, &vault_id, &prepared, Some(KEY), &verified).await;
    let done = finish_extract(&mut vault, &vault_id, &prepared, 31, outcome)
        .await
        .unwrap();
    assert_eq!(done.next, Next::Continue);
    cancel(&vault_id, &request);
    assert!(
        begin_extract(&mut vault, &vault_id, LESSON, &request, 32, true, &verified)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        jobs(&vault_id)
            .iter()
            .find(|j| j.id == request)
            .unwrap()
            .state,
        JobState::Cancelled
    );
}

pub async fn extraction_keeps_the_safety_guarantees_of_lessons() {
    let events = [teacher(1, "Объяснение для графа.")];
    let transport = StreamTransport::with(vec![one("a", "Утверждение", 0)]);

    // Выключенная функция: ни сети, ни журнала, ни файлов, ни задания.
    let mut off = vault_with(&AiSettings::default()).await;
    put_teach(&off, LESSON, &events);
    let verified = Connection::of_request(AiProvider::Anthropic, "https://api.anthropic.com/v1");
    let vault_id = unique("kv");
    assert_eq!(
        begin_extract(&mut off, &vault_id, LESSON, "r", 1, true, &verified)
            .await
            .unwrap_err(),
        AiFailure::Disabled
    );
    assert!(jobs(&vault_id).is_empty());

    // Подключение сменилось после проверки: отказ до журнала.
    let mut vault = anthropic_vault().await;
    put_teach(&vault, LESSON, &events);
    let stale = connection(&vault).await;
    let mut changed = load_settings(&vault).await;
    changed.provider = AiProvider::OpenAi;
    changed.model = Some(text("gpt"));
    save_settings(&mut vault, &changed).await.unwrap();
    assert_eq!(
        begin_extract(&mut vault, &vault_id, LESSON, &unique("r"), 1, true, &stale)
            .await
            .unwrap_err(),
        AiFailure::EndpointNotApproved
    );
    assert!(journal_entries(&vault).await.is_empty());
    // Ключ уходит только на адрес снимка.
    let mut ok = anthropic_vault().await;
    put_teach(&ok, LESSON, &events);
    let verified = connection(&ok).await;
    let prepared = begin_extract(&mut ok, &vault_id, LESSON, &unique("r"), 1, true, &verified)
        .await
        .unwrap()
        .unwrap();
    let other = Connection::of_request(AiProvider::OpenAi, "https://api.openai.com/v1");
    assert_eq!(
        send_extract(&transport, &vault_id, &prepared, Some(KEY), &other)
            .await
            .unwrap_err(),
        AiFailure::EndpointNotApproved
    );
    assert_eq!(transport.count(), 0);
    let _ = finish_extract(&mut ok, &vault_id, &prepared, 2, Err(AiFailure::Cancelled)).await;

    // Не урок и путь вне vault.
    for bad in [
        "Заметка.typ",
        "../Уроки/а.lesson",
        "Уроки/а.lesson/../../x.typ",
        ".typsidian/ai.json",
    ] {
        assert_eq!(
            begin_extract(&mut ok, &vault_id, bad, &unique("r"), 1, true, &verified)
                .await
                .unwrap_err(),
            AiFailure::LessonUnreadable,
            "{bad}"
        );
    }
    // Урок из запрещённой папки не уходит модели.
    let mut denied = load_settings(&ok).await;
    denied.denied_folders = vec![text("Уроки")];
    save_settings(&mut ok, &denied).await.unwrap();
    let verified = connection(&ok).await;
    assert_eq!(
        begin_extract(&mut ok, &vault_id, LESSON, &unique("r"), 1, true, &verified)
            .await
            .unwrap_err(),
        AiFailure::Forbidden
    );
    // Урок за символической ссылкой — тоже.
    let mut linked = anthropic_vault().await.with_symlink("Уроки");
    put_teach(&linked, LESSON, &events);
    let verified = connection(&linked).await;
    assert_eq!(
        begin_extract(
            &mut linked,
            &vault_id,
            LESSON,
            &unique("r"),
            1,
            true,
            &verified
        )
        .await
        .unwrap_err(),
        AiFailure::Forbidden
    );

    // Урок формата новее известного не извлекается.
    let mut newer = anthropic_vault().await;
    newer.replace_externally(
        LESSON,
        r#"{"format":99,"goal":"г","language":"ru","created_at":1}"#,
    );
    let verified = connection(&newer).await;
    assert_eq!(
        begin_extract(
            &mut newer,
            &vault_id,
            LESSON,
            &unique("r"),
            1,
            true,
            &verified
        )
        .await
        .unwrap_err(),
        AiFailure::LessonUnreadable
    );

    // Журнал не пишется — запроса нет, а задание закончено отказом.
    let mut broken = anthropic_vault().await.with_write_failure(JOURNAL);
    put_teach(&broken, LESSON, &events);
    let verified = connection(&broken).await;
    let request = unique("r");
    assert_eq!(
        begin_extract(&mut broken, &vault_id, LESSON, &request, 1, true, &verified)
            .await
            .unwrap_err(),
        AiFailure::JournalFailed
    );
    let job = jobs(&vault_id)
        .into_iter()
        .find(|j| j.id == request)
        .unwrap();
    assert_eq!(
        (job.state, job.failure),
        (JobState::Failed, Some(AiFailure::JournalFailed))
    );

    // Графа нельзя записать (ссылка): задание отказывает, покрытие не ставится, чужое не тронуто.
    let mut linked_graph = anthropic_vault().await.with_symlink(GRAPH);
    put_teach(&linked_graph, LESSON, &events);
    let t = StreamTransport::with(vec![one("a", "Утверждение", 0)]);
    let job = run(
        &mut linked_graph,
        &t,
        &unique("kv"),
        LESSON,
        &unique("r"),
        1,
    )
    .await
    .unwrap();
    assert_eq!(
        (job.state, job.failure),
        (JobState::Failed, Some(AiFailure::JournalFailed))
    );
    assert!(linked_graph.paths_under(GRAPH).is_empty());

    // Второе задание по тому же уроку, пока идёт первое, не заводится.
    let mut vault = anthropic_vault().await;
    put_teach(&vault, LESSON, &[teacher(1, &long("Много.", 25_000))]);
    let verified = connection(&vault).await;
    let vault_id = unique("kv");
    let first = begin_extract(&mut vault, &vault_id, LESSON, "первое", 1, true, &verified)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        begin_extract(&mut vault, &vault_id, LESSON, "второе", 1, true, &verified)
            .await
            .unwrap_err(),
        AiFailure::Cancelled
    );
    // Тот же request_id другого vault свободен.
    assert!(begin_extract(
        &mut vault,
        &unique("kv"),
        LESSON,
        "первое",
        1,
        true,
        &verified
    )
    .await
    .unwrap()
    .is_some());
    let _ = first;
}

// ── T005: автозапуск ────────────────────────────────────────────────────────

pub async fn auto_start_flag_follows_the_threshold_and_the_settings() {
    let mut vault = anthropic_vault().await;
    let path = create_lesson(
        &mut vault,
        "Замыкания",
        &[],
        Language::Ru,
        LessonPhase::Teach,
        1,
    )
    .await
    .unwrap();
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(
        (view.knowledge_pending_chars, view.knowledge_auto_due),
        (0, false)
    );

    // Короткий ответ учителя порога не достигает; ход учителя — настоящий, через цикл.
    let short = StreamTransport::with(vec![say(&"к".repeat(1_900))]);
    let done = cycle(
        &mut vault,
        &short,
        &path,
        &unique("r"),
        super::lesson_cases::answer("привет"),
        0,
    )
    .await
    .unwrap();
    assert_eq!(done.view.knowledge_pending_chars, 1_900);
    assert!(!done.view.knowledge_auto_due);

    // Следующий ход переваливает за 2000 непокрытых символов.
    let more = StreamTransport::with(vec![say(&"д".repeat(300))]);
    let done = cycle(
        &mut vault,
        &more,
        &path,
        &unique("r"),
        super::lesson_cases::answer("дальше"),
        done.view.events_len,
    )
    .await
    .unwrap();
    assert_eq!(done.view.knowledge_pending_chars, 2_200);
    assert!(done.view.knowledge_auto_due);
    // Открытие урока даёт тот же ответ.
    assert!(
        open_lesson(&mut vault, &path)
            .await
            .unwrap()
            .knowledge_auto_due
    );

    // Автозапуск выключен — «пора» нет, но непокрытый текст виден (кнопка «Пополнить граф»).
    let mut settings = load_settings(&vault).await;
    settings.knowledge_auto = false;
    save_settings(&mut vault, &settings).await.unwrap();
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert!(!view.knowledge_auto_due);
    assert_eq!(view.knowledge_pending_chars, 2_200);
    settings.knowledge_auto = true;
    save_settings(&mut vault, &settings).await.unwrap();

    // После извлечения непокрытого нет.
    let transport = StreamTransport::with(vec![one("a", "Утверждение", 1)]);
    let job = run(
        &mut vault,
        &transport,
        &unique("kv"),
        &path,
        &unique("r"),
        500,
    )
    .await
    .unwrap();
    assert_eq!(job.state, JobState::Done);
    let view = open_lesson(&mut vault, &path).await.unwrap();
    assert_eq!(
        (view.knowledge_pending_chars, view.knowledge_auto_due),
        (0, false)
    );
    // Новый ход снова копит непокрытое: считается только после покрытия.
    let once_more = StreamTransport::with(vec![say("ещё слово")]);
    let done = cycle(
        &mut vault,
        &once_more,
        &path,
        &unique("r"),
        super::lesson_cases::answer("и?"),
        view.events_len,
    )
    .await
    .unwrap();
    assert_eq!(
        done.view.knowledge_pending_chars,
        "ещё слово".chars().count() as u32
    );
    assert!(!done.view.knowledge_auto_due);

    // Практика и урок из запрещённой папки не запускаются сами.
    let mut denied = load_settings(&vault).await;
    denied.denied_folders = vec![text("Уроки")];
    save_settings(&mut vault, &denied).await.unwrap();
    let other = "Уроки/Длинный.lesson";
    put_teach(&vault, other, &[teacher(1, &"е".repeat(5_000))]);
    assert!(
        !open_lesson(&mut vault, other)
            .await
            .unwrap()
            .knowledge_auto_due
    );
    let mut open = load_settings(&vault).await;
    open.denied_folders.clear();
    save_settings(&mut vault, &open).await.unwrap();
    assert!(
        open_lesson(&mut vault, other)
            .await
            .unwrap()
            .knowledge_auto_due
    );
    put_lesson(
        &vault,
        other,
        LessonKind::Practice,
        LessonPhase::Teach,
        &[teacher(1, &"е".repeat(5_000))],
    );
    let view = open_lesson(&mut vault, other).await.unwrap();
    assert_eq!(
        (view.knowledge_pending_chars, view.knowledge_auto_due),
        (0, false)
    );
    // Замер: реплики стадии замера не считаются.
    put_lesson(
        &vault,
        other,
        LessonKind::Lesson,
        LessonPhase::Probe,
        &[teacher(1, &"е".repeat(5_000))],
    );
    assert_eq!(
        open_lesson(&mut vault, other)
            .await
            .unwrap()
            .knowledge_pending_chars,
        0
    );
    // Выключенное обучение — не пора.
    let mut disabled = load_settings(&vault).await;
    disabled.enabled = false;
    save_settings(&mut vault, &disabled).await.unwrap();
    put_teach(&vault, other, &[teacher(1, &"е".repeat(5_000))]);
    assert!(
        !open_lesson(&mut vault, other)
            .await
            .unwrap()
            .knowledge_auto_due
    );
}

pub async fn jobs_registry_is_per_vault_and_lists_running_first() {
    let mut vault = anthropic_vault().await;
    put_teach(
        &vault,
        LESSON,
        &[
            teacher(1, &long("Много.", 12_000)),
            teacher(2, &long("Много.", 12_000)),
        ],
    );
    put_teach(&vault, "Уроки/Второй.lesson", &[teacher(1, "Мало.")]);
    let verified = connection(&vault).await;
    let vault_id = unique("kv");
    let transport = StreamTransport::with(vec![one("a", "Утв", 0)]);
    let done = run(
        &mut vault,
        &transport,
        &vault_id,
        "Уроки/Второй.lesson",
        &unique("r"),
        5,
    )
    .await
    .unwrap();
    assert_eq!(done.state, JobState::Done);
    let running = begin_extract(&mut vault, &vault_id, LESSON, "идёт", 6, true, &verified)
        .await
        .unwrap()
        .unwrap();
    let list = jobs(&vault_id);
    assert_eq!(list.len(), 2);
    assert_eq!(
        (list[0].id.as_str(), list[0].state),
        ("идёт", JobState::Running)
    );
    assert_eq!(list[0].batches_total, 2);
    assert_eq!(list[0].source, LESSON);
    assert_eq!(list[1].state, JobState::Done);
    // Сериализация задания — плоская запись для интерфейса.
    let value = serde_json::to_value(&list[0]).unwrap();
    for key in [
        "id",
        "source",
        "state",
        "batches_total",
        "batches_done",
        "propositions_added",
        "propositions_merged",
        "chains_added",
        "chains_rejected",
        "failure",
        "started_at",
        "finished_at",
    ] {
        assert!(value.get(key).is_some(), "{key}");
    }
    let _ = running;
}

// ── T009: вид ───────────────────────────────────────────────────────────────

/// Кладёт готовый граф в vault: утверждения `(текст, урок)`, цепочки по индексам.
async fn put_graph(
    vault: &mut MemoryVault,
    props: &[(&str, &str)],
    chains: &[(&[usize], usize, &str)],
) -> Vec<String> {
    let ids: Vec<String> = props.iter().map(|(t, _)| proposition_id(t)).collect();
    let mut p = String::new();
    for (index, (text, source)) in props.iter().enumerate() {
        p.push_str(&format!(
            "{}\n",
            json!({"line":"Phrasing","id":ids[index],"text":text,"source":source,"event":index,"at":1})
        ));
    }
    let mut c = String::new();
    for (premises, conclusion, source) in chains {
        let premises: Vec<String> = premises.iter().map(|i| ids[*i].clone()).collect();
        c.push_str(&format!(
            "{}\n",
            json!({"line":"Chain","id":chain_id(&premises, &ids[*conclusion]),"premises":premises,
                "conclusion":ids[*conclusion],"source":source,"event":9,"at":1})
        ));
    }
    vault.replace_externally(&format!("{GRAPH}/{PROPOSITIONS_FILE}"), p);
    vault.replace_externally(&format!("{GRAPH}/{CHAINS_FILE}"), c);
    ids
}

pub async fn graph_view_has_the_shape_of_the_notes_graph() {
    let mut vault = anthropic_vault().await;
    let ids = put_graph(
        &mut vault,
        &[
            ("Замыкание хранит окружение", "Уроки/А.lesson"),
            ("Окружение — набор переменных", "Уроки/А.lesson"),
            ("Замыкание видит переменные", "Уроки/Б.lesson"),
            ("Свободная функция", "Уроки/Б.lesson"),
            ("Одинокое утверждение", "Уроки/Б.lesson"),
        ],
        &[
            (&[0, 1], 2, "Уроки/А.lesson"), // развилка
            (&[3], 0, "Уроки/Б.lesson"),    // прямое ребро
            (&[0], 3, "Уроки/Б.lesson"),    // обратное: слитое в двустороннее
        ],
    )
    .await;
    let view = graph_view(&vault, "").await;
    let graph = &view.graph;
    // 5 утверждений + 1 развилка; узлы по возрастанию id, рёбра по (from, to).
    assert_eq!(graph.nodes.len(), 6);
    assert_eq!(graph.total_notes, 5);
    assert_eq!((view.propositions, view.chains, view.sources), (5, 3, 2));
    assert!(graph.nodes.windows(2).all(|w| w[0].id < w[1].id));
    assert!(graph
        .edges
        .windows(2)
        .all(|w| (w[0].from, w[0].to) < (w[1].from, w[1].to)));
    let forks: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.id.starts_with("c_"))
        .collect();
    assert_eq!(forks.len(), 1);
    assert_eq!(format!("{:?}", forks[0].kind), "Tag");
    assert!(forks[0].path.is_none() && forks[0].group.is_none());
    // Развилка: две посылки в неё, одна из неё в вывод. Остальные цепочки — прямые рёбра.
    let index = |id: &str| graph.nodes.iter().position(|n| n.id == id).unwrap() as u32;
    let fork = index(&forks[0].id);
    let into: Vec<_> = graph.edges.iter().filter(|e| e.to == fork).collect();
    assert_eq!(into.len(), 2);
    assert!(graph
        .edges
        .iter()
        .any(|e| e.from == fork && e.to == index(&ids[2])));
    let (a, d) = (index(&ids[0]), index(&ids[3]));
    let pair = graph
        .edges
        .iter()
        .find(|e| (e.from, e.to) == (a.min(d), a.max(d)))
        .unwrap();
    assert!(pair.both_ways);
    // Степень — число рёбер; узлы утверждений знают свой урок и группу.
    for (i, node) in graph.nodes.iter().enumerate() {
        let degree = graph
            .edges
            .iter()
            .filter(|e| e.from == i as u32 || e.to == i as u32)
            .count();
        assert_eq!(node.degree as usize, degree, "{}", node.id);
    }
    let node = &graph.nodes[index(&ids[2]) as usize];
    assert_eq!(node.path.as_deref(), Some("Уроки/Б.lesson"));
    let group = &view.groups[node.group.unwrap() as usize];
    assert_eq!(
        (group.source.as_str(), group.label.as_str(), group.count),
        ("Уроки/Б.lesson", "Б", 3)
    );
    assert_eq!(view.groups.len(), 2);

    // Поиск локальный, по формулировкам, регистр не важен; находит и соседей по цепочкам.
    let found = graph_view(&vault, "ОДИНОКОЕ").await;
    assert_eq!(found.graph.nodes.len(), 1);
    assert_eq!(found.graph.total_notes, 5);
    let found = graph_view(&vault, "окружение переменных").await;
    assert!(found.graph.nodes.iter().any(|n| n.id == ids[1]));
    assert!(found.graph.nodes.iter().any(|n| n.id == ids[2]));
    assert!(graph_view(&vault, "нет такого слова")
        .await
        .graph
        .nodes
        .is_empty());

    // Таблица цепочек несёт то же содержимое.
    let table = chains_view(&vault, "").await;
    assert_eq!(table.len(), 3);
    let wide = table.iter().find(|c| c.premises.len() == 2).unwrap();
    assert_eq!(wide.conclusion.text, "Замыкание видит переменные");
    assert!(!wide.incomplete);
    assert_eq!(wide.sources[0].path, "Уроки/А.lesson");
    assert_eq!(chains_view(&vault, "Одинокое").await.len(), 0);
    assert_eq!(chains_view(&vault, "свободная").await.len(), 2);
}

pub async fn proposition_view_has_chains_and_sources_with_the_removed_mark() {
    let mut vault = anthropic_vault().await;
    let ids = put_graph(
        &mut vault,
        &[
            ("Первое", "Уроки/А.lesson"),
            ("Второе", "Уроки/Б.lesson"),
            ("Вывод", "Уроки/А.lesson"),
        ],
        &[(&[0, 1], 2, "Уроки/А.lesson")],
    )
    .await;
    // Вторая формулировка первого утверждения из другого урока.
    let settings = load_settings(&vault).await;
    store::append_propositions(
        &mut vault,
        &settings,
        &[PropositionLine::Phrasing {
            id: ids[0].clone(),
            text: text("Первое иначе"),
            source: text("Уроки/Б.lesson"),
            event: Some(7),
            at: 2,
        }],
    )
    .await
    .unwrap();
    store::append_sources(
        &mut vault,
        &settings,
        &[SourceLine::SourceRemoved {
            source: text("Уроки/Б.lesson"),
            keep: true,
            at: 3,
        }],
    )
    .await
    .unwrap();
    let view = proposition_view(&vault, &ids[0]).await.unwrap();
    assert_eq!(view.text, "Первое");
    assert_eq!(view.phrasings, vec![text("Первое"), text("Первое иначе")]);
    assert_eq!(view.sources.len(), 2);
    let removed = view
        .sources
        .iter()
        .find(|s| s.path == "Уроки/Б.lesson")
        .unwrap();
    assert!(removed.removed && removed.event == Some(7));
    let kept = view
        .sources
        .iter()
        .find(|s| s.path == "Уроки/А.lesson")
        .unwrap();
    assert!(!kept.removed && kept.event == Some(0));
    assert_eq!(view.chains.len(), 1);
    assert_eq!(view.chains[0].premises.len(), 2);
    assert!(proposition_view(&vault, "p_0123456789abcdef")
        .await
        .is_none());
}

pub async fn graph_of_2000_propositions_opens_and_searches_in_under_a_second() -> i64 {
    let vault = anthropic_vault().await;
    let total = 2_000usize;
    let texts: Vec<String> = (0..total)
        .map(|i| {
            format!(
                "Утверждение номер {i} про замыкание, окружение и область видимости {}",
                i % 37
            )
        })
        .collect();
    let mut p = String::new();
    for (i, text) in texts.iter().enumerate() {
        p.push_str(&format!(
            "{}\n",
            json!({"line":"Phrasing","id":proposition_id(text),"text":text,
                "source":format!("Уроки/Урок {}.lesson", i % 40),"event":i,"at":1})
        ));
    }
    let mut c = String::new();
    for i in 0..1_500usize {
        let premises: Vec<String> = (0..1 + i % 3)
            .map(|k| proposition_id(&texts[(i * 7 + k * 13) % total]))
            .collect();
        let conclusion = proposition_id(&texts[(i * 11 + 5) % total]);
        if premises.contains(&conclusion) {
            continue;
        }
        c.push_str(&format!(
            "{}\n",
            json!({"line":"Chain","id":chain_id(&premises, &conclusion),"premises":premises,
                "conclusion":conclusion,"source":"Уроки/Урок 0.lesson","event":1,"at":1})
        ));
    }
    vault.replace_externally(&format!("{GRAPH}/{PROPOSITIONS_FILE}"), p);
    vault.replace_externally(&format!("{GRAPH}/{CHAINS_FILE}"), c);
    let started = typsidian_core::properties::now_ms();
    let view = graph_view(&vault, "").await;
    assert_eq!(view.propositions as usize, total);
    assert!(view.graph.edges.len() > 1_000);
    let found = graph_view(&vault, "номер 1999").await;
    assert!(found
        .graph
        .nodes
        .iter()
        .any(|n| n.label.contains("номер 1999")));
    assert!(!chains_view(&vault, "номер 12").await.is_empty());
    let table = chains_view(&vault, "").await;
    assert!(table.len() > 1_000);
    typsidian_core::properties::now_ms() - started
}

// ── T012: чтение графа агентом и выбор пользователя ─────────────────────────

fn statements(count: usize) -> Vec<(String, &'static str)> {
    (0..count)
        .map(|i| {
            (
                format!("Замыкание вариант {i} хранит окружение"),
                "Уроки/А.lesson",
            )
        })
        .collect()
}

async fn graph_with_statements(vault: &mut MemoryVault, count: usize) -> Vec<String> {
    let owned = statements(count);
    let borrowed: Vec<(&str, &str)> = owned.iter().map(|(t, s)| (t.as_str(), *s)).collect();
    put_graph(vault, &borrowed, &[(&[0], 1, "Уроки/А.lesson")]).await
}

pub async fn read_knowledge_is_absent_unless_allowed_and_reads_up_to_thirty() {
    let mut vault = anthropic_vault().await;
    graph_with_statements(&mut vault, 50).await;
    let path = create_lesson(
        &mut vault,
        "Замыкания",
        &[],
        Language::Ru,
        LessonPhase::Teach,
        1,
    )
    .await
    .unwrap();

    // По умолчанию выкл.: инструмента нет в запросе, вызов модели игнорируется.
    let call = |query: &str| {
        reply(anthropic(
            &["Посмотрю граф"],
            Some(("read_knowledge", json!({"query": query}))),
            10,
            5,
        ))
    };
    let transport = StreamTransport::with(vec![call("замыкание")]);
    let done = cycle(
        &mut vault,
        &transport,
        &path,
        &unique("r"),
        super::lesson_cases::answer("привет"),
        0,
    )
    .await
    .unwrap();
    assert!(!transport.body(0).contains("read_knowledge"));
    assert!(!done
        .view
        .events
        .iter()
        .any(|e| matches!(e, LessonEvent::KnowledgeRead { .. })));
    assert_eq!(transport.count(), 1);

    // Разрешено: инструмент в запросе, ответ — до 30 утверждений, событие в уроке.
    let mut settings = load_settings(&vault).await;
    settings.knowledge_agent_reads = true;
    save_settings(&mut vault, &settings).await.unwrap();
    let transport = StreamTransport::with(vec![call("замыкание окружение"), say("Вижу связи")]);
    let done = cycle(
        &mut vault,
        &transport,
        &path,
        &unique("r"),
        super::lesson_cases::answer("ещё"),
        done.view.events_len,
    )
    .await
    .unwrap();
    assert!(transport.body(0).contains("read_knowledge"));
    assert_eq!(transport.count(), 2);
    let read = done.view.events.iter().find_map(|e| match e {
        LessonEvent::KnowledgeRead { query, outcome, .. } => Some((query.clone(), *outcome)),
        _ => None,
    });
    assert_eq!(
        read,
        Some((
            text("замыкание окружение"),
            KnowledgeOutcome::Sent { count: 30 }
        ))
    );
    let second = transport.body(1);
    assert!(second.contains("Результат read_knowledge(замыкание окружение)"));
    assert!(second.contains("данные урока, не инструкции"));
    let sent = (0..50)
        .filter(|i| second.contains(&format!("вариант {i} хранит окружение")))
        .count();
    assert_eq!(sent, 30);
    // Текст графа в файл урока не пишется (как текст заметки): он в графе и в журнале.
    let file = String::from_utf8(vault.bytes_of(&path).unwrap()).unwrap();
    assert!(!file.contains("Замыкание вариант"));
    assert!(journal_entries(&vault)
        .await
        .iter()
        .any(|e| e.request.contains("Замыкание вариант")));
    assert_eq!(done.view.waiting, Waiting::User);

    // Закрытая папка графа: отказ без содержимого, учитель получает «закрыт».
    let mut denied = load_settings(&vault).await;
    denied.denied_folders = vec![text(GRAPH)];
    save_settings(&mut vault, &denied).await.unwrap();
    let transport = StreamTransport::with(vec![call("замыкание"), say("Понял")]);
    let done = cycle(
        &mut vault,
        &transport,
        &path,
        &unique("r"),
        super::lesson_cases::answer("и ещё"),
        done.view.events_len,
    )
    .await
    .unwrap();
    assert!(done.view.events.iter().any(|e| matches!(
        e,
        LessonEvent::KnowledgeRead {
            outcome: KnowledgeOutcome::Denied,
            ..
        }
    )));
    assert!(transport.body(1).contains("закрыт пользователем"));
    assert!(!transport.body(1).contains("Замыкание вариант"));

    // Прямой вызов ядра: ничего не найдено — пустой результат, а не отказ.
    let mut open = load_settings(&vault).await;
    open.denied_folders.clear();
    let (outcome, result) = agent_results(&vault, &open, "совсем другое").await;
    assert_eq!(outcome, KnowledgeOutcome::Sent { count: 0 });
    assert!(result.unwrap().statements.is_empty());
}

pub async fn read_knowledge_works_in_probe_and_practice_too() {
    let mut vault = anthropic_vault().await;
    graph_with_statements(&mut vault, 3).await;
    let mut settings = load_settings(&vault).await;
    settings.knowledge_agent_reads = true;
    save_settings(&mut vault, &settings).await.unwrap();
    let call = || {
        reply(anthropic(
            &["Гляну"],
            Some(("read_knowledge", json!({"query": "замыкание"}))),
            10,
            5,
        ))
    };

    let probe = create_lesson(
        &mut vault,
        "Замер",
        &[],
        Language::Ru,
        LessonPhase::Probe,
        1,
    )
    .await
    .unwrap();
    let transport = StreamTransport::with(vec![call(), say("Продолжим")]);
    let done = cycle(
        &mut vault,
        &transport,
        &probe,
        &unique("r"),
        super::lesson_cases::answer("привет"),
        0,
    )
    .await
    .unwrap();
    assert!(transport.body(0).contains("read_knowledge"));
    assert!(done.view.events.iter().any(|e| matches!(
        e,
        LessonEvent::KnowledgeRead {
            outcome: KnowledgeOutcome::Sent { count: 3 },
            ..
        }
    )));

    let practice =
        create_practice_with_knowledge(&mut vault, "Тема", &[], &[], &[], None, Language::Ru, 2)
            .await
            .unwrap();
    let transport = StreamTransport::with(vec![call(), say("Готово")]);
    let done = cycle(
        &mut vault,
        &transport,
        &practice,
        &unique("r"),
        super::lesson_cases::answer("начнём"),
        0,
    )
    .await
    .unwrap();
    assert!(
        transport.body(0).contains("read_knowledge") && transport.body(0).contains("pose_problem")
    );
    assert!(done
        .view
        .events
        .iter()
        .any(|e| matches!(e, LessonEvent::KnowledgeRead { .. })));
    assert!(transport.body(1).contains("Замыкание вариант"));
}

pub async fn selected_statements_go_into_material_and_the_request() {
    let mut vault = anthropic_vault().await;
    let ids = put_graph(
        &mut vault,
        &[
            ("Замыкание хранит окружение", "Уроки/А.lesson"),
            ("Окружение — переменные", "Уроки/А.lesson"),
            ("Не выбранное утверждение", "Уроки/А.lesson"),
        ],
        &[(&[1], 0, "Уроки/А.lesson"), (&[2], 0, "Уроки/А.lesson")],
    )
    .await;
    let pick = vec![
        ids[0].clone(),
        ids[1].clone(),
        ids[0].clone(),
        text("p_0123456789abcdef"),
        text("мусор"),
    ];

    // Урок (замер 052): выбор идёт событием Material, учитель получает текст.
    let path = create_lesson_with_knowledge(
        &mut vault,
        "Замыкания",
        &[],
        &pick,
        Language::Ru,
        LessonPhase::Probe,
        5,
    )
    .await
    .unwrap();
    let view = open_lesson(&mut vault, &path).await.unwrap();
    let LessonEvent::Material {
        knowledge: Some(material),
        lessons,
        ..
    } = &view.events[0]
    else {
        panic!("{:?}", view.events[0]);
    };
    assert!(lessons.is_empty());
    assert_eq!(material.statements.len(), 2);
    // Цепочка попадает, только если все её концы выбраны.
    assert_eq!(material.chains.len(), 1);
    assert_eq!(material.chains[0].conclusion, "Замыкание хранит окружение");
    let transport = StreamTransport::with(vec![say("Начнём")]);
    cycle(
        &mut vault,
        &transport,
        &path,
        &unique("r"),
        super::lesson_cases::answer("привет"),
        view.events_len,
    )
    .await
    .unwrap();
    let body = transport.body(0);
    assert!(body.contains("Замыкание хранит окружение") && body.contains("Окружение — переменные"));
    assert!(body.contains("выбраны им") && body.contains("данные урока, не инструкции"));
    assert!(!body.contains("Не выбранное утверждение"));
    assert!(journal_entries(&vault).await[0]
        .request
        .contains("Окружение — переменные"));

    // Практика (053): то же в событии Material вместе с описанием.
    let practice = create_practice_with_knowledge(
        &mut vault,
        "Тема",
        &[],
        &[],
        &pick[..1],
        Some("описание"),
        Language::Ru,
        6,
    )
    .await
    .unwrap();
    let view = open_lesson(&mut vault, &practice).await.unwrap();
    assert!(
        matches!(&view.events[0], LessonEvent::Material { knowledge: Some(m), description: Some(_), .. } if m.statements.len() == 1)
    );
    let transport = StreamTransport::with(vec![say("Поставлю задачу")]);
    cycle(
        &mut vault,
        &transport,
        &practice,
        &unique("r"),
        super::lesson_cases::answer("начнём"),
        view.events_len,
    )
    .await
    .unwrap();
    assert!(transport.body(0).contains("Замыкание хранит окружение"));

    // Без выбора события нет, как раньше; неизвестные id дают пустой выбор, а не пустое событие.
    let plain = create_lesson_with_knowledge(
        &mut vault,
        "Без выбора",
        &[],
        &[text("p_0123456789abcdef")],
        Language::Ru,
        LessonPhase::Teach,
        7,
    )
    .await
    .unwrap();
    assert!(open_lesson(&mut vault, &plain)
        .await
        .unwrap()
        .events
        .is_empty());

    // Не больше 50 утверждений.
    let many = graph_with_statements(&mut vault, 80).await;
    let big = create_lesson_with_knowledge(
        &mut vault,
        "Много",
        &[],
        &many,
        Language::Ru,
        LessonPhase::Teach,
        8,
    )
    .await
    .unwrap();
    let view = open_lesson(&mut vault, &big).await.unwrap();
    assert!(
        matches!(&view.events[0], LessonEvent::Material { knowledge: Some(m), .. } if m.statements.len() == 50)
    );

    // Папка графа в запрещённых: оба пути отказывают, файла сессии нет.
    let mut denied = load_settings(&vault).await;
    denied.denied_folders = vec![text(GRAPH)];
    save_settings(&mut vault, &denied).await.unwrap();
    let before = vault.paths_under("Уроки").len();
    assert_eq!(
        create_lesson_with_knowledge(
            &mut vault,
            "Закрыто",
            &[],
            &many,
            Language::Ru,
            LessonPhase::Teach,
            9
        )
        .await
        .unwrap_err(),
        AiFailure::Forbidden
    );
    assert_eq!(
        create_practice_with_knowledge(
            &mut vault,
            "Закрыто",
            &[],
            &[],
            &many,
            None,
            Language::Ru,
            9
        )
        .await
        .unwrap_err(),
        AiFailure::Forbidden
    );
    assert_eq!(vault.paths_under("Уроки").len(), before);
    // Уже записанный выбор в закрытую позже папку в запрос не идёт.
    let transport = StreamTransport::with(vec![say("ответ")]);
    cycle(
        &mut vault,
        &transport,
        &path,
        &unique("r"),
        super::lesson_cases::answer("ещё"),
        3,
    )
    .await
    .ok();
    if transport.count() > 0 {
        assert!(!transport.body(0).contains("Окружение — переменные"));
    }
    // Пустой выбор разрешён и при закрытой папке.
    assert!(select_material(&vault, &load_settings(&vault).await, &[])
        .await
        .unwrap()
        .is_none());
}

// ── T014: удаление ──────────────────────────────────────────────────────────

pub async fn removing_a_proposition_hides_it_and_marks_chains() {
    let mut vault = anthropic_vault().await;
    let ids = put_graph(
        &mut vault,
        &[
            ("Посылка раз", "Уроки/А.lesson"),
            ("Посылка два", "Уроки/А.lesson"),
            ("Вывод", "Уроки/А.lesson"),
        ],
        &[(&[0, 1], 2, "Уроки/А.lesson"), (&[0], 1, "Уроки/А.lesson")],
    )
    .await;
    remove_proposition(&mut vault, &ids[0], 10).await.unwrap();
    let projection = graph_of(&vault).await;
    assert!(!projection.propositions.contains_key(&ids[0]));
    // Файлы только дописываются: надгробие — строка, прежние строки на месте.
    let raw = String::from_utf8(
        vault
            .bytes_of(&format!("{GRAPH}/{PROPOSITIONS_FILE}"))
            .unwrap(),
    )
    .unwrap();
    assert!(raw.contains("Посылка раз") && raw.lines().last().unwrap().contains("Removed"));
    let table = chains_view(&vault, "").await;
    // Цепочка, где удалённое — посылка, показывается неполной; где посылка последняя и
    // единственная, а вывод жив, — скрыта (посылок не осталось).
    assert_eq!(table.len(), 1);
    assert!(table[0].incomplete);
    assert_eq!(table[0].premises.len(), 1);
    assert_eq!(table[0].premises[0].text, "Посылка два");
    // Удалённое как вывод скрывает цепочку целиком.
    remove_proposition(&mut vault, &ids[2], 11).await.unwrap();
    assert!(chains_view(&vault, "").await.is_empty());
    // Неизвестный id и повтор — ничего.
    let size = vault
        .bytes_of(&format!("{GRAPH}/{PROPOSITIONS_FILE}"))
        .unwrap()
        .len();
    remove_proposition(&mut vault, "p_0123456789abcdef", 12)
        .await
        .unwrap();
    remove_proposition(&mut vault, &ids[0], 12).await.unwrap();
    assert_eq!(
        vault
            .bytes_of(&format!("{GRAPH}/{PROPOSITIONS_FILE}"))
            .unwrap()
            .len(),
        size
    );
    // Граф с удалённым узлом рисуется без него.
    let view = graph_view(&vault, "").await;
    assert!(view
        .graph
        .nodes
        .iter()
        .all(|n| n.id != ids[0] && n.id != ids[2]));
}

pub async fn removing_a_source_contribution_keeps_shared_statements() {
    let mut vault = anthropic_vault().await;
    let ids = put_graph(
        &mut vault,
        &[
            ("Только у А", "Уроки/А.lesson"),
            ("Общее", "Уроки/А.lesson"),
            ("Только у Б", "Уроки/Б.lesson"),
        ],
        &[(&[0], 1, "Уроки/А.lesson")],
    )
    .await;
    let settings = load_settings(&vault).await;
    store::append_propositions(
        &mut vault,
        &settings,
        &[PropositionLine::Phrasing {
            id: ids[1].clone(),
            text: text("Общее"),
            source: text("Уроки/Б.lesson"),
            event: Some(3),
            at: 5,
        }],
    )
    .await
    .unwrap();
    store::append_sources(
        &mut vault,
        &settings,
        &[SourceLine::Covered {
            source: text("Уроки/А.lesson"),
            kind: SourceKind::Lesson,
            upto: 6,
            at: 5,
        }],
    )
    .await
    .unwrap();

    remove_source(&mut vault, "Уроки/А.lesson", false, 10)
        .await
        .unwrap();
    let projection = graph_of(&vault).await;
    assert!(!projection.propositions.contains_key(&ids[0]));
    assert_eq!(projection.propositions[&ids[1]].phrasings.len(), 1);
    assert_eq!(
        projection.propositions[&ids[1]].phrasings[0].source,
        "Уроки/Б.lesson"
    );
    assert!(projection.propositions.contains_key(&ids[2]));
    assert!(projection.chains.is_empty());
    // Покрытие остаётся: убранный вклад существующего урока не извлекается заново сам.
    assert_eq!(projection.covered["Уроки/А.lesson"], 6);
    // Позднее извлечение из того же источника оживляет только новое.
    store::append_propositions(
        &mut vault,
        &settings,
        &[PropositionLine::Phrasing {
            id: ids[0].clone(),
            text: text("Только у А"),
            source: text("Уроки/А.lesson"),
            event: Some(8),
            at: 20,
        }],
    )
    .await
    .unwrap();
    assert!(graph_of(&vault).await.propositions.contains_key(&ids[0]));

    // «Оставить вклад»: утверждения на месте, источник помечен удалённым.
    remove_source(&mut vault, "Уроки/Б.lesson", true, 30)
        .await
        .unwrap();
    let projection = graph_of(&vault).await;
    assert!(projection.propositions.contains_key(&ids[2]));
    assert!(projection.removed_sources.contains("Уроки/Б.lesson"));
    let view = proposition_view(&vault, &ids[2]).await.unwrap();
    assert!(view.sources[0].removed);
    // Неизвестный источник — ничего не пишет.
    let size = vault
        .bytes_of(&format!("{GRAPH}/{SOURCES_FILE}"))
        .unwrap()
        .len();
    remove_source(&mut vault, "Уроки/Нет.lesson", false, 31)
        .await
        .unwrap();
    remove_source(&mut vault, "../снаружи", false, 31)
        .await
        .unwrap();
    assert_eq!(
        vault
            .bytes_of(&format!("{GRAPH}/{SOURCES_FILE}"))
            .unwrap()
            .len(),
        size
    );
}

pub async fn deleting_a_lesson_decides_the_fate_of_its_contribution() {
    for keep in [true, false] {
        let mut vault = anthropic_vault().await;
        put_teach(&vault, LESSON, &[teacher(1, "Объяснение.")]);
        let transport = StreamTransport::with(vec![one("a", "Вклад урока", 0)]);
        let vault_id = unique("kv");
        run(&mut vault, &transport, &vault_id, LESSON, &unique("r"), 10)
            .await
            .unwrap();
        delete_lesson(
            &mut vault,
            Some(&vault_id),
            "Уроки/./Замыкания.lesson",
            keep,
            50,
        )
        .await
        .unwrap();
        // Файл урока удалён обычным удалением vault.
        assert!(vault.bytes_of(LESSON).is_none());
        let projection = graph_of(&vault).await;
        if keep {
            assert_eq!(projection.propositions.len(), 1);
            assert!(projection.removed_sources.contains(LESSON));
        } else {
            assert!(projection.propositions.is_empty());
        }
    }

    // Урок без вклада: граф не создаётся.
    let mut vault = anthropic_vault().await;
    put_teach(&vault, LESSON, &[teacher(1, "Объяснение.")]);
    delete_lesson(&mut vault, Some("vd"), LESSON, false, 5)
        .await
        .unwrap();
    assert!(vault.paths_under(GRAPH).is_empty());
    // Это не урок, путь вне vault, файла нет: ничего не удалено.
    vault.replace_externally("Заметка.typ", "текст");
    for bad in ["Заметка.typ", "../а.lesson", ".typsidian/ai.json"] {
        assert_eq!(
            delete_lesson(&mut vault, Some("vd"), bad, true, 6)
                .await
                .unwrap_err(),
            AiFailure::LessonUnreadable,
            "{bad}"
        );
    }
    assert!(vault.bytes_of("Заметка.typ").is_some());

    // Идущий ход удалению мешает.
    put_teach(&vault, LESSON, &[teacher(1, "Объяснение.")]);
    typsidian_core::ai::lesson::partial::start("vd", "ход", LESSON);
    assert_eq!(
        delete_lesson(&mut vault, Some("vd"), LESSON, true, 7)
            .await
            .unwrap_err(),
        AiFailure::LessonChanged
    );
    assert!(vault.bytes_of(LESSON).is_some());
    typsidian_core::ai::lesson::partial::take("vd", "ход");
}

pub async fn related_search_is_local_stable_and_capped() {
    let mut vault = anthropic_vault().await;
    let owned = statements(60);
    let borrowed: Vec<(&str, &str)> = owned.iter().map(|(t, s)| (t.as_str(), *s)).collect();
    put_graph(&mut vault, &borrowed, &[]).await;
    let projection = graph_of(&vault).await;
    let found = related(&projection, "замыкания и окружения", 50, |_, _| true);
    assert_eq!(found.len(), 50);
    assert_eq!(
        found,
        related(&projection, "замыкания и окружения", 50, |_, _| true)
    );
    assert!(related(&projection, "фотосинтез", 50, |_, _| true).is_empty());
    assert!(related(&projection, "", 50, |_, _| true).is_empty());
    assert_eq!(related(&projection, "замыкание", 3, |_, _| true).len(), 3);
}

// ── Ревью безопасности T018: T020–T030 ──────────────────────────────────────

fn sources_path() -> String {
    format!("{GRAPH}/{SOURCES_FILE}")
}

fn teach_events(count: usize) -> Vec<LessonEvent> {
    (0..count)
        .map(|i| teacher(i as i64, &format!("Объяснение {i} о замыканиях.")))
        .collect()
}

/// H1: урок, пересозданный по тому же пути короче, снова становится покрытым; перенос с
/// будущим временем не утаскивает покрытие на другой путь; пачка без роста покрытия
/// заканчивает задание, а не крутит платное извлечение по кругу.
pub async fn a_recreated_shorter_lesson_is_covered_again_and_a_future_move_cannot_stall_it() {
    let mut vault = anthropic_vault().await;
    put_teach(&vault, LESSON, &teach_events(5));
    let transport = StreamTransport::with(vec![
        one("a", "Утверждение длинного урока", 0),
        one("b", "Утверждение короткого урока", 0),
    ]);
    let vault_id = unique("kv");
    let job = run(&mut vault, &transport, &vault_id, LESSON, &unique("r"), 10)
        .await
        .unwrap();
    assert_eq!(job.state, JobState::Done);
    assert_eq!(graph_of(&vault).await.covered_of(LESSON), 5);

    // Урок пересоздан по тому же пути короче прежнего: непокрытое видно, извлечение его берёт.
    put_teach(&vault, LESSON, &teach_events(2));
    let view = open_lesson(&mut vault, LESSON).await.unwrap();
    assert!(view.knowledge_pending_chars > 0, "{view:?}");
    let job = run(&mut vault, &transport, &vault_id, LESSON, &unique("r"), 20)
        .await
        .unwrap();
    assert_eq!(job.state, JobState::Done);
    assert_eq!(graph_of(&vault).await.covered_of(LESSON), 2);
    let view = open_lesson(&mut vault, LESSON).await.unwrap();
    assert_eq!(view.knowledge_pending_chars, 0);
    assert!(!view.knowledge_auto_due);
    // Третий запуск ничего не отправляет: бесконечного извлечения нет.
    let sent = transport.count();
    let again = run(&mut vault, &transport, &vault_id, LESSON, &unique("r"), 30)
        .await
        .unwrap();
    assert_eq!(again.state, JobState::Done);
    assert_eq!(transport.count(), sent);

    // Перенос с временем в будущем (из синхронизации) не отнимает у нового покрытия путь.
    let mut vault = anthropic_vault().await;
    put_teach(&vault, LESSON, &teach_events(2));
    let moved =
        json!({"line":"Moved","from":LESSON,"to":"Чужое/Х.lesson","at":9_000_000_000_000i64});
    vault.replace_externally(&sources_path(), format!("{moved}\n"));
    let transport = StreamTransport::with(vec![one("a", "Утверждение", 0)]);
    let job = run(
        &mut vault,
        &transport,
        &unique("kv"),
        LESSON,
        &unique("r"),
        10,
    )
    .await
    .unwrap();
    assert_eq!(job.state, JobState::Done);
    assert_eq!(graph_of(&vault).await.covered_of(LESSON), 2);

    // Если покрытие всё же не выросло (перенос «на конец времён»), задание кончается после
    // одной пачки отказом, а не идёт дальше.
    let mut vault = anthropic_vault().await;
    put_teach(&vault, LESSON, &teach_events(2));
    let moved = json!({"line":"Moved","from":LESSON,"to":"Чужое/Х.lesson","at":i64::MAX});
    vault.replace_externally(&sources_path(), format!("{moved}\n"));
    let transport = StreamTransport::with(vec![
        one("a", "Утверждение", 0),
        one("b", "Ещё утверждение", 0),
    ]);
    let job = run(
        &mut vault,
        &transport,
        &unique("kv"),
        LESSON,
        &unique("r"),
        10,
    )
    .await
    .unwrap();
    assert_eq!(job.state, JobState::Failed);
    assert_eq!(job.failure, Some(AiFailure::JournalFailed));
    assert_eq!(transport.count(), 1);
}

/// H1: предел пачек считает ядро, а не клиентский цикл: после `MAX_BATCHES` задание
/// кончается, следующая пачка не отдаётся.
pub async fn the_core_stops_a_job_after_the_batch_limit() {
    use typsidian_core::ai::knowledge::extract::MAX_BATCHES;
    let mut vault = anthropic_vault().await;
    let events: Vec<LessonEvent> = (0..MAX_BATCHES + 5)
        .map(|i| {
            teacher(
                i as i64,
                &long(&format!("Объяснение {i} о замыканиях."), 12_000),
            )
        })
        .collect();
    put_teach(&vault, LESSON, &events);
    let transport = StreamTransport::with(
        (0..MAX_BATCHES + 5)
            .map(|i| one("a", &format!("Утверждение номер {i}"), i as u32))
            .collect(),
    );
    let vault_id = unique("kv");
    let request = unique("r");
    let verified = connection(&vault).await;
    let mut taken = 0;
    // Клиент, который не знает предела и просит пачки без конца.
    for _ in 0..MAX_BATCHES * 2 {
        let Some(prepared) =
            begin_extract(&mut vault, &vault_id, LESSON, &request, 10, true, &verified)
                .await
                .unwrap()
        else {
            break;
        };
        taken += 1;
        let outcome = send_extract(&transport, &vault_id, &prepared, Some(KEY), &verified).await;
        finish_extract(&mut vault, &vault_id, &prepared, 11, outcome)
            .await
            .unwrap();
    }
    assert_eq!(taken, MAX_BATCHES);
    assert_eq!(transport.count(), MAX_BATCHES);
    let job = jobs(&vault_id)
        .into_iter()
        .find(|job| job.id == request)
        .unwrap();
    assert_eq!(job.state, JobState::Done);
    assert_eq!(job.batches_done as usize, MAX_BATCHES);
}

/// M1: утверждение, у которого нет открытого источника, не уходит модели: ни в «известных»
/// при извлечении, ни через `read_knowledge`, ни при выборе пользователя, ни снимком
/// `Material.knowledge` после того, как папку урока закрыли.
pub async fn statements_from_closed_lessons_never_reach_the_model() {
    let mut vault = anthropic_vault().await;
    let ids = put_graph(
        &mut vault,
        &[
            ("Закрытое утверждение про замыкания", "Закрытое/Б.lesson"),
            ("Открытое утверждение про замыкания", "Уроки/А.lesson"),
        ],
        &[(&[0], 1, "Закрытое/Б.lesson")],
    )
    .await;
    // Общее утверждение с двумя формулировками: первая из закрытой папки, вторая из открытой.
    let shared = proposition_id("Общее утверждение про замыкания");
    let settings = load_settings(&vault).await;
    store::append_propositions(
        &mut vault,
        &settings,
        &[
            PropositionLine::Phrasing {
                id: shared.clone(),
                text: "Закрытая формулировка общего про замыкания".to_owned(),
                source: "Закрытое/Б.lesson".to_owned(),
                event: Some(1),
                at: 1,
            },
            PropositionLine::Phrasing {
                id: shared.clone(),
                text: "Открытая формулировка общего про замыкания".to_owned(),
                source: "Уроки/А.lesson".to_owned(),
                event: Some(1),
                at: 1,
            },
        ],
    )
    .await
    .unwrap();

    // Выбор до закрытия папки: оба утверждения попадают в снимок.
    let pick = vec![ids[0].clone(), ids[1].clone(), shared.clone()];
    let path = create_lesson_with_knowledge(
        &mut vault,
        "Замыкания",
        &[],
        &pick,
        Language::Ru,
        LessonPhase::Probe,
        5,
    )
    .await
    .unwrap();
    let view = open_lesson(&mut vault, &path).await.unwrap();
    let LessonEvent::Material {
        knowledge: Some(material),
        ..
    } = &view.events[0]
    else {
        panic!("{:?}", view.events[0]);
    };
    assert_eq!(material.statements.len(), 3);

    // Папку закрывают.
    let mut denied = load_settings(&vault).await;
    denied.denied_folders = vec![text("Закрытое")];
    denied.knowledge_agent_reads = true;
    save_settings(&mut vault, &denied).await.unwrap();

    // Снимок: закрытое не уходит, общее — с открытой формулировкой, цепочка с закрытым пропала.
    let transport = StreamTransport::with(vec![say("Начнём")]);
    cycle(
        &mut vault,
        &transport,
        &path,
        &unique("r"),
        super::lesson_cases::answer("привет"),
        view.events_len,
    )
    .await
    .unwrap();
    let body = transport.body(0);
    assert!(body.contains("Открытое утверждение про замыкания"));
    assert!(body.contains("Открытая формулировка общего"));
    assert!(!body.contains("Закрытое утверждение") && !body.contains("Закрытая формулировка"));

    // Выбор после закрытия: закрытые пропускаются.
    let picked = select_material(&vault, &denied, &pick)
        .await
        .unwrap()
        .unwrap();
    let texts_picked: Vec<&str> = picked.statements.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        texts_picked,
        vec![
            "Открытое утверждение про замыкания",
            "Открытая формулировка общего про замыкания"
        ]
    );
    assert!(picked.chains.is_empty());
    let only_closed = select_material(&vault, &denied, &pick[..1]).await.unwrap();
    assert!(only_closed.is_none());

    // `read_knowledge`: то же.
    let (_, result) = agent_results(&vault, &denied, "замыкания").await;
    let result = result.unwrap();
    let all: String = result
        .statements
        .iter()
        .map(|s| s.text.clone())
        .collect::<Vec<_>>()
        .join("|");
    assert!(all.contains("Открытое утверждение") && all.contains("Открытая формулировка"));
    assert!(!all.contains("Закрытое") && !all.contains("Закрытая"));
    assert!(result.chains.is_empty());

    // Извлечение: «известные» в запросе без закрытого.
    put_teach(
        &vault,
        LESSON,
        &[teacher(1, "Расскажу про замыкания и их окружение.")],
    );
    let transport = StreamTransport::with(vec![one("a", "Новое утверждение", 1)]);
    run(
        &mut vault,
        &transport,
        &unique("kv"),
        LESSON,
        &unique("r"),
        10,
    )
    .await
    .unwrap();
    let sent = transport.body(0);
    assert!(sent.contains("Открытое утверждение про замыкания"));
    assert!(sent.contains("Открытая формулировка общего"));
    assert!(!sent.contains("Закрытое утверждение") && !sent.contains("Закрытая формулировка"));
}

/// M2: `Moved` пишется только для путей, которые префиксом касаются источников графа;
/// учитываются последние переносы, а не первые 256.
pub async fn moves_are_written_only_for_known_sources_and_the_latest_are_kept() {
    let mut vault = anthropic_vault().await;
    put_teach(&vault, LESSON, &teach_events(1));
    let transport = StreamTransport::with(vec![one("a", "Вклад урока", 0)]);
    run(
        &mut vault,
        &transport,
        &unique("kv"),
        LESSON,
        &unique("r"),
        10,
    )
    .await
    .unwrap();
    let before = vault_size(&vault);
    // Сколько угодно переносов посторонних файлов и папок строк не добавляют.
    for i in 0..300 {
        store::move_source(
            &mut vault,
            std::path::Path::new(&format!("Заметки/{i}.typ")),
            std::path::Path::new(&format!("Заметки/Архив/{i}.typ")),
            100 + i,
        )
        .await;
    }
    assert_eq!(vault_size(&vault), before);
    // Перенос урока и его папки записывается.
    store::move_source(
        &mut vault,
        std::path::Path::new("Уроки"),
        std::path::Path::new("Курсы"),
        500,
    )
    .await;
    assert!(vault_size(&vault) > before);
    let projection = graph_of(&vault).await;
    assert_eq!(projection.covered_of("Курсы/Замыкания.lesson"), 1);

    // Враждебный файл: 300 посторонних переносов раньше настоящего. Настоящий — последний —
    // действует, и вклад можно убрать.
    let mut vault = anthropic_vault().await;
    let a = proposition_id("Утверждение раз");
    let mut sources = String::new();
    for i in 0..300 {
        sources.push_str(&format!(
            "{}\n",
            json!({"line":"Moved","from":format!("Шум/{i}"),"to":format!("Шум/{i}х"),"at":1000 + i})
        ));
    }
    sources.push_str(&format!(
        "{}\n",
        json!({"line":"Moved","from":"Уроки/А.lesson","to":"Уроки/Б.lesson","at":2000})
    ));
    sources.push_str(&format!(
        "{}\n",
        json!({"line":"Covered","source":"Уроки/А.lesson","kind":"Lesson","upto":2,"at":10})
    ));
    vault.replace_externally(&sources_path(), sources);
    vault.replace_externally(
        &format!("{GRAPH}/{PROPOSITIONS_FILE}"),
        format!(
            "{}\n",
            json!({"line":"Phrasing","id":a,"text":"Утверждение раз","source":"Уроки/А.lesson","event":1,"at":10})
        ),
    );
    let projection = graph_of(&vault).await;
    assert_eq!(projection.covered_of("Уроки/Б.lesson"), 2);
    assert_eq!(
        projection.propositions[&a].phrasings[0].source,
        "Уроки/Б.lesson"
    );
    remove_source(&mut vault, "Уроки/Б.lesson", false, 3000)
        .await
        .unwrap();
    assert!(graph_of(&vault).await.propositions.is_empty());
}

fn vault_size(vault: &MemoryVault) -> usize {
    vault
        .bytes_of(&sources_path())
        .map_or(0, |bytes| bytes.len())
}

/// M3: разбор переносов не квадратичен на враждебном `sources.jsonl`, а цепочка переносов
/// по-прежнему применяется по порядку и с учётом времени строки. Возвращает мс с переносами и без них (одни и те же строки).
pub async fn resolving_moves_is_not_quadratic_on_a_hostile_sources_file() -> (i64, i64) {
    // Цепочка A -> B -> C: порядок и время строки важны.
    let chain = [
        json!({"line":"Moved","from":"A","to":"B","at":10}),
        json!({"line":"Moved","from":"B","to":"C","at":20}),
        json!({"line":"Covered","source":"A/х.lesson","kind":"Lesson","upto":1,"at":5}),
        json!({"line":"Covered","source":"B/у.lesson","kind":"Lesson","upto":2,"at":15}),
        json!({"line":"Covered","source":"A/з.lesson","kind":"Lesson","upto":3,"at":25}),
    ]
    .iter()
    .map(|line| format!("{line}\n"))
    .collect::<String>();
    let projection = project(b"", b"", chain.as_bytes());
    assert_eq!(projection.covered["C/х.lesson"], 1);
    assert_eq!(projection.covered["C/у.lesson"], 2);
    assert_eq!(projection.covered["A/з.lesson"], 3);

    // 256 переносов с одним первым компонентом и много строк с разными путями.
    let mut sources = String::new();
    for i in 0..256 {
        sources.push_str(&format!(
            "{}\n",
            json!({"line":"Moved","from":format!("Папка/Глубже{i}"),"to":format!("Новая/Глубже{i}"),"at":1_000_000})
        ));
    }
    for i in 0..150_000 {
        sources.push_str(&format!(
            "{}\n",
            json!({"line":"Covered","source":format!("Папка/Глубже{}/урок{i}.lesson", i % 500),
                "kind":"Lesson","upto":1,"at":i})
        ));
    }
    let started = typsidian_core::properties::now_ms();
    let projection = project(b"", b"", sources.as_bytes());
    let elapsed = typsidian_core::properties::now_ms() - started;
    let plain: String = sources
        .lines()
        .skip(256)
        .map(|l| format!("{l}\n"))
        .collect();
    let s2 = typsidian_core::properties::now_ms();
    let _ = project(b"", b"", plain.as_bytes());
    let plain_elapsed = typsidian_core::properties::now_ms() - s2;
    assert!(projection.covered.len() > 100_000);
    assert!(projection
        .covered
        .contains_key("Новая/Глубже7/урок7.lesson"));
    (elapsed, plain_elapsed)
}

/// L1: надгробие и удаление вклада строго позже всех строк своего id и источника, даже если
/// часы устройства отстают; свежая строка после этого оживляет.
pub async fn tombstones_are_later_than_every_line_of_their_id_and_source() {
    let mut vault = anthropic_vault().await;
    let a = proposition_id("Утверждение раз");
    vault.replace_externally(
        &format!("{GRAPH}/{PROPOSITIONS_FILE}"),
        format!(
            "{}\n",
            json!({"line":"Phrasing","id":a,"text":"Утверждение раз","source":LESSON,"event":1,"at":5000})
        ),
    );
    vault.replace_externally(
        &sources_path(),
        format!(
            "{}\n",
            json!({"line":"Covered","source":LESSON,"kind":"Lesson","upto":1,"at":5000})
        ),
    );
    // Часы отстают: `now` = 10 при строках на 5000.
    remove_proposition(&mut vault, &a, 10).await.unwrap();
    assert!(graph_of(&vault).await.propositions.is_empty());
    let tomb = String::from_utf8(
        vault
            .bytes_of(&format!("{GRAPH}/{PROPOSITIONS_FILE}"))
            .unwrap(),
    )
    .unwrap();
    assert!(tomb.contains("\"at\":5001"), "{tomb}");

    // Свежая строка после надгробия оживляет утверждение, даже при тех же отстающих часах.
    put_teach(
        &vault,
        LESSON,
        &[teacher(0, "Первое."), teacher(1, "Второе.")],
    );
    let transport = StreamTransport::with(vec![one("a", "Утверждение раз", 0)]);
    run(
        &mut vault,
        &transport,
        &unique("kv"),
        LESSON,
        &unique("r"),
        20,
    )
    .await
    .unwrap();
    assert!(graph_of(&vault).await.propositions.contains_key(&a));

    // Удаление вклада источника при отстающих часах скрывает и строки с будущим временем.
    remove_source(&mut vault, LESSON, false, 30).await.unwrap();
    assert!(graph_of(&vault).await.propositions.is_empty());
    // Покрытие сохранено: убранный вклад заново не извлекается.
    assert!(graph_of(&vault).await.covered_of(LESSON) >= 1);
}

/// L2 и T030: удаление урока смотрит на ходы своего vault, не идёт через ссылки, отменяет
/// идущее задание извлечения и сообщает об удалении событием vault.
pub async fn deleting_a_lesson_is_scoped_to_its_vault_and_cancels_the_extraction() {
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::events::VaultEvent;

    // Ход чужого vault на том же пути удалению не мешает; свой — мешает; без vault — любой.
    let mut vault = anthropic_vault().await;
    put_teach(&vault, LESSON, &teach_events(1));
    typsidian_core::ai::lesson::partial::start("va", "ход", LESSON);
    assert_eq!(
        delete_lesson(&mut vault, Some("va"), LESSON, true, 1)
            .await
            .unwrap_err(),
        AiFailure::LessonChanged
    );
    assert_eq!(
        delete_lesson(&mut vault, None, LESSON, true, 1)
            .await
            .unwrap_err(),
        AiFailure::LessonChanged
    );
    assert!(vault.bytes_of(LESSON).is_some());
    delete_lesson(&mut vault, Some("vb"), LESSON, true, 2)
        .await
        .unwrap();
    typsidian_core::ai::lesson::partial::take("va", "ход");
    assert!(vault.bytes_of(LESSON).is_none());
    // Событие удаления файла vault, как у любого удалённого файла (T030).
    let events = vault.poll_events();
    assert!(
        events.iter().any(|event| matches!(event,
            VaultEvent::NoteDeleted { path } if path == std::path::Path::new(LESSON))),
        "{events:?}"
    );

    // Путь через символическую ссылку не удаляется.
    let mut linked = anthropic_vault().await.with_symlink("Уроки");
    put_teach(&linked, LESSON, &teach_events(1));
    assert_eq!(
        delete_lesson(&mut linked, Some("va"), LESSON, true, 3)
            .await
            .unwrap_err(),
        AiFailure::Forbidden
    );
    assert!(linked.bytes_of(LESSON).is_some());
    assert!(linked.poll_events().is_empty());

    // Идущее задание извлечения по этому уроку отменяется; его пачка графа не пишет.
    let mut vault = anthropic_vault().await;
    put_teach(&vault, LESSON, &teach_events(2));
    let verified = connection(&vault).await;
    let vault_id = unique("kv");
    let request = unique("r");
    let prepared = begin_extract(&mut vault, &vault_id, LESSON, &request, 10, true, &verified)
        .await
        .unwrap()
        .unwrap();
    delete_lesson(&mut vault, Some(&vault_id), LESSON, false, 11)
        .await
        .unwrap();
    let job = jobs(&vault_id)
        .into_iter()
        .find(|job| job.id == request)
        .unwrap();
    assert_eq!(job.state, JobState::Cancelled);
    let transport = StreamTransport::with(vec![one("a", "Утверждение", 0)]);
    let outcome = send_extract(&transport, &vault_id, &prepared, Some(KEY), &verified).await;
    assert_eq!(
        finish_extract(&mut vault, &vault_id, &prepared, 12, outcome)
            .await
            .unwrap_err(),
        AiFailure::Cancelled
    );
    assert!(graph_of(&vault).await.propositions.is_empty());
    assert!(vault.paths_under(GRAPH).is_empty());
}

/// L3, L4, L5: пачка принимается, только пока идёт её задание по тому же уроку; `end()`
/// снимает только свою отметку отмены; охрана задания чужое не трогает.
pub async fn a_forged_or_stale_batch_is_not_written_and_end_removes_only_its_own_flag() {
    use typsidian_core::ai::cancel::{flag_cancelled, Cycle};
    use typsidian_core::ai::knowledge::jobs::{JobGuard, KNOWLEDGE_JOBS};

    let mut vault = anthropic_vault().await;
    put_teach(&vault, LESSON, &teach_events(2));
    let verified = connection(&vault).await;
    let vault_id = unique("kv");
    let request = unique("r");
    let prepared = begin_extract(&mut vault, &vault_id, LESSON, &request, 10, true, &verified)
        .await
        .unwrap()
        .unwrap();
    let transport = StreamTransport::with(vec![one("a", "Утверждение", 0)]);
    let outcome = send_extract(&transport, &vault_id, &prepared, Some(KEY), &verified).await;

    // Подделанный источник: задание идёт по другому уроку.
    let mut forged = prepared.clone();
    forged.source = "Уроки/Чужой.lesson".to_owned();
    assert_eq!(
        finish_extract(&mut vault, &vault_id, &forged, 11, outcome.clone())
            .await
            .unwrap_err(),
        AiFailure::Cancelled
    );
    // Чужой vault_id: задания нет.
    assert_eq!(
        finish_extract(&mut vault, "чужой", &prepared, 11, outcome.clone())
            .await
            .unwrap_err(),
        AiFailure::Cancelled
    );
    assert!(vault.paths_under(GRAPH).is_empty());
    assert_eq!(jobs(&vault_id)[0].state, JobState::Running);

    // Второе задание по тому же уроку не заводится и первое не задевает.
    let second = begin_extract(
        &mut vault,
        &vault_id,
        LESSON,
        &unique("r"),
        12,
        true,
        &verified,
    )
    .await
    .unwrap_err();
    assert_eq!(second, AiFailure::Cancelled);
    assert_eq!(jobs(&vault_id)[0].state, JobState::Running);

    // Закончено — запоздавшая пачка тоже не пишет.
    KNOWLEDGE_JOBS.end(
        &vault_id,
        &request,
        JobState::Cancelled,
        Some(AiFailure::Cancelled),
        13,
    );
    assert_eq!(
        finish_extract(&mut vault, &vault_id, &prepared, 14, outcome)
            .await
            .unwrap_err(),
        AiFailure::Cancelled
    );
    assert!(vault.paths_under(GRAPH).is_empty());

    // `end()` чужого id (цикл урока держит отметку) отметку не снимает.
    let other = unique("c");
    let cycle = Cycle::open(&vault_id, &other).expect("цикл");
    KNOWLEDGE_JOBS.end(&vault_id, &other, JobState::Done, None, 15);
    assert!(!flag_cancelled(&vault_id, &other));
    drop(cycle);

    // Охрана: взведённая отменяет идущее задание, снятая — нет.
    let mut second_vault = anthropic_vault().await;
    put_teach(&second_vault, LESSON, &teach_events(2));
    for (disarm, expected) in [(false, JobState::Cancelled), (true, JobState::Running)] {
        let id = unique("r");
        begin_extract(
            &mut second_vault,
            &vault_id,
            "Уроки/Другой.lesson",
            &id,
            20,
            true,
            &verified,
        )
        .await
        .ok();
        // Файла «Другой» нет: задания не завелось; берём настоящий путь с новым vault.
        let vault_key = unique("kv");
        begin_extract(
            &mut second_vault,
            &vault_key,
            LESSON,
            &id,
            20,
            true,
            &verified,
        )
        .await
        .unwrap();
        let mut guard = JobGuard::new(&vault_key, &id);
        if disarm {
            guard.disarm();
        }
        drop(guard);
        let job = jobs(&vault_key)
            .into_iter()
            .find(|job| job.id == id)
            .unwrap();
        assert_eq!(job.state, expected);
        KNOWLEDGE_JOBS.end(&vault_key, &id, JobState::Done, None, 21);
    }
}

/// L7, L8: модель ссылается только на отправленные «известные» id и ключи вызова;
/// надгробия пишутся и в заполненный файл.
pub async fn model_refs_go_only_to_sent_ids_and_tombstones_fit_in_a_full_file() {
    let mut vault = anthropic_vault().await;
    let ids = put_graph(
        &mut vault,
        &[
            ("Отправленное утверждение", "Уроки/А.lesson"),
            ("Неотправленное утверждение", "Уроки/А.lesson"),
        ],
        &[],
    )
    .await;
    let projection = graph_of(&vault).await;
    let found = parse_findings(
        &json!({"propositions":[
            {"key":"a","text":"Новая формулировка раз","same_as":ids[0]},
            {"key":"b","text":"Новая формулировка два","same_as":ids[1]},
            {"key":"c","text":"Третье новое"}],
          "chains":[
            {"premises":[ids[0]],"conclusion":"c"},
            {"premises":[ids[1]],"conclusion":"c"},
            {"premises":["b"],"conclusion":"c"}]})
        .to_string(),
    )
    .unwrap();
    let known: HashSet<String> = HashSet::from([ids[0].clone()]);
    let plan = merge(&projection, &found, LESSON, &[0], 100, &known);
    // Отправленное объединено, неотправленное осталось отдельным утверждением.
    assert_eq!((plan.propositions_added, plan.propositions_merged), (2, 1));
    let phrased: Vec<&String> = plan
        .propositions
        .iter()
        .filter_map(|line| match line {
            PropositionLine::Phrasing { id, text, .. } if text == "Новая формулировка два" => {
                Some(id)
            }
            _ => None,
        })
        .collect();
    assert_eq!(phrased, vec![&proposition_id("Новая формулировка два")]);
    // Цепочка со ссылкой на неотправленный id отклонена, остальные приняты.
    assert_eq!((plan.chains_added, plan.chains_rejected), (2, 1));

    // Файл заполнен почти до предела: новое утверждение не пишется, надгробие — пишется.
    let mut vault = anthropic_vault().await;
    let id = proposition_id("Утверждение раз");
    let first = format!(
        "{}\n",
        json!({"line":"Phrasing","id":id,"text":"Утверждение раз","source":LESSON,"event":1,"at":1})
    );
    let mut file = first.clone();
    let junk = format!("{}\n", "x".repeat(7_000));
    while file.len() + junk.len() < MAX_FILE_BYTES - 1_000 {
        file.push_str(&junk);
    }
    vault.replace_externally(&format!("{GRAPH}/{PROPOSITIONS_FILE}"), file);
    let settings = load_settings(&vault).await;
    let fresh = PropositionLine::Phrasing {
        id: proposition_id("Новое"),
        text: "Новое".to_owned(),
        source: LESSON.to_owned(),
        event: None,
        at: 2,
    };
    assert_eq!(
        store::append_propositions(&mut vault, &settings, &[fresh])
            .await
            .unwrap_err(),
        AiFailure::JournalFailed
    );
    remove_proposition(&mut vault, &id, 3).await.unwrap();
    assert!(graph_of(&vault).await.propositions.is_empty());
    let size = vault
        .bytes_of(&format!("{GRAPH}/{PROPOSITIONS_FILE}"))
        .unwrap()
        .len();
    assert!(size <= MAX_FILE_BYTES);
}

/// L9: покрытие и конфликт заданий сравнивают путь в свёрнутой форме `check_access`.
pub async fn coverage_and_job_conflict_use_the_folded_path() {
    let sources = format!(
        "{}\n",
        json!({"line":"Covered","source":LESSON,"kind":"Lesson","upto":5,"at":1})
    );
    let projection = project(b"", b"", sources.as_bytes());
    assert_eq!(projection.covered_of("уроки/ЗАМЫКАНИЯ.lesson"), 5);
    assert_eq!(projection.covered_of("Уроки/Замыкания.lesson"), 5);
    assert_eq!(projection.covered_of("Уроки/Иное.lesson"), 0);

    // Два написания одного пути на файловой системе без учёта регистра — одно задание.
    let mut vault = anthropic_vault().await;
    let lower = "уроки/замыкания.lesson";
    put_teach(&vault, LESSON, &teach_events(2));
    put_teach(&vault, lower, &teach_events(2));
    let verified = connection(&vault).await;
    let vault_id = unique("kv");
    let first = begin_extract(
        &mut vault,
        &vault_id,
        LESSON,
        &unique("r"),
        10,
        true,
        &verified,
    )
    .await
    .unwrap();
    assert!(first.is_some());
    assert_eq!(
        begin_extract(
            &mut vault,
            &vault_id,
            lower,
            &unique("r"),
            11,
            true,
            &verified
        )
        .await
        .unwrap_err(),
        AiFailure::Cancelled
    );
    assert_eq!(jobs(&vault_id).len(), 1);
}
