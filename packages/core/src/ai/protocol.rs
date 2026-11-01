//! Форма запросов и ответов: Anthropic и совместимый протокол (OpenAI, свой адрес).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::settings::AiProvider;
use super::transport::{HttpMethod, HttpRequest};
use super::AiFailure;

/// Версия API Anthropic (research.md §2).
const ANTHROPIC_VERSION: &str = "2026-10-21";
/// Текст пробного запроса: заметок в нём нет (FR-006).
pub const PROBE_TEXT: &str = "Ответь одним словом: готово.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ProbeReply {
    pub text: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

pub fn probe_request(provider: AiProvider, base: &str, model: &str) -> HttpRequest {
    let (path, body) = match provider {
        AiProvider::Anthropic => (
            "messages",
            json!({
                "model": model,
                "max_tokens": 1024,
                "messages": [{"role": "user", "content": PROBE_TEXT}],
            }),
        ),
        AiProvider::OpenAi | AiProvider::Custom => (
            "chat/completions",
            json!({
                "model": model,
                "messages": [{"role": "user", "content": PROBE_TEXT}],
            }),
        ),
    };
    HttpRequest {
        method: HttpMethod::Post,
        url: format!("{base}/{path}"),
        headers: vec![("content-type".to_owned(), "application/json".to_owned())],
        body: Some(body.to_string()),
    }
}

pub fn models_request(provider: AiProvider, base: &str) -> HttpRequest {
    let url = match provider {
        AiProvider::Anthropic => format!("{base}/models?limit=1000"),
        AiProvider::OpenAi | AiProvider::Custom => format!("{base}/models"),
    };
    HttpRequest {
        method: HttpMethod::Get,
        url,
        headers: Vec::new(),
        body: None,
    }
}

/// Заголовки ключа. Отдельный шаг: журнал получает запрос до этого вызова, поэтому
/// ключ туда попасть не может (FR-007).
pub fn authorize(request: &mut HttpRequest, provider: AiProvider, key: Option<&str>) {
    match provider {
        AiProvider::Anthropic => {
            request
                .headers
                .push(("anthropic-version".to_owned(), ANTHROPIC_VERSION.to_owned()));
            if cfg!(target_arch = "wasm32") {
                request.headers.push((
                    "anthropic-dangerous-direct-browser-access".to_owned(),
                    "true".to_owned(),
                ));
            }
            if let Some(key) = key {
                request
                    .headers
                    .push(("x-api-key".to_owned(), key.to_owned()));
            }
        }
        AiProvider::OpenAi | AiProvider::Custom => {
            if let Some(key) = key {
                request
                    .headers
                    .push(("authorization".to_owned(), format!("Bearer {key}")));
            }
        }
    }
}

/// Код ответа не об успехе — отказ. Перенаправление никогда не выполняется (FR-009).
pub fn check_status(status: u16) -> Result<(), AiFailure> {
    match status {
        200..=299 => Ok(()),
        300..=399 => Err(AiFailure::Redirected),
        401 => Err(AiFailure::BadKey),
        402 => Err(AiFailure::Billing),
        403 => Err(AiFailure::Forbidden),
        404 => Err(AiFailure::ModelNotFound),
        408 => Err(AiFailure::Timeout),
        429 => Err(AiFailure::RateLimited),
        500..=599 => Err(AiFailure::ProviderDown),
        _ => Err(AiFailure::BadResponse),
    }
}

pub fn parse_probe(provider: AiProvider, body: &str) -> Result<ProbeReply, AiFailure> {
    let value: Value = serde_json::from_str(body).map_err(|_| AiFailure::BadResponse)?;
    let text = match provider {
        // Блоков несколько: перед текстом может идти `thinking`. Отказ модели —
        // `stop_reason: refusal` — приходит с кодом 200: соединение при этом рабочее.
        AiProvider::Anthropic => value["content"]
            .as_array()
            .ok_or(AiFailure::BadResponse)?
            .iter()
            .filter(|block| block["type"] == "text")
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join(""),
        AiProvider::OpenAi | AiProvider::Custom => value["choices"][0]["message"]["content"]
            .as_str()
            .ok_or(AiFailure::BadResponse)?
            .to_owned(),
    };
    let usage = &value["usage"];
    let (input, output) = match provider {
        AiProvider::Anthropic => ("input_tokens", "output_tokens"),
        AiProvider::OpenAi | AiProvider::Custom => ("prompt_tokens", "completion_tokens"),
    };
    Ok(ProbeReply {
        text,
        input_tokens: usage[input].as_u64(),
        output_tokens: usage[output].as_u64(),
    })
}

pub fn parse_models(body: &str) -> Result<Vec<String>, AiFailure> {
    let value: Value = serde_json::from_str(body).map_err(|_| AiFailure::BadResponse)?;
    let mut models: Vec<String> = value["data"]
        .as_array()
        .ok_or(AiFailure::BadResponse)?
        .iter()
        .filter_map(|model| model["id"].as_str().map(str::to_owned))
        .collect();
    models.sort();
    Ok(models)
}
