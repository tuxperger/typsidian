//! Вход через провайдера (US3, T009) с поддельным провайдером OpenID Connect на
//! loopback: discovery, ключи, обмен кода на подписанный ID-токен.
//!
//! `oidc-test-key.pem` — ключ только для этого теста, ничего не защищает.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use openidconnect::core::{
    CoreIdToken, CoreIdTokenClaims, CoreIdTokenFields, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
    CoreProviderMetadata, CoreResponseType, CoreRsaPrivateSigningKey, CoreSubjectIdentifierType,
    CoreTokenResponse, CoreTokenType,
};
use openidconnect::{
    AccessToken, Audience, AuthUrl, EmptyAdditionalClaims, EmptyAdditionalProviderMetadata,
    EmptyExtraTokenFields, IssuerUrl, JsonWebKeyId, JsonWebKeySetUrl, Nonce, PrivateSigningKey,
    ResponseTypes, StandardClaims, SubjectIdentifier, TokenUrl,
};
use tower::ServiceExt;
use typsidian_server::config::{Config, OidcConfig};
use typsidian_server::store::Store;
use typsidian_server::{router, AppState, Shared};

const KEY: &str = include_str!("oidc-test-key.pem");
const CLIENT_ID: &str = "typsidian";

#[derive(Default)]
struct Provider {
    issuer: String,
    nonce: Mutex<Option<String>>,
    subject: Mutex<String>,
    verifier_seen: Mutex<bool>,
}

fn signing_key() -> CoreRsaPrivateSigningKey {
    CoreRsaPrivateSigningKey::from_pem(KEY, Some(JsonWebKeyId::new("test".to_owned()))).unwrap()
}

async fn discovery(State(p): State<Arc<Provider>>) -> Json<CoreProviderMetadata> {
    let issuer = &p.issuer;
    Json(
        CoreProviderMetadata::new(
            IssuerUrl::new(issuer.clone()).unwrap(),
            AuthUrl::new(format!("{issuer}/authorize")).unwrap(),
            JsonWebKeySetUrl::new(format!("{issuer}/jwks")).unwrap(),
            vec![ResponseTypes::new(vec![CoreResponseType::Code])],
            vec![CoreSubjectIdentifierType::Public],
            vec![CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256],
            EmptyAdditionalProviderMetadata {},
        )
        .set_token_endpoint(Some(TokenUrl::new(format!("{issuer}/token")).unwrap())),
    )
}

async fn jwks() -> Json<CoreJsonWebKeySet> {
    Json(CoreJsonWebKeySet::new(vec![
        signing_key().as_verification_key()
    ]))
}

async fn token(
    State(p): State<Arc<Provider>>,
    Form(form): Form<std::collections::HashMap<String, String>>,
) -> Json<CoreTokenResponse> {
    *p.verifier_seen.lock().unwrap() = form.contains_key("code_verifier");
    let access = AccessToken::new("access".to_owned());
    let nonce = p.nonce.lock().unwrap().clone().map(Nonce::new);
    let claims = CoreIdTokenClaims::new(
        IssuerUrl::new(p.issuer.clone()).unwrap(),
        vec![Audience::new(CLIENT_ID.to_owned())],
        chrono::Utc::now() + chrono::Duration::seconds(300),
        chrono::Utc::now(),
        StandardClaims::new(SubjectIdentifier::new(p.subject.lock().unwrap().clone())),
        EmptyAdditionalClaims {},
    )
    .set_nonce(nonce);
    let id_token = CoreIdToken::new(
        claims,
        &signing_key(),
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        Some(&access),
        None,
    )
    .unwrap();
    Json(CoreTokenResponse::new(
        access,
        CoreTokenType::Bearer,
        CoreIdTokenFields::new(Some(id_token), EmptyExtraTokenFields {}),
    ))
}

async fn provider() -> Arc<Provider> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider = Arc::new(Provider {
        issuer: format!("http://{}", listener.local_addr().unwrap()),
        ..Provider::default()
    });
    let app = Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/jwks", get(jwks))
        .route("/token", post(token))
        .with_state(provider.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    provider
}

async fn server(provider: &Provider) -> (Router, Shared, tempfile::TempDir) {
    let data = tempfile::tempdir().unwrap();
    let config = Config {
        data: data.path().to_path_buf(),
        public_url: Some("https://notes.example.org".to_owned()),
        oidc: Some(OidcConfig {
            issuer: provider.issuer.clone(),
            client_id: CLIENT_ID.to_owned(),
            client_secret: Some("secret".to_owned()),
            label: "Authentik".to_owned(),
        }),
        ..Config::default()
    };
    let state = AppState::new(config, Store::memory().await.unwrap());
    (router(state.clone()), state, data)
}

struct Started {
    state: String,
    cookie: String,
}

async fn start(app: &Router, provider: &Provider) -> Started {
    let response = app
        .clone()
        .oneshot(Request::get("/api/oidc/start").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(location.starts_with(&format!("{}/authorize?", provider.issuer)));
    let query: std::collections::HashMap<String, String> =
        url_decode_pairs(location.split_once('?').unwrap().1);
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(
        query["redirect_uri"],
        "https://notes.example.org/api/oidc/callback"
    );
    *provider.nonce.lock().unwrap() = Some(query["nonce"].clone());
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    assert!(cookie.starts_with("__Host-typsidian-oidc="));
    Started {
        state: query["state"].clone(),
        cookie,
    }
}

/// Пары адреса провайдера (`application/x-www-form-urlencoded`).
fn url_decode_pairs(query: &str) -> std::collections::HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (decode(k), decode(v)))
        .collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                out.push(u8::from_str_radix(&text[i + 1..i + 3], 16).unwrap());
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap()
}

async fn callback(app: &Router, state: &str, cookie: &str) -> (String, Option<String>) {
    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/api/oidc/callback?code=abc&state={state}"))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned();
    let session = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_owned())
        .find(|c| c.starts_with("__Host-typsidian="));
    (location, session)
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_sign_in_maps_by_issuer_and_subject() {
    let provider = provider().await;
    *provider.subject.lock().unwrap() = "sub-anna".to_owned();
    let (app, state, _data) = server(&provider).await;
    let anna = state.store.add_account("anna", "Анна", None).await.unwrap();

    // Кнопка есть только при настроенном провайдере.
    let info = app
        .clone()
        .oneshot(Request::get("/api/info").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let bytes = http_body_util::BodyExt::collect(info.into_body())
        .await
        .unwrap()
        .to_bytes();
    let info: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(info["oidc"], "Authentik");

    // Не сопоставлен — отказ с объяснением, сессии нет.
    let started = start(&app, &provider).await;
    let (location, session) = callback(&app, &started.state, &started.cookie).await;
    assert_eq!(location, "/?login=unknown");
    assert!(session.is_none());

    // Привязка администратора — вход в этот аккаунт.
    state
        .store
        .link_oidc(&anna.id, &provider.issuer, "sub-anna")
        .await
        .unwrap();
    let started = start(&app, &provider).await;
    let (location, session) = callback(&app, &started.state, &started.cookie).await;
    assert_eq!(location, "/");
    assert!(
        *provider.verifier_seen.lock().unwrap(),
        "PKCE: нет code_verifier"
    );
    let me = app
        .clone()
        .oneshot(
            Request::get("/api/me")
                .header(header::COOKIE, session.unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);

    // Повтор того же state — отказ.
    let (location, session) = callback(&app, &started.state, &started.cookie).await;
    assert_eq!(location, "/?login=failed");
    assert!(session.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn state_must_belong_to_this_browser() {
    let provider = provider().await;
    *provider.subject.lock().unwrap() = "sub-anna".to_owned();
    let (app, state, _data) = server(&provider).await;
    let anna = state.store.add_account("anna", "Анна", None).await.unwrap();
    state
        .store
        .link_oidc(&anna.id, &provider.issuer, "sub-anna")
        .await
        .unwrap();

    // Ссылка возврата, подсунутая в другой браузер (без cookie начала входа).
    let started = start(&app, &provider).await;
    let (location, session) = callback(&app, &started.state, "other=1").await;
    assert_eq!(location, "/?login=failed");
    assert!(session.is_none());

    // Чужой nonce в ID-токене — отказ.
    let started = start(&app, &provider).await;
    *provider.nonce.lock().unwrap() = Some("подменённый".to_owned());
    let (location, session) = callback(&app, &started.state, &started.cookie).await;
    assert_eq!(location, "/?login=failed");
    assert!(session.is_none());

    // Заблокированный аккаунт не входит и через провайдера.
    state.store.set_blocked(&anna.id, true).await.unwrap();
    let started = start(&app, &provider).await;
    let (location, session) = callback(&app, &started.state, &started.cookie).await;
    assert_eq!(location, "/?login=failed");
    assert!(session.is_none());
}
