//! Вход через провайдера OpenID Connect (US3, research.md §3, конституция XI).
//!
//! Authorization code + PKCE. `state`, `nonce` и проверочный код PKCE живут в памяти
//! сервера под случайным `state` не дольше [`PENDING_TTL`]; тот же `state` лежит в
//! короткой cookie браузера, начавшего вход, — возврат от провайдера в другой браузер
//! (подсунутая ссылка) не пройдёт. Аккаунт ищется только по привязке `(issuer, sub)`,
//! которую задал администратор; почта не используется.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::http::header::{LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::Deserialize;

use crate::auth::{client_ip, open_session, Peer};
use crate::config::OidcConfig;
use crate::error::{ApiError, ApiResult};
use crate::Shared;

/// Сколько ждать возврата от провайдера.
const PENDING_TTL: Duration = Duration::from_secs(10 * 60);
/// Как долго держать сведения о провайдере (discovery), прежде чем спросить снова.
const METADATA_TTL: Duration = Duration::from_secs(60 * 60);
/// Предел незавершённых входов: поток запросов `start` не должен съесть память.
const MAX_PENDING: usize = 10_000;
const STATE_COOKIE: &str = "__Host-typsidian-oidc";

struct Pending {
    verifier: PkceCodeVerifier,
    nonce: Nonce,
    created: Instant,
}

#[derive(Default)]
pub struct OidcState {
    pending: Mutex<HashMap<String, Pending>>,
    metadata: tokio::sync::Mutex<Option<(Instant, CoreProviderMetadata)>>,
}

impl OidcState {
    /// Убирает просроченные незавершённые входы.
    pub fn sweep(&self) {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending.retain(|_, p| p.created.elapsed() < PENDING_TTL);
    }

    fn take(&self, state: &str) -> Option<Pending> {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending
            .remove(state)
            .filter(|p| p.created.elapsed() < PENDING_TTL)
    }

    fn put(&self, state: String, value: Pending) {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        if pending.len() >= MAX_PENDING {
            pending.retain(|_, p| p.created.elapsed() < PENDING_TTL);
        }
        if pending.len() < MAX_PENDING {
            pending.insert(state, value);
        }
    }
}

fn http_client() -> anyhow::Result<openidconnect::reqwest::Client> {
    Ok(openidconnect::reqwest::ClientBuilder::new()
        // Переходы по перенаправлениям открывают дорогу SSRF.
        .redirect(openidconnect::reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()?)
}

/// Внешний адрес сервера: туда провайдер вернёт браузер.
fn redirect_url(state: &Shared) -> anyhow::Result<RedirectUrl> {
    let base = state
        .config
        .public_url
        .clone()
        .unwrap_or_else(|| format!("http://{}", state.config.listen));
    Ok(RedirectUrl::new(format!(
        "{}/api/oidc/callback",
        base.trim_end_matches('/')
    ))?)
}

async fn metadata(state: &Shared, config: &OidcConfig) -> anyhow::Result<CoreProviderMetadata> {
    let mut cached = state.oidc.metadata.lock().await;
    if let Some((at, metadata)) = cached.as_ref() {
        if at.elapsed() < METADATA_TTL {
            return Ok(metadata.clone());
        }
    }
    let issuer = IssuerUrl::new(config.issuer.clone())?;
    let metadata = CoreProviderMetadata::discover_async(issuer, &http_client()?)
        .await
        .map_err(|error| anyhow::anyhow!("discovery провайдера: {error}"))?;
    *cached = Some((Instant::now(), metadata.clone()));
    Ok(metadata)
}

fn redirect(to: &str) -> Response {
    (
        StatusCode::SEE_OTHER,
        [(LOCATION, HeaderValue::from_str(to).expect("адрес"))],
    )
        .into_response()
}

fn state_cookie(value: &str, max_age: u64) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{STATE_COOKIE}={value}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={max_age}"
    ))
    .expect("state — base64url")
}

fn cookie_state(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == STATE_COOKIE)
        .map(|(_, value)| value.to_owned())
}

/// `GET /api/oidc/start`: перенаправление к провайдеру.
pub async fn start(State(state): State<Shared>) -> ApiResult<Response> {
    let Some(config) = state.config.oidc.clone() else {
        return Err(ApiError::NotFound);
    };
    let client = CoreClient::from_provider_metadata(
        metadata(&state, &config).await?,
        ClientId::new(config.client_id.clone()),
        config.client_secret.clone().map(ClientSecret::new),
    )
    .set_redirect_uri(redirect_url(&state)?);
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, csrf, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".to_owned()))
        .set_pkce_challenge(challenge)
        .url();
    let key = csrf.secret().clone();
    state.oidc.put(
        key.clone(),
        Pending {
            verifier,
            nonce,
            created: Instant::now(),
        },
    );
    let mut response = redirect(url.as_str());
    response
        .headers_mut()
        .insert(SET_COOKIE, state_cookie(&key, PENDING_TTL.as_secs()));
    Ok(response)
}

#[derive(Deserialize)]
pub struct Callback {
    code: Option<String>,
    state: Option<String>,
}

/// Итог возврата: на стартовую страницу, с причиной отказа в адресе.
fn finish(to: &str, cookie: Option<HeaderValue>) -> Response {
    let mut response = redirect(to);
    let headers = response.headers_mut();
    headers.append(SET_COOKIE, state_cookie("", 0));
    if let Some(cookie) = cookie {
        headers.append(SET_COOKIE, cookie);
    }
    response
}

/// `GET /api/oidc/callback`: обмен кода на ID-токен, проверка, вход.
pub async fn callback(
    State(state): State<Shared>,
    Peer(peer): Peer,
    headers: HeaderMap,
    Query(query): Query<Callback>,
) -> ApiResult<Response> {
    let Some(config) = state.config.oidc.clone() else {
        return Err(ApiError::NotFound);
    };
    let ip = client_ip(&state, &headers, peer);
    let (Some(code), Some(returned)) = (query.code, query.state) else {
        tracing::warn!(ip = %ip, "вход через провайдера: нет кода или state");
        return Ok(finish("/?login=failed", None));
    };
    // `state` из адреса должен совпасть с cookie этого браузера и с ожидаемым входом.
    let bound = cookie_state(&headers).is_some_and(|cookie| {
        cookie.len() == returned.len()
            && cookie
                .bytes()
                .zip(returned.bytes())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0
    });
    let Some(pending) = state.oidc.take(&returned).filter(|_| bound) else {
        tracing::warn!(ip = %ip, "вход через провайдера: чужой или просроченный state");
        return Ok(finish("/?login=failed", None));
    };
    let identity = async {
        let client = CoreClient::from_provider_metadata(
            metadata(&state, &config).await?,
            ClientId::new(config.client_id.clone()),
            config.client_secret.clone().map(ClientSecret::new),
        )
        .set_redirect_uri(redirect_url(&state)?);
        let http = http_client()?;
        let token = client
            .exchange_code(AuthorizationCode::new(code))?
            .set_pkce_verifier(pending.verifier)
            .request_async(&http)
            .await
            .map_err(|error| anyhow::anyhow!("обмен кода: {error}"))?;
        let id_token = token
            .id_token()
            .ok_or_else(|| anyhow::anyhow!("провайдер не вернул ID-токен"))?;
        let claims = id_token.claims(&client.id_token_verifier(), &pending.nonce)?;
        anyhow::Ok((
            claims.issuer().as_str().to_owned(),
            claims.subject().as_str().to_owned(),
        ))
    }
    .await;
    let (issuer, subject) = match identity {
        Ok(found) => found,
        Err(error) => {
            tracing::warn!(ip = %ip, "вход через провайдера не удался: {error:#}");
            return Ok(finish("/?login=failed", None));
        }
    };
    let Some(account) = state.store.account_by_oidc(&issuer, &subject).await? else {
        tracing::warn!(ip = %ip, issuer = %issuer, "вход через провайдера: аккаунт не заведён");
        return Ok(finish("/?login=unknown", None));
    };
    if account.blocked {
        tracing::warn!(login = %account.login, ip = %ip, "вход через провайдера: аккаунт заблокирован");
        return Ok(finish("/?login=failed", None));
    }
    tracing::info!(login = %account.login, ip = %ip, "вход через провайдера");
    let (cookie, _) = open_session(&state, account, &headers, &ip).await?;
    Ok(finish("/", Some(cookie)))
}
