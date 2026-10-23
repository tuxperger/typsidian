# Data Model: Задачи из заметок

```text
VaultTask   { path, line (с 1), indent, done, text (без срока), raw (строка целиком),
              due: Option<"ГГГГ-ММ-ДД">, parent: Option<line> }
TaskIndex   { version, notes: { path → { stamp, tasks: [VaultTask] } } }   // .typsidian/tasks.json
TaskFilter  { state: Open|Done|All, due: Any|Overdue|Today|Week|None, folder: Option<String>,
              conditions: Vec<Condition>, group: ByNote|ByDue }
TasksView   { groups: [TaskGroup { key, title, folder?, tasks: [TaskRow] }],
              open: u32, overdue: u32, total: u32 }
TaskRow     { task: VaultTask, note_title, due_state: None|Overdue|Today|Week|Later }
TaskToggleRequest { path, line, raw }
TaskToggleOutcome = EditorEdit { edit } | Written | Stale | ReadOnly | Failed
```
