//! Обрыв хода урока на сервере (051, ревью безопасности M6). Отдельный тестовый бинарник:
//! тесту нужна модель на loopback, то есть `sync_private_remotes`, а политика сервера —
//! глобальная на процесс (`server_policy::restrict_to_server`), и параллельные тесты
//! `api.rs` с другой настройкой её перезаписывали.
#![allow(dead_code)]

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use typsidian_server::auth::hash_password;
use typsidian_server::config::Config;
use typsidian_server::store::Store;
use typsidian_server::{router, AppState, Shared};

const PASSWORD: &str = "правильный пароль";

struct Server {
    app: Router,
    state: Shared,
    _data: tempfile::TempDir,
}

struct Client {
    cookie: String,
    csrf: String,
}

impl Server {
    async fn new() -> Server {
        Server::with_config(|_| {}).await
    }

    async fn with_config(edit: impl FnOnce(&mut Config)) -> Server {
        let data = tempfile::tempdir().unwrap();
        let mut config = Config {
            data: data.path().to_path_buf(),
            ..Config::default()
        };
        edit(&mut config);
        let store = Store::memory().await.unwrap();
        let hash = hash_password(PASSWORD).unwrap();
        for login in ["anna", "bob"] {
            store.add_account(login, login, Some(&hash)).await.unwrap();
        }
        let state = AppState::new(config, store);
        Server {
            app: router(state.clone()),
            state,
            _data: data,
        }
    }

    async fn call(
        &self,
        method: Method,
        uri: &str,
        client: Option<&Client>,
        body: Option<Value>,
    ) -> (StatusCode, Value, Option<String>) {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(client) = client {
            request = request
                .header(header::COOKIE, &client.cookie)
                .header("x-typsidian-csrf", &client.csrf);
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .map(|v| v.to_str().unwrap().to_owned());
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value, cookie)
    }

    async fn login(&self, login: &str, password: &str) -> (StatusCode, Value, Option<Client>) {
        let (status, body, cookie) = self
            .call(
                Method::POST,
                "/api/login",
                None,
                Some(json!({ "login": login, "password": password })),
            )
            .await;
        let client = cookie.map(|cookie| Client {
            cookie: cookie.split(';').next().unwrap().to_owned(),
            csrf: body["csrf"].as_str().unwrap_or_default().to_owned(),
        });
        (status, body, client)
    }

    async fn enter(&self, login: &str) -> Client {
        self.login(login, PASSWORD).await.2.expect("вход")
    }

    async fn invoke(
        &self,
        client: &Client,
        vault: &str,
        command: &str,
        args: Value,
    ) -> (StatusCode, Value) {
        let uri = format!("/api/vaults/{}/invoke/{command}", encode(vault));
        let (status, body, _) = self
            .call(Method::POST, &uri, Some(client), Some(args))
            .await;
        (status, body)
    }
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// 051, ревью безопасности M6: клиент оборвал соединение посреди хода — буфер хода, слот
/// отмены, место аккаунта и цикл освобождаются, и тот же `requestId` годится снова.
#[tokio::test]
async fn dropped_lesson_request_releases_everything() {
    let port = hanging_model().await;

    let server = Server::with_config(|config| {
        config.ai_custom_endpoint = true;
        config.sync_private_remotes = true;
    })
    .await;
    let anna = server.enter("anna").await;
    let (status, _, _) = server
        .call(
            Method::POST,
            "/api/vaults",
            Some(&anna),
            Some(json!({ "name": "Общее имя" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let settings = json!({ "settings": {
        "enabled": true,
        "provider": "Custom",
        "custom_endpoint": format!("http://127.0.0.1:{port}/v1"),
        "model": "m",
    }});
    let (status, body) = server
        .invoke(&anna, "Общее имя", "ai_settings_set", settings)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = server
        .invoke(
            &anna,
            "Общее имя",
            "ai_approve_endpoint",
            json!({ "insecureOk": false }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, path) = server
        .invoke(
            &anna,
            "Общее имя",
            "ai_lesson_create",
            json!({ "goal": "Обрыв", "notes": [], "language": "ru", "start": "Teach", "now": 1 }),
        )
        .await;
    let path = path.as_str().expect("путь урока").to_owned();

    let answer = json!({
        "path": path, "requestId": "r1",
        "answer": { "text": "привет" }, "expected": 0, "now": 2,
    });
    {
        // Ответ идёт и не заканчивается: соединение «обрывается» сбросом ожидания. Дальше
        // повтор с тем же `requestId`: если бы цикл или слот остались, он вернулся бы сразу
        // отказом (`Cancelled`, `RateLimited`), а не повис бы в ожидании модели.
        let waiting = tokio::time::timeout(
            std::time::Duration::from_millis(700),
            server.invoke(&anna, "Общее имя", "ai_lesson_answer", answer.clone()),
        )
        .await;
        assert!(waiting.is_err(), "ответ завершился: {waiting:?}");

        let (_, progress) = server
            .invoke(
                &anna,
                "Общее имя",
                "ai_lesson_progress",
                json!({ "requestId": "r1" }),
            )
            .await;
        assert_eq!(progress, Value::Null, "буфер хода остался");
        // Урок не «идёт»: оборванный ответ можно запросить заново, а не ждать вечно.
        let (_, view) = server
            .invoke(
                &anna,
                "Общее имя",
                "ai_lesson_open",
                json!({ "path": path }),
            )
            .await;
        assert_eq!(view["waiting"], "Retry", "{view}");
        // Для второй попытки ответ пользователя уже в файле: повтор без нового ответа.
        let events = view["events_len"].as_u64().unwrap();
        let retry = json!({
            "path": path, "requestId": "r1", "answer": null, "expected": events, "now": 3,
        });
        let again = tokio::time::timeout(
            std::time::Duration::from_millis(700),
            server.invoke(&anna, "Общее имя", "ai_lesson_answer", retry),
        )
        .await;
        assert!(again.is_err(), "повтор завершился: {again:?}");
    }
}

/// Модель, которая начинает ответ и замолкает. Возвращает порт на loopback.
async fn hanging_model() -> u16 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buffer = [0u8; 65536];
                let _ = socket.read(&mut buffer).await;
                let part = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Идёт\"}}\n\n";
                let _ = socket
                    .write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n")
                    .await;
                let _ = socket.write_all(part.as_bytes()).await;
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            });
        }
    });
    port
}

/// 054, ревью безопасности L3 и L6: пополнение графа держит общий предел запросов только на
/// время пачки, фоновое место аккаунта — на время вызова; клиент, оборвавший вызов посреди
/// пачки, не оставляет ни задания «идёт», ни занятых мест.
#[tokio::test]
async fn dropped_knowledge_extraction_releases_everything() {
    use typsidian_core::ai::lesson::file::{initial_text, FORMAT};
    use typsidian_core::ai::lesson::{
        Language, LessonEvent, LessonHeader, LessonKind, LessonPhase,
    };
    use typsidian_server::MAX_AI_REQUESTS;

    let port = hanging_model().await;
    let server = Server::with_config(|config| {
        config.ai_custom_endpoint = true;
        config.sync_private_remotes = true;
    })
    .await;
    let anna = server.enter("anna").await;
    let (status, _, _) = server
        .call(
            Method::POST,
            "/api/vaults",
            Some(&anna),
            Some(json!({ "name": "Общее имя" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let settings = json!({ "settings": {
        "enabled": true,
        "provider": "Custom",
        "custom_endpoint": format!("http://127.0.0.1:{port}/v1"),
        "model": "m",
    }});
    let (status, body) = server
        .invoke(&anna, "Общее имя", "ai_settings_set", settings)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = server
        .invoke(
            &anna,
            "Общее имя",
            "ai_approve_endpoint",
            json!({ "insecureOk": false }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // Урок с объяснением учителя кладётся обычной записью файла vault.
    let header = LessonHeader {
        format: FORMAT,
        goal: "Понять замыкания".to_owned(),
        language: Language::Ru,
        created_at: 1,
        start: LessonPhase::Teach,
        kind: LessonKind::Lesson,
    };
    let teacher = LessonEvent::Teacher {
        at: 1,
        text: "Замыкание хранит окружение функции.".to_owned(),
        question: None,
        interrupted: false,
        failure: None,
        model: "m".to_owned(),
        input_tokens: None,
        output_tokens: None,
        cost_usd: None,
    };
    let (_, created) = server
        .invoke(
            &anna,
            "Общее имя",
            "ai_lesson_create",
            json!({ "goal": "Замыкания", "notes": [], "language": "ru", "start": "Teach", "now": 1 }),
        )
        .await;
    let path = created.as_str().expect("путь урока").to_owned();
    let path = path.as_str();
    let (status, body) = server
        .invoke(
            &anna,
            "Общее имя",
            "write",
            json!({ "path": path, "content": initial_text(&header, &[teacher]).unwrap(), "expected": null }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let anna_id = server
        .state
        .store
        .account_by_login("anna")
        .await
        .unwrap()
        .unwrap()
        .id;
    let extract = |request: &'static str| {
        server.invoke(
            &anna,
            "Общее имя",
            "ai_knowledge_extract",
            json!({ "source": path, "requestId": request, "now": 5 }),
        )
    };
    {
        // Вызов идёт и не заканчивается; посреди пачки заняты общее и фоновое места.
        let watcher = async {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            assert_eq!(
                server.state.ai_requests.available_permits(),
                MAX_AI_REQUESTS - 1,
                "пачка не заняла общий предел"
            );
            assert!(server
                .state
                .ai_background
                .lock()
                .unwrap()
                .contains_key(&anna_id));
            let (_, jobs) = server
                .invoke(&anna, "Общее имя", "ai_knowledge_jobs", json!({}))
                .await;
            assert_eq!(jobs[0]["state"], "Running", "{jobs}");
        };
        let (waiting, ()) = tokio::join!(
            tokio::time::timeout(std::time::Duration::from_millis(900), extract("k1")),
            watcher
        );
        assert!(waiting.is_err(), "пополнение завершилось: {waiting:?}");
    }
    // Вызов оборван: все места возвращены, задание не «идёт».
    assert_eq!(
        server.state.ai_requests.available_permits(),
        MAX_AI_REQUESTS
    );
    assert!(server.state.ai_background.lock().unwrap().is_empty());
    assert!(server.state.ai_by_account.lock().unwrap().is_empty());
    let (_, jobs) = server
        .invoke(&anna, "Общее имя", "ai_knowledge_jobs", json!({}))
        .await;
    assert_eq!(jobs[0]["id"], "k1", "{jobs}");
    assert_eq!(jobs[0]["state"], "Cancelled", "{jobs}");

    // Новое пополнение берёт места снова и не получает отказа (повисает в ожидании модели).
    let again = tokio::time::timeout(std::time::Duration::from_millis(700), extract("k2")).await;
    assert!(again.is_err(), "повтор завершился: {again:?}");
}

/// 056: исследование в уроке держит фоновое место аккаунта на время вызова и общий предел
/// запросов — на время одного запроса; клиент, оборвавший вызов посреди запроса, не оставляет
/// ни задания «идёт», ни занятых мест.
#[tokio::test]
async fn dropped_research_releases_everything() {
    use typsidian_server::MAX_AI_REQUESTS;

    let port = hanging_model().await;
    let server = Server::with_config(|config| {
        config.ai_custom_endpoint = true;
        config.sync_private_remotes = true;
    })
    .await;
    let anna = server.enter("anna").await;
    let (status, _, _) = server
        .call(
            Method::POST,
            "/api/vaults",
            Some(&anna),
            Some(json!({ "name": "Общее имя" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let settings = json!({ "settings": {
        "enabled": true,
        "provider": "Custom",
        "custom_endpoint": format!("http://127.0.0.1:{port}/v1"),
        "model": "m",
    }});
    let (status, body) = server
        .invoke(&anna, "Общее имя", "ai_settings_set", settings)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = server
        .invoke(
            &anna,
            "Общее имя",
            "ai_approve_endpoint",
            json!({ "insecureOk": false }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, created) = server
        .invoke(
            &anna,
            "Общее имя",
            "ai_lesson_create",
            json!({ "goal": "Замыкания", "notes": [], "language": "ru", "start": "Teach", "now": 1 }),
        )
        .await;
    let path = created.as_str().expect("путь урока").to_owned();
    // PDF кладётся файлом в папку vault аккаунта: запись команды `write` принимает текст.
    let account = server
        .state
        .store
        .account_by_login("anna")
        .await
        .unwrap()
        .unwrap()
        .id;
    let dir = typsidian_server::vaults::vault_dir(&server.state, &account, "Общее имя").unwrap();
    std::fs::write(
        dir.join("Текст.pdf"),
        include_bytes!("../../../packages/core/tests/fixtures/pdf/text.pdf"),
    )
    .unwrap();
    // Оценка читает файл без сети.
    let (status, estimate) = server
        .invoke(
            &anna,
            "Общее имя",
            "ai_research_estimate",
            json!({ "pdf": "Текст.pdf" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{estimate}");
    assert_eq!(estimate["problem"], Value::Null, "{estimate}");
    assert_eq!(estimate["pages"], 2, "{estimate}");

    let anna_id = server
        .state
        .store
        .account_by_login("anna")
        .await
        .unwrap()
        .unwrap()
        .id;
    let start = |request: &'static str| {
        server.invoke(
            &anna,
            "Общее имя",
            "ai_research_start",
            json!({ "path": path, "requestId": request, "kind": "Pdf",
                "question": "Что хранит замыкание?", "pdf": "Текст.pdf", "now": 5 }),
        )
    };
    {
        let watcher = async {
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            assert_eq!(
                server.state.ai_requests.available_permits(),
                MAX_AI_REQUESTS - 1,
                "запрос не занял общий предел"
            );
            assert!(server
                .state
                .ai_background
                .lock()
                .unwrap()
                .contains_key(&anna_id));
            let (_, jobs) = server
                .invoke(&anna, "Общее имя", "ai_jobs", json!({}))
                .await;
            assert_eq!(jobs[0]["type"], "Research", "{jobs}");
            assert_eq!(jobs[0]["state"], "Running", "{jobs}");
        };
        let (waiting, ()) = tokio::join!(
            tokio::time::timeout(std::time::Duration::from_millis(1_000), start("r1")),
            watcher
        );
        assert!(waiting.is_err(), "исследование завершилось: {waiting:?}");
    }
    // Вызов оборван: места возвращены, задание не «идёт».
    assert_eq!(
        server.state.ai_requests.available_permits(),
        MAX_AI_REQUESTS
    );
    assert!(server.state.ai_background.lock().unwrap().is_empty());
    assert!(server.state.ai_by_account.lock().unwrap().is_empty());
    let (_, jobs) = server
        .invoke(&anna, "Общее имя", "ai_jobs", json!({}))
        .await;
    assert_eq!(jobs[0]["id"], "r1", "{jobs}");
    assert_eq!(jobs[0]["state"], "Cancelled", "{jobs}");
    // В уроке итога нет: карточка «прервано».
    let (_, lesson) = server
        .invoke(
            &anna,
            "Общее имя",
            "ai_lesson_open",
            json!({ "path": path }),
        )
        .await;
    assert_eq!(lesson["research"][0]["status"], "Interrupted", "{lesson}");

    // Повторный запуск берёт места снова и не получает отказа.
    let again = tokio::time::timeout(std::time::Duration::from_millis(800), start("r2")).await;
    assert!(again.is_err(), "повтор завершился: {again:?}");
}
