//! Вход паролем, сессии, CSRF и ограничение попыток (research.md §3, конституция XI).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{Path, Request, State};
use axum::http::header::{COOKIE, SET_COOKIE, USER_AGENT};
use axum::http::{HeaderMap, HeaderValue, Method};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};
use crate::store::{now, valid_login, Account, Session};
use crate::Shared;

pub const COOKIE_NAME: &str = "__Host-typsidian";
pub const CSRF_HEADER: &str = "x-typsidian-csrf";
/// Пароль не короче (clarifications).
pub const MIN_PASSWORD_CHARS: usize = 12;
/// Как часто продлевать сессию: каждое действие писать в базу незачем.
const TOUCH_EVERY_SECS: i64 = 60;

// ── Пароли ─────────────────────────────────────────────────────────────────

/// argon2id с параметрами по умолчанию крейта (рекомендация OWASP).
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| anyhow::anyhow!("argon2: {error}"))
}

fn verify_password(hash: &str, password: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}

/// Хеш для неизвестного логина: проверка идёт столько же, сколько для известного, и
/// по времени ответа не узнать, есть ли такой аккаунт.
fn dummy_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| hash_password("typsidian-dummy-password").unwrap_or_default())
}

/// Пароль длиннее не проверяется вовсе: argon2 над мегабайтами — даром отданное время.
pub const MAX_PASSWORD_BYTES: usize = 1024;

/// Одновременных argon2 не больше этого: каждая проверка — ~19 МиБ и ядро
/// процессора, и поток входов без ограничения занял бы и то и другое целиком.
static ARGON2: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

/// Сверка пароля вне рабочих потоков, не больше [`ARGON2`] сразу.
async fn check_password(hash: String, password: String) -> bool {
    let Ok(_permit) = ARGON2.acquire().await else {
        return false;
    };
    tokio::task::spawn_blocking(move || verify_password(&hash, &password))
        .await
        .unwrap_or(false)
}

pub fn check_new_password(password: &str) -> ApiResult<()> {
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(ApiError::Invalid {
            field: "password",
            reason: "TooLong",
        });
    }
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(ApiError::Invalid {
            field: "password",
            reason: "TooShort",
        });
    }
    Ok(())
}

// ── Ограничение попыток ─────────────────────────────────────────────────────

/// После 5 неверных паролей подряд вход с этим логином закрыт на 1, 2, 4… минут, до
/// часа (clarifications). В памяти: перезапуск сервера счётчик сбрасывает.
#[derive(Default)]
pub struct Limiter(Mutex<HashMap<String, Attempts>>);

struct Attempts {
    failures: u32,
    locked_until: Option<Instant>,
    /// Проверка пароля для этого логина идёт прямо сейчас. Попытки одного логина —
    /// строго по одной: иначе пачка параллельных запросов прошла бы проверку
    /// лимита раньше, чем первый из них успел бы его увеличить.
    in_flight: bool,
}

const FREE_ATTEMPTS: u32 = 5;
const MAX_LOCK: Duration = Duration::from_secs(60 * 60);
/// Предел записей: перебор несуществующих логинов не должен съесть память.
const MAX_TRACKED: usize = 10_000;

/// Начатая попытка входа: [`Attempt::fail`] или [`Attempt::succeed`]; брошенная
/// (внутренняя ошибка) просто освобождает логин.
pub struct Attempt<'a> {
    limiter: &'a Limiter,
    login: String,
    done: bool,
}

impl Limiter {
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Attempts>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Начинает попытку; `Err(секунды)` — вход с этим логином пока закрыт (или
    /// другая попытка ещё идёт).
    pub fn begin(&self, login: &str) -> Result<Attempt<'_>, u64> {
        let mut map = self.map();
        let now = Instant::now();
        if map.len() >= MAX_TRACKED && !map.contains_key(login) {
            map.retain(|_, a| a.in_flight || a.locked_until.is_some_and(|until| until > now));
            if map.len() >= MAX_TRACKED {
                return Err(60);
            }
        }
        let attempts = map.entry(login.to_owned()).or_insert(Attempts {
            failures: 0,
            locked_until: None,
            in_flight: false,
        });
        if let Some(until) = attempts.locked_until.filter(|until| *until > now) {
            return Err((until - now).as_secs().max(1));
        }
        if attempts.in_flight {
            return Err(1);
        }
        attempts.in_flight = true;
        Ok(Attempt {
            limiter: self,
            login: login.to_owned(),
            done: false,
        })
    }
}

impl Attempt<'_> {
    /// Неверный пароль; `Some(секунды)` — вход теперь закрыт.
    pub fn fail(mut self) -> Option<u64> {
        self.done = true;
        let mut map = self.limiter.map();
        let attempts = map.get_mut(&self.login)?;
        attempts.in_flight = false;
        attempts.failures += 1;
        if attempts.failures < FREE_ATTEMPTS {
            return None;
        }
        let exponent = (attempts.failures - FREE_ATTEMPTS).min(6);
        let lock = Duration::from_secs(60 * (1u64 << exponent)).min(MAX_LOCK);
        attempts.locked_until = Some(Instant::now() + lock);
        Some(lock.as_secs())
    }

    /// Верный пароль сбрасывает счётчик.
    pub fn succeed(mut self) {
        self.done = true;
        self.limiter.map().remove(&self.login);
    }
}

impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        if !self.done {
            if let Some(attempts) = self.limiter.map().get_mut(&self.login) {
                attempts.in_flight = false;
            }
        }
    }
}

// ── Cookie и сессия запроса ─────────────────────────────────────────────────

pub fn session_cookie(token: &str, max_age: i64) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={max_age}"
    ))
    .expect("токен — hex")
}

pub fn cleared_cookie() -> HeaderValue {
    HeaderValue::from_static("__Host-typsidian=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0")
}

pub fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE_NAME)
        .map(|(_, value)| value.to_owned())
        .filter(|value| !value.is_empty())
}

/// Адрес клиента для журнала и списка сессий.
pub fn client_ip(
    state: &Shared,
    headers: &HeaderMap,
    peer: Option<std::net::SocketAddr>,
) -> String {
    if state.config.trust_proxy {
        if let Some(forwarded) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            // Последнее значение — то, что дописал свой прокси; левые задаёт клиент.
            .and_then(|v| v.rsplit(',').next())
        {
            return forwarded.trim().to_owned();
        }
    }
    peer.map(|peer| peer.ip().to_string()).unwrap_or_default()
}

/// Адрес собеседника, если сервер запущен с `ConnectInfo` (в тестах его нет).
pub struct Peer(pub Option<std::net::SocketAddr>);

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for Peer {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(Peer(
            parts
                .extensions
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .map(|info| info.0),
        ))
    }
}

/// Сессия запроса — её кладёт [`require_session`].
#[derive(Clone)]
pub struct Authed {
    pub session: Session,
    pub account: Account,
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Пропускает только запросы с живой сессией; изменяющие — только с CSRF-токеном
/// этой сессии. Продлевает сессию и обновляет срок cookie.
pub async fn require_session(
    State(state): State<Shared>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(token) = cookie_token(request.headers()) else {
        return ApiError::Unauthenticated.into_response();
    };
    let idle = state.config.session_idle_secs();
    let (session, account) = match state.store.session_by_token(&token, idle).await {
        Ok(Some(found)) => found,
        Ok(None) => return ApiError::Unauthenticated.into_response(),
        Err(error) => return ApiError::Internal(error).into_response(),
    };
    let safe = matches!(*request.method(), Method::GET | Method::HEAD);
    if !safe {
        let header = request
            .headers()
            .get(CSRF_HEADER)
            .map(|v| v.as_bytes())
            .unwrap_or_default();
        if !constant_time_eq(header, session.csrf.as_bytes()) {
            tracing::warn!(login = %account.login, "отказ: нет CSRF-токена");
            return ApiError::Forbidden.into_response();
        }
    }
    let now = now();
    let touched = now - session.last_seen >= TOUCH_EVERY_SECS;
    if touched {
        if let Err(error) = state.store.touch_session(&session.id, now).await {
            return ApiError::Internal(error).into_response();
        }
    }
    request.extensions_mut().insert(Authed { session, account });
    let mut response = next.run(request).await;
    if touched && !response.headers().contains_key(SET_COOKIE) {
        response
            .headers_mut()
            .insert(SET_COOKIE, session_cookie(&token, idle));
    }
    response
}

// ── Обработчики ────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    name: String,
    password: bool,
    oidc: Option<String>,
}

pub async fn info(State(state): State<Shared>) -> Json<Info> {
    Json(Info {
        name: state.config.name.clone(),
        password: true,
        oidc: state.config.oidc.as_ref().map(|oidc| oidc.label.clone()),
    })
}

#[derive(Deserialize)]
pub struct LoginRequest {
    login: String,
    password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    login: String,
    display_name: String,
    has_password: bool,
    csrf: String,
}

fn me_of(authed: &Authed) -> Me {
    Me {
        login: authed.account.login.clone(),
        display_name: authed.account.display_name.clone(),
        has_password: authed.account.password_hash.is_some(),
        csrf: authed.session.csrf.clone(),
    }
}

/// Открывает сессию аккаунту и отдаёт cookie (общая часть входа паролем и OIDC).
pub async fn open_session(
    state: &Shared,
    account: Account,
    headers: &HeaderMap,
    ip: &str,
) -> ApiResult<(HeaderValue, Me)> {
    let agent = headers
        .get(USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let new = state.store.create_session(&account.id, agent, ip).await?;
    let cookie = session_cookie(&new.token, state.config.session_idle_secs());
    let authed = Authed {
        session: new.session,
        account,
    };
    Ok((cookie, me_of(&authed)))
}

/// `POST /api/login`. Тело — только JSON: форма с чужого сайта так не отправится
/// без предварительного запроса CORS, которого сервер не разрешает.
pub async fn login(
    State(state): State<Shared>,
    Peer(peer): Peer,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> ApiResult<Response> {
    let ip = client_ip(&state, &headers, peer);
    let login = request.login.trim().to_lowercase();
    // Такого логина быть не может (его задаёт администратор по `valid_login`):
    // ни лимитер, ни argon2 на него не тратятся — длинная строка не займёт память.
    if !valid_login(&login) || request.password.len() > MAX_PASSWORD_BYTES {
        return Err(ApiError::BadCredentials);
    }
    let attempt = match state.limiter.begin(&login) {
        Ok(attempt) => attempt,
        Err(retry_after_s) => {
            tracing::warn!(login = %login, ip = %ip, "вход закрыт после неверных попыток");
            return Err(ApiError::TooManyAttempts { retry_after_s });
        }
    };
    let account = state.store.account_by_login(&login).await?;
    let hash = account
        .as_ref()
        .and_then(|a| a.password_hash.clone())
        .unwrap_or_else(|| dummy_hash().to_owned());
    let matches = check_password(hash, request.password).await;
    let account = match account {
        Some(account) if matches && account.password_hash.is_some() && !account.blocked => account,
        _ => {
            tracing::warn!(login = %login, ip = %ip, "неверный вход");
            if let Some(retry_after_s) = attempt.fail() {
                return Err(ApiError::TooManyAttempts { retry_after_s });
            }
            return Err(ApiError::BadCredentials);
        }
    };
    attempt.succeed();
    tracing::info!(login = %login, ip = %ip, "вход");
    let (cookie, me) = open_session(&state, account, &headers, &ip).await?;
    Ok(([(SET_COOKIE, cookie)], Json(me)).into_response())
}

pub async fn me(Extension(authed): Extension<Authed>) -> Json<Me> {
    Json(me_of(&authed))
}

pub async fn logout(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
) -> ApiResult<Response> {
    state
        .store
        .end_session(&authed.account.id, &authed.session.id)
        .await?;
    tracing::info!(login = %authed.account.login, "выход");
    Ok(([(SET_COOKIE, cleared_cookie())], Json(())).into_response())
}

/// «Выйти на всех устройствах» (US5 AS3): все сессии аккаунта, текущая тоже.
pub async fn logout_all(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
) -> ApiResult<Response> {
    state.store.end_sessions(&authed.account.id, None).await?;
    tracing::info!(login = %authed.account.login, "выход на всех устройствах");
    Ok(([(SET_COOKIE, cleared_cookie())], Json(())).into_response())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    #[serde(flatten)]
    session: Session,
    current: bool,
}

pub async fn sessions(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
) -> ApiResult<Json<Vec<SessionView>>> {
    let list = state.store.sessions(&authed.account.id).await?;
    Ok(Json(
        list.into_iter()
            .map(|session| SessionView {
                current: session.id == authed.session.id,
                session,
            })
            .collect(),
    ))
}

pub async fn end_session(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
    Path(id): Path<String>,
) -> ApiResult<Json<()>> {
    if !state.store.end_session(&authed.account.id, &id).await? {
        return Err(ApiError::NotFound);
    }
    Ok(Json(()))
}

#[derive(Deserialize)]
pub struct PasswordRequest {
    current: String,
    next: String,
}

/// Смена пароля: нужен текущий; прочие сессии завершаются (US5 AS3).
pub async fn change_password(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
    Json(request): Json<PasswordRequest>,
) -> ApiResult<Json<()>> {
    let login = authed.account.login.clone();
    let Some(hash) = authed.account.password_hash.clone() else {
        return Err(ApiError::Invalid {
            field: "current",
            reason: "NoPassword",
        });
    };
    check_new_password(&request.next)?;
    if request.current.len() > MAX_PASSWORD_BYTES {
        return Err(ApiError::Invalid {
            field: "current",
            reason: "Wrong",
        });
    }
    let attempt = state
        .limiter
        .begin(&login)
        .map_err(|retry_after_s| ApiError::TooManyAttempts { retry_after_s })?;
    if !check_password(hash, request.current).await {
        tracing::warn!(login = %login, "неверный текущий пароль при смене");
        if let Some(retry_after_s) = attempt.fail() {
            return Err(ApiError::TooManyAttempts { retry_after_s });
        }
        return Err(ApiError::Invalid {
            field: "current",
            reason: "Wrong",
        });
    }
    attempt.succeed();
    let next = request.next;
    let _permit = ARGON2.acquire().await.map_err(|e| anyhow::anyhow!(e))?;
    let hash = tokio::task::spawn_blocking(move || hash_password(&next))
        .await
        .map_err(|e| anyhow::anyhow!(e))??;
    state.store.set_password(&authed.account.id, &hash).await?;
    state
        .store
        .end_sessions(&authed.account.id, Some(&authed.session.id))
        .await?;
    tracing::info!(login = %login, "пароль сменён");
    Ok(Json(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_roundtrip() {
        let hash = hash_password("длинный пароль 1").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password(&hash, "длинный пароль 1"));
        assert!(!verify_password(&hash, "другой пароль 12"));
        assert!(!verify_password("не хеш", "x"));
    }

    #[test]
    fn limiter_locks_after_five_failures() {
        let limiter = Limiter::default();
        for _ in 0..4 {
            assert_eq!(limiter.begin("anna").unwrap().fail(), None);
        }
        // Пока идёт одна попытка, вторая параллельная не начинается.
        let pending = limiter.begin("anna").unwrap();
        assert_eq!(limiter.begin("anna").err(), Some(1));
        assert_eq!(pending.fail(), Some(60));
        assert!(limiter.begin("anna").is_err());
        assert!(limiter.begin("bob").is_ok());
        // Брошенная попытка (внутренняя ошибка) логин не держит.
        drop(limiter.begin("carol").unwrap());
        assert!(limiter.begin("carol").is_ok());
        limiter.begin("dave").unwrap().succeed();
        assert!(limiter.begin("dave").is_ok());
    }

    #[test]
    fn cookie_parsing() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_static("a=1; __Host-typsidian=abc; b=2"),
        );
        assert_eq!(cookie_token(&headers).as_deref(), Some("abc"));
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
