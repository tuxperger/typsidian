//! Syncthing глазами Typsidian (фича 011, research §8, FR-021…FR-023).
//!
//! Только чтение (поправка конституции 1.2.0): приложение узнаёт, что папка
//! доставляется Syncthing, и — с разрешения пользователя — спрашивает у
//! запущенного Syncthing состояние этой папки через его локальный API. Ничего
//! в Syncthing не меняет, не запускает и не останавливает его.
//!
//! Разбор ответов и конфигурации — чистые функции, одинаковые на обоих
//! таргетах; сам сетевой запрос есть только у native: в браузере GUI Syncthing
//! не отвечает на обращения с чужого адреса (CORS), и это честное
//! «недоступно здесь» (FR-036), а не ошибка.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Маркер папки Syncthing в корне (FR-021).
pub const FOLDER_MARKER: &str = ".stfolder";

/// Где слушает API Syncthing. Ключ доступа — в хранилище секретов.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SyncthingEndpoint {
    /// `host:port`, только адрес этого устройства.
    pub address: String,
    pub tls: bool,
    /// Отпечаток сертификата GUI, закреплённый при первом обращении (research
    /// §8): сертификат самоподписанный, проверить его нечем, и без закрепления
    /// ключ API ушёл бы любому процессу, занявшему этот порт. `None` — ещё
    /// не закреплён либо адрес без TLS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub cert_fingerprint: Option<String>,
}

impl SyncthingEndpoint {
    pub fn base_url(&self) -> String {
        let scheme = if self.tls { "https" } else { "http" };
        format!("{scheme}://{}", self.address)
    }

    /// Адрес на этом устройстве. Чужие адреса не принимаются: ключ API даёт
    /// полный доступ к Syncthing, и отправлять его в сеть приложение не будет.
    pub fn is_loopback(&self) -> bool {
        // Адрес — строго `хост:порт`: всё, что разборщик HTTP-клиента мог бы
        // прочесть иначе (`127.0.0.1:80@чужой:8384`), адресом не считается.
        super::settings::remote_url::host_of_hostport(&self.address).is_some_and(|host| {
            super::settings::remote_url::is_loopback(&host) || host == "0.0.0.0"
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum LinkState {
    /// Пользователь не подключал чтение состояния.
    NotConfigured,
    /// Syncthing не отвечает по указанному адресу.
    Unreachable,
    /// Ключ API не подошёл.
    Unauthorized,
    /// В этой оболочке состояние недоступно (FR-036).
    UnavailableInShell,
    /// Syncthing отвечает, но этой папки в нём нет.
    FolderNotShared,
    /// Сертификат по этому адресу не тот, что был закреплён: ключ API туда не
    /// отправляется, пока пользователь не подключит Syncthing заново.
    CertificateChanged,
    Connected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum FolderState {
    Idle,
    Scanning,
    Syncing,
    Paused,
    Error,
}

/// Устройство Syncthing — для подписи конфликтных копий (research §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SyncthingDevice {
    /// Первые семь знаков идентификатора — как в имени конфликтной копии.
    pub short_id: String,
    pub name: String,
    pub connected: bool,
}

/// Папка vault в Syncthing (data-model «SyncthingFolder»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SyncthingFolder {
    /// В корне vault есть маркер папки Syncthing.
    pub detected: bool,
    pub link: LinkState,
    pub folder_id: Option<String>,
    pub label: Option<String>,
    pub state: Option<FolderState>,
    /// Доля уже доставленного, проценты.
    pub progress: Option<u8>,
    pub error: Option<String>,
    /// Устройства, с которыми папка общая (кроме этого).
    pub devices: Vec<SyncthingDevice>,
}

impl SyncthingFolder {
    pub fn unlinked(detected: bool, link: LinkState) -> Self {
        Self {
            detected,
            link,
            folder_id: None,
            label: None,
            state: None,
            progress: None,
            error: None,
            devices: Vec::new(),
        }
    }

    pub fn connected_devices(&self) -> u32 {
        self.devices
            .iter()
            .filter(|device| device.connected)
            .count() as u32
    }

    /// Имя устройства по короткому идентификатору из имени конфликтной копии.
    pub fn device_name(&self, short_id: &str) -> Option<&str> {
        self.devices
            .iter()
            .find(|device| device.short_id == short_id)
            .map(|device| device.name.as_str())
    }
}

/// Параметры GUI из `config.xml` Syncthing (FR-022a): адрес, TLS и ключ API.
pub fn parse_gui_config(xml: &str) -> Option<(SyncthingEndpoint, String)> {
    let gui_start = xml.find("<gui")?;
    let gui_end = xml[gui_start..].find("</gui>")? + gui_start;
    let gui = &xml[gui_start..gui_end];
    let open_tag_end = gui.find('>')?;
    let tls = gui[..open_tag_end].contains("tls=\"true\"");
    let address = element_text(gui, "address")?;
    let apikey = element_text(gui, "apikey")?;
    if address.is_empty() || apikey.is_empty() {
        return None;
    }
    // `0.0.0.0` в конфиге означает «слушать везде», а обращаться надо к себе.
    let address = address.replace("0.0.0.0", "127.0.0.1");
    Some((
        SyncthingEndpoint {
            address,
            tls,
            cert_fingerprint: None,
        },
        apikey,
    ))
}

fn element_text(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}

/// Ищет в `/rest/config/folders` папку с путём корня vault.
///
/// `home` — домашний каталог: Syncthing хранит пути с `~`.
pub fn find_folder(folders: &Value, vault_root: &str, home: Option<&str>) -> Option<FolderRef> {
    let wanted = normalize_path(vault_root, home);
    folders.as_array()?.iter().find_map(|folder| {
        let path = folder.get("path")?.as_str()?;
        if normalize_path(path, home) != wanted {
            return None;
        }
        Some(FolderRef {
            id: folder.get("id")?.as_str()?.to_string(),
            label: folder
                .get("label")
                .and_then(Value::as_str)
                .map(str::to_string),
            paused: folder
                .get("paused")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            device_ids: folder
                .get("devices")
                .and_then(Value::as_array)
                .map(|devices| {
                    devices
                        .iter()
                        .filter_map(|device| device.get("deviceID")?.as_str())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
        })
    })
}

/// Папка из конфигурации Syncthing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRef {
    pub id: String,
    pub label: Option<String>,
    pub paused: bool,
    pub device_ids: Vec<String>,
}

fn normalize_path(path: &str, home: Option<&str>) -> String {
    let expanded = match (path.strip_prefix('~'), home) {
        (Some(rest), Some(home)) => format!("{home}{rest}"),
        _ => path.to_string(),
    };
    let trimmed = expanded.replace('\\', "/");
    trimmed.trim_end_matches('/').to_string()
}

/// Состояние папки из `/rest/db/status`.
pub fn folder_state(db_status: &Value, paused: bool) -> (FolderState, Option<u8>, Option<String>) {
    if paused {
        return (FolderState::Paused, None, None);
    }
    let state = db_status.get("state").and_then(Value::as_str).unwrap_or("");
    let error = db_status
        .get("error")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string);
    let global = db_status
        .get("globalBytes")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let need = db_status
        .get("needBytes")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let progress = if global == 0 {
        100
    } else {
        (((global.saturating_sub(need)) * 100) / global).min(100) as u8
    };
    let folder_state = match state {
        "idle" if error.is_none() => FolderState::Idle,
        "scanning" | "scan-waiting" | "cleaning" | "clean-waiting" => FolderState::Scanning,
        "syncing" | "sync-waiting" | "sync-preparing" => FolderState::Syncing,
        "error" => FolderState::Error,
        _ if error.is_some() => FolderState::Error,
        _ => FolderState::Idle,
    };
    (folder_state, Some(progress), error)
}

/// Устройства папки с именами и подключённостью из `/rest/config/devices` и
/// `/rest/system/connections`. `my_id` — идентификатор этого Syncthing.
pub fn devices(
    folder: &FolderRef,
    config_devices: &Value,
    connections: &Value,
    my_id: Option<&str>,
) -> Vec<SyncthingDevice> {
    folder
        .device_ids
        .iter()
        .filter(|id| Some(id.as_str()) != my_id)
        .map(|id| {
            let name = config_devices
                .as_array()
                .and_then(|devices| {
                    devices.iter().find(|device| {
                        device.get("deviceID").and_then(Value::as_str) == Some(id.as_str())
                    })
                })
                .and_then(|device| device.get("name").and_then(Value::as_str))
                .filter(|name| !name.is_empty())
                .unwrap_or(id)
                .to_string();
            let connected = connections
                .get("connections")
                .and_then(|map| map.get(id))
                .and_then(|connection| connection.get("connected"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            SyncthingDevice {
                short_id: id.chars().take(7).collect(),
                name,
                connected,
            }
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{discover, pin, query, wait_events};

/// Номер последнего события в ответе `/rest/events`.
pub fn last_event_id(events: &Value) -> Option<u64> {
    events
        .as_array()?
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_u64))
        .max()
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    /// Обычный запрос к локальному Syncthing: он либо отвечает сразу, либо не
    /// запущен (принцип VII).
    const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
    /// Ожидание событий: Syncthing держит запрос до 60 с, если ничего не происходит.
    const EVENTS_WAIT_SECS: u64 = 60;

    /// Где искать `config.xml` Syncthing этого пользователя (research §8).
    fn config_candidates() -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        if let Ok(state) = std::env::var("XDG_STATE_HOME") {
            candidates.push(PathBuf::from(state).join("syncthing/config.xml"));
        }
        if let Ok(config) = std::env::var("XDG_CONFIG_HOME") {
            candidates.push(PathBuf::from(config).join("syncthing/config.xml"));
        }
        if let Ok(home) = std::env::var("HOME") {
            let home = PathBuf::from(home);
            candidates.push(home.join(".local/state/syncthing/config.xml"));
            candidates.push(home.join(".config/syncthing/config.xml"));
            candidates.push(home.join("Library/Application Support/Syncthing/config.xml"));
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            candidates.push(PathBuf::from(local).join("Syncthing/config.xml"));
        }
        candidates
    }

    /// Находит параметры локального Syncthing (FR-022a). Ничего не сохраняет:
    /// подключение — отдельным подтверждением пользователя.
    pub fn discover() -> Option<(SyncthingEndpoint, String)> {
        config_candidates()
            .into_iter()
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .find_map(|xml| parse_gui_config(&xml))
            .filter(|(endpoint, _)| endpoint.is_loopback())
    }

    async fn get(
        client: &reqwest::Client,
        endpoint: &SyncthingEndpoint,
        api_key: &str,
        path: &str,
    ) -> Result<Value, LinkState> {
        let response = client
            .get(format!("{}{path}", endpoint.base_url()))
            .header("X-API-Key", api_key)
            .send()
            .await
            .map_err(|_| LinkState::Unreachable)?;
        match response.status().as_u16() {
            200 => {
                let text = response.text().await.map_err(|_| LinkState::Unreachable)?;
                serde_json::from_str(&text).map_err(|_| LinkState::Unreachable)
            }
            401 | 403 => Err(LinkState::Unauthorized),
            _ => Err(LinkState::Unreachable),
        }
    }

    /// Клиент для локального Syncthing. Короткие тайм-ауты: он на этом же
    /// устройстве и либо отвечает сразу, либо не запущен (принцип VII).
    ///
    /// Сертификат GUI самоподписанный (Android, `tls="true"`), проверить его по
    /// цепочке нельзя — вместо этого он сверяется с закреплённым отпечатком
    /// (см. [`peer_fingerprint`]), и только для адреса этого устройства.
    fn client(endpoint: &SyncthingEndpoint, timeout: Duration) -> Option<reqwest::Client> {
        let mut builder = reqwest::Client::builder()
            // Перенаправление увело бы запрос с ключом API на другой адрес.
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(timeout);
        if endpoint.tls {
            builder = builder.danger_accept_invalid_certs(true).tls_info(true);
        }
        builder.build().ok()
    }

    /// Отпечаток сертификата — тот же, что закрепляется при первом обращении.
    pub(super) fn certificate_fingerprint(certificate: &[u8]) -> String {
        blake3::hash(certificate).to_hex().to_string()
    }

    /// Проверка сертификата внутри TLS-рукопожатия (T076): сервер обязан
    /// предъявить ровно закреплённый сертификат, иначе соединение рвётся до
    /// первого байта запроса — ключ API не уходит никому другому. Подпись
    /// рукопожатия по-прежнему проверяется: предъявить чужой сертификат, не
    /// владея его ключом, нельзя.
    #[derive(Debug)]
    pub(super) struct PinnedCertificate {
        pinned: String,
        provider: std::sync::Arc<rustls::crypto::CryptoProvider>,
    }

    impl PinnedCertificate {
        pub(super) fn new(pinned: &str) -> Self {
            Self {
                pinned: pinned.to_owned(),
                provider: std::sync::Arc::new(rustls::crypto::ring::default_provider()),
            }
        }
    }

    impl rustls::client::danger::ServerCertVerifier for PinnedCertificate {
        fn verify_server_cert(
            &self,
            end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            if certificate_fingerprint(end_entity.as_ref()) == self.pinned {
                Ok(rustls::client::danger::ServerCertVerified::assertion())
            } else {
                Err(rustls::Error::InvalidCertificate(
                    rustls::CertificateError::ApplicationVerificationFailure,
                ))
            }
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &rustls::pki_types::CertificateDer<'_>,
            dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls12_signature(
                message,
                cert,
                dss,
                &self.provider.signature_verification_algorithms,
            )
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &rustls::pki_types::CertificateDer<'_>,
            dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls13_signature(
                message,
                cert,
                dss,
                &self.provider.signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            self.provider
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    /// Клиент, который говорит только с сервером закреплённого сертификата.
    pub(super) fn pinned_client(pinned: &str, timeout: Duration) -> Option<reqwest::Client> {
        let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .ok()?
            .dangerous()
            .with_custom_certificate_verifier(std::sync::Arc::new(PinnedCertificate::new(pinned)))
            .with_no_client_auth();
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(timeout)
            .use_preconfigured_tls(config)
            .build()
            .ok()
    }

    /// Отпечаток сертификата, который предъявляет адрес. Запрос идёт **без
    /// ключа API** — на точку `noauth`, которая отвечает всем: узнать, тот ли
    /// это Syncthing, нужно до того, как ему что-то доверить.
    async fn peer_fingerprint(
        client: &reqwest::Client,
        endpoint: &SyncthingEndpoint,
    ) -> Option<String> {
        let response = client
            .get(format!("{}/rest/noauth/health", endpoint.base_url()))
            .send()
            .await
            .ok()?;
        let info = response.extensions().get::<reqwest::tls::TlsInfo>()?;
        let certificate = info.peer_certificate()?;
        Some(certificate_fingerprint(certificate))
    }

    /// Отпечаток для закрепления при первом обращении (TOFU). `None` — адрес
    /// без TLS (закреплять нечего) или Syncthing сейчас не отвечает.
    pub async fn pin(endpoint: &SyncthingEndpoint) -> Option<String> {
        if !endpoint.tls || !endpoint.is_loopback() {
            return None;
        }
        peer_fingerprint(&client(endpoint, REQUEST_TIMEOUT)?, endpoint).await
    }

    /// Ждёт событий папок и устройств (long-poll `/rest/events`, FR-022,
    /// research §8): возвращает номер, с которого ждать дальше. Ответ приходит,
    /// как только что-то случилось, или через минуту ожидания. Тот же порядок
    /// доверия, что у [`query`]: при TLS ключ уходит только закреплённому
    /// сертификату.
    pub async fn wait_events(
        endpoint: &SyncthingEndpoint,
        api_key: &str,
        since: u64,
    ) -> Result<u64, LinkState> {
        if !endpoint.is_loopback() {
            return Err(LinkState::Unreachable);
        }
        let timeout = Duration::from_secs(EVENTS_WAIT_SECS + 15);
        let client = if endpoint.tls {
            let pinned = endpoint
                .cert_fingerprint
                .as_deref()
                .ok_or(LinkState::Unreachable)?;
            pinned_client(pinned, timeout)
        } else {
            client(endpoint, timeout)
        }
        .ok_or(LinkState::Unreachable)?;
        let events = get(
            &client,
            endpoint,
            api_key,
            &format!(
                "/rest/events?events=FolderSummary,StateChanged,DeviceConnected,DeviceDisconnected&since={since}&timeout={EVENTS_WAIT_SECS}"
            ),
        )
        .await?;
        Ok(last_event_id(&events).unwrap_or(since))
    }

    /// Состояние папки vault.
    pub async fn query(
        endpoint: &SyncthingEndpoint,
        api_key: &str,
        vault_root: &str,
        detected: bool,
    ) -> SyncthingFolder {
        if !endpoint.is_loopback() {
            return SyncthingFolder::unlinked(detected, LinkState::Unreachable);
        }
        // Ключ API уходит только тому, кто предъявил закреплённый сертификат, —
        // проверка внутри рукопожатия того же соединения, по которому идёт ключ
        // (T076). Без закрепления (ещё не было первого обращения) ключ не уходит
        // вовсе: закрепляет вызывающая сторона через [`pin`].
        let pinned = if endpoint.tls {
            let Some(pinned) = endpoint.cert_fingerprint.as_deref() else {
                return SyncthingFolder::unlinked(detected, LinkState::Unreachable);
            };
            Some(pinned)
        } else {
            None
        };
        let client = match pinned {
            Some(pinned) => pinned_client(pinned, REQUEST_TIMEOUT),
            None => client(endpoint, REQUEST_TIMEOUT),
        };
        let Some(client) = client else {
            return SyncthingFolder::unlinked(detected, LinkState::Unreachable);
        };
        let result: Result<SyncthingFolder, LinkState> = async {
            let status = get(&client, endpoint, api_key, "/rest/system/status").await?;
            let my_id = status
                .get("myID")
                .and_then(Value::as_str)
                .map(str::to_string);
            let folders = get(&client, endpoint, api_key, "/rest/config/folders").await?;
            let home = std::env::var("HOME").ok();
            let Some(folder) = find_folder(&folders, vault_root, home.as_deref()) else {
                return Err(LinkState::FolderNotShared);
            };
            let db = get(
                &client,
                endpoint,
                api_key,
                &format!("/rest/db/status?folder={}", url_encode(&folder.id)),
            )
            .await?;
            let config_devices = get(&client, endpoint, api_key, "/rest/config/devices")
                .await
                .unwrap_or(Value::Null);
            let connections = get(&client, endpoint, api_key, "/rest/system/connections")
                .await
                .unwrap_or(Value::Null);
            let (state, progress, error) = folder_state(&db, folder.paused);
            Ok(SyncthingFolder {
                detected,
                link: LinkState::Connected,
                folder_id: Some(folder.id.clone()),
                label: folder.label.clone(),
                state: Some(state),
                progress,
                error,
                devices: devices(&folder, &config_devices, &connections, my_id.as_deref()),
            })
        }
        .await;
        // Соединение не сложилось: сменился ли сертификат, выясняется запросом
        // без ключа на точку `noauth` — только чтобы назвать причину.
        let result = match (result, pinned) {
            (Err(LinkState::Unreachable), Some(pinned)) => {
                let actual = match self::client(endpoint, REQUEST_TIMEOUT) {
                    Some(probe) => peer_fingerprint(&probe, endpoint).await,
                    None => None,
                };
                Err(match actual {
                    Some(actual) if actual != pinned => LinkState::CertificateChanged,
                    _ => LinkState::Unreachable,
                })
            }
            (result, _) => result,
        };
        result.unwrap_or_else(|link| SyncthingFolder::unlinked(detected, link))
    }

    fn url_encode(value: &str) -> String {
        value
            .bytes()
            .map(|byte| match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (byte as char).to_string()
                }
                other => format!("%{other:02X}"),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_last_event_id_is_the_largest() {
        assert_eq!(
            last_event_id(&json!([{"id": 4}, {"id": 9}, {"id": 7}])),
            Some(9)
        );
        assert_eq!(last_event_id(&json!([])), None);
        assert_eq!(last_event_id(&json!({"error": "x"})), None);
    }

    #[test]
    fn gui_config_is_read() {
        let xml = r#"<configuration><gui enabled="true" tls="false" debugging="false">
            <address>127.0.0.1:8384</address>
            <apikey>abc123</apikey>
        </gui></configuration>"#;
        let (endpoint, key) = parse_gui_config(xml).unwrap();
        assert_eq!(endpoint.address, "127.0.0.1:8384");
        assert!(!endpoint.tls);
        assert_eq!(key, "abc123");
        assert!(endpoint.is_loopback());
    }

    #[test]
    fn listen_everywhere_becomes_loopback() {
        let xml = r#"<gui tls="true"><address>0.0.0.0:8384</address><apikey>k</apikey></gui>"#;
        let (endpoint, _) = parse_gui_config(xml).unwrap();
        assert_eq!(endpoint.base_url(), "https://127.0.0.1:8384");
    }

    #[test]
    fn folder_is_found_by_path_with_tilde() {
        let folders = json!([
            {"id": "other", "path": "/data/x", "devices": []},
            {"id": "notes-1", "label": "notes", "path": "~/notes/", "paused": false,
             "devices": [{"deviceID": "AAAAAAA-1"}, {"deviceID": "BBBBBBB-2"}]}
        ]);
        let folder = find_folder(&folders, "/home/u/notes", Some("/home/u")).unwrap();
        assert_eq!(folder.id, "notes-1");
        assert_eq!(folder.device_ids.len(), 2);
    }

    #[test]
    fn progress_and_state_come_from_db_status() {
        let db = json!({"state": "syncing", "globalBytes": 200, "needBytes": 50});
        assert_eq!(
            folder_state(&db, false),
            (FolderState::Syncing, Some(75), None)
        );
        assert_eq!(folder_state(&db, true).0, FolderState::Paused);
        let failed = json!({"state": "idle", "error": "disk full"});
        assert_eq!(folder_state(&failed, false).0, FolderState::Error);
    }

    #[test]
    fn devices_have_names_and_connection_state() {
        let folder = FolderRef {
            id: "f".into(),
            label: None,
            paused: false,
            device_ids: vec!["MEMEMEM-X".into(), "PHONEAB-Y".into()],
        };
        let config = json!([{"deviceID": "PHONEAB-Y", "name": "телефон"}]);
        let connections = json!({"connections": {"PHONEAB-Y": {"connected": true}}});
        let list = devices(&folder, &config, &connections, Some("MEMEMEM-X"));
        assert_eq!(
            list,
            vec![SyncthingDevice {
                short_id: "PHONEAB".into(),
                name: "телефон".into(),
                connected: true,
            }]
        );
    }

    #[test]
    fn lookalike_addresses_are_not_this_device() {
        for address in [
            "127.evil.com:8384",
            "127.0.0.1:80@evil.com:8384",
            "localhost.evil.com:8384",
            "127.0.0.1:8384/../x",
        ] {
            let endpoint = SyncthingEndpoint {
                address: address.into(),
                tls: false,
                cert_fingerprint: None,
            };
            assert!(!endpoint.is_loopback(), "{address}");
        }
        let local = SyncthingEndpoint {
            address: "[::1]:8384".into(),
            tls: true,
            cert_fingerprint: None,
        };
        assert!(local.is_loopback());
    }

    #[test]
    fn foreign_endpoints_are_not_loopback() {
        let endpoint = SyncthingEndpoint {
            address: "192.168.1.5:8384".into(),
            tls: false,
            cert_fingerprint: None,
        };
        assert!(!endpoint.is_loopback());
    }
}

/// Закрепление сертификата внутри рукопожатия (T076) — на настоящем TLS.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod pinning {
    use super::native::{certificate_fingerprint, pinned_client};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    const CERT_A: &[u8] = include_bytes!("testdata/cert-a.der");
    const KEY_A: &[u8] = include_bytes!("testdata/key-a.der");
    const CERT_B: &[u8] = include_bytes!("testdata/cert-b.der");
    const KEY_B: &[u8] = include_bytes!("testdata/key-b.der");

    /// TLS-сервер на свободном порту; запоминает заголовки каждого запроса,
    /// который дошёл до него после рукопожатия.
    fn serve(cert: &[u8], key: &[u8]) -> (u16, Arc<Mutex<Vec<String>>>) {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = Arc::new(
            rustls::ServerConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(
                    vec![rustls::pki_types::CertificateDer::from(cert.to_vec())],
                    rustls::pki_types::PrivateKeyDer::Pkcs8(key.to_vec().into()),
                )
                .unwrap(),
        );
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let connection = rustls::ServerConnection::new(config.clone()).unwrap();
                let mut tls = rustls::StreamOwned::new(connection, stream);
                let mut request = Vec::new();
                let mut buffer = [0u8; 1024];
                // Рукопожатие с чужим сертификатом клиент рвёт — чтение
                // кончается ошибкой, запроса нет.
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    match tls.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => request.extend_from_slice(&buffer[..read]),
                    }
                }
                if request.is_empty() {
                    continue;
                }
                record
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&request).into_owned());
                let _ = tls.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                );
                let _ = tls.flush();
            }
        });
        (port, seen)
    }

    #[tokio::test]
    async fn the_api_key_reaches_only_the_pinned_certificate() {
        let pinned = certificate_fingerprint(CERT_A);

        let (port, seen) = serve(CERT_A, KEY_A);
        let response = pinned_client(&pinned, std::time::Duration::from_secs(5))
            .unwrap()
            .get(format!("https://127.0.0.1:{port}/rest/system/status"))
            .header("X-API-Key", "секрет-a")
            .send()
            .await
            .expect("закреплённый сертификат принят");
        assert_eq!(response.status().as_u16(), 200);
        assert!(seen
            .lock()
            .unwrap()
            .iter()
            .any(|request| request.contains("секрет-a")));

        // Тот же адрес, другой сертификат: соединение рвётся в рукопожатии,
        // и запрос с ключом до сервера не доходит.
        let (port, seen) = serve(CERT_B, KEY_B);
        let refused = pinned_client(&pinned, std::time::Duration::from_secs(5))
            .unwrap()
            .get(format!("https://127.0.0.1:{port}/rest/system/status"))
            .header("X-API-Key", "секрет-b")
            .send()
            .await;
        assert!(refused.is_err());
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(
            seen.lock().unwrap().is_empty(),
            "{:?}",
            seen.lock().unwrap()
        );
    }
}

/// Ожидание событий (T078) — на поддельном Syncthing по HTTP.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod events {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn waiting_returns_the_last_event_id() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(String::new()));
        let record = seen.clone();
        std::thread::spawn(move || {
            if let Some(Ok(mut stream)) = listener.incoming().next() {
                let mut buffer = [0u8; 4096];
                let read = stream.read(&mut buffer).unwrap_or(0);
                *record.lock().unwrap() = String::from_utf8_lossy(&buffer[..read]).into_owned();
                let body =
                    r#"[{"id": 12, "type": "StateChanged"}, {"id": 13, "type": "FolderSummary"}]"#;
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        let endpoint = SyncthingEndpoint {
            address: format!("127.0.0.1:{port}"),
            tls: false,
            cert_fingerprint: None,
        };
        assert_eq!(wait_events(&endpoint, "ключ", 11).await, Ok(13));
        let request = seen.lock().unwrap().clone();
        assert!(request.contains("since=11"), "{request}");
        assert!(
            request.contains("events=FolderSummary,StateChanged"),
            "{request}"
        );
        assert!(request.to_lowercase().contains("x-api-key"), "{request}");
        // Чужой адрес — без запроса.
        let remote = SyncthingEndpoint {
            address: "192.0.2.1:8384".into(),
            tls: false,
            cert_fingerprint: None,
        };
        assert_eq!(
            wait_events(&remote, "ключ", 0).await,
            Err(LinkState::Unreachable)
        );
    }
}
