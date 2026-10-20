//! Настоящий движок git (libgit2) под native (фича 011, T017, T044).
//!
//! Политику проверяет общая таблица `sync_cases` на двойнике движка; здесь —
//! то, что двойник подтвердить не может: снимки читаются стандартным git
//! (SC-008), обмен идёт по настоящему протоколу, а «нет сети» завершается
//! за ограниченное время (SC-004).
#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;
use std::time::{Duration, Instant};

use typsidian_core::sync::engine::native::NativeGitEngine;
use typsidian_core::sync::engine::CancelToken;
use typsidian_core::sync::secrets::MemorySecretStore;
use typsidian_core::sync::session::{self, Credentials};
use typsidian_core::sync::{
    Author, DeviceIdentity, DeviceVaultSync, HistoryState, ShellCapabilities, SyncContext,
    SyncError, SyncSession,
};
use typsidian_core::vault::backend::VaultBackend;
use typsidian_core::vault::native::NativeBackend;

fn ctx(now: i64, id: char) -> SyncContext {
    SyncContext {
        device: Some(DeviceIdentity {
            id: id.to_string().repeat(32),
            name: "тест".into(),
        }),
        device_sync: DeviceVaultSync {
            author: Some(Author {
                name: "Тест".into(),
                email: "test@example.com".into(),
            }),
            ..DeviceVaultSync::default()
        },
        now,
        capabilities: ShellCapabilities::current(false),
    }
}

async fn open(dir: &Path) -> (NativeBackend, SyncSession<NativeGitEngine>) {
    let backend = NativeBackend::open(dir).await.expect("vault открывается");
    let session = SyncSession::new(NativeGitEngine::open(dir), None, "vault-1");
    (backend, session)
}

#[tokio::test]
async fn snapshots_are_plain_git_commits() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.typ"), "= A\n").unwrap();
    std::fs::write(dir.path().join("b.typ"), "= B\n").unwrap();
    let (mut backend, mut session) = open(dir.path()).await;
    let first = session
        .enable_history(&mut backend, &ctx(1_700_000_000_000, 'a'), "Первый")
        .await
        .unwrap();

    let repo = git2::Repository::open(dir.path()).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.id().to_string(), first.id);
    assert_eq!(head.message().ok(), Some("Первый"));
    let tree = head.tree().unwrap();
    assert!(tree.get_path(Path::new("a.typ")).is_ok());
    // Кэш индекса исключён правилами — его нет ни в снимке, ни в изменениях.
    assert!(tree.get_path(Path::new(".typsidian/cache.sqlite")).is_err());
    assert!(session.changes(&backend).await.unwrap().is_empty());

    std::fs::write(dir.path().join("a.typ"), "= A2\n").unwrap();
    std::fs::write(dir.path().join("b.typ"), "= B2\n").unwrap();
    session
        .snapshot(
            &backend,
            &ctx(1_700_000_001_000, 'a'),
            Some(vec!["a.typ".into()]),
            "только A",
            "",
            true,
            false,
        )
        .await
        .unwrap();
    let statuses = repo.statuses(None).unwrap();
    let dirty: Vec<_> = statuses
        .iter()
        .filter(|entry| {
            !matches!(
                entry.status(),
                git2::Status::CURRENT | git2::Status::IGNORED
            )
        })
        .filter_map(|entry| entry.path().ok().map(str::to_string))
        .collect();
    assert_eq!(dirty, ["b.typ"]);
}

#[tokio::test]
async fn exchange_through_a_bare_remote_with_merge() {
    let root = tempfile::tempdir().unwrap();
    let remote = root.path().join("remote.git");
    git2::Repository::init_bare(&remote).unwrap();
    let url = format!("file://{}", remote.display());
    let secrets = MemorySecretStore::new();

    let a_dir = root.path().join("a");
    std::fs::create_dir(&a_dir).unwrap();
    std::fs::write(
        a_dir.join("n.typ"),
        "= Заметка\n\nПервый.\n\nВторой.\n\nТретий.\n",
    )
    .unwrap();
    let (mut a, mut a_session) = open(&a_dir).await;
    a_session
        .enable_history(&mut a, &ctx(1_700_000_000_000, 'a'), "Первый")
        .await
        .unwrap();
    a_session
        .connect_remote(
            &mut a,
            &secrets,
            &ctx(1_700_000_000_500, 'a'),
            &url,
            Credentials::default(),
        )
        .await
        .unwrap();
    a_session
        .snapshot(
            &a,
            &ctx(1_700_000_001_000, 'a'),
            None,
            "адрес",
            "",
            true,
            false,
        )
        .await
        .unwrap();
    a_session
        .push(&a, &secrets, &ctx(1_700_000_001_500, 'a'), false)
        .await
        .unwrap();

    let b_dir = root.path().join("b");
    std::fs::create_dir(&b_dir).unwrap();
    session::clone_vault(
        &NativeGitEngine::open(&b_dir),
        &secrets,
        "vault-2",
        &ShellCapabilities::current(false),
        &url,
        Credentials::default(),
        None,
        &CancelToken::new(),
    )
    .await
    .unwrap();
    assert!(b_dir.join("n.typ").is_file());
    let (mut b, mut b_session) = open(&b_dir).await;

    std::fs::write(
        a_dir.join("n.typ"),
        "= Заметка\n\nПервый, A.\n\nВторой.\n\nТретий.\n",
    )
    .unwrap();
    a_session
        .snapshot(&a, &ctx(1_700_000_002_000, 'a'), None, "A", "", true, false)
        .await
        .unwrap();
    a_session
        .push(&a, &secrets, &ctx(1_700_000_002_500, 'a'), false)
        .await
        .unwrap();
    std::fs::write(
        b_dir.join("n.typ"),
        "= Заметка\n\nПервый.\n\nВторой.\n\nТретий, B.\n",
    )
    .unwrap();
    b_session
        .snapshot(&b, &ctx(1_700_000_003_000, 'b'), None, "B", "", true, false)
        .await
        .unwrap();

    let outcome = b_session
        .pull(&mut b, &secrets, &ctx(1_700_000_004_000, 'b'), false)
        .await
        .unwrap();
    assert_eq!(outcome.conflicts, 0);
    let merged = std::fs::read_to_string(b_dir.join("n.typ")).unwrap();
    assert!(merged.contains("Первый, A.") && merged.contains("Третий, B."));
    let repo = git2::Repository::open(&b_dir).unwrap();
    assert_eq!(
        repo.head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .parent_count(),
        2
    );
    assert!(b_session.changes(&b).await.unwrap().is_empty());
    b_session
        .push(&b, &secrets, &ctx(1_700_000_005_000, 'b'), false)
        .await
        .unwrap();

    // Не-хранитель читает историю из зеркала.
    let c_dir = root.path().join("c");
    std::fs::create_dir(&c_dir).unwrap();
    std::fs::create_dir(c_dir.join(".typsidian")).unwrap();
    std::fs::copy(
        b_dir.join(".typsidian/vault.json"),
        c_dir.join(".typsidian/vault.json"),
    )
    .unwrap();
    let mut c_backend = NativeBackend::open(&c_dir).await.unwrap();
    let mut settings = typsidian_core::sync::settings::read(&c_backend).await;
    settings.keeper = Some(typsidian_core::sync::Keeper {
        device_id: "a".repeat(32),
        device_name: "A".into(),
        since: 1,
    });
    typsidian_core::sync::settings::write(&mut c_backend, &settings)
        .await
        .unwrap();
    let mut c_session = SyncSession::new(
        NativeGitEngine::open(&c_dir),
        Some(NativeGitEngine::mirror(root.path().join("mirror.git"))),
        "vault-3",
    );
    let c_ctx = ctx(1_700_000_006_000, 'c');
    // Адрес пришёл с настройками vault; обмен начинается после того, как его
    // подтвердили на этом устройстве.
    c_session
        .connect_remote(
            &mut c_backend,
            &secrets,
            &c_ctx,
            &url,
            Credentials::default(),
        )
        .await
        .unwrap();
    c_session
        .refresh_mirror(&c_backend, &secrets, &c_ctx)
        .await
        .unwrap();
    let overview = c_session.overview(&c_backend, &c_ctx).await;
    assert!(matches!(overview.history, HistoryState::Mirror { .. }));
    let history = c_session
        .history(&c_backend, Some("n.typ"), None, 10)
        .await
        .unwrap();
    assert!(history.snapshots.len() >= 3, "{history:#?}");
}

#[tokio::test]
async fn an_unreachable_server_fails_fast_and_keeps_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.typ"), "= A\n").unwrap();
    let (mut backend, mut session) = open(dir.path()).await;
    session
        .enable_history(&mut backend, &ctx(1_700_000_000_000, 'a'), "Первый")
        .await
        .unwrap();
    let secrets = MemorySecretStore::new();
    let started = Instant::now();
    // Порт 9 (discard) на loopback закрыт — соединение отклоняется сразу.
    let error = session
        .connect_remote(
            &mut backend,
            &secrets,
            &ctx(1_700_000_001_000, 'a'),
            "http://127.0.0.1:9/notes.git",
            Credentials::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error, SyncError::Offline);
    assert!(started.elapsed() < Duration::from_secs(30));
    let error = session
        .push(&backend, &secrets, &ctx(1_700_000_002_000, 'a'), false)
        .await
        .unwrap_err();
    assert_eq!(error, SyncError::Offline);
    assert_eq!(
        session
            .history(&backend, None, None, 10)
            .await
            .unwrap()
            .snapshots
            .len(),
        1
    );
}

#[tokio::test]
async fn conflict_copies_stay_out_of_the_tree_but_are_listed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("n.typ"), "наше\n").unwrap();
    std::fs::write(
        dir.path()
            .join("n.sync-conflict-20261003-120000-ABCDEFG.typ"),
        "их\n",
    )
    .unwrap();
    let backend = NativeBackend::open(dir.path()).await.unwrap();
    std::fs::write(
        dir.path()
            .join(".typsidian/vault.sync-conflict-20261003-120000-ABCDEFG.json"),
        "{}",
    )
    .unwrap();
    let listed: Vec<_> = backend
        .list()
        .await
        .into_iter()
        .map(|note| note.path)
        .collect();
    assert_eq!(listed, [Path::new("n.typ").to_path_buf()]);
    let copies = backend.list_conflict_copies().await;
    assert_eq!(copies.len(), 2);
}

// ── Smart HTTP против настоящего сервера git (T044) ────────────────────────
//
// Транспорт `transport_http` — путь, которым ходит обмен с GitHub и GitLab, и
// двойник движка его не касается. Сервер здесь — `git http-backend` (эталонная
// реализация протокола) за несколькими строками HTTP поверх `std::net`; он же
// требует Basic-авторизацию, чтобы проверить и её.

mod smart_http {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    pub const TOKEN: &str = "s3cr3t-token";
    /// `x-access-token:s3cr3t-token` — имя, которое ядро подставляет к токену.
    const EXPECTED_AUTHORIZATION: &str = "Basic eC1hY2Nlc3MtdG9rZW46czNjcjN0LXRva2Vu";

    /// Путь к `git-http-backend`; `None` — git на этой машине не установлен.
    pub fn backend_binary() -> Option<PathBuf> {
        let output = Command::new("git").arg("--exec-path").output().ok()?;
        let dir = String::from_utf8(output.stdout).ok()?;
        let path = Path::new(dir.trim()).join("git-http-backend");
        path.is_file().then_some(path)
    }

    fn respond(stream: &mut TcpStream, status: &str, headers: &str, body: &[u8]) {
        let _ = write!(
            stream,
            "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(body);
    }

    fn handle(mut stream: TcpStream, backend: &Path, project_root: &Path) {
        let mut reader = BufReader::new(stream.try_clone().expect("клон потока"));
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
            return;
        }
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("GET").to_string();
        let target = parts.next().unwrap_or("/").to_string();
        let (path, query) = target.split_once('?').unwrap_or((&target, ""));

        let mut content_length = 0usize;
        let mut content_type = String::new();
        let mut authorization = None;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                break;
            }
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match name.to_ascii_lowercase().as_str() {
                "content-length" => content_length = value.parse().unwrap_or(0),
                "content-type" => content_type = value.to_string(),
                "authorization" => authorization = Some(value.to_string()),
                _ => {}
            }
        }
        let mut body = vec![0; content_length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }

        match authorization.as_deref() {
            Some(EXPECTED_AUTHORIZATION) => {}
            Some(_) => return respond(&mut stream, "403 Forbidden", "", b""),
            None => {
                return respond(
                    &mut stream,
                    "401 Unauthorized",
                    "WWW-Authenticate: Basic realm=\"test\"\r\n",
                    b"",
                )
            }
        }

        let mut child = Command::new(backend)
            .env("GIT_PROJECT_ROOT", project_root)
            .env("GIT_HTTP_EXPORT_ALL", "1")
            .env("REQUEST_METHOD", &method)
            .env("PATH_INFO", path)
            .env("QUERY_STRING", query)
            .env("CONTENT_TYPE", &content_type)
            .env("CONTENT_LENGTH", content_length.to_string())
            // Авторизованный пользователь — условие, при котором сервер git
            // принимает отправку.
            .env("REMOTE_USER", "tester")
            .env("REMOTE_ADDR", "127.0.0.1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("запуск git http-backend");
        let mut stdin = child.stdin.take().expect("stdin сервера git");
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(&body);
        });
        let mut output = Vec::new();
        child
            .stdout
            .take()
            .expect("stdout сервера git")
            .read_to_end(&mut output)
            .expect("ответ сервера git");
        let _ = writer.join();
        let _ = child.wait();

        // Ответ CGI: заголовки, пустая строка, тело.
        let split = output
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|at| (at, 4))
            .or_else(|| {
                output
                    .windows(2)
                    .position(|window| window == b"\n\n")
                    .map(|at| (at, 2))
            });
        let Some((at, gap)) = split else {
            return respond(&mut stream, "500 Internal Server Error", "", b"");
        };
        let head = String::from_utf8_lossy(&output[..at]).into_owned();
        let mut status = "200 OK".to_string();
        let mut headers = String::new();
        for line in head.lines() {
            match line.split_once(':') {
                Some((name, value)) if name.eq_ignore_ascii_case("status") => {
                    status = value.trim().to_string();
                }
                Some(_) => {
                    headers.push_str(line.trim_end());
                    headers.push_str("\r\n");
                }
                None => {}
            }
        }
        respond(&mut stream, &status, &headers, &output[at + gap..]);
    }

    /// Поднимает сервер git на свободном порту loopback; отдаёт его адрес.
    pub fn serve(backend: PathBuf, project_root: PathBuf) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("порт loopback");
        let address = listener.local_addr().expect("адрес").to_string();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let backend = backend.clone();
                let project_root = project_root.clone();
                std::thread::spawn(move || handle(stream, &backend, &project_root));
            }
        });
        format!("http://{address}")
    }
}

#[tokio::test]
async fn exchange_over_smart_http_with_a_token() {
    let Some(backend_binary) = smart_http::backend_binary() else {
        eprintln!("git http-backend не найден — обмен по smart HTTP не проверялся");
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let served = root.path().join("srv");
    std::fs::create_dir(&served).unwrap();
    git2::Repository::init_bare(served.join("notes.git")).unwrap();
    let url = format!(
        "{}/notes.git",
        smart_http::serve(backend_binary, served.clone())
    );
    let token = Credentials {
        user: None,
        token: Some(smart_http::TOKEN.to_string()),
    };

    let a_dir = root.path().join("a");
    std::fs::create_dir(&a_dir).unwrap();
    std::fs::write(a_dir.join("n.typ"), "= Заметка\n").unwrap();
    let (mut a, mut a_session) = open(&a_dir).await;
    a_session
        .enable_history(&mut a, &ctx(1_700_000_000_000, 'a'), "Первый")
        .await
        .unwrap();

    // Без токена сервер просит данные доступа, с неверным — отклоняет их; ни
    // то ни другое адрес не подключает.
    let secrets = MemorySecretStore::new();
    let error = a_session
        .connect_remote(
            &mut a,
            &secrets,
            &ctx(1_700_000_000_100, 'a'),
            &url,
            Credentials::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error, SyncError::AuthRequired);
    let error = a_session
        .connect_remote(
            &mut a,
            &secrets,
            &ctx(1_700_000_000_200, 'a'),
            &url,
            Credentials {
                user: None,
                token: Some("не тот".to_string()),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error, SyncError::AuthRejected);

    let secrets = MemorySecretStore::new();
    a_session
        .connect_remote(
            &mut a,
            &secrets,
            &ctx(1_700_000_000_500, 'a'),
            &url,
            token.clone(),
        )
        .await
        .unwrap();
    a_session
        .snapshot(
            &a,
            &ctx(1_700_000_001_000, 'a'),
            None,
            "адрес",
            "",
            true,
            false,
        )
        .await
        .unwrap();
    a_session
        .push(&a, &secrets, &ctx(1_700_000_001_500, 'a'), false)
        .await
        .unwrap();

    // Снимки дошли до сервера: их видит обычный git в голом репозитории.
    let server = git2::Repository::open_bare(served.join("notes.git")).unwrap();
    let pushed = server
        .find_reference("refs/heads/main")
        .unwrap()
        .peel_to_commit()
        .unwrap();
    assert_eq!(pushed.message().ok(), Some("адрес"));

    // Токен — только в хранилище секретов: ни в настройках vault, ни в
    // конфигурации истории его нет (FR-014, SC-008).
    for file in [".typsidian/vault.json", ".git/config"] {
        let text = std::fs::read_to_string(a_dir.join(file)).unwrap();
        assert!(!text.contains(smart_http::TOKEN), "{file} содержит токен");
    }

    // Второе устройство получает vault клонированием по тому же адресу.
    let b_dir = root.path().join("b");
    std::fs::create_dir(&b_dir).unwrap();
    let b_secrets = MemorySecretStore::new();
    session::clone_vault(
        &NativeGitEngine::open(&b_dir),
        &b_secrets,
        "vault-2",
        &ShellCapabilities::current(false),
        &url,
        token,
        None,
        &CancelToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(b_dir.join("n.typ")).unwrap(),
        "= Заметка\n"
    );

    // Встречный снимок с первого устройства доходит до второго получением.
    std::fs::write(a_dir.join("n.typ"), "= Заметка\n\nдописано\n").unwrap();
    a_session
        .snapshot(
            &a,
            &ctx(1_700_000_002_000, 'a'),
            None,
            "дописано",
            "",
            true,
            false,
        )
        .await
        .unwrap();
    a_session
        .push(&a, &secrets, &ctx(1_700_000_002_500, 'a'), false)
        .await
        .unwrap();
    // Сессия — под тем же идентификатором vault, под которым клонирование
    // сохранило данные доступа.
    let mut b = NativeBackend::open(&b_dir).await.unwrap();
    let mut b_session = SyncSession::new(NativeGitEngine::open(&b_dir), None, "vault-2");
    let outcome = b_session
        .pull(&mut b, &b_secrets, &ctx(1_700_000_003_000, 'b'), false)
        .await
        .unwrap();
    assert_eq!(outcome.conflicts, 0);
    assert_eq!(
        std::fs::read_to_string(b_dir.join("n.typ")).unwrap(),
        "= Заметка\n\nдописано\n"
    );
}

/// T068: путь из чужого репозитория проходит проверку строки, но на диске
/// ведёт за корень vault через символическую ссылку-каталог. Запись сквозь неё
/// — это запись в произвольный каталог пользователя.
#[cfg(unix)]
#[tokio::test]
async fn a_pull_never_writes_through_a_symlinked_folder() {
    let root = tempfile::tempdir().unwrap();
    let remote = root.path().join("remote.git");
    git2::Repository::init_bare(&remote).unwrap();
    let url = format!("file://{}", remote.display());
    let secrets = MemorySecretStore::new();

    let a_dir = root.path().join("a");
    std::fs::create_dir(&a_dir).unwrap();
    std::fs::write(a_dir.join("n.typ"), "= Заметка\n").unwrap();
    let (mut a, mut a_session) = open(&a_dir).await;
    a_session
        .enable_history(&mut a, &ctx(1_700_000_000_000, 'a'), "Первый")
        .await
        .unwrap();
    a_session
        .connect_remote(
            &mut a,
            &secrets,
            &ctx(1_700_000_000_500, 'a'),
            &url,
            Credentials::default(),
        )
        .await
        .unwrap();
    let snap = |message: &'static str, now: i64| (message, ctx(now, 'a'));
    let (message, at) = snap("адрес", 1_700_000_001_000);
    a_session
        .snapshot(&a, &at, None, message, "", true, false)
        .await
        .unwrap();
    a_session
        .push(&a, &secrets, &ctx(1_700_000_001_500, 'a'), false)
        .await
        .unwrap();

    let b_dir = root.path().join("b");
    std::fs::create_dir(&b_dir).unwrap();
    let b_secrets = MemorySecretStore::new();
    session::clone_vault(
        &NativeGitEngine::open(&b_dir),
        &b_secrets,
        "vault-2",
        &ShellCapabilities::current(false),
        &url,
        Credentials::default(),
        None,
        &CancelToken::new(),
    )
    .await
    .unwrap();

    // На втором устройстве в vault есть ссылка на каталог вне его.
    let outside = root.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, b_dir.join("h")).unwrap();

    // С сервера приходит файл «внутри» этой ссылки.
    std::fs::create_dir(a_dir.join("h")).unwrap();
    std::fs::write(a_dir.join("h/autostart.typ"), "чужое\n").unwrap();
    let (message, at) = snap("ловушка", 1_700_000_002_000);
    a_session
        .snapshot(&a, &at, None, message, "", true, false)
        .await
        .unwrap();
    a_session
        .push(&a, &secrets, &ctx(1_700_000_002_500, 'a'), false)
        .await
        .unwrap();

    let mut b = NativeBackend::open(&b_dir).await.unwrap();
    let mut b_session = SyncSession::new(NativeGitEngine::open(&b_dir), None, "vault-2");
    let error = b_session
        .pull(&mut b, &b_secrets, &ctx(1_700_000_003_000, 'b'), false)
        .await
        .unwrap_err();
    assert!(matches!(error, SyncError::Io { .. }), "{error:?}");
    assert!(
        std::fs::read_dir(&outside).unwrap().next().is_none(),
        "получение записало файл вне vault"
    );
}

#[tokio::test]
async fn a_silent_server_is_given_up_on_after_the_idle_limit() {
    // T072, FR-020, SC-004: сервер принял соединение и молчит. Обмен обязан
    // закончиться отказом по пределу простоя, а не висеть до общего предела
    // передачи; снимки при этом целы.
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("порт loopback");
    let address = listener.local_addr().unwrap();
    // Соединения принимаются и удерживаются открытыми без единого байта в ответ.
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming().flatten() {
            held.push(stream);
        }
    });

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.typ"), "= A\n").unwrap();
    let mut backend = NativeBackend::open(dir.path()).await.unwrap();
    let engine = NativeGitEngine::open(dir.path()).with_idle_limit(Duration::from_millis(600));
    let mut session = SyncSession::new(engine, None, "vault-1");
    session
        .enable_history(&mut backend, &ctx(1_700_000_000_000, 'a'), "Первый")
        .await
        .unwrap();

    let secrets = MemorySecretStore::new();
    let started = Instant::now();
    let error = session
        .connect_remote(
            &mut backend,
            &secrets,
            &ctx(1_700_000_001_000, 'a'),
            &format!("http://{address}/notes.git"),
            Credentials::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error, SyncError::Timeout);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "ждали {:?}",
        started.elapsed()
    );
    assert_eq!(
        session
            .history(&backend, None, None, 10)
            .await
            .unwrap()
            .snapshots
            .len(),
        1
    );
}

/// Vault внутри чужого репозитория (Edge Cases, T074): история видна как
/// чужая, а приложение в неё не пишет — ни снимка, ни своей истории поверх.
#[tokio::test]
async fn a_vault_inside_a_foreign_repository_is_shown_not_written() {
    let outer = tempfile::TempDir::new().unwrap();
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(outer.path())
        .status()
        .expect("системный git");
    assert!(status.success());
    let vault = outer.path().join("notes");
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(vault.join("a.typ"), "= A\n").unwrap();

    let mut backend = NativeBackend::open(&vault).await.unwrap();
    let mut session = SyncSession::new(NativeGitEngine::open(&vault), None, "vault-f");
    let context = ctx(1_000, 'f');
    let overview = session.overview(&backend, &context).await;
    assert!(
        matches!(overview.history, HistoryState::ForeignParent { .. }),
        "{:?}",
        overview.history
    );
    assert_eq!(
        session
            .snapshot(&backend, &context, None, "снимок", "Снимок", true, false)
            .await
            .unwrap_err(),
        SyncError::ForeignRepository
    );
    assert_eq!(
        session
            .enable_history(&mut backend, &context, "начало")
            .await
            .unwrap_err(),
        SyncError::ForeignRepository
    );
    // Во внешней истории не появилось ни одного снимка.
    let log = std::process::Command::new("git")
        .args(["rev-list", "--all", "--count"])
        .current_dir(outer.path())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&log.stdout).trim(), "0");
    assert!(!vault.join(".git").exists());
}
