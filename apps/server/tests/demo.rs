//! Демо-сервер (фича 039) без сети: гость, его vault, закрытые точки, квота,
//! предел по адресу, истечение и уборка (FR-001–FR-008, SC-002, SC-003).

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use typsidian_server::config::{Config, DemoConfig};
use typsidian_server::store::Store;
use typsidian_server::{router, AppState, Shared};

struct Server {
    app: Router,
    state: Shared,
    _data: tempfile::TempDir,
}

struct Guest {
    cookie: String,
    csrf: String,
    login: String,
}

impl Server {
    async fn new(demo_mode: bool, demo: DemoConfig) -> Server {
        let data = tempfile::tempdir().unwrap();
        let config = Config {
            data: data.path().to_path_buf(),
            demo_mode,
            demo,
            ..Config::default()
        };
        let state = AppState::new(config, Store::memory().await.unwrap());
        Server {
            app: router(state.clone()),
            state,
            _data: data,
        }
    }

    async fn demo() -> Server {
        Server::new(true, DemoConfig::default()).await
    }

    async fn call(
        &self,
        method: Method,
        uri: &str,
        guest: Option<&Guest>,
        body: Option<Value>,
    ) -> (StatusCode, Value, Option<String>) {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(guest) = guest {
            request = request
                .header(header::COOKIE, &guest.cookie)
                .header("x-typsidian-csrf", &guest.csrf);
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
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            cookie,
        )
    }

    async fn start(&self) -> (StatusCode, Value, Option<Guest>) {
        let (status, body, cookie) = self
            .call(Method::POST, "/api/demo/start", None, Some(json!({})))
            .await;
        let guest = cookie.map(|cookie| Guest {
            cookie: cookie.split(';').next().unwrap().to_owned(),
            csrf: body["csrf"].as_str().unwrap_or_default().to_owned(),
            login: body["login"].as_str().unwrap_or_default().to_owned(),
        });
        (status, body, guest)
    }

    async fn guest(&self) -> Guest {
        self.start().await.2.expect("гость")
    }

    async fn invoke(&self, guest: &Guest, command: &str, args: Value) -> (StatusCode, Value) {
        let uri = format!("/api/vaults/{}/invoke/{command}", encode("Демо"));
        let (status, body, _) = self.call(Method::POST, &uri, Some(guest), Some(args)).await;
        (status, body)
    }

    async fn account_id(&self, guest: &Guest) -> String {
        self.state
            .store
            .account_by_login(&guest.login)
            .await
            .unwrap()
            .unwrap()
            .id
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
async fn a_visitor_gets_an_own_vault_with_examples() {
    let server = Server::demo().await;
    let (_, info, _) = server.call(Method::GET, "/api/info", None, None).await;
    assert_eq!(info["password"], false);
    assert_eq!(info["demo"]["ttlMinutes"], 60);

    let (status, body, anna) = server.start().await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["vault"], "Демо");
    let anna = anna.unwrap();
    assert!(anna.cookie.starts_with("__Host-typsidian="));
    let (_, me, _) = server.call(Method::GET, "/api/me", Some(&anna), None).await;
    let left = me["expiresAt"].as_i64().unwrap() - typsidian_server::store::now();
    assert!((3590..=3600).contains(&left), "{left}");

    let (status, list) = server.invoke(&anna, "list", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let paths: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|note| note["path"].as_str())
        .collect();
    for expected in [
        "Начните здесь.typ",
        "Исследование/Методика.typ",
        "Доклад.typ",
    ] {
        assert!(paths.contains(&expected), "{expected}: {paths:?}");
    }
    // Примеры показывают связи — битых ссылок в них нет.
    for path in paths.iter().filter(|path| path.ends_with(".typ")) {
        let (status, broken) = server
            .invoke(&anna, "broken_links", json!({ "path": path }))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(broken, json!([]), "{path}");
    }

    // Второй посетитель правок первого не видит.
    let (status, _) = server
        .invoke(
            &anna,
            "write",
            json!({ "path": "Начните здесь.typ", "content": "= Правка Анны\n" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let boris = server.guest().await;
    let (_, text) = server
        .invoke(&boris, "read", json!({ "path": "Начните здесь.typ" }))
        .await;
    assert!(text.as_str().unwrap().starts_with("= Начните здесь"));
}

#[tokio::test]
async fn profile_login_and_vault_operations_are_closed() {
    let server = Server::demo().await;
    let guest = server.guest().await;
    let (status, _, _) = server
        .call(
            Method::POST,
            "/api/login",
            None,
            Some(json!({ "login": "anna", "password": "правильный пароль" })),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = server
        .call(Method::GET, "/api/oidc/start", None, None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    for (method, uri, body) in [
        (Method::POST, "/api/password".to_owned(), Some(json!({}))),
        (Method::GET, "/api/sessions".to_owned(), None),
        (Method::POST, "/api/logout-all".to_owned(), None),
        (
            Method::POST,
            "/api/vaults".to_owned(),
            Some(json!({ "name": "Ещё" })),
        ),
        (
            Method::PATCH,
            format!("/api/vaults/{}", encode("Демо")),
            Some(json!({ "name": "Другое" })),
        ),
        (
            Method::DELETE,
            format!("/api/vaults/{}", encode("Демо")),
            None,
        ),
    ] {
        let (status, _, _) = server.call(method, &uri, Some(&guest), body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
    }
    // Удалённый репозиторий закрыт ядром: возможность `remote = false`.
    server
        .invoke(&guest, "sync_open", json!({ "vaultId": "" }))
        .await;
    let (_, caps) = server.invoke(&guest, "sync_capabilities", json!({})).await;
    assert_eq!(caps["remote"], false);
    let (status, body) = server
        .invoke(
            &guest,
            "sync_connect_remote",
            json!({ "url": "https://github.com/me/notes.git", "credentials": {} }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["kind"], "RemoteUnavailable");
}

#[tokio::test]
async fn writes_past_the_quota_are_refused_and_space_can_be_freed() {
    let server = Server::new(
        true,
        DemoConfig {
            quota_mb: 1,
            ..DemoConfig::default()
        },
    )
    .await;
    let guest = server.guest().await;
    let big = "x".repeat(700 * 1024);
    let (status, _) = server
        .invoke(&guest, "create", json!({ "path": "Большая.typ" }))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = server
        .invoke(
            &guest,
            "write",
            json!({ "path": "Большая.typ", "content": big }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = server
        .invoke(
            &guest,
            "write",
            json!({ "path": "Ещё.typ", "content": big }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["kind"], "DemoQuota");
    assert_eq!(body["quota_mb"], 1);
    // Чтение и удаление работают и сверх квоты; после удаления запись снова можно.
    let (status, _) = server
        .invoke(&guest, "read", json!({ "path": "Начните здесь.typ" }))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = server
        .invoke(&guest, "delete", json!({ "path": "Большая.typ" }))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = server
        .invoke(&guest, "create", json!({ "path": "Маленькая.typ" }))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, usage, _) = server
        .call(Method::GET, "/api/demo/usage", Some(&guest), None)
        .await;
    assert_eq!(usage["quota"], 1024 * 1024);
    assert!(usage["used"].as_u64().unwrap() < 1024 * 1024);
}

#[tokio::test]
async fn one_address_starts_a_limited_number_of_guests() {
    let server = Server::new(
        true,
        DemoConfig {
            per_ip_per_hour: 2,
            ..DemoConfig::default()
        },
    )
    .await;
    assert_eq!(server.start().await.0, StatusCode::OK);
    assert_eq!(server.start().await.0, StatusCode::OK);
    let (status, body, guest) = server.start().await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["kind"], "TooManyDemos");
    assert!(body["retry_after_s"].as_u64().unwrap() > 3000);
    assert!(guest.is_none());
}

#[tokio::test]
async fn an_expired_guest_is_refused_and_removed_without_traces() {
    let server = Server::demo().await;
    let guest = server.guest().await;
    let id = server.account_id(&guest).await;
    server
        .invoke(&guest, "sync_open", json!({ "vaultId": "" }))
        .await;
    let config = &server.state.config;
    let dirs = [config.users_dir().join(&id), config.sync_dir().join(&id)];
    for dir in &dirs {
        assert!(dir.exists(), "{}", dir.display());
    }

    server
        .state
        .store
        .set_guest_expiry(&id, typsidian_server::store::now() - 1)
        .await
        .unwrap();
    let (status, body) = server.invoke(&guest, "list", json!({})).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["kind"], "DemoExpired");

    typsidian_server::demo::sweep(&server.state).await;
    for dir in &dirs {
        assert!(!dir.exists(), "{}", dir.display());
    }
    assert!(server
        .state
        .store
        .account_by_id(&id)
        .await
        .unwrap()
        .is_none());
    let (status, _, _) = server
        .call(Method::GET, "/api/me", Some(&guest), None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_ordinary_server_has_no_demo() {
    let server = Server::new(false, DemoConfig::default()).await;
    let (status, _, _) = server.start().await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, info, _) = server.call(Method::GET, "/api/info", None, None).await;
    assert!(info.get("demo").is_none());
    assert_eq!(info["password"], true);
}

#[tokio::test]
async fn a_start_without_json_is_refused() {
    let server = Server::demo().await;
    let (status, _, cookie) = server
        .call(Method::POST, "/api/demo/start", None, None)
        .await;
    assert_ne!(status, StatusCode::OK);
    assert!(cookie.is_none());
}

/// Удалённое гостем не копится в корзине вне квоты (ревью безопасности, К1).
#[tokio::test]
async fn deleting_frees_space_for_good() {
    let server = Server::demo().await;
    let guest = server.guest().await;
    let id = server.account_id(&guest).await;
    server
        .invoke(&guest, "create", json!({ "path": "Черновик.typ" }))
        .await;
    server
        .invoke(
            &guest,
            "write",
            json!({ "path": "Черновик.typ", "content": "x".repeat(300 * 1024) }),
        )
        .await;
    let (status, _) = server
        .invoke(&guest, "delete", json!({ "path": "Черновик.typ" }))
        .await;
    assert_eq!(status, StatusCode::OK);
    let trash = server.state.config.trash_dir().join("notes").join(&id);
    assert!(!trash.exists(), "{}", trash.display());
}

#[tokio::test]
async fn the_server_takes_a_limited_number_of_live_guests() {
    let server = Server::new(
        true,
        DemoConfig {
            max_guests: 2,
            per_ip_per_hour: 100,
            ..DemoConfig::default()
        },
    )
    .await;
    assert_eq!(server.start().await.0, StatusCode::OK);
    assert_eq!(server.start().await.0, StatusCode::OK);
    let (status, body, _) = server.start().await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["kind"], "DemoFull");
}

/// На демо-сервере сессия обычного аккаунта той же папки данных не работает.
#[tokio::test]
async fn ordinary_sessions_do_not_work_on_a_demo_server() {
    let server = Server::demo().await;
    let anna = server
        .state
        .store
        .add_account("anna", "Анна", None)
        .await
        .unwrap();
    let session = server
        .state
        .store
        .create_session(&anna.id, "тест", "127.0.0.1")
        .await
        .unwrap();
    let guest = Guest {
        cookie: format!("__Host-typsidian={}", session.token),
        csrf: session.session.csrf.clone(),
        login: "anna".into(),
    };
    let (status, _, _) = server
        .call(Method::GET, "/api/me", Some(&guest), None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
