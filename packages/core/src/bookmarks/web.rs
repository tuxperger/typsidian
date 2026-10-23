//! wasm-биндинги закладок (фича 032). Тонкие обёртки: правила — в `bookmarks`.

use wasm_bindgen::prelude::*;

use super::{AddOutcome, BookmarkError, BookmarkPlace, BookmarkTarget, BookmarksView};
use crate::vault::web::WebBackend;

fn rejected(error: BookmarkError) -> JsValue {
    serde_wasm_bindgen::to_value(&error).unwrap_or_else(|_| JsValue::from_str("Io"))
}

#[wasm_bindgen(js_name = bookmarksView)]
pub async fn bookmarks_view_js(backend: &WebBackend) -> BookmarksView {
    super::bookmarks_view(backend).await
}

#[wasm_bindgen(js_name = bookmarkAdd)]
pub async fn bookmark_add_js(
    backend: &mut WebBackend,
    target: BookmarkTarget,
    name: Option<String>,
) -> Result<AddOutcome, JsValue> {
    super::bookmark_add(backend, target, name)
        .await
        .map_err(rejected)
}

#[wasm_bindgen(js_name = bookmarkRemove)]
pub async fn bookmark_remove_js(
    backend: &mut WebBackend,
    id: &str,
) -> Result<BookmarksView, JsValue> {
    super::bookmark_remove(backend, id).await.map_err(rejected)
}

#[wasm_bindgen(js_name = bookmarkRename)]
pub async fn bookmark_rename_js(
    backend: &mut WebBackend,
    id: &str,
    name: &str,
) -> Result<BookmarksView, JsValue> {
    super::bookmark_rename(backend, id, name)
        .await
        .map_err(rejected)
}

#[wasm_bindgen(js_name = bookmarkMove)]
pub async fn bookmark_move_js(
    backend: &mut WebBackend,
    id: &str,
    place: BookmarkPlace,
) -> Result<BookmarksView, JsValue> {
    super::bookmark_move(backend, id, &place)
        .await
        .map_err(rejected)
}

#[wasm_bindgen(js_name = bookmarkGroupCreate)]
pub async fn bookmark_group_create_js(
    backend: &mut WebBackend,
    name: &str,
) -> Result<BookmarksView, JsValue> {
    super::bookmark_group_create(backend, name)
        .await
        .map_err(rejected)
}

#[wasm_bindgen(js_name = bookmarkGroupRename)]
pub async fn bookmark_group_rename_js(
    backend: &mut WebBackend,
    id: &str,
    name: &str,
) -> Result<BookmarksView, JsValue> {
    super::bookmark_group_rename(backend, id, name)
        .await
        .map_err(rejected)
}

#[wasm_bindgen(js_name = bookmarkGroupToggle)]
pub async fn bookmark_group_toggle_js(
    backend: &mut WebBackend,
    id: &str,
) -> Result<BookmarksView, JsValue> {
    super::bookmark_group_toggle(backend, id)
        .await
        .map_err(rejected)
}

#[wasm_bindgen(js_name = bookmarkGroupDelete)]
pub async fn bookmark_group_delete_js(
    backend: &mut WebBackend,
    id: &str,
) -> Result<BookmarksView, JsValue> {
    super::bookmark_group_delete(backend, id)
        .await
        .map_err(rejected)
}
