//! Шов сети (research.md §1, §4). Native — `reqwest`, wasm — `fetch` браузера с
//! `redirect: "error"`: у `reqwest` под wasm этой настройки нет, а при 307/308
//! браузер повторил бы тело запроса на другой хост.

use serde::{Deserialize, Serialize};

use super::AiFailure;

/// Предел ожидания ответа (SC-005).
pub const TIMEOUT_SECS: u32 = 30;
/// Предел тела ответа: необычно большой ответ — отказ, а не исчерпание памяти.
pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// Потоковый запрос (фича 051, research.md §4): общего предела в 30 с нет — длинный ответ
/// законно идёт минутами. Предел — до начала ответа...
pub const STREAM_START_SECS: u32 = 30;
/// ...и тишина между частями потока.
pub const STREAM_SILENCE_SECS: u32 = 60;
/// Предел всего потока: больше — отказ, а не исчерпание памяти.
pub const MAX_STREAM_BYTES: usize = 8 * 1024 * 1024;
/// Общий предел времени потока: ответ, который идёт дольше, обрывается (ревью безопасности
/// 051, M5) — тишина между частями короче 60 секунд сама по себе поток не ограничивает.
pub const STREAM_TOTAL_SECS: u32 = 600;

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

    /// Потоковая отправка: части тела отдаются `on_chunk` по мере получения, а
    /// возвращается код ответа. Части отдаются только при коде 2xx: тело отказа ядру
    /// не нужно и текст сервера дальше не идёт (FR-007). Умолчание — обычный запрос с
    /// телом одной частью, для транспортов без потока.
    async fn send_stream(
        &self,
        request: HttpRequest,
        on_chunk: &mut (dyn FnMut(&[u8]) + Send),
    ) -> Result<u16, AiFailure> {
        let response = self.send(request).await?;
        if (200..300).contains(&response.status) {
            on_chunk(response.body.as_bytes());
        }
        Ok(response.status)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultTransport;

#[cfg(not(target_arch = "wasm32"))]
impl AiTransport for DefaultTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiFailure> {
        let builder = native_builder(
            request,
            Some(std::time::Duration::from_secs(u64::from(TIMEOUT_SECS))),
        )?;
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

    async fn send_stream(
        &self,
        request: HttpRequest,
        on_chunk: &mut (dyn FnMut(&[u8]) + Send),
    ) -> Result<u16, AiFailure> {
        self.send_stream_within(
            request,
            on_chunk,
            std::time::Duration::from_secs(u64::from(STREAM_TOTAL_SECS)),
        )
        .await
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl DefaultTransport {
    /// Поток с заданным общим пределом времени; `send_stream` берёт [`STREAM_TOTAL_SECS`].
    pub async fn send_stream_within(
        &self,
        request: HttpRequest,
        on_chunk: &mut (dyn FnMut(&[u8]) + Send),
        total: std::time::Duration,
    ) -> Result<u16, AiFailure> {
        tokio::time::timeout(total, self.stream_inner(request, on_chunk))
            .await
            .map_err(|_| AiFailure::Timeout)?
    }

    async fn stream_inner(
        &self,
        request: HttpRequest,
        on_chunk: &mut (dyn FnMut(&[u8]) + Send),
    ) -> Result<u16, AiFailure> {
        use std::time::Duration;
        use tokio::time::timeout;

        let builder = native_builder(request, None)?;
        let mut response = timeout(
            Duration::from_secs(u64::from(STREAM_START_SECS)),
            builder.send(),
        )
        .await
        .map_err(|_| AiFailure::Timeout)?
        .map_err(network_failure)?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Ok(status);
        }
        let mut total = 0usize;
        loop {
            let next = timeout(
                Duration::from_secs(u64::from(STREAM_SILENCE_SECS)),
                response.chunk(),
            )
            .await
            .map_err(|_| AiFailure::Timeout)?
            .map_err(network_failure)?;
            let Some(chunk) = next else {
                return Ok(status);
            };
            total += chunk.len();
            if total > MAX_STREAM_BYTES {
                return Err(AiFailure::BadResponse);
            }
            on_chunk(&chunk);
        }
    }
}

/// Запрос `reqwest` с проверками сервера — общий для обычной и потоковой отправки.
/// `total_timeout` — общий предел обычного запроса; у потока его нет.
#[cfg(not(target_arch = "wasm32"))]
fn native_builder(
    request: HttpRequest,
    total_timeout: Option<std::time::Duration>,
) -> Result<reqwest::RequestBuilder, AiFailure> {
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
    builder = builder.redirect(reqwest::redirect::Policy::none());
    if let Some(limit) = total_timeout {
        builder = builder.timeout(limit);
    }
    let client = builder.build().map_err(|_| AiFailure::Offline)?;
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
    Ok(builder)
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
        use wasm_bindgen_futures::JsFuture;

        let signal = web_sys::AbortSignal::timeout_with_u32(TIMEOUT_SECS * 1000);
        let response = fetch_response(&request, &signal).await?;
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

    /// Поток через `ReadableStream` (research.md §4). `AbortController` срабатывает при
    /// любом выходе, в том числе когда отмена (`cancel.rs`) просто бросает ожидание:
    /// соединение при этом закрывается, а не остаётся читать ответ в пустоту.
    async fn send_stream(
        &self,
        request: HttpRequest,
        on_chunk: &mut (dyn FnMut(&[u8]) + Send),
    ) -> Result<u16, AiFailure> {
        use wasm_bindgen::{JsCast, JsValue};
        use wasm_bindgen_futures::JsFuture;

        struct AbortOnDrop(web_sys::AbortController);
        impl Drop for AbortOnDrop {
            fn drop(&mut self) {
                self.0.abort();
            }
        }

        let controller =
            web_sys::AbortController::new().map_err(|_| AiFailure::BlockedByBrowser)?;
        let signal = controller.signal();
        let _guard = AbortOnDrop(controller);
        let started = js_sys::Date::now();
        let response = within(
            STREAM_START_SECS * 1000,
            Box::pin(fetch_response(&request, &signal)),
        )
        .await??;
        let status = response.status();
        if !(200..300).contains(&status) {
            return Ok(status);
        }
        let stream = response.body().ok_or(AiFailure::BadResponse)?;
        let reader: web_sys::ReadableStreamDefaultReader = stream
            .get_reader()
            .dyn_into()
            .map_err(|_| AiFailure::BadResponse)?;
        let mut total = 0usize;
        loop {
            // Общий предел времени потока: пауза не длиннее тишины и не длиннее остатка.
            let left = f64::from(STREAM_TOTAL_SECS) * 1000.0 - (js_sys::Date::now() - started);
            if left <= 0.0 {
                return Err(AiFailure::Timeout);
            }
            let wait = (f64::from(STREAM_SILENCE_SECS) * 1000.0).min(left) as u32;
            let read = within(wait, Box::pin(JsFuture::from(reader.read())))
                .await?
                .map_err(|error| fetch_failure(&error))?;
            let done = js_sys::Reflect::get(&read, &JsValue::from_str("done"))
                .ok()
                .and_then(|done| done.as_bool())
                .unwrap_or(true);
            if done {
                return Ok(status);
            }
            let value = js_sys::Reflect::get(&read, &JsValue::from_str("value"))
                .map_err(|_| AiFailure::BadResponse)?;
            let chunk = js_sys::Uint8Array::new(&value).to_vec();
            total += chunk.len();
            if total > MAX_STREAM_BYTES {
                return Err(AiFailure::BadResponse);
            }
            on_chunk(&chunk);
        }
    }
}

/// Ждёт `work` не дольше `millis`; по истечении — `Timeout`.
#[cfg(target_arch = "wasm32")]
async fn within<T>(
    millis: u32,
    work: std::pin::Pin<Box<dyn std::future::Future<Output = T> + '_>>,
) -> Result<T, AiFailure> {
    use futures_util::future::{select, Either};
    match select(work, Box::pin(sleep_ms(millis))).await {
        Either::Left((value, _)) => Ok(value),
        Either::Right(_) => Err(AiFailure::Timeout),
    }
}

/// Таймер через глобальный `setTimeout`: приложение может работать и в окне, и в воркере.
#[cfg(target_arch = "wasm32")]
async fn sleep_ms(ms: u32) {
    use wasm_bindgen::{JsCast, JsValue};
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let global = js_sys::global();
        if let Ok(set_timeout) = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
            .and_then(|function| function.dyn_into::<js_sys::Function>())
        {
            let _ = set_timeout.call2(&global, &resolve, &JsValue::from(ms));
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Запрос `fetch` с `redirect: "error"`, общий для обычной и потоковой отправки.
#[cfg(target_arch = "wasm32")]
async fn fetch_response(
    request: &HttpRequest,
    signal: &web_sys::AbortSignal,
) -> Result<web_sys::Response, AiFailure> {
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
    init.set_signal(Some(signal));
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
    Ok(response)
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
