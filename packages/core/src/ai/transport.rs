//! Шов сети (research.md §1, §4). Native — `reqwest`, wasm — `fetch` браузера с
//! `redirect: "error"`: у `reqwest` под wasm этой настройки нет, а при 307/308
//! браузер повторил бы тело запроса на другой хост.

use serde::{Deserialize, Serialize};

use super::AiFailure;

/// Предел ожидания ответа (SC-005).
pub const TIMEOUT_SECS: u32 = 30;
/// Предел тела ответа: необычно большой ответ — отказ, а не исчерпание памяти.
pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum HttpMethod {
    Get,
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

#[allow(async_fn_in_trait)]
pub trait AiTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiFailure>;
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultTransport;

#[cfg(not(target_arch = "wasm32"))]
impl AiTransport for DefaultTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiFailure> {
        use crate::sync::engine::server_policy;

        // На сервере «это устройство» — сам сервер: свой адрес пользователя не должен
        // вести в его внутреннюю сеть. Имя проверяется после разрешения на каждом
        // соединении (DNS rebinding), адрес числом клиент не разрешает — его здесь.
        // Разрешает внутренние адреса тот же флаг, что и для синхронизации
        // (`sync_private_remotes`; ревью безопасности 050, M2).
        let literal = reqwest::Url::parse(&request.url)
            .map_err(|_| AiFailure::BadEndpoint)?
            .host_str()
            .map(|host| {
                host.trim_start_matches('[')
                    .trim_end_matches(']')
                    .to_owned()
            })
            .and_then(|host| host.parse::<std::net::IpAddr>().ok());
        if literal.is_some_and(server_policy::refuses_ip) {
            return Err(AiFailure::InternalAddress);
        }
        let mut builder = reqwest::Client::builder();
        if server_policy::on_server() {
            builder = builder.dns_resolver(std::sync::Arc::new(server_policy::ServerResolver));
        }
        let client = builder
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(u64::from(TIMEOUT_SECS)))
            .build()
            .map_err(|_| AiFailure::Offline)?;
        let method = match request.method {
            HttpMethod::Get => reqwest::Method::GET,
            HttpMethod::Post => reqwest::Method::POST,
        };
        let mut builder = client.request(method, &request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        let mut response = builder.send().await.map_err(network_failure)?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network_failure)? {
            bytes.extend_from_slice(&chunk);
            if bytes.len() > MAX_RESPONSE_BYTES {
                return Err(AiFailure::BadResponse);
            }
        }
        Ok(HttpResponse {
            status,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn network_failure(error: reqwest::Error) -> AiFailure {
    if error.is_timeout() {
        AiFailure::Timeout
    } else {
        AiFailure::Offline
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultTransport;

#[cfg(target_arch = "wasm32")]
impl AiTransport for DefaultTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiFailure> {
        use wasm_bindgen::{JsCast, JsValue};
        use wasm_bindgen_futures::JsFuture;

        let init = web_sys::RequestInit::new();
        init.set_method(match request.method {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
        });
        init.set_redirect(web_sys::RequestRedirect::Error);
        let headers = web_sys::Headers::new().map_err(|_| AiFailure::BadResponse)?;
        for (name, value) in &request.headers {
            headers
                .set(name, value)
                .map_err(|_| AiFailure::BadResponse)?;
        }
        init.set_headers(&headers);
        if let Some(body) = &request.body {
            init.set_body(&JsValue::from_str(body));
        }
        let signal = web_sys::AbortSignal::timeout_with_u32(TIMEOUT_SECS * 1000);
        init.set_signal(Some(&signal));
        let fetch_request = web_sys::Request::new_with_str_and_init(&request.url, &init)
            .map_err(|_| AiFailure::BadEndpoint)?;

        // Через глобальный объект: приложение может работать и в окне, и в воркере.
        let global = js_sys::global();
        let fetch: js_sys::Function = js_sys::Reflect::get(&global, &JsValue::from_str("fetch"))
            .map_err(|_| AiFailure::BlockedByBrowser)?
            .dyn_into()
            .map_err(|_| AiFailure::BlockedByBrowser)?;
        let promise: js_sys::Promise = fetch
            .call1(&global, &fetch_request)
            .map_err(|_| AiFailure::BlockedByBrowser)?
            .dyn_into()
            .map_err(|_| AiFailure::BlockedByBrowser)?;
        let response: web_sys::Response = JsFuture::from(promise)
            .await
            .map_err(|error| fetch_failure(&error))?
            .dyn_into()
            .map_err(|_| AiFailure::BadResponse)?;
        let status = response.status();
        // Объявленный размер проверяется до чтения: `text()` читает тело целиком,
        // и огромный ответ уронил бы вкладку раньше проверки ниже (ревью
        // безопасности 050, L1). Без заголовка остаётся проверка после чтения.
        let declared = response
            .headers()
            .get("content-length")
            .ok()
            .flatten()
            .and_then(|length| length.trim().parse::<u64>().ok());
        if declared.is_some_and(|length| length > MAX_RESPONSE_BYTES as u64) {
            return Err(AiFailure::BadResponse);
        }
        let text = JsFuture::from(response.text().map_err(|_| AiFailure::BadResponse)?)
            .await
            .map_err(|error| fetch_failure(&error))?
            .as_string()
            .ok_or(AiFailure::BadResponse)?;
        if text.len() > MAX_RESPONSE_BYTES {
            return Err(AiFailure::BadResponse);
        }
        Ok(HttpResponse { status, body: text })
    }
}

/// `fetch` не отличает блокировку CORS от перенаправления: оба дают `TypeError`, и
/// сообщение UI называет обе причины (research.md §4).
#[cfg(target_arch = "wasm32")]
fn fetch_failure(error: &wasm_bindgen::JsValue) -> AiFailure {
    use wasm_bindgen::JsValue;
    let name = js_sys::Reflect::get(error, &JsValue::from_str("name"))
        .ok()
        .and_then(|name| name.as_string());
    if name.as_deref() == Some("TimeoutError") {
        return AiFailure::Timeout;
    }
    let online = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("navigator"))
        .and_then(|navigator| js_sys::Reflect::get(&navigator, &JsValue::from_str("onLine")))
        .ok()
        .and_then(|online| online.as_bool())
        .unwrap_or(true);
    if online {
        AiFailure::BlockedByBrowser
    } else {
        AiFailure::Offline
    }
}
