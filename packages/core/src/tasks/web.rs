//! wasm-биндинги задач (фича 033). Тонкие обёртки: правила — в `tasks`.

use wasm_bindgen::prelude::*;

use super::{TaskFilter, TaskToggleOutcome, TaskToggleRequest, TasksView};
use crate::elements::edit::TextEdit;
use crate::vault::web::WebBackend;

/// Окно «Задачи»; `today` — `ГГГГ-ММ-ДД` по часам устройства.
#[wasm_bindgen(js_name = tasksView)]
pub async fn tasks_view_js(backend: &mut WebBackend, filter: TaskFilter, today: &str) -> TasksView {
    super::tasks_view(backend, &filter, today).await
}

#[wasm_bindgen(js_name = taskToggle)]
pub async fn task_toggle_js(
    backend: &mut WebBackend,
    request: TaskToggleRequest,
    editor_text: Option<String>,
) -> TaskToggleOutcome {
    super::task_toggle(backend, &request, editor_text.as_deref()).await
}

/// «Переключить задачу» у каретки; `undefined` — строка не пункт списка.
#[wasm_bindgen(js_name = taskToggleAt)]
pub fn task_toggle_at_js(text: &str, offset: u32) -> Option<TextEdit> {
    super::task_toggle_at(text, offset)
}
