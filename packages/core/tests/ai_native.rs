//! cargo test — только native (принцип II). Парный файл — `ai_wasm.rs`.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::ai_cases as cases;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use typsidian_core::ai::transport::{AiTransport, DefaultTransport, HttpMethod, HttpRequest};
use typsidian_core::ai::{protocol::check_status, AiFailure};

#[tokio::test]
async fn anthropic_gets_a_default_model() {
    cases::anthropic_gets_a_default_model().await;
}

#[tokio::test]
async fn missing_or_broken_settings_mean_disabled() {
    cases::missing_or_broken_settings_mean_disabled().await;
}

#[tokio::test]
async fn disabled_makes_no_requests() {
    cases::disabled_makes_no_requests().await;
}

#[tokio::test]
async fn journal_gets_request_before_sending_and_never_the_key() {
    cases::journal_gets_request_before_sending_and_never_the_key().await;
}

#[tokio::test]
async fn journal_failure_stops_the_request() {
    cases::journal_failure_stops_the_request().await;
}

#[tokio::test]
async fn unknown_price_is_not_zero() {
    cases::unknown_price_is_not_zero().await;
}

#[tokio::test]
async fn failure_is_recorded_and_carries_no_server_text() {
    cases::failure_is_recorded_and_carries_no_server_text().await;
}

#[tokio::test]
async fn statuses_map_to_failures() {
    cases::statuses_map_to_failures().await;
}

#[tokio::test]
async fn journal_keeps_last_entries_and_clears() {
    cases::journal_keeps_last_entries_and_clears().await;
}

#[tokio::test]
async fn journal_stays_out_of_sync_and_settings_travel() {
    cases::journal_stays_out_of_sync_and_settings_travel().await;
}

#[tokio::test]
async fn endpoints_are_guarded() {
    cases::endpoints_are_guarded().await;
}

#[tokio::test]
async fn cloud_key_never_goes_to_custom_endpoint() {
    cases::cloud_key_never_goes_to_custom_endpoint().await;
}

#[tokio::test]
async fn models_are_listed_sorted() {
    cases::models_are_listed_sorted().await;
}

#[tokio::test]
async fn access_stays_inside_the_vault() {
    cases::access_stays_inside_the_vault().await;
}

#[tokio::test]
async fn access_ignores_unicode_form() {
    cases::access_ignores_unicode_form().await;
}

#[tokio::test]
async fn duplicate_request_id_keeps_the_first_cancellable() {
    cases::duplicate_request_id_keeps_the_first_cancellable().await;
}

#[tokio::test]
async fn custom_endpoint_needs_approval_on_this_device() {
    cases::custom_endpoint_needs_approval_on_this_device().await;
}

#[tokio::test]
async fn denied_folder_follows_the_move() {
    cases::denied_folder_follows_the_move().await;
}

#[tokio::test]
async fn cancel_stops_waiting_at_once() {
    cases::cancel_stops_waiting_at_once().await;
}

fn request(url: String) -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Post,
        url,
        headers: vec![("x-api-key".to_owned(), "test-key".to_owned())],
        body: Some("{}".to_owned()),
    }
}

/// Отказ соединения: понятный отказ, а не зависание (принцип VII).
#[tokio::test]
async fn refused_connection_is_offline() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let failure = DefaultTransport
        .send(request(format!("http://127.0.0.1:{port}/v1")))
        .await
        .unwrap_err();
    assert_eq!(failure, AiFailure::Offline);
}

/// FR-009: перенаправление не выполняется, второго соединения нет.
#[tokio::test]
async fn redirect_is_not_followed() {
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target.local_addr().unwrap().port();
    let origin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin_port = origin.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut socket, _) = origin.accept().await.unwrap();
        let mut buffer = [0u8; 4096];
        let _ = socket.read(&mut buffer).await;
        let reply = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:{target_port}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        socket.write_all(reply.as_bytes()).await.unwrap();
    });
    let response = DefaultTransport
        .send(request(format!("http://127.0.0.1:{origin_port}/v1")))
        .await
        .unwrap();
    assert_eq!(response.status, 307);
    assert_eq!(check_status(response.status), Err(AiFailure::Redirected));
    let accepted =
        tokio::time::timeout(std::time::Duration::from_millis(200), target.accept()).await;
    assert!(accepted.is_err(), "запрос ушёл на адрес перенаправления");
}

/// Ссылка уводит чтение в запрещённую папку или служебное место — по строке пути
/// этого не видно, поэтому такой путь закрыт (ревью безопасности 050, M4).
#[cfg(unix)]
#[tokio::test]
async fn symlinks_are_closed_to_the_agent() {
    use std::os::unix::fs::symlink;
    use typsidian_core::ai::access::check_access_in;
    use typsidian_core::ai::settings::AiSettings;
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("Личное")).unwrap();
    std::fs::create_dir_all(root.join("Публичное")).unwrap();
    std::fs::create_dir_all(root.join(".typsidian")).unwrap();
    std::fs::write(root.join("Личное/секрет.typ"), "секрет").unwrap();
    std::fs::write(root.join(".typsidian/ai-journal.jsonl"), "").unwrap();
    std::fs::write(root.join("Публичное/а.typ"), "а").unwrap();
    symlink(root.join("Личное"), root.join("Публичное/папка")).unwrap();
    symlink(
        root.join(".typsidian/ai-journal.jsonl"),
        root.join("Публичное/журнал.typ"),
    )
    .unwrap();
    let backend = NativeBackend::open(root).await.unwrap();
    let settings = AiSettings {
        enabled: true,
        denied_folders: vec!["Личное".to_owned()],
        ..AiSettings::default()
    };
    assert!(check_access_in(&backend, "Публичное/а.typ", &settings).await);
    assert!(!check_access_in(&backend, "Публичное/папка/секрет.typ", &settings).await);
    assert!(!check_access_in(&backend, "Публичное/журнал.typ", &settings).await);
}
