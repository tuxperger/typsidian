//! Сервер Typsidian (фича 037): вход, свои vault на диске сервера и команды ядра по
//! HTTP для web-версии.
//!
//! Работа с vault — те же команды `typsidian-host`, что у оболочки Tauri; здесь
//! только транспорт, аккаунты, сессии и изоляция аккаунтов (конституция XI).

use std::sync::Arc;
use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::http::header::{HeaderName, HeaderValue};
use axum::middleware::from_fn_with_state;
use axum::routing::{delete, get, post};
use axum::Router;

pub mod allowed;
pub mod auth;
pub mod config;
pub mod demo;
pub mod error;
pub mod events;
pub mod invoke;
pub mod oidc;
pub mod store;
pub mod sync;
pub mod vaults;

use config::Config;
use store::Store;

/// Сколько обменов с удалёнными репозиториями идут одновременно на весь сервер.
pub const MAX_EXCHANGES: usize = 32;

/// Состояние сервера, общее для всех запросов.
pub struct AppState {
    pub config: Config,
    pub store: Store,
    pub limiter: auth::Limiter,
    pub vaults: vaults::OpenVaults,
    pub oidc: oidc::OidcState,
    pub sync: sync::SyncHomes,
    /// Одновременных обменов с удалёнными на весь сервер (038): каждый занимает
    /// блокирующий поток, а пул потоков общий для всех аккаунтов.
    pub exchanges: tokio::sync::Semaphore,
    /// Старты гостей по одному: проверка предела по адресу и запись гостя
    /// (фича 039).
    pub demo_starts: tokio::sync::Mutex<()>,
    /// Секрет ключей адресов демо-сервера; читается из папки данных при
    /// первом старте гостя.
    pub demo_secret: std::sync::OnceLock<String>,
}

pub type Shared = Arc<AppState>;

impl AppState {
    pub fn new(config: Config, store: Store) -> Shared {
        // Обмены с удалёнными — только по внешнему HTTPS, без ssh и конфигурации git
        // администратора (038, ревью безопасности).
        typsidian_core::sync::engine::server_policy::restrict_to_server(
            config.sync_private_remotes,
        );
        Arc::new(AppState {
            config,
            store,
            limiter: auth::Limiter::default(),
            vaults: vaults::OpenVaults::default(),
            oidc: oidc::OidcState::default(),
            sync: sync::SyncHomes::default(),
            exchanges: tokio::sync::Semaphore::new(MAX_EXCHANGES),
            demo_starts: tokio::sync::Mutex::new(()),
            demo_secret: std::sync::OnceLock::new(),
        })
    }
}

/// Создаёт папку данных (или вложенную в неё) с доступом только для процесса
/// сервера: заметки, хеши паролей и токены CSRF другим пользователям машины не
/// видны.
pub fn private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Самый большой файл, который можно вставить вложением.
const MAX_BODY: usize = 256 * 1024 * 1024;

async fn closed() -> error::ApiError {
    error::ApiError::Forbidden
}

async fn absent() -> error::ApiError {
    error::ApiError::NotFound
}

pub fn router(state: Shared) -> Router {
    // На демо-сервере тело не больше квоты гостя: тело читается в память целиком
    // до проверки квоты, а отправить его может любой (ревью безопасности 039).
    let body_limit = if state.config.demo_mode {
        (state.config.demo.quota_bytes() as usize)
            .saturating_add(1024 * 1024)
            .min(MAX_BODY)
    } else {
        MAX_BODY
    };
    let protected = Router::new()
        .route("/api/me", get(auth::me))
        .route("/api/logout", post(auth::logout))
        .route(
            "/api/vaults/{name}/invoke/{command}",
            post(invoke::invoke).layer(DefaultBodyLimit::max(body_limit)),
        )
        .route("/api/vaults/{name}/events", get(invoke::events))
        .route("/api/vaults/{name}/file/{*path}", get(invoke::file));
    // Демо-сервер (фича 039): только гости — ни профиля, ни операций над vault,
    // ни входа паролем и через провайдера (research.md §3).
    let protected = if state.config.demo_mode {
        protected
            .route("/api/demo/usage", get(demo::usage))
            .route("/api/logout-all", post(closed))
            .route("/api/password", post(closed))
            .route("/api/sessions", get(closed))
            .route("/api/sessions/{id}", delete(closed))
            .route("/api/vaults", get(vaults::list).post(closed))
            .route(
                "/api/vaults/{name}",
                get(vaults::summary).patch(closed).delete(closed),
            )
    } else {
        protected
            .route("/api/logout-all", post(auth::logout_all))
            .route("/api/password", post(auth::change_password))
            .route("/api/sessions", get(auth::sessions))
            .route("/api/sessions/{id}", delete(auth::end_session))
            .route("/api/vaults", get(vaults::list).post(vaults::create))
            .route(
                "/api/vaults/{name}",
                get(vaults::summary)
                    .patch(vaults::rename)
                    .delete(vaults::delete),
            )
    }
    .route_layer(from_fn_with_state(state.clone(), auth::require_session));

    let public = Router::new().route("/api/info", get(auth::info));
    let public = if state.config.demo_mode {
        public
            .route("/api/demo/start", post(demo::start))
            .route("/api/login", post(absent))
            .route("/api/oidc/start", get(absent))
            .route("/api/oidc/callback", get(absent))
    } else {
        public
            .route("/api/login", post(auth::login))
            .route("/api/oidc/start", get(oidc::start))
            .route("/api/oidc/callback", get(oidc::callback))
    };

    let mut app = Router::new().merge(public).merge(protected);
    app = match &state.config.static_dir {
        Some(dir) => app.fallback_service(
            tower_http::services::ServeDir::new(dir)
                .append_index_html_on_directories(true)
                .fallback(tower_http::services::ServeFile::new(dir.join("index.html"))),
        ),
        None => app.fallback(|| async { error::ApiError::NotFound }),
    };
    app.layer(axum::middleware::map_response(security_headers))
        .with_state(state)
}

/// Заголовки, которые запрещают встраивать страницу и угадывать тип ответа.
async fn security_headers(mut response: axum::response::Response) -> axum::response::Response {
    let headers = response.headers_mut();
    for (name, value) in [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "same-origin"),
        ("cross-origin-opener-policy", "same-origin"),
        // Браузер учитывает HSTS только из ответа по HTTPS — за прокси с TLS; по
        // http://localhost заголовок ничего не делает.
        ("strict-transport-security", "max-age=31536000"),
        // Скриптов CSP не ограничивает: статика Next.js несёт встроенные скрипты.
        // Встраивание страницы, `<base>` и плагины закрыты.
        (
            "content-security-policy",
            "frame-ancestors 'none'; base-uri 'self'; object-src 'none'; form-action 'self'",
        ),
    ] {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    response
}

/// Фоновая уборка: простаивающие vault и истёкшие сессии.
pub fn spawn_housekeeping(state: Shared) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            state.vaults.sweep().await;
            demo::sweep(&state).await;
            if let Err(error) = state
                .store
                .purge_sessions(state.config.session_idle_secs())
                .await
            {
                tracing::error!("уборка сессий: {error:#}");
            }
            state.oidc.sweep();
        }
    });
}
