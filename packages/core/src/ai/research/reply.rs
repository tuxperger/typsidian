//! Разбор ответа исследования (фича 056, research.md §7): поток Anthropic с серверным
//! поиском (`server_tool_use`, `web_search_tool_result`, `citations`, `pause_turn`) и поток
//! API ответов OpenAI (`web_search_call`, `url_citation`). Части приходят как придётся,
//! разбираются только законченные строки; разбор линейный, предел строки и числа блоков
//! защищают от враждебного потока. Текст провайдера дальше не идёт: наружу выходит только
//! разобранное.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ResearchKind, ResearchSource};
use crate::ai::lesson::stream::{TurnReply, MAX_LINE_BYTES};
use crate::ai::protocol::SUBMIT_RESEARCH;
use crate::ai::settings::AiProvider;
use crate::ai::AiFailure;

/// Блоков ответа, которые разбираются; остальные игнорируются.
const MAX_BLOCKS: usize = 500;
/// Источников, которые копятся из ответа (цитаты и результаты поиска по отдельности).
const MAX_FOUND: usize = 100;
/// Кодов ошибок поиска, которые запоминаются.
const MAX_ERRORS: usize = 10;
/// Название и адрес источника из ответа провайдера.
const TITLE_MAX_CHARS: usize = 200;
pub const URL_MAX_CHARS: usize = 500;

/// Законченный ответ исследования.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ResearchReply {
    pub text: String,
    /// Аргументы вызова `submit_research` (JSON-строка), если модель его вызвала.
    pub submit: Option<String>,
    /// Источники из цитат текста: провайдер сам привязал их к утверждениям.
    pub cited: Vec<ResearchSource>,
    /// Страницы из результатов поиска, которые модель видела.
    pub found: Vec<ResearchSource>,
    /// Выполнено поисков.
    pub searches: u32,
    /// Коды ошибок поиска из ответов с кодом 200.
    pub search_errors: Vec<String>,
    /// Модель просит продолжить (`pause_turn`).
    pub pause: bool,
    /// Ответ ассистента для продолжения (JSON-массив блоков), только при `pause`.
    pub assistant: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

impl ResearchReply {
    /// Ответ без поиска (чтение PDF): тот же разбор потока, что у хода учителя.
    pub fn from_turn(reply: TurnReply) -> Self {
        ResearchReply {
            submit: reply
                .tool_calls
                .iter()
                .find(|call| call.name == SUBMIT_RESEARCH)
                .map(|call| call.arguments.clone()),
            text: reply.text,
            cited: Vec::new(),
            found: Vec::new(),
            searches: 0,
            search_errors: Vec::new(),
            pause: false,
            assistant: None,
            input_tokens: reply.input_tokens,
            output_tokens: reply.output_tokens,
        }
    }
}

/// Адрес источника: только `http`/`https`, одна строка без пробелов, управляющих символов и
/// знаков, которыми разметка или строка Typst могли бы выйти из своих границ.
pub fn clean_url(raw: &str) -> Option<String> {
    let url = raw.trim();
    if url.is_empty() || url.chars().count() > URL_MAX_CHARS {
        return None;
    }
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    if rest.is_empty()
        || url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "\"\\<>`^{}|".contains(c))
    {
        return None;
    }
    // Хост не пуст: после схемы идёт что-то, кроме разделителей пути.
    if rest.starts_with(['/', '?', '#']) {
        return None;
    }
    Some(url.to_owned())
}

fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn source_of(title: Option<&str>, url: Option<&str>) -> Option<ResearchSource> {
    let url = url.and_then(clean_url)?;
    let title = title.map(str::trim).filter(|t| !t.is_empty());
    Some(ResearchSource {
        title: clip(title.unwrap_or(&url), TITLE_MAX_CHARS),
        url: Some(url),
        pages: None,
    })
}

fn push_unique(list: &mut Vec<ResearchSource>, source: ResearchSource) {
    if list.len() < MAX_FOUND && !list.iter().any(|known| known.url == source.url) {
        list.push(source);
    }
}

fn error_failure(event: &Value) -> AiFailure {
    let kind = event["error"]["type"]
        .as_str()
        .or(event["error"]["code"].as_str())
        .or(event["response"]["error"]["code"].as_str())
        .unwrap_or_default();
    match kind {
        "overloaded_error" | "api_error" | "server_error" => AiFailure::ProviderDown,
        "rate_limit_error" | "rate_limit_exceeded" => AiFailure::RateLimited,
        "authentication_error" | "invalid_api_key" => AiFailure::BadKey,
        "permission_error" => AiFailure::Forbidden,
        "not_found_error" | "model_not_found" => AiFailure::ModelNotFound,
        _ => AiFailure::BadResponse,
    }
}

#[derive(Default)]
struct Lines {
    pending: Vec<u8>,
    scanned: usize,
    too_long: bool,
}

impl Lines {
    /// Отдаёт законченные строки; хвост без перевода строки ждёт следующей части. Просмотр
    /// продолжается с того места, где закончился прошлый, поэтому разбор линейный.
    fn feed(&mut self, chunk: &[u8], mut each: impl FnMut(&str)) {
        if self.too_long {
            return;
        }
        self.pending.extend_from_slice(chunk);
        let mut start = 0;
        let mut from = self.scanned.min(self.pending.len());
        while let Some(offset) = self.pending[from..].iter().position(|byte| *byte == b'\n') {
            let end = from + offset;
            let line = String::from_utf8_lossy(&self.pending[start..end]);
            each(line.trim_end_matches('\r'));
            start = end + 1;
            from = start;
        }
        // Хвост сдвигается только когда строки закончились: часть без перевода строки не
        // копируется заново на каждый кусок.
        if start > 0 {
            self.pending.drain(..start);
        }
        if self.pending.len() > MAX_LINE_BYTES {
            self.too_long = true;
            self.pending = Vec::new();
            return;
        }
        self.scanned = self.pending.len();
    }
}

#[derive(Default)]
struct Usage {
    input: Option<u64>,
    output: Option<u64>,
    searches: Option<u64>,
}

/// Инкрементальный разбор потока исследования.
pub struct ResearchParser {
    provider: AiProvider,
    lines: Lines,
    /// Блоки ответа Anthropic по номеру.
    blocks: BTreeMap<u64, Value>,
    /// Аргументы вызовов инструментов, собираемые из частей.
    json: HashMap<u64, String>,
    stop_reason: Option<String>,
    usage: Usage,
    /// Итоговый объект ответа OpenAI.
    completed: Option<Value>,
    complete: bool,
    failure: Option<AiFailure>,
}

impl ResearchParser {
    pub fn new(provider: AiProvider) -> Self {
        ResearchParser {
            provider,
            lines: Lines::default(),
            blocks: BTreeMap::new(),
            json: HashMap::new(),
            stop_reason: None,
            usage: Usage::default(),
            completed: None,
            complete: false,
            failure: None,
        }
    }

    pub fn feed(&mut self, chunk: &[u8]) {
        let mut lines = std::mem::take(&mut self.lines);
        lines.feed(chunk, |line| self.handle_line(line));
        if lines.too_long {
            self.failure = Some(AiFailure::BadResponse);
        }
        self.lines = lines;
    }

    fn handle_line(&mut self, line: &str) {
        let Some(data) = line.strip_prefix("data:") else {
            return;
        };
        let Ok(event) = serde_json::from_str::<Value>(data.trim()) else {
            return;
        };
        if event.get("error").is_some_and(|error| !error.is_null()) || event["type"] == "error" {
            self.failure = Some(error_failure(&event));
            return;
        }
        match self.provider {
            AiProvider::Anthropic => self.anthropic_event(&event),
            AiProvider::OpenAi | AiProvider::Custom => self.responses_event(&event),
        }
    }

    fn usage(&mut self, usage: &Value) {
        self.usage.input = usage["input_tokens"].as_u64().or(self.usage.input);
        self.usage.output = usage["output_tokens"].as_u64().or(self.usage.output);
        self.usage.searches = usage["server_tool_use"]["web_search_requests"]
            .as_u64()
            .or(self.usage.searches);
    }

    fn anthropic_event(&mut self, event: &Value) {
        let index = event["index"].as_u64().unwrap_or(0);
        match event["type"].as_str() {
            Some("message_start") => self.usage(&event["message"]["usage"]),
            Some("content_block_start") => {
                if self.blocks.len() < MAX_BLOCKS {
                    self.blocks.insert(index, event["content_block"].clone());
                }
            }
            Some("content_block_delta") => {
                let delta = &event["delta"];
                let Some(block) = self.blocks.get_mut(&index) else {
                    return;
                };
                match delta["type"].as_str() {
                    Some("text_delta") => {
                        if let (Some(text), Some(Value::String(own))) =
                            (delta["text"].as_str(), block.get_mut("text"))
                        {
                            own.push_str(text);
                        }
                    }
                    Some("citations_delta") => {
                        if !block["citations"].is_array() {
                            block["citations"] = Value::Array(Vec::new());
                        }
                        if let Some(list) = block["citations"].as_array_mut() {
                            if list.len() < MAX_FOUND {
                                list.push(delta["citation"].clone());
                            }
                        }
                    }
                    Some("input_json_delta") => {
                        if let Some(part) = delta["partial_json"].as_str() {
                            self.json.entry(index).or_default().push_str(part);
                        }
                    }
                    _ => {}
                }
            }
            Some("content_block_stop") => {
                if let (Some(raw), Some(block)) =
                    (self.json.remove(&index), self.blocks.get_mut(&index))
                {
                    // Вход серверного или клиентского инструмента — собранный JSON.
                    block["input"] = serde_json::from_str(&raw).unwrap_or(Value::Null);
                }
            }
            Some("message_delta") => {
                if let Some(reason) = event["delta"]["stop_reason"].as_str() {
                    self.stop_reason = Some(reason.to_owned());
                }
                self.usage(&event["usage"]);
            }
            Some("message_stop") => self.complete = true,
            _ => {}
        }
    }

    /// API ответов OpenAI: итог берётся из `response.completed`, где лежит весь вывод.
    fn responses_event(&mut self, event: &Value) {
        match event["type"].as_str() {
            Some("response.completed") => {
                self.completed = Some(event["response"].clone());
                self.complete = true;
            }
            Some("response.failed") => self.failure = Some(error_failure(event)),
            Some("response.incomplete") => self.failure = Some(AiFailure::BadResponse),
            _ => {}
        }
    }

    /// Итог потока. Поток, оборванный до завершающего события, — не ответ.
    pub fn finish(self) -> Result<ResearchReply, AiFailure> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        if !self.complete {
            return Err(AiFailure::Offline);
        }
        match self.provider {
            AiProvider::Anthropic => Ok(finish_anthropic(
                &self.blocks,
                self.stop_reason.as_deref(),
                &self.usage,
            )),
            AiProvider::OpenAi | AiProvider::Custom => {
                let response = self.completed.ok_or(AiFailure::BadResponse)?;
                Ok(finish_responses(&response))
            }
        }
    }
}

fn finish_anthropic(
    blocks: &BTreeMap<u64, Value>,
    stop_reason: Option<&str>,
    usage: &Usage,
) -> ResearchReply {
    let mut reply = ResearchReply {
        text: String::new(),
        submit: None,
        cited: Vec::new(),
        found: Vec::new(),
        searches: 0,
        search_errors: Vec::new(),
        pause: stop_reason == Some("pause_turn"),
        assistant: None,
        input_tokens: usage.input,
        output_tokens: usage.output,
    };
    let mut counted = 0u32;
    for block in blocks.values() {
        match block["type"].as_str() {
            Some("text") => {
                reply
                    .text
                    .push_str(block["text"].as_str().unwrap_or_default());
                for citation in block["citations"].as_array().into_iter().flatten() {
                    if citation["type"] == "web_search_result_location" {
                        if let Some(source) =
                            source_of(citation["title"].as_str(), citation["url"].as_str())
                        {
                            push_unique(&mut reply.cited, source);
                        }
                    }
                }
            }
            Some("server_tool_use") if block["name"] == "web_search" => counted += 1,
            Some("web_search_tool_result") => match &block["content"] {
                Value::Array(results) => {
                    for result in results {
                        if result["type"] == "web_search_result" {
                            if let Some(source) =
                                source_of(result["title"].as_str(), result["url"].as_str())
                            {
                                push_unique(&mut reply.found, source);
                            }
                        }
                    }
                }
                // Ошибка поиска приходит внутри ответа с кодом 200.
                error
                    if error["type"] == "web_search_tool_result_error"
                        && reply.search_errors.len() < MAX_ERRORS =>
                {
                    reply
                        .search_errors
                        .push(clip(error["error_code"].as_str().unwrap_or("unknown"), 64));
                }
                _ => {}
            },
            Some("tool_use")
                if block["name"] == SUBMIT_RESEARCH
                    && reply.submit.is_none()
                    && !block["input"].is_null() =>
            {
                reply.submit = Some(block["input"].to_string());
            }
            _ => {}
        }
    }
    reply.searches = usage
        .searches
        .map_or(counted, |n| n.min(u64::from(u32::MAX)) as u32);
    if reply.pause {
        // Ответ возвращается провайдеру как есть; пустые `null` в нём лишние.
        let all: Vec<Value> = blocks
            .values()
            .map(|block| {
                let mut block = block.clone();
                if block["citations"].is_null() {
                    if let Some(map) = block.as_object_mut() {
                        map.remove("citations");
                    }
                }
                block
            })
            .collect();
        reply.assistant = serde_json::to_string(&all).ok();
    }
    reply
}

fn finish_responses(response: &Value) -> ResearchReply {
    let mut reply = ResearchReply {
        text: String::new(),
        submit: None,
        cited: Vec::new(),
        found: Vec::new(),
        searches: 0,
        search_errors: Vec::new(),
        pause: false,
        assistant: None,
        input_tokens: response["usage"]["input_tokens"].as_u64(),
        output_tokens: response["usage"]["output_tokens"].as_u64(),
    };
    for item in response["output"].as_array().into_iter().flatten() {
        match item["type"].as_str() {
            Some("web_search_call") => {
                if item["status"] == "failed" {
                    if reply.search_errors.len() < MAX_ERRORS {
                        reply.search_errors.push("failed".to_owned());
                    }
                } else {
                    reply.searches = reply.searches.saturating_add(1);
                }
            }
            Some("message") => {
                for part in item["content"].as_array().into_iter().flatten() {
                    if part["type"] == "output_text" {
                        reply
                            .text
                            .push_str(part["text"].as_str().unwrap_or_default());
                        for note in part["annotations"].as_array().into_iter().flatten() {
                            if note["type"] == "url_citation" {
                                if let Some(source) =
                                    source_of(note["title"].as_str(), note["url"].as_str())
                                {
                                    push_unique(&mut reply.cited, source);
                                }
                            }
                        }
                    }
                }
            }
            Some("function_call") if item["name"] == SUBMIT_RESEARCH && reply.submit.is_none() => {
                reply.submit = item["arguments"].as_str().map(str::to_owned);
            }
            _ => {}
        }
    }
    reply
}

// ── Результат ───────────────────────────────────────────────────────────────

/// Сколько абзацев, символов в абзаце и символов всего берётся из ответа модели.
pub const MAX_PARAGRAPHS: usize = 12;
pub const PARAGRAPH_MAX_CHARS: usize = 4_000;
pub const ANSWER_MAX_CHARS: usize = 20_000;
/// Источников в результате.
pub const MAX_SOURCES: usize = 30;
/// Страниц в одном источнике PDF.
const MAX_PAGES_PER_SOURCE: usize = 50;

/// Сданный результат: абзацы и источники, уже с пределами.
#[derive(Debug, Clone, PartialEq)]
pub struct Submission {
    pub paragraphs: Vec<String>,
    pub sources: Vec<ResearchSource>,
}

/// Абзацы из свободного текста: разделитель — пустая строка.
pub fn paragraphs_of(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut total = 0usize;
    for part in text.split("\n\n") {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let part = clip(part, PARAGRAPH_MAX_CHARS);
        total += part.chars().count();
        if out.len() >= MAX_PARAGRAPHS || total > ANSWER_MAX_CHARS {
            break;
        }
        out.push(part);
    }
    out
}

fn pages_of(value: &Value, total: u32) -> Option<Vec<u32>> {
    let mut pages: Vec<u32> = value
        .as_array()?
        .iter()
        .filter_map(|page| {
            page.as_u64()
                .or_else(|| page.as_str().and_then(|s| s.trim().parse().ok()))
        })
        .filter(|page| *page >= 1 && *page <= u64::from(total.max(1)))
        .map(|page| page as u32)
        .collect();
    pages.sort_unstable();
    pages.dedup();
    pages.truncate(MAX_PAGES_PER_SOURCE);
    (!pages.is_empty()).then_some(pages)
}

/// Разбирает аргументы `submit_research`. Для PDF страницы, которых нет в документе
/// (`pages_total`), отбрасываются: ссылка на несуществующую страницу — не источник; для
/// веба источник без годного адреса отбрасывается. `None` — абзацев нет.
pub fn parse_submit(arguments: &str, kind: ResearchKind, pages_total: u32) -> Option<Submission> {
    let value: Value = serde_json::from_str(arguments).ok()?;
    let paragraphs = match &value["paragraphs"] {
        Value::Array(items) => {
            let mut out = Vec::new();
            let mut total = 0usize;
            for item in items {
                let text = item.as_str().map(str::trim).unwrap_or_default();
                if text.is_empty() {
                    continue;
                }
                let text = clip(text, PARAGRAPH_MAX_CHARS);
                total += text.chars().count();
                if out.len() >= MAX_PARAGRAPHS || total > ANSWER_MAX_CHARS {
                    break;
                }
                out.push(text);
            }
            out
        }
        Value::String(text) => paragraphs_of(text),
        _ => Vec::new(),
    };
    if paragraphs.is_empty() {
        return None;
    }
    let mut sources: Vec<ResearchSource> = Vec::new();
    for item in value["sources"].as_array().into_iter().flatten() {
        if sources.len() >= MAX_SOURCES {
            break;
        }
        let title = item["title"].as_str().map(str::trim).unwrap_or_default();
        match kind {
            ResearchKind::Web => {
                if let Some(source) = source_of(Some(title), item["url"].as_str()) {
                    push_unique(&mut sources, source);
                }
            }
            ResearchKind::Pdf => {
                if let Some(pages) = pages_of(&item["pages"], pages_total) {
                    sources.push(ResearchSource {
                        title: clip(title, TITLE_MAX_CHARS),
                        url: None,
                        pages: Some(pages),
                    });
                }
            }
        }
    }
    Some(Submission {
        paragraphs,
        sources,
    })
}
