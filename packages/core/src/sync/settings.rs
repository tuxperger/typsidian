//! Настройки синхронизации уровня vault — ключ `sync` файла
//! `.typsidian/vault.json` (фича 011, data-model «VaultSyncSettings»).
//!
//! Отдельно от `VaultOverrides` (006): там лежат перекрытия значений
//! устройства, а здесь — свойства самой папки заметок, у которых на устройстве
//! пары нет: адрес удалённого репозитория и хранитель истории. Они доставляются
//! Syncthing вместе с vault, и именно поэтому хранитель виден всем устройствам
//! (FR-029).
//!
//! Запись правит **только** ключ `sync` поверх прочитанного файла: остальные
//! ключи, свои и пользовательские, переносятся как есть — то же правило, что у
//! `vault_settings::render`.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::vault::backend::{ReadError, VaultBackend};
use crate::vault::service_dir;

use super::{Millis, SyncError};

const KEY: &str = "sync";

/// Пределы паузы автоснимка, секунды (data-model).
pub const MIN_PAUSE_SECS: u32 = 30;
pub const MAX_PAUSE_SECS: u32 = 86_400;
pub const DEFAULT_PAUSE_SECS: u32 = 300;
/// Наименьший интервал автополучения, секунды.
pub const MIN_PULL_INTERVAL_SECS: u32 = 60;

/// Хранитель истории (FR-029).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Keeper {
    pub device_id: String,
    pub device_name: String,
    pub since: Millis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(default)]
pub struct VaultSyncSettings {
    /// Адрес удалённого репозитория без учётных данных.
    pub remote_url: Option<String>,
    /// Ветка обмена. `None` — текущая ветка истории, а без неё `main`.
    pub branch: Option<String>,
    pub keeper: Option<Keeper>,
    pub auto_snapshot: bool,
    pub auto_snapshot_pause_secs: u32,
    pub auto_push: bool,
    pub auto_pull_on_open: bool,
    pub auto_pull_interval_secs: Option<u32>,
}

impl Default for VaultSyncSettings {
    fn default() -> Self {
        Self {
            remote_url: None,
            branch: None,
            keeper: None,
            auto_snapshot: false,
            auto_snapshot_pause_secs: DEFAULT_PAUSE_SECS,
            auto_push: false,
            auto_pull_on_open: false,
            auto_pull_interval_secs: None,
        }
    }
}

impl VaultSyncSettings {
    /// Приводит значения к допустимым: файл мог править человек, и одна
    /// негодная цифра не повод отказать в остальном.
    pub fn normalized(mut self) -> Self {
        self.auto_snapshot_pause_secs = self
            .auto_snapshot_pause_secs
            .clamp(MIN_PAUSE_SECS, MAX_PAUSE_SECS);
        self.auto_pull_interval_secs = self
            .auto_pull_interval_secs
            .map(|secs| secs.max(MIN_PULL_INTERVAL_SECS));
        // Адрес — только в каноническом виде: файл могли править руками или
        // чужими, и сырой адрес с учётными данными или двусмысленным хостом
        // дальше не идёт (T068).
        self.remote_url = self
            .remote_url
            .and_then(|url| remote_url::normalize(&url).ok())
            .map(|parsed| parsed.url);
        // Ветка идёт в refspec обмена: `*` сделал бы из него шаблон, `:` — второй
        // refspec (038, ревью безопасности). Негодное имя — ветка по умолчанию.
        self.branch = self.branch.filter(|branch| valid_branch(branch));
        self
    }
}

/// Годится ли имя ветки (правила `git check-ref-format` для одной ветки).
pub fn valid_branch(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with(['-', '/', '.'])
        && !name.ends_with(['/', '.'])
        && !name.ends_with(".lock")
        && !name.contains("..")
        && !name.contains("@{")
        && !name.contains("//")
        && !name.chars().any(|c| {
            c.is_control()
                || c.is_whitespace()
                || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
        })
}

/// Частичное изменение настроек из панели (FR-019, FR-031).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(default)]
pub struct VaultSyncSettingsPatch {
    pub auto_snapshot: Option<bool>,
    pub auto_snapshot_pause_secs: Option<u32>,
    pub auto_push: Option<bool>,
    pub auto_pull_on_open: Option<bool>,
    /// `Some(0)` — выключить автополучение по интервалу.
    pub auto_pull_interval_secs: Option<u32>,
}

impl VaultSyncSettingsPatch {
    pub fn apply_to(&self, base: &VaultSyncSettings) -> VaultSyncSettings {
        let mut next = base.clone();
        if let Some(value) = self.auto_snapshot {
            next.auto_snapshot = value;
        }
        if let Some(value) = self.auto_snapshot_pause_secs {
            next.auto_snapshot_pause_secs = value;
        }
        if let Some(value) = self.auto_push {
            next.auto_push = value;
        }
        if let Some(value) = self.auto_pull_on_open {
            next.auto_pull_on_open = value;
        }
        if let Some(value) = self.auto_pull_interval_secs {
            next.auto_pull_interval_secs = (value > 0).then_some(value);
        }
        next.normalized()
    }
}

/// Читает настройки синхронизации vault. Нет файла или ключа — умолчания.
/// Неразбираемый файл — тоже умолчания: синхронизация не повод отказывать в
/// работе с vault, а сам файл здесь не перезаписывается (запись правит ключ
/// поверх того, что разобралось).
pub async fn read<B: VaultBackend>(backend: &B) -> VaultSyncSettings {
    match backend.read(&service_dir::vault_settings_path()).await {
        Ok(text) => parse(&text),
        Err(ReadError::NotFound | ReadError::Io | ReadError::TooLarge) => {
            VaultSyncSettings::default()
        }
    }
}

pub fn parse(text: &str) -> VaultSyncSettings {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| value.get(KEY).cloned())
        .and_then(|value| serde_json::from_value::<VaultSyncSettings>(value).ok())
        .unwrap_or_default()
        .normalized()
}

/// Подставляет ключ `sync` в текст файла, сохраняя остальные ключи.
pub fn render(previous: &str, settings: &VaultSyncSettings) -> Option<String> {
    let mut document = match serde_json::from_str::<Value>(previous) {
        Ok(Value::Object(map)) => map,
        // Пустой файл или его нет — начинаем с чистого.
        _ if previous.trim().is_empty() => Map::new(),
        // Файл есть, но не разбирается: перезаписать его значило бы потерять
        // то, что в нём написал человек (006 FR-034).
        _ => return None,
    };
    let value = serde_json::to_value(settings).ok()?;
    if *settings == VaultSyncSettings::default() {
        document.remove(KEY);
    } else {
        document.insert(KEY.to_string(), value);
    }
    let mut text = serde_json::to_string_pretty(&Value::Object(document)).ok()?;
    text.push('\n');
    Some(text)
}

pub async fn write<B: VaultBackend>(
    backend: &mut B,
    settings: &VaultSyncSettings,
) -> Result<(), SyncError> {
    crate::vault::vault_settings::ensure_service_dir(backend)
        .await
        .map_err(|error| SyncError::Io {
            message: error.to_string(),
        })?;
    let path = service_dir::vault_settings_path();
    let previous = match backend.read(&path).await {
        Ok(text) => text,
        Err(ReadError::NotFound) => String::new(),
        Err(error) => {
            return Err(SyncError::Io {
                message: error.to_string(),
            })
        }
    };
    let text = render(&previous, settings).ok_or_else(|| SyncError::Io {
        message: "vault.json не разбирается".to_string(),
    })?;
    backend
        .write(&path, &text, None)
        .await
        .map_err(|error| SyncError::Io {
            message: error.to_string(),
        })
}

/// Адреса удалённых репозиториев (FR-013, FR-014, FR-014a).
pub mod remote_url {
    use serde::{Deserialize, Serialize};

    /// Схема доступа после нормализации.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
    #[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
    pub enum Scheme {
        Https,
        /// `http://` — только для адресов на этом устройстве.
        HttpLoopback,
        Ssh,
        /// Каталог на этом устройстве (`file://` или путь) — для проверки и
        /// для удалённого на сменном диске; только native.
        File,
    }

    /// Разобранный адрес.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
    #[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct RemoteUrl {
        /// Адрес без учётных данных — то, что пишется в настройки.
        pub url: String,
        pub scheme: Scheme,
        pub host: Option<String>,
        /// Имя пользователя, найденное в адресе (`https://user@host/…`).
        pub user: Option<String>,
        /// Токен или пароль, найденный в адресе. Переносится в хранилище
        /// секретов и **не сохраняется** в адресе (research §10).
        pub secret: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum UrlProblem {
        Empty,
        Unsupported(String),
        Malformed,
    }

    /// Разбирает и нормализует адрес. `user@host:path` становится
    /// `ssh://user@host/path`: libgit2 без встроенного ssh понимает только
    /// форму с схемой (research §2).
    pub fn normalize(raw: &str) -> Result<RemoteUrl, UrlProblem> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(UrlProblem::Empty);
        }
        if raw.chars().any(char::is_whitespace) {
            return Err(UrlProblem::Malformed);
        }
        if let Some((scheme, rest)) = raw.split_once("://") {
            let scheme_lower = scheme.to_ascii_lowercase();
            if scheme_lower == "file" {
                if rest.is_empty() {
                    return Err(UrlProblem::Malformed);
                }
                return Ok(RemoteUrl {
                    url: format!("file://{rest}"),
                    scheme: Scheme::File,
                    host: None,
                    user: None,
                    secret: None,
                });
            }
            // Знаки, по которым разные разборщики адресов расходятся в том, где
            // кончается хост (`http://evil#@127.0.0.1/`): здесь адрес был бы
            // «этим устройством», а HTTP-клиент ушёл бы на чужой сервер (T068).
            if rest.contains(['#', '?', '\\']) {
                return Err(UrlProblem::Malformed);
            }
            let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
            let (userinfo, hostport) = match authority.rsplit_once('@') {
                Some((info, host)) => (Some(info), host),
                None => (None, authority),
            };
            let host = host_of_hostport(hostport).ok_or(UrlProblem::Malformed)?;
            let (user, secret) = match userinfo {
                Some(info) => match info.split_once(':') {
                    Some((user, secret)) => (
                        (!user.is_empty()).then(|| user.to_string()),
                        (!secret.is_empty()).then(|| secret.to_string()),
                    ),
                    None => ((!info.is_empty()).then(|| info.to_string()), None),
                },
                None => (None, None),
            };
            let scheme = match scheme_lower.as_str() {
                "https" => Scheme::Https,
                "http" if is_loopback(&host) => Scheme::HttpLoopback,
                "ssh" | "git+ssh" | "ssh+git" => Scheme::Ssh,
                other => return Err(UrlProblem::Unsupported(other.to_string())),
            };
            let scheme_text = match scheme {
                Scheme::Https => "https",
                Scheme::HttpLoopback => "http",
                Scheme::Ssh => "ssh",
                Scheme::File => unreachable!(),
            };
            // В адрес возвращается только имя пользователя ssh: для https оно
            // уходит в хранилище секретов вместе с токеном.
            let kept_user = match (scheme, &user) {
                (Scheme::Ssh, Some(user)) => format!("{user}@"),
                _ => String::new(),
            };
            let url = format!("{scheme_text}://{kept_user}{hostport}/{path}");
            return Ok(RemoteUrl {
                url,
                scheme,
                host: Some(host),
                user,
                secret: if scheme == Scheme::Ssh { None } else { secret },
            });
        }
        // Путь на диске: абсолютный (`/…`, `C:\…`) — локальный удалённый.
        if raw.starts_with('/') || raw.get(1..3) == Some(":\\") {
            return Ok(RemoteUrl {
                url: raw.to_string(),
                scheme: Scheme::File,
                host: None,
                user: None,
                secret: None,
            });
        }
        // scp-подобная форма `user@host:path`.
        if let Some((authority, path)) = raw.split_once(':') {
            // `@` в пути — это вставленный без схемы `user:токен@host/…`: принять
            // его за scp-форму значило бы записать токен в адрес (T068).
            if path.is_empty() || authority.contains('/') || path.contains('@') {
                return Err(UrlProblem::Malformed);
            }
            let (user, host) = match authority.rsplit_once('@') {
                Some((user, host)) => (Some(user.to_string()), host.to_string()),
                None => (None, authority.to_string()),
            };
            if host_of_hostport(&host).as_deref() != Some(host.to_ascii_lowercase().as_str()) {
                return Err(UrlProblem::Malformed);
            }
            // Относительный путь scp — от домашнего каталога на сервере.
            let path = if path.starts_with('/') {
                path.to_string()
            } else {
                format!("/~/{path}")
            };
            let path = path
                .strip_prefix("/~/")
                .filter(|_| looks_like_forge(&host))
                .map(|rest| format!("/{rest}"))
                .unwrap_or(path);
            let user_part = user
                .as_ref()
                .map(|user| format!("{user}@"))
                .unwrap_or_default();
            return Ok(RemoteUrl {
                url: format!("ssh://{user_part}{host}{path}"),
                scheme: Scheme::Ssh,
                host: Some(host.to_ascii_lowercase()),
                user,
                secret: None,
            });
        }
        Err(UrlProblem::Malformed)
    }

    /// Хостинги, где путь scp-формы относителен корня, а не домашнего каталога.
    fn looks_like_forge(host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        host == "github.com"
            || host == "gitlab.com"
            || host == "bitbucket.org"
            || host == "codeberg.org"
            || host.starts_with("gitlab.")
            || host.starts_with("git.")
    }

    /// Годится ли адрес посредником обмена из браузера (FR-037): `https://…`
    /// либо `http://` на этом устройстве, без учётных данных. Через
    /// посредника идут токен и заметки, поэтому открытый `http` в сеть и
    /// адрес с вписанным паролем посредником не становятся.
    pub fn is_proxy(raw: &str) -> bool {
        matches!(
            normalize(raw),
            Ok(RemoteUrl {
                scheme: Scheme::Https | Scheme::HttpLoopback,
                user: None,
                secret: None,
                ..
            })
        )
    }

    /// Хост из `хост[:порт]` в нижнем регистре; `None` — это не хост: в имени
    /// допустимы только буквы, цифры, точка и дефис (или адрес IPv6 в скобках),
    /// порт — число. Строгость намеренная: всё прочее (`@`, `/`, `#`) — способ
    /// выдать чужой адрес за свой.
    pub fn host_of_hostport(hostport: &str) -> Option<String> {
        let port_ok = |port: &str| port.parse::<u16>().is_ok();
        if let Some(rest) = hostport.strip_prefix('[') {
            let (address, tail) = rest.split_once(']')?;
            address.parse::<std::net::Ipv6Addr>().ok()?;
            let tail_ok = tail.is_empty() || tail.strip_prefix(':').is_some_and(port_ok);
            return tail_ok.then(|| format!("[{}]", address.to_ascii_lowercase()));
        }
        let (host, port) = match hostport.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (hostport, None),
        };
        let name_ok = !host.is_empty()
            && !host.starts_with('-')
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
        (name_ok && port.is_none_or(port_ok)).then(|| host.to_ascii_lowercase())
    }

    /// Адрес этого устройства: `localhost` или loopback-адрес IP. Не «всё, что
    /// начинается с 127.» — `127.evil.com` — чужой сервер (T068).
    pub fn is_loopback(host: &str) -> bool {
        let host = host.trim_start_matches('[').trim_end_matches(']');
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    }

    /// Имя пользователя по умолчанию для токена (research §2): хостинги
    /// принимают любое непустое имя, но некоторые — только своё.
    pub fn token_user(host: Option<&str>) -> &'static str {
        match host {
            Some(host) if host.contains("gitlab") => "oauth2",
            _ => "x-access-token",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::remote_url::{normalize, Scheme};
    use super::*;

    #[test]
    fn branch_names_that_would_bend_the_refspec_are_dropped() {
        for good in ["main", "master", "feature/заметки", "v1.2"] {
            assert!(valid_branch(good), "{good}");
        }
        for bad in [
            "",
            "*",
            "main:other",
            "a..b",
            "-x",
            "x.lock",
            "a b",
            "x/",
            "@{u}",
        ] {
            assert!(!valid_branch(bad), "{bad}");
        }
    }

    #[test]
    fn credentials_are_stripped_from_https() {
        let parsed = normalize("https://me:ghp_secret@github.com/me/notes.git").unwrap();
        assert_eq!(parsed.url, "https://github.com/me/notes.git");
        assert_eq!(parsed.user.as_deref(), Some("me"));
        assert_eq!(parsed.secret.as_deref(), Some("ghp_secret"));
        assert_eq!(parsed.scheme, Scheme::Https);
    }

    #[test]
    fn scp_form_becomes_ssh_url() {
        let parsed = normalize("git@github.com:me/notes.git").unwrap();
        assert_eq!(parsed.url, "ssh://git@github.com/me/notes.git");
        let home = normalize("me@box:notes.git").unwrap();
        assert_eq!(home.url, "ssh://me@box/~/notes.git");
    }

    #[test]
    fn a_proxy_is_https_or_local_and_carries_no_credentials() {
        use super::remote_url::is_proxy;
        assert!(is_proxy("https://proxy.example.org"));
        assert!(is_proxy("http://127.0.0.1:9999/git"));
        assert!(!is_proxy("http://proxy.example.org"));
        assert!(!is_proxy("https://user:secret@proxy.example.org"));
        assert!(!is_proxy("ssh://proxy.example.org/x"));
        assert!(!is_proxy("не адрес"));
    }

    #[test]
    fn lookalike_hosts_are_not_this_device() {
        use super::remote_url::is_loopback;
        assert!(is_loopback("127.0.0.1") && is_loopback("127.8.8.8") && is_loopback("::1"));
        assert!(is_loopback("[::1]") && is_loopback("LocalHost"));
        for host in [
            "127.evil.com",
            "localhost.evil.com",
            "0.0.0.0",
            "127.0.0.1.evil.com",
        ] {
            assert!(!is_loopback(host), "{host}");
        }
        // Открытый http — только этому устройству, как бы адрес ни выглядел.
        for url in [
            "http://127.evil.com/r.git",
            "http://evil.com#@127.0.0.1/r.git",
            "http://evil.com?@127.0.0.1/r.git",
            "http://127.0.0.1:80@evil.com:8384/r.git",
        ] {
            assert!(normalize(url).is_err(), "{url}");
        }
        assert_eq!(
            normalize("http://[::1]:8080/r.git").unwrap().scheme,
            Scheme::HttpLoopback
        );
        // Вставленный без схемы адрес с токеном — не scp-форма.
        assert!(normalize("user:ghp_secret@github.com/me/notes.git").is_err());
        // Адрес из файла настроек приводится к каноническому, без учётных данных.
        let settings = VaultSyncSettings {
            remote_url: Some("https://user:secret@Example.com/me/notes.git".into()),
            ..VaultSyncSettings::default()
        }
        .normalized();
        assert_eq!(
            settings.remote_url.as_deref(),
            Some("https://Example.com/me/notes.git")
        );
    }

    #[test]
    fn plain_http_is_only_for_loopback() {
        assert_eq!(
            normalize("http://127.0.0.1:8080/r.git").unwrap().scheme,
            Scheme::HttpLoopback
        );
        assert!(normalize("http://example.com/r.git").is_err());
    }

    #[test]
    fn settings_round_trip_keeps_other_keys() {
        let previous = r#"{"assets": {"placement": "vault-root"}, "мой": 1}"#;
        let settings = VaultSyncSettings {
            remote_url: Some("https://github.com/me/notes.git".into()),
            ..VaultSyncSettings::default()
        };
        let text = render(previous, &settings).unwrap();
        assert!(text.contains("\"мой\""));
        assert!(text.contains("vault-root"));
        assert_eq!(parse(&text), settings);
        let cleared = render(&text, &VaultSyncSettings::default()).unwrap();
        assert!(!cleared.contains("\"sync\""));
    }

    #[test]
    fn broken_file_is_not_overwritten() {
        assert!(render("{не json", &VaultSyncSettings::default()).is_none());
    }

    #[test]
    fn out_of_range_pause_is_clamped() {
        let parsed = parse(r#"{"sync": {"auto_snapshot_pause_secs": 1}}"#);
        assert_eq!(parsed.auto_snapshot_pause_secs, MIN_PAUSE_SECS);
    }
}
