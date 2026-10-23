# Data Model: Локальные снимки заметок

- **SnapshotReason** — `Open | Save | Close | External | BeforeRestore | Delete`.
- **SnapshotEntry** — `id`, `at_ms`, `reason`, `size` (байт), `added`, `removed` (строк
  относительно предыдущего снимка заметки).
- **NoteRecord** (опись) — `path`, `deleted_at_ms: Option`, `entries: Vec<SnapshotEntry>`
  (старые первыми).
- **SnapshotIndex** — `version: 1`, `notes: Vec<NoteRecord>`; файл `.typsidian/snapshots/index.json`.
- **TakeOutcome** — `Taken { entry } | Skipped { why: Same | TooSoon | NotNote } | Failed { why: ReadOnly | Io }`.
  Нехватку места backend не отличает от прочих отказов записи (`WriteError::Io`), поэтому
  UI показывает предупреждение S4 «снимки не сохраняются» на любой `Failed { Io }`.
- **NoteHistory** — `path`, `entries` (новые первыми), `deleted_at_ms`.
- **DeletedNote** — `path`, `title`, `folder`, `deleted_at_ms`, `snapshots: u32`.
- **RestoreOutcome** — `EditorEdit { edit } | Written | Missing | ReadOnly | Failed`.
- **RecoverOutcome** — `Recovered { path } | Occupied { suggested } | Missing | ReadOnly | Failed`.
- **SnapshotUsage** — `snapshots: u32`, `bytes: u64`.
- **SnapshotSettings** — `keep_days` (1–365, по умолчанию 7), `interval_minutes`
  (1–120, по умолчанию 5); в `VaultOverrides.snapshots`.
