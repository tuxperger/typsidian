//! Настройки сервера (research.md §2): файл TOML, поверх него — переменные окружения
//! `TYPSIDIAN_*` и флаги командной строки.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Провайдер входа OpenID Connect (US3).
#[derive(Debug, Clone, Deserialize)]
pub struct OidcConfig {
    /// Адрес издателя: `https://id.example.org/realms/team`.
    pub issuer: String,
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Option<String>,
    /// Подпись кнопки «Войти через …».
    #[serde(default = "default_oidc_label")]
    pub label: String,
}

fn default_oidc_label() -> String {
    "OpenID Connect".to_owned()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Адрес, на котором слушает сервер.
    pub listen: SocketAddr,
    /// Папка данных: база аккаунтов, vault пользователей, корзина.
    pub data: PathBuf,
    /// Внешний адрес, по которому сервер видят браузеры (за прокси с TLS) —
    /// нужен возврату от провайдера входа.
    pub public_url: Option<String>,
    /// Через сколько дней без действий сессия истекает.
    pub session_idle_days: u32,
    /// Собранная статикой web-версия (`apps/web/out`).
    pub static_dir: Option<PathBuf>,
    /// Имя сервера на экране входа.
    pub name: String,
    /// Доверять `X-Forwarded-For` (сервер за своим обратным прокси).
    pub trust_proxy: bool,
    /// Разрешить свои адреса шаблонов из настроек vault (запросы сервера по адресам
    /// пользователей). По умолчанию — только реестр Typst.
    pub template_sources: bool,
    pub oidc: Option<OidcConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            listen: SocketAddr::from(([127, 0, 0, 1], 8787)),
            data: PathBuf::from("typsidian-data"),
            public_url: None,
            session_idle_days: 30,
            static_dir: None,
            name: "Typsidian".to_owned(),
            trust_proxy: false,
            template_sources: false,
            oidc: None,
        }
    }
}

impl Config {
    /// Читает файл настроек; нет файла — значения по умолчанию.
    pub fn load(path: Option<&Path>) -> anyhow::Result<Config> {
        let mut config = match path {
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
                toml::from_str(&text)?
            }
            None => Config::default(),
        };
        if let Ok(value) = std::env::var("TYPSIDIAN_DATA") {
            config.data = PathBuf::from(value);
        }
        if let Ok(value) = std::env::var("TYPSIDIAN_LISTEN") {
            config.listen = value.parse()?;
        }
        if let Ok(value) = std::env::var("TYPSIDIAN_PUBLIC_URL") {
            config.public_url = Some(value);
        }
        if let Ok(value) = std::env::var("TYPSIDIAN_STATIC") {
            config.static_dir = Some(PathBuf::from(value));
        }
        if let (Some(oidc), Ok(secret)) = (
            config.oidc.as_mut(),
            std::env::var("TYPSIDIAN_OIDC_CLIENT_SECRET"),
        ) {
            oidc.client_secret = Some(secret);
        }
        Ok(config)
    }

    pub fn session_idle_secs(&self) -> i64 {
        i64::from(self.session_idle_days.max(1)) * 24 * 60 * 60
    }

    pub fn database(&self) -> PathBuf {
        self.data.join("server.sqlite")
    }

    pub fn users_dir(&self) -> PathBuf {
        self.data.join("users")
    }

    pub fn trash_dir(&self) -> PathBuf {
        self.data.join("trash")
    }

    /// `Secure`-cookie без HTTPS работает только на loopback (research.md §3): без
    /// внешнего адреса с `https://` сервер слушает только его.
    pub fn check_transport(&self) -> anyhow::Result<()> {
        let https = self
            .public_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://"));
        if !https && !self.listen.ip().is_loopback() {
            anyhow::bail!(
                "без public_url с https:// сервер слушает только loopback (сейчас {})",
                self.listen
            );
        }
        if !self.listen.ip().is_loopback() {
            // Сам сервер HTTPS не ведёт: открытый наружу порт отдал бы пароль при входе
            // открытым текстом мимо прокси.
            tracing::warn!(
                "сервер слушает {} без TLS: закройте порт снаружи, оставив доступ только прокси",
                self.listen
            );
        }
        Ok(())
    }
}
