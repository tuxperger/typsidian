//! wasm-биндинг графа связей (contracts/core-graph-api.md).
//!
//! Тонкая обёртка: правил здесь нет, граф строит [`super::view::vault_graph`].

use wasm_bindgen::prelude::*;

use super::view::{vault_graph, GraphRequest, VaultGraph};
use crate::vault::web::WebBackend;

#[wasm_bindgen]
impl WebBackend {
    /// Граф vault одним запросом: узлы и связи с учётом фильтра, показа видов,
    /// групп и локальной выборки.
    #[wasm_bindgen(js_name = vaultGraph)]
    pub async fn vault_graph_js(&self, request: GraphRequest) -> VaultGraph {
        vault_graph(self, &request).await
    }
}
