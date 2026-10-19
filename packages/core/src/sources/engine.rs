//! Фасад библиотеки и индекса цитат (contracts/core-sources-api.md § Движок).
//!
//! Состояние живёт по обе стороны границы — wasm-объект на web, состояние
//! Tauri на desktop: записи цитат 5 000 заметок на каждый шаг были бы
//! мегабайтами сериализации (research.md §8).

use std::collections::BTreeSet;

use super::check::{check_note, NoteCheck};
use super::citations::{
    CitationIndex, CitationScanPhase, CitationScanView, CitingNote, MissingKey,
};
use super::cite::{
    bibliography_offer, plan_citation, BibliographyOffer, CitationPlan, CitationPlanError,
};
use super::library::{LibrarySummary, RefreshStep, SourceHit, SourceHits, SourceLibrary};
use crate::vault::backend::VaultBackend;
use crate::vault::index::Note;

#[derive(Debug, Clone, Default)]
pub struct SourceEngine {
    library: SourceLibrary,
    index: CitationIndex,
}

impl SourceEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Шаг обновления библиотеки; `registered` — пути из настроек vault.
    pub async fn refresh<B: VaultBackend>(
        &mut self,
        backend: &B,
        registered: &[String],
    ) -> RefreshStep {
        let registered: BTreeSet<String> = registered.iter().cloned().collect();
        let referenced = self.index.referenced();
        self.library
            .refresh(backend, &registered, &referenced)
            .await
    }

    pub fn summary(&self) -> LibrarySummary {
        self.library.summary()
    }

    pub fn search(&self, query: &str, limit: u32, uncited_only: bool) -> SourceHits {
        let index = &self.index;
        let done = index.phase() == CitationScanPhase::Done;
        let counts = if done {
            index.counts()
        } else {
            Default::default()
        };
        let count = |key: &str| counts.get(key).copied().unwrap_or(0);
        let cited: Option<&dyn Fn(&str) -> u32> = done.then_some(&count);
        self.library.search(query, limit, cited, uncited_only)
    }

    /// Источники для дополнения по `@` (018, research.md §18).
    pub fn sources_for_completion(&self, prefix: &str, limit: u32) -> Vec<SourceHit> {
        self.search(prefix, limit, false).hits
    }

    pub fn scan_start(&mut self, notes: &[Note]) -> CitationScanView {
        self.index.scan_start(notes)
    }

    pub async fn scan_step<B: VaultBackend>(
        &mut self,
        backend: &B,
        max_notes: u32,
    ) -> CitationScanView {
        self.index.scan_step(backend, max_notes).await
    }

    pub fn scan_cancel(&mut self) -> CitationScanView {
        self.index.scan_cancel()
    }

    pub fn scan_view(&self) -> CitationScanView {
        self.index.view()
    }

    pub async fn note_changed<B: VaultBackend>(&mut self, backend: &B, path: &str) {
        self.index.note_changed(backend, path).await;
    }

    pub fn note_removed(&mut self, path: &str) {
        self.index.note_removed(path);
    }

    pub fn note_renamed(&mut self, from: &str, to: &str) {
        self.index.note_renamed(from, to);
    }

    pub fn citing(&self, key: &str) -> Vec<CitingNote> {
        self.index.citing(key)
    }

    pub fn missing_keys(&self) -> Vec<MissingKey> {
        let library = &self.library;
        self.index
            .missing_keys(&|key| library.has_key(key), library.has_files())
    }

    pub fn check_note(&self, text: &str, known_labels: &[String]) -> NoteCheck {
        let library = &self.library;
        check_note(
            text,
            &|key| library.has_key(key),
            library.has_files(),
            known_labels,
        )
    }

    pub fn plan_citation(
        &self,
        text: &str,
        caret: usize,
        file: &str,
        key: &str,
    ) -> Result<CitationPlan, CitationPlanError> {
        plan_citation(text, caret, key, self.library.source(file, key).is_some())
    }

    pub fn bibliography_offer(&self, text: &str, note_path: &str, file: &str) -> BibliographyOffer {
        bibliography_offer(text, note_path, file)
    }
}
