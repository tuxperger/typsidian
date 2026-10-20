//! Smart HTTP для libgit2 поверх `reqwest` (research §2).
//!
//! libgit2 собран без своего HTTPS (иначе — системный OpenSSL), и этот модуль
//! даёт ему транспорт на том же `reqwest` с `rustls`, что уже грузит внешние
//! шаблоны (007). Протокол — «stateless RPC» git: `GET …/info/refs?service=…`
//! объявляет ссылки, `POST …/git-upload-pack|git-receive-pack` передаёт пакет.
//!
//! Данные доступа и отмена приходят через контекст потока ([`with_context`]):
//! libgit2 вызывает транспорт на том же потоке, на котором идёт операция, а
//! глобальное хранение секрета было бы ровно тем, чего FR-014 не допускает.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::time::Duration;

use git2::transport::{Service, SmartSubtransport, SmartSubtransportStream};

use super::{Auth, CancelToken, EngineError};
use crate::sync::settings::remote_url;

/// Всё, что транспорту нужно знать об операции.
#[derive(Clone)]
pub struct TransportContext {
    pub auth: Auth,
    pub cancel: CancelToken,
    /// Сколько обмен может молчать, прежде чем считается зависшим.
    pub idle_limit: Duration,
}

/// Предел простоя сетевого обмена (research §9, FR-020, SC-004): столько сервер
/// может не присылать ни байта — ни заголовков ответа, ни очередной порции
/// тела. Не предел всей передачи: большой клон по медленному каналу идёт
/// сколько нужно, пока данные приходят.
pub const NETWORK_IDLE_LIMIT: Duration = Duration::from_secs(20);

/// Самое большое тело одной отправки (T077): pack-файл собирается в памяти, и
/// без предела отправка огромной истории заняла бы её всю. Отказ виден
/// пользователю как «больше N МБ», а не как падение приложения.
pub const PUSH_BODY_LIMIT_MB: u32 = 1024;

/// Наименьшая скорость отправки, при которой она ещё не считается зависшей.
/// Ход отправки тела запроса отсюда не виден — виден только её конец, —
/// поэтому предел для неё считается от размера тела.
const SLOWEST_UPLOAD_BYTES_PER_SEC: u64 = 16 * 1024;

/// Сколько ждать ответа на запрос с телом такого размера.
fn response_limit(idle_limit: Duration, body_len: usize) -> Duration {
    idle_limit + Duration::from_secs(body_len as u64 / SLOWEST_UPLOAD_BYTES_PER_SEC)
}

thread_local! {
    static CONTEXT: RefCell<Option<TransportContext>> = const { RefCell::new(None) };
    /// Точная причина отказа, которую libgit2 сводит к строке.
    static FAILURE: RefCell<Option<EngineError>> = const { RefCell::new(None) };
}

/// Выполняет операцию libgit2 с контекстом транспорта. При ошибке возвращает
/// и ошибку libgit2, и причину, записанную транспортом, если она была.
pub fn with_context<T>(
    context: TransportContext,
    operation: impl FnOnce() -> Result<T, git2::Error>,
) -> Result<T, (git2::Error, Option<EngineError>)> {
    CONTEXT.with(|slot| *slot.borrow_mut() = Some(context));
    FAILURE.with(|slot| *slot.borrow_mut() = None);
    let result = operation();
    CONTEXT.with(|slot| *slot.borrow_mut() = None);
    let failure = FAILURE.with(|slot| slot.borrow_mut().take());
    result.map_err(|error| (error, failure))
}

pub(super) fn record_failure(error: EngineError) {
    FAILURE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(error);
        }
    });
}

pub(super) fn current_context() -> Option<TransportContext> {
    CONTEXT.with(|slot| slot.borrow().clone())
}

/// Подтранспорт smart HTTP.
pub struct HttpSubtransport;

impl SmartSubtransport for HttpSubtransport {
    fn action(
        &self,
        url: &str,
        action: Service,
    ) -> Result<Box<dyn SmartSubtransportStream>, git2::Error> {
        let context = current_context()
            .ok_or_else(|| git2::Error::from_str("сетевая операция вне контекста"))?;
        let base = url.trim_end_matches('/');
        let (method, url, content_type, accept) = match action {
            Service::UploadPackLs => (
                Method::Get,
                format!("{base}/info/refs?service=git-upload-pack"),
                None,
                "application/x-git-upload-pack-advertisement",
            ),
            Service::UploadPack => (
                Method::Post,
                format!("{base}/git-upload-pack"),
                Some("application/x-git-upload-pack-request"),
                "application/x-git-upload-pack-result",
            ),
            Service::ReceivePackLs => (
                Method::Get,
                format!("{base}/info/refs?service=git-receive-pack"),
                None,
                "application/x-git-receive-pack-advertisement",
            ),
            Service::ReceivePack => (
                Method::Post,
                format!("{base}/git-receive-pack"),
                Some("application/x-git-receive-pack-request"),
                "application/x-git-receive-pack-result",
            ),
        };
        Ok(Box::new(HttpStream {
            method,
            url,
            content_type,
            accept,
            body: Vec::new(),
            response: None,
            context,
        }))
    }

    fn close(&self) -> Result<(), git2::Error> {
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Method {
    Get,
    Post,
}

struct HttpStream {
    method: Method,
    url: String,
    content_type: Option<&'static str>,
    accept: &'static str,
    body: Vec<u8>,
    response: Option<reqwest::blocking::Response>,
    context: TransportContext,
}

fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    // Хост кончается там же, где его закончит HTTP-клиент: на `/`, `?` или
    // `#`. Разбор строже клиентского — иначе чужой адрес сошёл бы за свой.
    let authority = rest.split(['/', '?', '#']).next()?;
    let hostport = authority.rsplit('@').next()?;
    remote_url::host_of_hostport(hostport)
}

fn client(idle_limit: Duration) -> std::io::Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        // Перенаправление — только на тот же хост и не с https на http: иначе
        // заголовок с токеном или сам пакет ушли бы открытым каналом (T068).
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let same_origin = attempt.previous().last().is_some_and(|from| {
                from.host_str() == attempt.url().host_str()
                    && from.port_or_known_default() == attempt.url().port_or_known_default()
                    && (from.scheme() != "https" || attempt.url().scheme() == "https")
            });
            if same_origin && attempt.previous().len() < 5 {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .connect_timeout(Duration::from_secs(10))
        // У блокирующего клиента это предел ожидания, а не всей передачи: он
        // применяется к ответу сервера и затем к каждому чтению тела по
        // отдельности — то есть ровно предел простоя.
        .timeout(idle_limit)
        .user_agent(concat!(
            "git/2.0 (typsidian/",
            env!("CARGO_PKG_VERSION"),
            ")"
        ))
        .build()
        .map_err(std::io::Error::other)
}

impl HttpStream {
    fn send(&mut self) -> std::io::Result<()> {
        if self.context.cancel.is_cancelled() {
            record_failure(EngineError::Cancelled);
            return Err(std::io::Error::other("отменено"));
        }
        let host = host_of(&self.url);
        let scheme_ok = self.url.starts_with("https://")
            || host.as_deref().is_some_and(remote_url::is_loopback);
        if !scheme_ok {
            record_failure(EngineError::UnsupportedScheme {
                scheme: "http".to_string(),
            });
            return Err(std::io::Error::other("http только для этого устройства"));
        }
        let idle_limit = self.context.idle_limit;
        let client = client(idle_limit)?;
        let mut request = match self.method {
            Method::Get => client.get(&self.url),
            Method::Post => {
                let body = std::mem::take(&mut self.body);
                // Ответ придёт только после отправки всего тела, поэтому ждать
                // его нужно дольше — соразмерно тому, что отправляется.
                client
                    .post(&self.url)
                    .timeout(response_limit(idle_limit, body.len()))
                    .body(body)
            }
        }
        .header("Accept", self.accept)
        .header("Git-Protocol", "version=0");
        if let Some(content_type) = self.content_type {
            request = request.header("Content-Type", content_type);
        }
        let auth = &self.context.auth;
        if let Some(token) = &auth.token {
            let user = auth
                .user
                .clone()
                .unwrap_or_else(|| remote_url::token_user(host.as_deref()).to_string());
            request = request.basic_auth(user, Some(token));
        }
        let response = request.send().map_err(|error| {
            record_failure(if error.is_timeout() {
                EngineError::Timeout
            } else {
                EngineError::Offline
            });
            std::io::Error::other("сервер недоступен")
        })?;
        match response.status().as_u16() {
            200..=299 => {
                self.response = Some(response);
                Ok(())
            }
            401 | 403 => {
                record_failure(if auth.token.is_some() {
                    EngineError::AuthRejected
                } else {
                    EngineError::AuthRequired
                });
                Err(std::io::Error::other("доступ запрещён"))
            }
            404 => {
                record_failure(EngineError::NotFound);
                Err(std::io::Error::other("репозиторий не найден"))
            }
            status => {
                record_failure(EngineError::Io {
                    message: format!("сервер ответил {status}"),
                });
                Err(std::io::Error::other("ошибка сервера"))
            }
        }
    }
}

impl Read for HttpStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.response.is_none() {
            self.send()?;
        }
        if self.context.cancel.is_cancelled() {
            record_failure(EngineError::Cancelled);
            return Err(std::io::Error::other("отменено"));
        }
        let response = self.response.as_mut().expect("ответ получен выше");
        response.read(buf).map_err(|error| {
            record_failure(EngineError::Timeout);
            error
        })
    }
}

/// Дописывает к телу, если оно не выйдет за предел; `false` — отказ, тело прежнее.
fn append_capped(body: &mut Vec<u8>, data: &[u8], limit: usize) -> bool {
    if body.len() + data.len() > limit {
        return false;
    }
    body.extend_from_slice(data);
    true
}

impl Write for HttpStream {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if !append_capped(
            &mut self.body,
            data,
            PUSH_BODY_LIMIT_MB as usize * 1024 * 1024,
        ) {
            record_failure(EngineError::TooLarge {
                limit_mb: PUSH_BODY_LIMIT_MB,
            });
            return Err(std::io::Error::other("отправка больше предела"));
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_push_body_stops_at_its_limit() {
        let mut body = Vec::new();
        assert!(append_capped(&mut body, b"abcd", 6));
        assert!(!append_capped(&mut body, b"efg", 6), "за пределом — отказ");
        assert_eq!(body, b"abcd", "тело при отказе прежнее");
        assert!(append_capped(&mut body, b"ef", 6));
    }

    #[test]
    fn host_is_extracted_without_credentials_and_port() {
        assert_eq!(
            host_of("https://u:p@GitHub.com:443/a/b.git").as_deref(),
            Some("github.com")
        );
    }

    #[test]
    fn context_is_cleared_after_the_operation() {
        let result: Result<(), _> = with_context(
            TransportContext {
                auth: Auth::default(),
                cancel: CancelToken::new(),
                idle_limit: NETWORK_IDLE_LIMIT,
            },
            || {
                assert!(current_context().is_some());
                record_failure(EngineError::Offline);
                Err(git2::Error::from_str("x"))
            },
        );
        assert_eq!(result.unwrap_err().1, Some(EngineError::Offline));
        assert!(current_context().is_none());
    }
}
