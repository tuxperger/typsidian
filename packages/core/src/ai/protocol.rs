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

// ── Исследование (фича 056, research.md §7) ─────────────────────────────────

/// Инструмент, которым модель сдаёт результат исследования.
pub const SUBMIT_RESEARCH: &str = "submit_research";
/// Серверный инструмент веб-поиска Anthropic: базовая версия, работает на всех моделях и
/// платформах (версии с фильтрацией через среду выполнения не подходят — research.md §7).
pub const ANTHROPIC_WEB_SEARCH: &str = "web_search_20250305";
/// Поисков за одно исследование.
pub const WEB_SEARCH_MAX_USES: u32 = 5;
/// Цена одного поиска Anthropic: 10 долларов за 1000 (research.md §7). У OpenAI цена
/// поиска не названа в документации — стоимость поиска неизвестна (050, FR-014).
pub const ANTHROPIC_SEARCH_USD: f64 = 0.01;

/// Есть ли у подключения встроенный веб-поиск. Свой адрес поиска не получает.
pub fn web_search_supported(provider: AiProvider) -> bool {
    matches!(provider, AiProvider::Anthropic | AiProvider::OpenAi)
}

/// Цена поисков: известна только у Anthropic.
pub fn search_cost_usd(provider: AiProvider, searches: u64) -> Option<f64> {
    match provider {
        AiProvider::Anthropic => Some(searches as f64 * ANTHROPIC_SEARCH_USD),
        AiProvider::OpenAi | AiProvider::Custom => None,
    }
}

/// Описание инструмента сдачи результата: схема для обоих протоколов.
pub fn submit_research_def() -> (&'static str, &'static str, Value) {
    (
        SUBMIT_RESEARCH,
        "Submit the result of the research: the answer as plain-text paragraphs and the \
         sources it rests on. Call it once, when the research is finished.",
        json!({
            "type": "object",
            "properties": {
                "paragraphs": {
                    "type": "array",
                    "maxItems": 12,
                    "items": {"type": "string"},
                    "description": "The answer in plain text, one paragraph per item, no markup."
                },
                "sources": {
                    "type": "array",
                    "maxItems": 20,
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": {"type": "string"},
                            "url": {"type": "string", "description": "Address of the page, for web research."},
                            "pages": {"type": "array", "items": {"type": "integer"},
                                "description": "Page numbers, for a PDF."}
                        },
                        "required": ["title"]
                    }
                }
            },
            "required": ["paragraphs", "sources"]
        }),
    )
}

/// Запрос веб-исследования с поиском провайдера и инструментом сдачи результата. `None` —
/// у подключения поиска нет (свой адрес): запрос с поиском не строится вовсе.
///
/// Anthropic — API сообщений с `web_search_20250305` и `max_uses`; результат принуждать
/// нельзя (принуждение к инструменту отключило бы поиск), поэтому модель просят сдать его
/// `submit_research`, а ответ без вызова разбирается как текст. OpenAI — только API ответов
/// (`/responses`): в `chat/completions` поиск есть лишь у отдельных моделей.
pub fn research_web_request(
    provider: AiProvider,
    base: &str,
    model: &str,
    system: &str,
    message: &str,
    max_tokens: u32,
) -> Option<HttpRequest> {
    let (name, description, schema) = submit_research_def();
    let (path, body) = match provider {
        AiProvider::Anthropic => (
            "messages",
            json!({
                "model": model,
                "max_tokens": max_tokens,
                "stream": true,
                "system": system,
                "messages": [{"role": "user", "content": message}],
                "tools": [
                    {"type": ANTHROPIC_WEB_SEARCH, "name": "web_search", "max_uses": WEB_SEARCH_MAX_USES},
                    {"name": name, "description": description, "input_schema": schema},
                ],
            }),
        ),
        AiProvider::OpenAi => (
            "responses",
            json!({
                "model": model,
                "stream": true,
                "store": false,
                "max_output_tokens": max_tokens,
                "instructions": system,
                "input": [{"role": "user", "content": message}],
                "tools": [
                    {"type": "web_search"},
                    {"type": "function", "name": name, "description": description, "parameters": schema},
                ],
            }),
        ),
        AiProvider::Custom => return None,
    };
    Some(HttpRequest {
        method: HttpMethod::Post,
        url: format!("{base}/{path}"),
        headers: vec![("content-type".to_owned(), "application/json".to_owned())],
        body: Some(body.to_string()),
    })
}

/// Запрос чтения PDF: без поиска, результат принуждается инструментом (где протокол это
/// позволяет). Форма та же, что у извлечения графа знаний (054).
pub fn research_pdf_request(
    provider: AiProvider,
    base: &str,
    model: &str,
    system: &str,
    message: &str,
    max_tokens: u32,
) -> HttpRequest {
    let (name, description, schema) = submit_research_def();
    let (path, body) = match provider {
        AiProvider::Anthropic => (
            "messages",
            json!({
                "model": model,
                "max_tokens": max_tokens,
                "stream": true,
                "system": system,
                "messages": [{"role": "user", "content": message}],
                "tools": [{"name": name, "description": description, "input_schema": schema}],
                "tool_choice": {"type": "tool", "name": name},
            }),
        ),
        AiProvider::OpenAi | AiProvider::Custom => {
            let mut body = json!({
                "model": model,
                "stream": true,
                "stream_options": {"include_usage": true},
                "messages": [
                    {"role": "system", "content": system},
                    {"role": "user", "content": message},
                ],
                "tools": [{"type": "function", "function": {
                    "name": name, "description": description, "parameters": schema}}],
            });
            if provider == AiProvider::OpenAi {
                body["tool_choice"] = json!({"type": "function", "function": {"name": name}});
            }
            ("chat/completions", body)
        }
    };
    HttpRequest {
        method: HttpMethod::Post,
        url: format!("{base}/{path}"),
        headers: vec![("content-type".to_owned(), "application/json".to_owned())],
        body: Some(body.to_string()),
    }
}

/// Есть ли в теле запроса инструмент поиска. Разбирается JSON, а не ищется подстрока:
/// слово «web_search» может стоять в тексте пользователя или PDF.
pub fn request_has_search_tool(body: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        // Тело, которое не разобрать, поиском не считается, но и не отправляется:
        // сервер его отвергнет. Осторожная сторона — «есть».
        return true;
    };
    value["tools"].as_array().is_some_and(|tools| {
        tools.iter().any(|tool| {
            let kind = tool["type"].as_str().unwrap_or_default();
            let name = tool["name"].as_str().unwrap_or_default();
            kind.starts_with("web_search") || name.starts_with("web_search")
        })
    })
}
