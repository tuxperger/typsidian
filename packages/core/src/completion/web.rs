//! wasm-биндинги дополнения (contracts/core-completion-api.md § Биндинги).
//!
//! Тонкие обёртки: правил здесь нет. Перечень связанных файлов стоит на
//! `PreviewEngine` (`preview/web.rs`): кэш связанных файлов принадлежит ему.

use wasm_bindgen::prelude::*;

use super::{
    completions, definition_at, reference_marks, CompletionList, CompletionRequest,
    DefinitionOutcome, DefinitionRequest, MarksRequest, ReferenceMark,
};
use crate::vault::web::WebBackend;

#[wasm_bindgen]
impl WebBackend {
    #[wasm_bindgen(js_name = completions)]
    pub async fn completions_js(&self, request: CompletionRequest) -> CompletionList {
        completions(self, &request).await
    }

    #[wasm_bindgen(js_name = definitionAt)]
    pub async fn definition_at_js(&self, request: DefinitionRequest) -> DefinitionOutcome {
        definition_at(self, &request).await
    }

    #[wasm_bindgen(js_name = referenceMarks)]
    pub async fn reference_marks_js(&self, request: MarksRequest) -> Vec<ReferenceMark> {
        reference_marks(self, &request).await
    }
}
