# Contract: замена по vault

Ядро (`search::replace`), одинаково для wasm (`WebBackend`) и Tauri:

| Вызов | Вход | Выход |
|---|---|---|
| `replace_preview` | `ReplaceQuery`, тексты открытых заметок | `ReplacePreview` либо `ReplaceError` |
| `replace_in_note` | `ReplaceNoteRequest`, текст редактора или `None` | `ReplaceNoteOutcome` |
| `replace_undo` | `UndoRecord`, текст редактора или `None` | `UndoOutcome` |

Обязательства: `replace_in_note` меняет только отмеченные совпадения; при расхождении
отпечатка — `Stale` без записи; для открытой заметки на диск не пишет. `replace_undo` пишет
только при совпадении `after_digest`.
