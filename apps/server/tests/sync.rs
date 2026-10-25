//! Синхронизация в серверном режиме (фича 038): история через invoke, отказы
//! адресов, изоляция секретов на двух аккаунтах (FR-002, FR-004, FR-005, SC-002).

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

/// Журнал сервера в память: проверка, что токен в него не попадает.
fn captured_log() -> std::sync::Arc<std::sync::Mutex<Vec<u8>>> {
    use std::sync::{Arc, Mutex, OnceLock};
    static LOG: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();
    LOG.get_or_init(|| {
        let log = Arc::new(Mutex::new(Vec::new()));
        let sink = log.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || Sink(sink.clone()))
            .finish();
        tracing::subscriber::set_global_default(subscriber).unwrap();
        log
    })
    .clone()
}

struct Sink(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

async fn create_vault(server: &Server, client: &Client, name: &str) {
    let (status, _, _) = server
        .call(
            Method::POST,
            "/api/vaults",
            Some(client),
            Some(json!({ "name": name })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

async fn ok(server: &Server, client: &Client, vault: &str, command: &str, args: Value) -> Value {
    let (status, body) = server.invoke(client, vault, command, args).await;
    assert_eq!(status, StatusCode::OK, "{command}: {body}");
    body
}

async fn open_sync(server: &Server, client: &Client, vault: &str) {
    // Чужой или выдуманный id клиента сервер не принимает — подставляет свой.
    ok(
        server,
        client,
        vault,
        "sync_open",
        json!({ "vaultId": "ffff" }),
    )
    .await;
    ok(
        server,
        client,
        vault,
        "sync_set_author",
        json!({ "author": { "name": "Анна", "email": "anna@example.org" } }),
    )
    .await;
}

fn files_under(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn contains(path: &std::path::Path, needle: &str) -> bool {
    std::fs::read(path)
        .map(|bytes| bytes.windows(needle.len()).any(|w| w == needle.as_bytes()))
        .unwrap_or(false)
}

#[tokio::test]
async fn history_works_on_the_server() {
    let server = Server::new().await;
    let anna = server.enter("anna").await;
    create_vault(&server, &anna, "Заметки").await;
    ok(
        &server,
        &anna,
        "Заметки",
        "create",
        json!({ "path": "Глава.typ" }),
    )
    .await;
    ok(
        &server,
        &anna,
        "Заметки",
        "write",
        json!({ "path": "Глава.typ", "content": "= Первая\n" }),
    )
    .await;
    open_sync(&server, &anna, "Заметки").await;
    let capabilities = ok(&server, &anna, "Заметки", "sync_capabilities", json!({})).await;
    assert_eq!(capabilities["ssh"], false);
    assert_eq!(capabilities["syncthing_status"], false);
    assert_eq!(capabilities["server"], true);

    let first = ok(
        &server,
        &anna,
        "Заметки",
        "sync_enable_history",
        json!({ "message": "Начало" }),
    )
    .await;
    let first_id = first["id"].as_str().unwrap().to_owned();
    let (_, stamp) = server
        .invoke(&anna, "Заметки", "read", json!({ "path": "Глава.typ" }))
        .await;
    assert_eq!(stamp, "= Первая\n");
    ok(
        &server,
        &anna,
        "Заметки",
        "write",
        json!({ "path": "Глава.typ", "content": "= Вторая\n" }),
    )
    .await;
    ok(
        &server,
        &anna,
        "Заметки",
        "sync_snapshot",
        json!({
            "paths": null,
            "message": "Правка",
            "autoPrefix": "Авто",
            "flushed": true,
            "confirmNotKeeper": false,
        }),
    )
    .await;
    let history = ok(
        &server,
        &anna,
        "Заметки",
        "sync_history",
        json!({ "path": "Глава.typ", "cursor": null, "limit": 20 }),
    )
    .await;
    assert_eq!(history["snapshots"].as_array().unwrap().len(), 2);
    let restored = ok(
        &server,
        &anna,
        "Заметки",
        "sync_restore",
        json!({ "path": "Глава.typ", "snapshot": first_id }),
    )
    .await;
    assert_eq!(restored["text"], "= Первая\n");
    let (_, text) = server
        .invoke(&anna, "Заметки", "read", json!({ "path": "Глава.typ" }))
        .await;
    assert_eq!(text, "= Первая\n");

    // В папке vault — обычный `.git`.
    let account = server
        .state
        .store
        .account_by_login("anna")
        .await
        .unwrap()
        .unwrap();
    let vault_dir = server
        .state
        .config
        .users_dir()
        .join(&account.id)
        .join("Заметки");
    assert!(vault_dir.join(".git").is_dir());

    // Переименование сохраняет id, удаление — убирает зеркало и id.
    let id = server
        .state
        .store
        .vault_sync_id(&account.id, "Заметки")
        .await
        .unwrap();
    let (status, _, _) = server
        .call(
            Method::PATCH,
            &format!("/api/vaults/{}", encode("Заметки")),
            Some(&anna),
            Some(json!({ "name": "Архив" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        server
            .state
            .store
            .vault_sync_id(&account.id, "Архив")
            .await
            .unwrap(),
        id
    );
}

#[tokio::test]
async fn addresses_of_the_server_itself_are_refused() {
    let server = Server::new().await;
    let anna = server.enter("anna").await;
    create_vault(&server, &anna, "Заметки").await;
    open_sync(&server, &anna, "Заметки").await;
    ok(
        &server,
        &anna,
        "Заметки",
        "sync_enable_history",
        json!({ "message": "Начало" }),
    )
    .await;
    for url in [
        "git@github.com:me/notes.git",
        "ssh://git@github.com/me/notes.git",
        "file:///etc",
        "/srv/typsidian/users",
        "http://localhost:8787/x.git",
        "https://localhost/x.git",
        "https://10.0.0.1/x.git",
        "https://169.254.169.254/latest/meta-data",
        "https://[::1]/x.git",
    ] {
        let (status, body) = server
            .invoke(
                &anna,
                "Заметки",
                "sync_connect_remote",
                json!({ "url": url, "credentials": { "user": null, "token": null } }),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{url}");
        assert_eq!(body["kind"], "UnsupportedScheme", "{url}: {body}");
    }
    // Команд Syncthing и клонирования на сервере нет.
    for command in [
        "sync_syncthing_discover",
        "sync_clone_vault",
        "device_settings",
    ] {
        let (status, _) = server.invoke(&anna, "Заметки", command, json!({})).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{command}");
    }
}

#[tokio::test]
async fn a_token_stays_with_its_account() {
    let log = captured_log();
    let server = Server::new().await;
    let anna = server.enter("anna").await;
    let bob = server.enter("bob").await;
    const TOKEN: &str = "ghp_secretTOKEN0123456789";
    const IN_URL: &str = "ghp_inURLsecret987";
    let mut replies = String::new();
    for (client, name) in [(&anna, "Заметки"), (&bob, "Заметки")] {
        create_vault(&server, client, name).await;
        open_sync(&server, client, name).await;
        ok(
            &server,
            client,
            name,
            "sync_enable_history",
            json!({ "message": "Начало" }),
        )
        .await;
    }
    let (_, reply) = server
        .invoke(
            &anna,
            "Заметки",
            "sync_connect_remote",
            json!({
                "url": "https://example.invalid/me/notes.git",
                "credentials": { "user": "anna", "token": TOKEN },
            }),
        )
        .await;
    replies.push_str(&reply.to_string());
    let (_, reply) = server
        .invoke(
            &anna,
            "Заметки",
            "sync_connect_remote",
            json!({
                "url": format!("https://anna:{IN_URL}@example.invalid/me/notes.git"),
                "credentials": { "user": null, "token": null },
            }),
        )
        .await;
    replies.push_str(&reply.to_string());
    for (client, command) in [
        (&anna, "sync_overview"),
        (&anna, "sync_push"),
        (&bob, "sync_overview"),
        (&bob, "sync_push"),
        (&bob, "sync_pull"),
    ] {
        let (_, reply) = server
            .invoke(
                client,
                "Заметки",
                command,
                json!({ "confirmNotKeeper": false }),
            )
            .await;
        replies.push_str(&reply.to_string());
    }
    // У Бориса удалённого нет: секреты Анны его vault не достались.
    let (_, overview) = server
        .invoke(&bob, "Заметки", "sync_overview", json!({}))
        .await;
    assert!(overview["remote"].is_null(), "{overview}");

    assert!(!replies.contains(IN_URL), "{replies}");
    let log = String::from_utf8_lossy(&log.lock().unwrap()).into_owned();
    for secret in [TOKEN, IN_URL] {
        assert!(!replies.contains(secret), "ответы: {replies}");
        assert!(!log.contains(secret), "журнал");
    }

    let data = server.state.config.data.clone();
    let mut files = Vec::new();
    files_under(&server.state.config.users_dir(), &mut files);
    for file in &files {
        assert!(!contains(file, IN_URL), "{}", file.display());
        assert!(!contains(file, TOKEN), "{}", file.display());
    }
    // Токен — только в файле секретов Анны, с правами 0600.
    let anna_id = server
        .state
        .store
        .account_by_login("anna")
        .await
        .unwrap()
        .unwrap()
        .id;
    let bob_id = server
        .state
        .store
        .account_by_login("bob")
        .await
        .unwrap()
        .unwrap()
        .id;
    let anna_secrets = data.join("sync").join(&anna_id).join("secrets.json");
    assert!(contains(&anna_secrets, IN_URL));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&anna_secrets)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let mut bob_files = Vec::new();
    files_under(&data.join("sync").join(&bob_id), &mut bob_files);
    for file in &bob_files {
        assert!(
            !contains(file, TOKEN) && !contains(file, IN_URL),
            "{}",
            file.display()
        );
    }

    // Удаление vault удаляет его секреты.
    let (status, _, _) = server
        .call(
            Method::DELETE,
            &format!("/api/vaults/{}", encode("Заметки")),
            Some(&anna),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!contains(&anna_secrets, IN_URL));
    assert!(!contains(&anna_secrets, TOKEN));
}

/// Каталог истории недоступен командам файлов: `.git/config` с `insteadOf`
/// переписал бы проверенный адрес обмена, файл `.git` со ссылкой `gitdir:`
/// открыл бы чужой репозиторий (038, ревью безопасности).
#[tokio::test]
async fn the_history_folder_is_not_reachable_through_file_commands() {
    let server = Server::new().await;
    let anna = server.enter("anna").await;
    create_vault(&server, &anna, "Заметки").await;
    // До включения истории — файл `.git` со ссылкой на чужой репозиторий.
    for path in [".git", ".GIT.", "git~1"] {
        let (status, _) = server
            .invoke(&anna, "Заметки", "create", json!({ "path": path }))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        let (status, _) = server
            .invoke(
                &anna,
                "Заметки",
                "write",
                json!({ "path": path, "content": "gitdir: ../../other/.git\n" }),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
    }
    open_sync(&server, &anna, "Заметки").await;
    ok(
        &server,
        &anna,
        "Заметки",
        "sync_enable_history",
        json!({ "message": "Начало" }),
    )
    .await;
    for path in [".git/config", "./.git/config", ".git/hooks/post-checkout"] {
        let (status, _) = server
            .invoke(
                &anna,
                "Заметки",
                "write",
                json!({
                    "path": path,
                    "content": "[url \"http://127.0.0.1:9000/\"]\n\tinsteadOf = https://example.com/\n",
                }),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        let (status, _) = server
            .invoke(&anna, "Заметки", "read", json!({ "path": path }))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
    }
    let account = server
        .state
        .store
        .account_by_login("anna")
        .await
        .unwrap()
        .unwrap();
    let config = server
        .state
        .config
        .users_dir()
        .join(&account.id)
        .join("Заметки/.git/config");
    assert!(!std::fs::read_to_string(config)
        .unwrap()
        .contains("insteadOf"));
}

/// Ошибки не называют каталог данных сервера.
#[tokio::test]
async fn refusals_do_not_name_server_paths() {
    let server = Server::new().await;
    let anna = server.enter("anna").await;
    create_vault(&server, &anna, "Заметки").await;
    open_sync(&server, &anna, "Заметки").await;
    // Истории нет — отказы ядра и libgit2.
    let data = server.state.config.data.to_string_lossy().into_owned();
    for command in ["sync_history", "sync_changes", "sync_push"] {
        let (_, body) = server
            .invoke(
                &anna,
                "Заметки",
                command,
                json!({ "path": null, "cursor": null, "limit": 5, "confirmNotKeeper": false }),
            )
            .await;
        assert!(!body.to_string().contains(&data), "{command}: {body}");
    }
}
