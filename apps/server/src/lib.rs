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

/// Сколько запросов к моделям идут одновременно на весь сервер (050, ревью
/// безопасности L2): каждый живёт до 30 с и держит буфер ответа.
pub const MAX_AI_REQUESTS: usize = 16;

/// Сколько запросов к моделям идут одновременно у одного аккаунта (051, ревью безопасности
/// M5): поток урока живёт до 10 минут, и без предела один аккаунт занял бы весь общий
/// предел сервера.
pub const MAX_AI_REQUESTS_PER_ACCOUNT: u32 = 2;

/// Фоновых заданий с моделью (пополнение графа знаний, 054) у одного аккаунта: автопополнение
/// идёт без ведома пользователя и не должно занимать место хода урока (ревью безопасности
/// 054, L6). Само задание общий предел запросов держит только на время одной пачки.
pub const MAX_AI_BACKGROUND_PER_ACCOUNT: u32 = 1;

/// Оценок импорта заметок (055) на весь сервер: оценка читает и разбирает чужие файлы, а не
/// ходит в сеть, но так же занимает блокирующие потоки и память (ревью безопасности 055, M2).
pub const MAX_IMPORT_ESTIMATES: usize = 2;

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
    /// Одновременных запросов к моделям на весь сервер (050).
    pub ai_requests: Arc<tokio::sync::Semaphore>,
    /// Идущих запросов к моделям по аккаунтам (051).
    pub ai_by_account: std::sync::Mutex<std::collections::HashMap<String, u32>>,
    /// Идущих фоновых заданий с моделью по аккаунтам (054).
    pub ai_background: std::sync::Mutex<std::collections::HashMap<String, u32>>,
    /// Аккаунты с идущей оценкой импорта (055): одна оценка на аккаунт.
    pub import_estimating: std::sync::Mutex<std::collections::HashSet<String>>,
    /// Оценок импорта на весь сервер (055).
    pub import_estimates: Arc<tokio::sync::Semaphore>,
    /// Старты гостей по одному: проверка предела по адресу и запись гостя
    /// (фича 039).
    pub demo_starts: tokio::sync::Mutex<()>,
    /// Секрет ключей адресов демо-сервера; читается из папки данных при
    /// первом старте гостя.
    pub demo_secret: std::sync::OnceLock<String>,
}

pub type Shared = Arc<AppState>;

/// Место аккаунта среди идущих запросов к моделям; освобождается при сбросе, в том числе
/// когда клиент оборвал соединение и ожидание ответа бросили.
pub struct AccountAiSlot {
    state: Shared,
    account: String,
}

impl AccountAiSlot {
    pub fn acquire(state: &Shared, account: &str) -> Option<AccountAiSlot> {
        let mut running = state.ai_by_account.lock().ok()?;
        let count = running.entry(account.to_owned()).or_insert(0);
        if *count >= MAX_AI_REQUESTS_PER_ACCOUNT {
            return None;
        }
        *count += 1;
        Some(AccountAiSlot {
            state: state.clone(),
            account: account.to_owned(),
        })
    }
}

impl Drop for AccountAiSlot {
    fn drop(&mut self) {
        if let Ok(mut running) = self.state.ai_by_account.lock() {
            if let Some(count) = running.get_mut(&self.account) {
                *count -= 1;
                if *count == 0 {
                    running.remove(&self.account);
                }
            }
        }
    }
}

/// Место аккаунта среди фоновых заданий с моделью (054, L6).
pub struct BackgroundAiSlot {
    state: Shared,
    account: String,
}

impl BackgroundAiSlot {
    pub fn acquire(state: &Shared, account: &str) -> Option<BackgroundAiSlot> {
        let mut running = state.ai_background.lock().ok()?;
        let count = running.entry(account.to_owned()).or_insert(0);
        if *count >= MAX_AI_BACKGROUND_PER_ACCOUNT {
            return None;
        }
        *count += 1;
        Some(BackgroundAiSlot {
            state: state.clone(),
            account: account.to_owned(),
        })
    }
}

impl Drop for BackgroundAiSlot {
    fn drop(&mut self) {
        if let Ok(mut running) = self.state.ai_background.lock() {
            if let Some(count) = running.get_mut(&self.account) {
                *count -= 1;
                if *count == 0 {
                    running.remove(&self.account);
                }
            }
        }
    }
}

/// Место оценки импорта заметок (055, ревью безопасности M2): одно на аккаунт и общий предел на
/// сервер. Возвращается при сбросе, в том числе когда вызов оборван.
pub struct ImportEstimateSlot {
    state: Shared,
    account: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl ImportEstimateSlot {
    pub fn acquire(state: &Shared, account: &str) -> Option<ImportEstimateSlot> {
        let mut running = state.import_estimating.lock().ok()?;
        if running.contains(account) {
            return None;
        }
        let permit = state.import_estimates.clone().try_acquire_owned().ok()?;
        running.insert(account.to_owned());
        Some(ImportEstimateSlot {
            state: state.clone(),
            account: account.to_owned(),
            _permit: permit,
        })
    }
}

impl Drop for ImportEstimateSlot {
    fn drop(&mut self) {
        if let Ok(mut running) = self.state.import_estimating.lock() {
            running.remove(&self.account);
        }
    }
}

/// Место под одну пачку фонового задания: место аккаунта и место общего предела запросов к
/// моделям, оба на время одного сетевого запроса. Состояние сервера не держится: окружение
/// аккаунта живёт в нём самом.
pub struct AiBatchGate {
    pub state: std::sync::Weak<AppState>,
    pub account: String,
}

impl typsidian_host::sync::RequestGate for AiBatchGate {
    fn enter(&self) -> Option<Box<dyn Send>> {
        let state = self.state.upgrade()?;
        let account = AccountAiSlot::acquire(&state, &self.account)?;
        let permit = state.ai_requests.clone().try_acquire_owned().ok()?;
        Some(Box::new((account, permit)))
    }
}

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
            ai_requests: Arc::new(tokio::sync::Semaphore::new(MAX_AI_REQUESTS)),
            ai_by_account: std::sync::Mutex::default(),
            ai_background: std::sync::Mutex::default(),
            import_estimating: std::sync::Mutex::default(),
            import_estimates: Arc::new(tokio::sync::Semaphore::new(MAX_IMPORT_ESTIMATES)),
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
