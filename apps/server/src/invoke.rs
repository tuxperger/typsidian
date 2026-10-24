//! Команды ядра по HTTP, события vault по SSE и скачивание файлов vault
//! (research.md §4–§6).

use std::convert::Infallible;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use futures_util::stream;
use serde_json::Value;
use tokio::sync::broadcast::error::RecvError;

use crate::allowed::allowed;
use crate::auth::Authed;
use crate::error::{ApiError, ApiResult};
use crate::vaults::{vault_dir, OpenVault};
use crate::Shared;

/// Заголовок с id вкладки: свои правки вкладка эхом не получает (research.md §5).
pub const CLIENT_HEADER: &str = "x-typsidian-client";

/// Тело `attachments_batch_store` — байты, а не JSON.
const RAW_COMMAND: &str = "attachments_batch_store";
/// Пересчёт свойств отдаёт ход событием — вкладке, которая его начала.
const REINDEX_COMMAND: &str = "vault_reindex_properties";

async fn open(state: &Shared, authed: &Authed, name: &str) -> ApiResult<std::sync::Arc<OpenVault>> {
    crate::vaults::open_vault(state, &authed.account.id, name).await
}

fn client_of(headers: &HeaderMap) -> Option<String> {
    headers
        .get(CLIENT_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.len() <= 64)
        .map(str::to_owned)
}

/// JSON-аргументы команды не больше этого; байты вложений идут сырым телом.
const MAX_JSON_BODY: usize = 16 * 1024 * 1024;

/// Свои адреса шаблонов (`template_sources` настроек vault) — запросы сервера по
/// адресу, который назвал пользователь, в том числе во внутреннюю сеть (SSRF).
/// Если администратор не разрешил их в настройках, адреса из аргументов убираются,
/// и шаблоны ставятся только из реестра Typst.
fn strip_template_sources(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(sources) = map.get_mut("template_sources") {
                *sources = Value::Array(Vec::new());
            }
            for field in map.values_mut() {
                strip_template_sources(field);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip_template_sources),
        _ => {}
    }
}

/// `POST /api/vaults/{vault}/invoke/{command}`: тело — аргументы команды (JSON).
/// Успех — значение команды, отказ — `400` с отказом команды: ровно то, что
/// получил бы клиент Tauri.
pub async fn invoke(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
    UrlPath((name, command)): UrlPath<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Response> {
    if !allowed(&command) {
        tracing::warn!(login = %authed.account.login, command = %command, "отказ: команда вне белого списка");
        return Err(ApiError::Forbidden);
    }
    let open = open(&state, &authed, &name).await?;
    let reply = if command == REINDEX_COMMAND {
        let origin = client_of(&headers);
        let sender = open.events.clone();
        let result = typsidian_host::reindex_properties(
            &open.host.vault,
            &open.host.properties_reindex,
            |done, total| {
                let _ = sender.send(std::sync::Arc::new(serde_json::json!({
                    "progress": { "done": done, "total": total },
                    "origin": origin,
                })));
            },
        )
        .await;
        match result {
            Ok(value) => Ok(serde_json::to_value(value).unwrap_or(Value::Null)),
            Err(error) => Err(serde_json::to_value(error).unwrap_or(Value::Null)),
        }
    } else if command == RAW_COMMAND {
        let result = typsidian_host::store_attachment_body(&open.host.vault, &body).await;
        match result {
            Ok(value) => Ok(serde_json::to_value(value).unwrap_or(Value::Null)),
            Err(error) => Err(serde_json::to_value(error).unwrap_or(Value::Null)),
        }
    } else {
        if body.len() > MAX_JSON_BODY {
            return Err(ApiError::Invalid {
                field: "body",
                reason: "TooLarge",
            });
        }
        let mut arguments: Value = if body.is_empty() {
            Value::Object(Default::default())
        } else {
            serde_json::from_slice(&body).map_err(|_| ApiError::Invalid {
                field: "body",
                reason: "NotJson",
            })?
        };
        if !state.config.template_sources {
            strip_template_sources(&mut arguments);
        }
        let echo = crate::events::echo(&command, &arguments);
        let reply = typsidian_host::dispatch::dispatch(&open.host, &command, arguments)
            .await
            .ok_or(ApiError::Forbidden)?;
        if reply.is_ok() && !echo.is_empty() {
            crate::events::send_echo(&open, echo, client_of(&headers));
        }
        reply
    };
    open.touch();
    Ok(match reply {
        Ok(value) => Json(value).into_response(),
        Err(value) => (StatusCode::BAD_REQUEST, Json(value)).into_response(),
    })
}

/// Как часто поток событий перепроверяет сессию.
const SESSION_RECHECK: Duration = Duration::from_secs(30);

struct SessionCheck {
    state: Shared,
    token: String,
    tick: tokio::time::Interval,
}

impl SessionCheck {
    async fn alive(&self) -> bool {
        let idle = self.state.config.session_idle_secs();
        matches!(
            self.state.store.session_by_token(&self.token, idle).await,
            Ok(Some(_))
        )
    }
}

#[derive(serde::Deserialize)]
pub struct EventsQuery {
    client: Option<String>,
}

/// `GET /api/vaults/{vault}/events`: события бэкенда и правки других вкладок.
pub async fn events(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
    UrlPath(name): UrlPath<String>,
    Query(query): Query<EventsQuery>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let open = open(&state, &authed, &name).await?;
    let token = crate::auth::cookie_token(&headers).unwrap_or_default();
    // `EventSource` заголовков не шлёт: id вкладки — в адресе.
    let client = query.client.filter(|c| c.len() <= 64);
    let receiver = open.events.subscribe();
    tracing::debug!(login = %authed.account.login, "подписка на события vault");
    // Первое событие — «подключено»: клиент по нему перечитывает дерево после
    // обрыва (research.md §5).
    let hello =
        stream::once(async { Ok::<_, Infallible>(Event::default().event("ready").data("{}")) });
    // Сессию поток перепроверяет: выход, «выйти везде», смена пароля и блокировка
    // закрывают и открытые потоки, а не только следующие запросы.
    let session = SessionCheck {
        state: state.clone(),
        token,
        tick: tokio::time::interval(SESSION_RECHECK),
    };
    let updates = stream::unfold(
        (receiver, open, client, session),
        |(mut receiver, open, client, mut session)| async move {
            loop {
                let received = tokio::select! {
                    received = receiver.recv() => received,
                    _ = session.tick.tick() => {
                        if !session.alive().await {
                            return None;
                        }
                        continue;
                    }
                };
                match received {
                    Ok(message) => {
                        if let Some(progress) = message.get("progress") {
                            // Ход пересчёта — только вкладке, которая его начала.
                            let origin = message.get("origin").and_then(Value::as_str);
                            if origin.is_none() || origin != client.as_deref() {
                                continue;
                            }
                            let event = Event::default()
                                .event("progress")
                                .data(progress.to_string());
                            return Some((Ok(event), (receiver, open, client, session)));
                        }
                        if let (Some(origin), Some(client)) =
                            (message.get("origin").and_then(Value::as_str), &client)
                        {
                            if origin == client {
                                continue;
                            }
                        }
                        let data = message.get("events").cloned().unwrap_or(Value::Null);
                        open.touch();
                        let event = Event::default().event("vault").data(data.to_string());
                        return Some((Ok(event), (receiver, open, client, session)));
                    }
                    // Отстали: часть событий потеряна — клиент перечитает дерево.
                    Err(RecvError::Lagged(_)) => {
                        let event = Event::default().event("lagged").data("{}");
                        return Some((Ok(event), (receiver, open, client, session)));
                    }
                    Err(RecvError::Closed) => return None,
                }
            }
        },
    );
    use futures_util::StreamExt;
    let mut response = Sse::new(hello.chain(updates))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();
    // Прокси (nginx, `next dev`) не должны ни сжимать, ни копить поток: сжатие
    // буферизует его, и события приходят пачкой при закрытии.
    let headers = response.headers_mut();
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    Ok(response)
}

/// Путь внутри vault из адреса: только обычные компоненты, без `..` и корня.
fn relative(path: &str) -> Option<PathBuf> {
    let candidate = Path::new(path);
    let mut out = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => out.push(part),
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// `GET /api/vaults/{vault}/file/{*path}`: файл vault — например, экспорт.
pub async fn file(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
    UrlPath((name, path)): UrlPath<(String, String)>,
) -> ApiResult<Response> {
    let root = vault_dir(&state, &authed.account.id, &name)?;
    let relative = relative(&path).ok_or(ApiError::NotFound)?;
    // Ссылки внутри vault не выводят из него: сравниваются настоящие пути.
    let root = std::fs::canonicalize(&root)?;
    let full = std::fs::canonicalize(root.join(&relative)).map_err(|_| ApiError::NotFound)?;
    if !full.starts_with(&root) || !full.is_file() {
        tracing::warn!(login = %authed.account.login, "отказ: файл вне vault");
        return Err(ApiError::NotFound);
    }
    let bytes = tokio::fs::read(&full).await?;
    let file_name = full
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let encoded: String = file_name
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    let disposition = HeaderValue::from_str(&format!("attachment; filename*=UTF-8''{encoded}"))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    Ok((
        [
            (
                CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            ),
            (CONTENT_DISPOSITION, disposition),
        ],
        Body::from(bytes),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::relative;

    #[test]
    fn relative_paths_stay_inside() {
        assert!(relative("exports/a.pdf").is_some());
        assert!(relative("../x").is_none());
        assert!(relative("a/../../x").is_none());
        assert!(relative("/etc/passwd").is_none());
        assert!(relative("").is_none());
    }
}
