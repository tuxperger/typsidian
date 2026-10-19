//! wasm-биндинги библиотеки источников (contracts/core-sources-api.md →
//! Биндинги). Состояние — объект `SourceEngine` на стороне JS; вызовы с
//! `backend` идут через очередь вызовов wasm, как поиск и экспорт.

use wasm_bindgen::prelude::*;

use super::check::NoteCheck;
use super::citations::{CitationScanView, CitingNote, MissingKey};
use super::cite::{BibliographyOffer, CitationPlan, CitationPlanError};
use super::library::{LibrarySummary, RefreshStep, SourceHit, SourceHits};
use super::{create_source_file, engine, CreateSourceFileError};
use crate::vault::backend::VaultBackend;
use crate::vault::web::WebBackend;

#[wasm_bindgen(js_name = SourceEngine)]
pub struct WebSourceEngine {
    inner: engine::SourceEngine,
}

impl Default for WebSourceEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen(js_class = SourceEngine)]
impl WebSourceEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: engine::SourceEngine::new(),
        }
    }

    pub async fn refresh(&mut self, backend: &WebBackend, registered: Vec<String>) -> RefreshStep {
        self.inner.refresh(backend, &registered).await
    }

    pub fn summary(&self) -> LibrarySummary {
        self.inner.summary()
    }

    pub fn search(&self, query: &str, limit: u32, uncited_only: bool) -> SourceHits {
        self.inner.search(query, limit, uncited_only)
    }

    #[wasm_bindgen(js_name = sourcesForCompletion)]
    pub fn sources_for_completion(&self, prefix: &str, limit: u32) -> Vec<SourceHit> {
        self.inner.sources_for_completion(prefix, limit)
    }

    #[wasm_bindgen(js_name = scanStart)]
    pub async fn scan_start(&mut self, backend: &WebBackend) -> CitationScanView {
        let notes = backend.list().await;
        self.inner.scan_start(&notes)
    }

    #[wasm_bindgen(js_name = scanStep)]
    pub async fn scan_step(&mut self, backend: &WebBackend, max_notes: u32) -> CitationScanView {
        self.inner.scan_step(backend, max_notes).await
    }

    #[wasm_bindgen(js_name = scanCancel)]
    pub fn scan_cancel(&mut self) -> CitationScanView {
        self.inner.scan_cancel()
    }

    #[wasm_bindgen(js_name = scanView)]
    pub fn scan_view(&self) -> CitationScanView {
        self.inner.scan_view()
    }

    #[wasm_bindgen(js_name = noteChanged)]
    pub async fn note_changed(&mut self, backend: &WebBackend, path: &str) {
        self.inner.note_changed(backend, path).await;
    }

    #[wasm_bindgen(js_name = noteRemoved)]
    pub fn note_removed(&mut self, path: &str) {
        self.inner.note_removed(path);
    }

    #[wasm_bindgen(js_name = noteRenamed)]
    pub fn note_renamed(&mut self, from: &str, to: &str) {
        self.inner.note_renamed(from, to);
    }

    pub fn citing(&self, key: &str) -> Vec<CitingNote> {
        self.inner.citing(key)
    }

    #[wasm_bindgen(js_name = missingKeys)]
    pub fn missing_keys(&self) -> Vec<MissingKey> {
        self.inner.missing_keys()
    }

    #[wasm_bindgen(js_name = checkNote)]
    pub fn check_note(&self, text: &str, known_labels: Vec<String>) -> NoteCheck {
        self.inner.check_note(text, &known_labels)
    }

    #[wasm_bindgen(js_name = planCitation)]
    pub fn plan_citation(
        &self,
        text: &str,
        caret: usize,
        file: &str,
        key: &str,
    ) -> Result<CitationPlan, CitationPlanError> {
        self.inner.plan_citation(text, caret, file, key)
    }

    #[wasm_bindgen(js_name = bibliographyOffer)]
    pub fn bibliography_offer(&self, text: &str, note_path: &str, file: &str) -> BibliographyOffer {
        self.inner.bibliography_offer(text, note_path, file)
    }
}

/// Создаёт пустой `references.bib` в корне vault (research.md §5).
#[wasm_bindgen(js_name = createSourceFile)]
pub async fn create_source_file_js(
    backend: &mut WebBackend,
) -> Result<String, CreateSourceFileError> {
    create_source_file(backend).await
}
