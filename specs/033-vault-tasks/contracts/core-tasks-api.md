# Contract: ядро — задачи

```text
tasks::tasks_in(text) -> Vec<VaultTask>                       // разбор одной заметки
tasks::tasks_view(backend, filter, today) -> TasksView        // индекс + фильтры
tasks::task_toggle(backend, request, editor_text) -> TaskToggleOutcome
tasks::task_toggle_at(text, offset) -> Option<TextEdit>
```

Клиент: `client.tasks.view(filter)`, `client.tasks.toggle(request, editorText)`,
`client.tasks.toggleAt(text, offset)`; «сегодня» берёт граница (часы устройства).
