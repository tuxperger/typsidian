//! API сервера без сети (`tower::ServiceExt::oneshot`): вход, сессии, CSRF, лимит
//! попыток, команды vault, изоляция аккаунтов (T004, T005, T007, T011; FR-002,
//! FR-004, FR-011, SC-002).

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
        let data = tempfile::tempdir().unwrap();
        let config = Config {
            data: data.path().to_path_buf(),
            ..Config::default()
        };
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

#[tokio::test]
async fn login_does_not_say_what_was_wrong() {
    let server = Server::new().await;
    let (status, body, client) = server.login("anna", PASSWORD).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["login"], "anna");
    let client = client.unwrap();
    assert!(client.cookie.starts_with("__Host-typsidian="));

    let (wrong_password, a, _) = server.login("anna", "неверный пароль!").await;
    let (unknown_login, b, _) = server.login("nobody", PASSWORD).await;
    assert_eq!(wrong_password, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown_login, StatusCode::UNAUTHORIZED);
    assert_eq!(a, b);

    // Заблокированный аккаунт отвечает так же и теряет сессии.
    let anna = server
        .state
        .store
        .account_by_login("anna")
        .await
        .unwrap()
        .unwrap();
    server
        .state
        .store
        .set_blocked(&anna.id, true)
        .await
        .unwrap();
    let (blocked, c, _) = server.login("anna", PASSWORD).await;
    assert_eq!(blocked, StatusCode::UNAUTHORIZED);
    assert_eq!(a, c);
    let (me, _, _) = server
        .call(Method::GET, "/api/me", Some(&client), None)
        .await;
    assert_eq!(me, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn attempts_are_limited_per_login() {
    let server = Server::new().await;
    for _ in 0..4 {
        let (status, _, _) = server.login("anna", "неверный пароль!").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, body, _) = server.login("anna", "неверный пароль!").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["retry_after_s"], 60);
    // Даже верный пароль не проходит, пока вход закрыт; другой логин — проходит.
    let (status, _, _) = server.login("anna", PASSWORD).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let (status, _, _) = server.login("bob", PASSWORD).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn session_and_csrf_are_required() {
    let server = Server::new().await;
    let (status, body, _) = server.call(Method::GET, "/api/vaults", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["kind"], "Unauthenticated");

    let client = server.enter("anna").await;
    let no_csrf = Client {
        cookie: client.cookie.clone(),
        csrf: String::new(),
    };
    let (status, _, _) = server
        .call(
            Method::POST,
            "/api/vaults",
            Some(&no_csrf),
            Some(json!({ "name": "A" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Чтение без токена можно.
    let (status, _, _) = server
        .call(Method::GET, "/api/vaults", Some(&no_csrf), None)
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _, _) = server
        .call(
            Method::POST,
            "/api/vaults",
            Some(&client),
            Some(json!({ "name": "A" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // Выход завершает сессию.
    let (status, _, _) = server
        .call(Method::POST, "/api/logout", Some(&client), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = server
        .call(Method::GET, "/api/me", Some(&client), None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn notes_are_plain_files_on_the_server() {
    let server = Server::new().await;
    let anna = server.enter("anna").await;
    let (status, _, _) = server
        .call(
            Method::POST,
            "/api/vaults",
            Some(&anna),
            Some(json!({ "name": "Диссертация" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = server
        .invoke(
            &anna,
            "Диссертация",
            "create",
            json!({ "path": "глава.typ" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = server
        .invoke(
            &anna,
            "Диссертация",
            "write",
            json!({ "path": "глава.typ", "content": "= Введение\n" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, text) = server
        .invoke(&anna, "Диссертация", "read", json!({ "path": "глава.typ" }))
        .await;
    assert_eq!(text, "= Введение\n");

    let id = server
        .state
        .store
        .account_by_login("anna")
        .await
        .unwrap()
        .unwrap()
        .id;
    let on_disk = server
        .state
        .config
        .users_dir()
        .join(id)
        .join("Диссертация")
        .join("глава.typ");
    assert_eq!(std::fs::read_to_string(on_disk).unwrap(), "= Введение\n");

    // Отказ ядра — 400 с тем же телом, что получил бы клиент Tauri.
    let (status, body) = server
        .invoke(&anna, "Диссертация", "read", json!({ "path": "нет.typ" }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["kind"].is_string(), "{body}");

    // Команда вне белого списка — отказ.
    let (status, _) = server
        .invoke(&anna, "Диссертация", "root_path", json!({}))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = server
        .invoke(&anna, "Диссертация", "open_vault", json!({}))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Скачивание файла vault.
    let uri = format!(
        "/api/vaults/{}/file/{}",
        encode("Диссертация"),
        encode("глава.typ")
    );
    let (status, _, _) = server.call(Method::GET, &uri, Some(&anna), None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn other_accounts_vaults_are_unreachable() {
    let server = Server::new().await;
    let anna = server.enter("anna").await;
    let bob = server.enter("bob").await;
    server
        .call(
            Method::POST,
            "/api/vaults",
            Some(&anna),
            Some(json!({ "name": "Тайное" })),
        )
        .await;
    server
        .invoke(&anna, "Тайное", "create", json!({ "path": "a.typ" }))
        .await;

    let (_, list, _) = server
        .call(Method::GET, "/api/vaults", Some(&bob), None)
        .await;
    assert_eq!(list, json!([]));

    let vault = encode("Тайное");
    let attempts = [
        (Method::GET, format!("/api/vaults/{vault}")),
        (Method::PATCH, format!("/api/vaults/{vault}")),
        (Method::DELETE, format!("/api/vaults/{vault}")),
        (Method::POST, format!("/api/vaults/{vault}/invoke/read")),
        (Method::GET, format!("/api/vaults/{vault}/events")),
        (Method::GET, format!("/api/vaults/{vault}/file/a.typ")),
    ];
    for (method, uri) in attempts {
        let body = (method == Method::PATCH).then(|| json!({ "name": "Моё" }));
        let body = body.or_else(|| (method == Method::POST).then(|| json!({ "path": "a.typ" })));
        let (status, value, _) = server.call(method.clone(), &uri, Some(&bob), body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}");
        assert_eq!(value["kind"], "NoVault", "{method} {uri}");
    }
    // Тот же ответ, что у несуществующего vault: имя чужого не раскрывается.
    let (status, value) = server.invoke(&bob, "Нет такого", "list", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(value["kind"], "NoVault");

    // Имена-пути и выход из vault.
    for name in ["..", ".", "../anna", "a/b"] {
        let (status, _) = server.invoke(&bob, name, "list", json!({})).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{name}");
    }
    server
        .call(
            Method::POST,
            "/api/vaults",
            Some(&bob),
            Some(json!({ "name": "Своё" })),
        )
        .await;
    // Пути команд не выводят из vault: ни абсолютный путь машины, ни путь к vault
    // другого аккаунта (security review 037, находка 1).
    let anna_id = server
        .state
        .store
        .account_by_login("anna")
        .await
        .unwrap()
        .unwrap()
        .id;
    let secret = server
        .state
        .config
        .users_dir()
        .join(&anna_id)
        .join("Тайное")
        .join("a.typ");
    std::fs::write(&secret, "тайна").unwrap();
    let neighbour = format!("../../{anna_id}/Тайное/a.typ");
    let absolute = secret.to_string_lossy().into_owned();
    for path in [neighbour.as_str(), absolute.as_str(), "/etc/hostname"] {
        let (status, body) = server
            .invoke(&bob, "Своё", "read", json!({ "path": path }))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {body}");
        assert_ne!(body, json!("тайна"));
        for (command, args) in [
            ("write", json!({ "path": path, "content": "взлом" })),
            ("delete", json!({ "path": path })),
            ("rename", json!({ "from": path, "to": "украдено.typ" })),
            ("create", json!({ "path": format!("{path}.typ") })),
        ] {
            let (status, _) = server.invoke(&bob, "Своё", command, args).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{command} {path}");
        }
    }
    assert_eq!(std::fs::read_to_string(&secret).unwrap(), "тайна");
    let uri = format!(
        "/api/vaults/{}/file/..%2F..%2Fserver.sqlite",
        encode("Своё")
    );
    let (status, _, _) = server.call(Method::GET, &uri, Some(&bob), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn vaults_are_created_renamed_and_trashed() {
    let server = Server::new().await;
    let anna = server.enter("anna").await;
    let create = |name: &str| json!({ "name": name });
    let (status, _, _) = server
        .call(
            Method::POST,
            "/api/vaults",
            Some(&anna),
            Some(create("Черновики")),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = server
        .call(
            Method::POST,
            "/api/vaults",
            Some(&anna),
            Some(create("Черновики")),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["reason"], "Taken");
    let (status, body, _) = server
        .call(
            Method::POST,
            "/api/vaults",
            Some(&anna),
            Some(create("a/b")),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["reason"], "BadName");

    server
        .invoke(&anna, "Черновики", "create", json!({ "path": "x.typ" }))
        .await;
    let (_, summary, _) = server
        .call(
            Method::GET,
            &format!("/api/vaults/{}", encode("Черновики")),
            Some(&anna),
            None,
        )
        .await;
    assert_eq!(summary["notes"], 1);

    let (status, _, _) = server
        .call(
            Method::PATCH,
            &format!("/api/vaults/{}", encode("Черновики")),
            Some(&anna),
            Some(create("Статьи")),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, text) = server
        .invoke(&anna, "Статьи", "read", json!({ "path": "x.typ" }))
        .await;
    assert_eq!(text, "");

    let (status, _, _) = server
        .call(
            Method::DELETE,
            &format!("/api/vaults/{}", encode("Статьи")),
            Some(&anna),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, list, _) = server
        .call(Method::GET, "/api/vaults", Some(&anna), None)
        .await;
    assert_eq!(list, json!([]));
    let trash: Vec<_> = std::fs::read_dir(server.state.config.trash_dir())
        .unwrap()
        .collect();
    assert_eq!(trash.len(), 1);
}

#[tokio::test]
async fn sessions_can_be_listed_and_ended() {
    let server = Server::new().await;
    let first = server.enter("anna").await;
    let second = server.enter("anna").await;

    let (_, list, _) = server
        .call(Method::GET, "/api/sessions", Some(&first), None)
        .await;
    let list = list.as_array().unwrap().clone();
    assert_eq!(list.len(), 2);
    assert_eq!(list.iter().filter(|s| s["current"] == true).count(), 1);
    let other = list.iter().find(|s| s["current"] == false).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let (status, _, _) = server
        .call(
            Method::DELETE,
            &format!("/api/sessions/{other}"),
            Some(&first),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = server
        .call(Method::GET, "/api/me", Some(&second), None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Чужую сессию завершить нельзя.
    let bob = server.enter("bob").await;
    let (_, mine, _) = server
        .call(Method::GET, "/api/sessions", Some(&first), None)
        .await;
    let mine = mine[0]["id"].as_str().unwrap().to_owned();
    let (status, _, _) = server
        .call(
            Method::DELETE,
            &format!("/api/sessions/{mine}"),
            Some(&bob),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Смена пароля: нужен текущий, новый не короче 12 знаков, прочие сессии завершены.
    let third = server.enter("anna").await;
    let change = |current: &str, next: &str| json!({ "current": current, "next": next });
    let (status, body, _) = server
        .call(
            Method::POST,
            "/api/password",
            Some(&first),
            Some(change(PASSWORD, "коротко")),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["reason"], "TooShort");
    let (status, body, _) = server
        .call(
            Method::POST,
            "/api/password",
            Some(&first),
            Some(change("не тот пароль!", "новый длинный пароль")),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["reason"], "Wrong");
    let (status, _, _) = server
        .call(
            Method::POST,
            "/api/password",
            Some(&first),
            Some(change(PASSWORD, "новый длинный пароль")),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = server
        .call(Method::GET, "/api/me", Some(&third), None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = server
        .call(Method::GET, "/api/me", Some(&first), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = server.login("anna", "новый длинный пароль").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn impossible_logins_are_refused_without_work() {
    let server = Server::new().await;
    // Логина такого вида быть не может: отказ тот же, что у неверного пароля, а
    // счётчик попыток не заводится.
    let long = "a".repeat(100_000);
    let (status, body, _) = server.login(&long, PASSWORD).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["kind"], "BadCredentials");
    let (status, _, _) = server.login("anna", &"x".repeat(5000)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = server.login("anna", PASSWORD).await;
    assert_eq!(status, StatusCode::OK);
}
