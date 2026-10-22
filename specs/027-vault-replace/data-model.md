# Data Model: Поиск с заменой по vault

- **ReplaceQuery** — `search: SearchQuery`, `replacement: String`.
- **ReplacePreview** — `notes: Vec<ReplaceNote>`, `total_matches`, `total_notes`.
- **ReplaceNote** — `path`, `name`, `folder`, `digest` (отпечаток текста),
  `matches: Vec<ReplaceMatch>`, `open_in_editor: bool`.
- **ReplaceMatch** — `index` (порядковый номер в заметке), `line`, `pieces: Vec<ReplacePiece>`.
- **ReplacePiece** — `text`, `kind: Plain | Removed | Inserted`.
- **ReplaceNoteRequest** — `query: ReplaceQuery`, `path`, `digest`, `selected: Vec<u32>`.
- **ReplaceNoteOutcome** — `Written { undo }` · `EditorEdit { edit, undo }` · `Stale` ·
  `UnsavedElsewhere` · `ReadOnly` · `Failed` · `NothingSelected`.
- **UndoRecord** — `path`, `before: String`, `after_digest`.
- **UndoOutcome** — `Restored` · `EditorEdit { edit }` · `ChangedSince` · `ReadOnly` ·
  `Failed` · `Missing`.
- **ReplaceError** — `BadPattern { message }` · `TooMany { count }`.
