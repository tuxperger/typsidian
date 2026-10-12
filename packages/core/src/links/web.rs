//! wasm-биндинги связей заметки (contracts/core-links-api.md → Биндинги).
//!
//! Тонкие обёртки: правил здесь нет. Проход упоминаний — чистые переходы
//! состояния, которое между вызовами держит интерфейс, как пакет экспорта.

use wasm_bindgen::prelude::*;

use super::link::{self, LinkOutcome, LinkRequest};
use super::mentions::{self, MentionScan, MentionStep, MentionsAvailability, MentionsStart};
use super::{backlinks, BacklinksPage, OpenText, SourceGroup};
use crate::vault::web::WebBackend;

#[wasm_bindgen(js_name = linksBacklinksPage)]
pub async fn links_backlinks_page_js(
    backend: &WebBackend,
    target: &str,
    open_texts: Vec<OpenText>,
    offset: u32,
    limit: u32,
) -> BacklinksPage {
    backlinks::backlinks_page(backend, target, &open_texts, offset, limit).await
}

#[wasm_bindgen(js_name = linksMentionsAvailability)]
pub fn links_mentions_availability_js(target: &str) -> MentionsAvailability {
    mentions::availability(target)
}

#[wasm_bindgen(js_name = linksMentionsStart)]
pub async fn links_mentions_start_js(backend: &WebBackend, target: &str) -> MentionsStart {
    mentions::start(backend, target).await.into()
}

#[wasm_bindgen(js_name = linksMentionsStep)]
pub async fn links_mentions_step_js(
    backend: &WebBackend,
    state: MentionScan,
    open_texts: Vec<OpenText>,
) -> MentionStep {
    mentions::step(backend, state, &open_texts).await
}

#[wasm_bindgen(js_name = linksMentionsCancel)]
pub fn links_mentions_cancel_js(state: MentionScan) -> MentionScan {
    mentions::cancel(state)
}

#[wasm_bindgen(js_name = linksMentionsIn)]
pub async fn links_mentions_in_js(
    backend: &WebBackend,
    target: &str,
    paths: Vec<String>,
    open_texts: Vec<OpenText>,
) -> Vec<SourceGroup> {
    mentions::in_notes(backend, target, &paths, &open_texts).await
}

#[wasm_bindgen(js_name = linksLinkMention)]
pub async fn links_link_mention_js(
    backend: &mut WebBackend,
    request: LinkRequest,
    editor_text: Option<String>,
) -> LinkOutcome {
    link::link_mention(backend, &request, editor_text.as_deref()).await
}
