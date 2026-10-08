//! Клиент состояния Syncthing против настоящего HTTP на loopback (фича 011,
//! T053, FR-021…FR-023). Только native: в браузере GUI Syncthing не отвечает
//! на обращения со страницы, и запроса там нет вовсе.
//!
//! Сервер — несколько строк поверх `std::net`: он отвечает ровно теми
//! документами, что отдаёт REST API Syncthing, и считает обращения — чтобы
//! проверить не только разбор, но и то, что ключ не уходит куда не надо.
#![cfg(not(target_arch = "wasm32"))]

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use typsidian_core::sync::syncthing::{self, FolderState, LinkState, SyncthingEndpoint};

const KEY: &str = "ключ-не-для-сети";
const VAULT: &str = "/home/user/notes";

fn body_for(path: &str) -> Option<String> {
    let body = match path.split('?').next().unwrap_or(path) {
        "/rest/system/status" => r#"{"myID":"MYSELF1-AAAAAAA"}"#.to_string(),
        "/rest/config/folders" => format!(
            r#"[{{"id":"other","path":"/srv/else","devices":[]}},
                {{"id":"notes","label":"Заметки","path":"{VAULT}/","paused":false,
                  "devices":[{{"deviceID":"MYSELF1-AAAAAAA"}},{{"deviceID":"PHONEAB-CCCCCCC"}}]}}]"#
        ),
        "/rest/db/status" => {
            r#"{"state":"syncing","globalBytes":200,"needBytes":50,"error":""}"#.to_string()
        }
        "/rest/config/devices" => {
            r#"[{"deviceID":"PHONEAB-CCCCCCC","name":"телефон"}]"#.to_string()
        }
        "/rest/system/connections" => {
            r#"{"connections":{"PHONEAB-CCCCCCC":{"connected":true}}}"#.to_string()
        }
        _ => return None,
    };
    Some(body)
}

fn answer(mut stream: TcpStream, keyed: &AtomicUsize) {
    let mut reader = BufReader::new(stream.try_clone().expect("клон потока"));
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let mut key = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("x-api-key") {
                key = Some(value.trim().to_string());
            }
        }
    }
    if key.is_some() {
        keyed.fetch_add(1, Ordering::SeqCst);
    }
    let (status, body) = match (key.as_deref(), body_for(path)) {
        (Some(KEY), Some(body)) => ("200 OK", body),
        (Some(KEY), None) => ("404 Not Found", String::new()),
        _ => ("403 Forbidden", String::new()),
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

/// Поднимает «Syncthing» на свободном порту loopback. Возвращает адрес и
/// счётчик обращений, пришедших с ключом API.
fn serve() -> (SyncthingEndpoint, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("порт loopback");
    let address = listener.local_addr().expect("адрес").to_string();
    let keyed = Arc::new(AtomicUsize::new(0));
    let counter = keyed.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            answer(stream, &counter);
        }
    });
    let endpoint = SyncthingEndpoint {
        address,
        tls: false,
        cert_fingerprint: None,
    };
    (endpoint, keyed)
}

#[tokio::test]
async fn the_folder_state_is_read_from_a_running_syncthing() {
    let (endpoint, _) = serve();
    let folder = syncthing::query(&endpoint, KEY, VAULT, true).await;

    assert_eq!(folder.link, LinkState::Connected);
    assert!(folder.detected);
    assert_eq!(folder.folder_id.as_deref(), Some("notes"));
    assert_eq!(folder.label.as_deref(), Some("Заметки"));
    assert_eq!(folder.state, Some(FolderState::Syncing));
    assert_eq!(folder.progress, Some(75));
    // Это устройство в список не входит; остальные — с именем и связью.
    assert_eq!(folder.devices.len(), 1);
    assert_eq!(folder.devices[0].short_id, "PHONEAB");
    assert_eq!(folder.devices[0].name, "телефон");
    assert!(folder.devices[0].connected);
    assert_eq!(folder.device_name("PHONEAB"), Some("телефон"));
}

#[tokio::test]
async fn a_wrong_key_is_reported_as_such() {
    let (endpoint, _) = serve();
    let folder = syncthing::query(&endpoint, "не тот ключ", VAULT, true).await;
    assert_eq!(folder.link, LinkState::Unauthorized);
    assert_eq!(folder.state, None);
}

#[tokio::test]
async fn a_folder_syncthing_does_not_know_is_not_an_error() {
    let (endpoint, _) = serve();
    let folder = syncthing::query(&endpoint, KEY, "/home/user/other", false).await;
    assert_eq!(folder.link, LinkState::FolderNotShared);
    assert!(!folder.detected);
}

#[tokio::test]
async fn a_stopped_syncthing_is_unreachable_and_answers_quickly() {
    // Порт занят и сразу отпущен: на нём никто не слушает.
    let address = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("порт loopback");
        listener.local_addr().expect("адрес").to_string()
    };
    let endpoint = SyncthingEndpoint {
        address,
        tls: false,
        cert_fingerprint: None,
    };
    let started = std::time::Instant::now();
    let folder = syncthing::query(&endpoint, KEY, VAULT, true).await;
    assert_eq!(folder.link, LinkState::Unreachable);
    // Принцип VII: локальный Syncthing либо отвечает сразу, либо не запущен.
    assert!(started.elapsed() < std::time::Duration::from_secs(6));
}

#[tokio::test]
async fn the_key_is_never_sent_off_this_device() {
    // Адрес не этого устройства: запроса нет вовсе, а не «запрос с отказом».
    let endpoint = SyncthingEndpoint {
        address: "192.0.2.1:8384".to_string(),
        tls: false,
        cert_fingerprint: None,
    };
    let started = std::time::Instant::now();
    let folder = syncthing::query(&endpoint, KEY, VAULT, true).await;
    assert_eq!(folder.link, LinkState::Unreachable);
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
}

#[tokio::test]
async fn the_key_waits_for_a_pinned_certificate() {
    // Адрес с TLS, сертификат которого ещё не закреплён: ключ не отправляется,
    // пока оболочка не закрепит отпечаток первым обращением (research §8).
    let (plain, keyed) = serve();
    let endpoint = SyncthingEndpoint {
        address: plain.address,
        tls: true,
        cert_fingerprint: None,
    };
    let folder = syncthing::query(&endpoint, KEY, VAULT, true).await;
    assert_eq!(folder.link, LinkState::Unreachable);
    assert_eq!(keyed.load(Ordering::SeqCst), 0);

    // И с закреплённым отпечатком — тоже нет, если по адресу отвечает не тот,
    // кто его предъявлял (здесь — вообще не TLS).
    let pinned = SyncthingEndpoint {
        cert_fingerprint: Some("0".repeat(64)),
        ..endpoint
    };
    let folder = syncthing::query(&pinned, KEY, VAULT, true).await;
    assert_ne!(folder.link, LinkState::Connected);
    assert_eq!(keyed.load(Ordering::SeqCst), 0);
    assert_eq!(syncthing::pin(&pinned).await, None);
}
