//! Какой адрес куда ведёт (FR-009, FR-010).

use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::settings::{AiProvider, AiSettings};
use super::AiFailure;

pub const ANTHROPIC_BASE: &str = "https://api.anthropic.com/v1";
pub const OPENAI_BASE: &str = "https://api.openai.com/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum EndpointClass {
    /// Зашифровано либо не покидает устройство.
    Safe,
    /// Открытым текстом по сети: нужно отдельное подтверждение.
    NeedsConfirmation,
    Invalid,
}

pub fn classify_endpoint(endpoint: &str) -> EndpointClass {
    let Ok(url) = Url::parse(endpoint.trim()) else {
        return EndpointClass::Invalid;
    };
    // Секрет в адресе попал бы в журнал отправленного: такой адрес не принимается.
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return EndpointClass::Invalid;
    }
    let Some(host) = url.host_str() else {
        return EndpointClass::Invalid;
    };
    match url.scheme() {
        "https" => EndpointClass::Safe,
        "http" if is_loopback(host) => EndpointClass::Safe,
        "http" => EndpointClass::NeedsConfirmation,
        _ => EndpointClass::Invalid,
    }
}

fn is_loopback(host: &str) -> bool {
    host == "localhost"
        || host == "[::1]"
        || host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Базовый адрес текущего подключения или причина, почему туда нельзя.
///
/// `allow_custom` — решение оболочки: сервер пускает свой адрес только с разрешения
/// администратора, иначе пользователь мог бы слать запросы во внутреннюю сеть
/// сервера (FR-019).
pub fn base_url(settings: &AiSettings, allow_custom: bool) -> Result<String, AiFailure> {
    match settings.provider {
        AiProvider::Anthropic => Ok(ANTHROPIC_BASE.to_owned()),
        AiProvider::OpenAi => Ok(OPENAI_BASE.to_owned()),
        AiProvider::Custom => {
            if !allow_custom {
                return Err(AiFailure::CustomEndpointDisabled);
            }
            let endpoint = settings
                .custom_endpoint
                .as_deref()
                .ok_or(AiFailure::BadEndpoint)?;
            match classify_endpoint(endpoint) {
                EndpointClass::Safe => {}
                EndpointClass::Invalid => return Err(AiFailure::BadEndpoint),
                EndpointClass::NeedsConfirmation => {
                    if settings.insecure_confirmed_for.as_deref() != Some(endpoint) {
                        return Err(AiFailure::InsecureNotConfirmed);
                    }
                }
            }
            Ok(endpoint.trim().trim_end_matches('/').to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_addresses() {
        use EndpointClass::*;
        assert_eq!(classify_endpoint("https://models.example.org/v1"), Safe);
        assert_eq!(classify_endpoint("http://127.0.0.1:11434/v1"), Safe);
        assert_eq!(classify_endpoint("http://localhost:8080"), Safe);
        assert_eq!(classify_endpoint("http://[::1]:8080"), Safe);
        assert_eq!(
            classify_endpoint("http://192.168.1.20/v1"),
            NeedsConfirmation
        );
        assert_eq!(
            classify_endpoint("http://localhost.evil.org/v1"),
            NeedsConfirmation
        );
        assert_eq!(classify_endpoint("ftp://example.org"), Invalid);
        assert_eq!(classify_endpoint("https://u:p@example.org"), Invalid);
        assert_eq!(classify_endpoint("https://example.org/?key=1"), Invalid);
        assert_eq!(classify_endpoint("не адрес"), Invalid);
    }
}
