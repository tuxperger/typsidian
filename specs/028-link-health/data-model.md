# Data Model: Битые ссылки и заметки-сироты

- **LinkHealth** — `broken: Vec<BrokenTarget>`, `broken_links: u32`, `orphans: Vec<Orphan>`,
  `intentional: u32`.
- **BrokenTarget** — `target` (путь), `title`, `kind: Note | Board | Attachment`,
  `sources: u32`, `sites: Vec<BrokenSite>`.
- **BrokenSite** — `source`, `source_title`, `digest`, `index`, `line`,
  `pieces: Vec<ReplacePiece>` (было — запись ссылки).
- **Orphan** — `path`, `title`, `folder`, `modified_ms`, `size`, `kind: Note | Entry`.
- **LinkFixRequest** — `source`, `target`, `digest`, `action: Redirect { to } | Unlink`,
  `only: Option<u32>`.
- Исход — `ReplaceNoteOutcome` 027; отмена — `UndoRecord` / `UndoOutcome` 027.
