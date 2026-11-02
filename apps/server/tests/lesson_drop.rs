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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // Модель, которая начинает ответ и замолкает.
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
            json!({ "goal": "Обрыв", "notes": [], "language": "ru", "now": 1 }),
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
