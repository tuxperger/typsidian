//! Инкрементальный разбор потока ответа модели (фича 051, research.md §3): события
//! Anthropic и совместимого протокола. Части приходят как придётся — посреди строки и
//! даже посреди символа UTF-8, — поэтому разбираются только законченные строки.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai::settings::AiProvider;
use crate::ai::AiFailure;

/// Вызов инструмента: аргументы — собранный из частей JSON как строка.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ToolCall {
    pub name: String,
    pub arguments: String,
}

/// Законченный ответ модели.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TurnReply {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

struct CallAcc {
    key: u64,
    name: String,
    arguments: String,
}

/// Предел одной строки потока: строка без перевода строки длиннее — не SSE, а попытка
/// набить память (ревью безопасности 051, M5).
pub const MAX_LINE_BYTES: usize = 1024 * 1024;

pub struct StreamParser {
    provider: AiProvider,
    pending: Vec<u8>,
    /// Сколько байт `pending` уже просмотрено в поиске перевода строки: разбор линейный,
    /// а не квадратичный от длины строки.
    scanned: usize,
    reading: Option<String>,
    text: String,
    calls: Vec<CallAcc>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    complete: bool,
    line_too_long: bool,
    failure: Option<AiFailure>,
}

impl StreamParser {
    pub fn new(provider: AiProvider) -> Self {
        StreamParser {
            provider,
            pending: Vec::new(),
            scanned: 0,
            reading: None,
            text: String::new(),
            calls: Vec::new(),
            input_tokens: None,
            output_tokens: None,
            complete: false,
            line_too_long: false,
            failure: None,
        }
    }

    /// Текст, накопленный к этой минуте.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Путь заметки из последнего вызова `read_note`, аргументы которого уже собраны.
    /// Считается один раз, когда аргументы закрылись, а не на каждой части потока.
    pub fn reading(&self) -> Option<String> {
        self.reading.clone()
    }

    /// Аргументы вызова `read_note` дописаны: если они уже целый JSON, путь известен.
    fn note_reading(&mut self, position: usize) {
        let call = &self.calls[position];
        if call.name != super::prompt::READ_NOTE
            || call.arguments.len() > 4096
            || !call.arguments.trim_end().ends_with('}')
        {
            return;
        }
        if let Some(path) = serde_json::from_str::<Value>(&call.arguments)
            .ok()
            .and_then(|value| value.get("path")?.as_str().map(str::to_owned))
        {
            self.reading = Some(path);
        }
    }

    /// Принимает очередную часть потока.
    pub fn feed(&mut self, chunk: &[u8]) {
        if self.line_too_long {
            return;
        }
        self.pending.extend_from_slice(chunk);
        let pending = std::mem::take(&mut self.pending);
        let mut start = 0;
        let mut from = self.scanned.min(pending.len());
        while let Some(offset) = pending[from..].iter().position(|byte| *byte == b'\n') {
            let end = from + offset;
            let line = String::from_utf8_lossy(&pending[start..end]);
            self.handle_line(line.trim_end_matches('\r'));
            start = end + 1;
            from = start;
        }
        if pending.len() - start > MAX_LINE_BYTES {
            self.line_too_long = true;
            self.failure = Some(AiFailure::BadResponse);
            return;
        }
        self.pending = pending[start..].to_vec();
        self.scanned = self.pending.len();
    }

    fn handle_line(&mut self, line: &str) {
        let Some(data) = line.strip_prefix("data:") else {
            return;
        };
        let data = data.trim();
        if data == "[DONE]" {
            self.complete = true;
            return;
        }
        let Ok(event) = serde_json::from_str::<Value>(data) else {
            return;
        };
        if event.get("error").is_some_and(|error| !error.is_null()) || event["type"] == "error" {
            self.failure = Some(error_failure(&event));
            return;
        }
        match self.provider {
            AiProvider::Anthropic => self.anthropic_event(&event),
            AiProvider::OpenAi | AiProvider::Custom => self.compatible_event(&event),
        }
    }

    fn anthropic_event(&mut self, event: &Value) {
        match event["type"].as_str() {
            Some("message_start") => self.usage(&event["message"]["usage"], "input_tokens"),
            Some("content_block_start") => {
                let block = &event["content_block"];
                match block["type"].as_str() {
                    Some("tool_use") => self.calls.push(CallAcc {
                        key: event["index"].as_u64().unwrap_or(0),
                        name: block["name"].as_str().unwrap_or_default().to_owned(),
                        arguments: String::new(),
                    }),
                    Some("text") => {
                        if let Some(text) = block["text"].as_str() {
                            self.text.push_str(text);
                        }
                    }
                    _ => {}
                }
            }
            Some("content_block_delta") => {
                let delta = &event["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => {
                        if let Some(text) = delta["text"].as_str() {
                            self.text.push_str(text);
                        }
                    }
                    Some("input_json_delta") => {
                        let key = event["index"].as_u64().unwrap_or(0);
                        let position = self.calls.iter().rposition(|call| call.key == key);
                        if let (Some(position), Some(part)) =
                            (position, delta["partial_json"].as_str())
                        {
                            self.calls[position].arguments.push_str(part);
                            self.note_reading(position);
                        }
                    }
                    _ => {}
                }
            }
            Some("message_delta") => self.usage(&event["usage"], "input_tokens"),
            Some("message_stop") => self.complete = true,
            _ => {}
        }
    }

    fn compatible_event(&mut self, event: &Value) {
        let choice = &event["choices"][0];
        let delta = &choice["delta"];
        if let Some(text) = delta["content"].as_str() {
            self.text.push_str(text);
        }
        if let Some(calls) = delta["tool_calls"].as_array() {
            for call in calls {
                let key = call["index"].as_u64().unwrap_or(0);
                let function = &call["function"];
                let position = match self.calls.iter().position(|acc| acc.key == key) {
                    Some(position) => position,
                    None => {
                        self.calls.push(CallAcc {
                            key,
                            name: String::new(),
                            arguments: String::new(),
                        });
                        self.calls.len() - 1
                    }
                };
                let acc = &mut self.calls[position];
                if let Some(name) = function["name"].as_str() {
                    acc.name.push_str(name);
                }
                if let Some(part) = function["arguments"].as_str() {
                    acc.arguments.push_str(part);
                    self.note_reading(position);
                }
            }
        }
        if choice["finish_reason"].is_string() {
            self.complete = true;
        }
        // Объём приходит последним куском с пустым списком вариантов.
        if event["usage"].is_object() {
            self.input_tokens = event["usage"]["prompt_tokens"]
                .as_u64()
                .or(self.input_tokens);
            self.output_tokens = event["usage"]["completion_tokens"]
                .as_u64()
                .or(self.output_tokens);
        }
    }

    fn usage(&mut self, usage: &Value, input_key: &str) {
        self.input_tokens = usage[input_key].as_u64().or(self.input_tokens);
        self.output_tokens = usage["output_tokens"].as_u64().or(self.output_tokens);
    }

    /// Итог потока. Поток, оборванный до завершающего события, — не ответ: текст
    /// остаётся в буфере прогресса, а ход записывается оборванным.
    pub fn finish(self) -> Result<TurnReply, AiFailure> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        if !self.complete {
            return Err(AiFailure::Offline);
        }
        Ok(TurnReply {
            text: self.text,
            tool_calls: self
                .calls
                .into_iter()
                .filter(|call| !call.name.is_empty())
                .map(|call| ToolCall {
                    name: call.name,
                    arguments: call.arguments,
                })
                .collect(),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
        })
    }
}

/// Ошибка внутри потока (код ответа при этом был 200). Текст сервера дальше не идёт.
fn error_failure(event: &Value) -> AiFailure {
    let kind = event["error"]["type"]
        .as_str()
        .or(event["error"]["code"].as_str())
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
