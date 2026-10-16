# Data Model: Импорт из Obsidian

## ImportJob

| Поле | Тип | Смысл |
|---|---|---|
| `destination` | `String` | папка vault |
| `on_conflict` | `Skip \| Beside` | FR-009 |
| `copy_other_files` | `bool` | |

## ImportSurvey

Ответ после выбора источника (C1): `notes`, `images`, `other`, `excluded` (служебные папки),
`resume: Option<{ done, total, at }>` (C4).

## ImportState

Держит окно: `plan` (файлы, имена назначения, карта ссылок), `next`, `done`, `total`,
`report`, `cancelled`. Переход — `import_step(state, backends) -> ImportState`.

## ReportEntry

| Поле | Тип |
|---|---|
| `source` | `String` |
| `destination` | `Option<String>` |
| `outcome` | `Clean \| Lossy \| Skipped` |
| `losses` | `Vec<{ kind: LossKind, line: u32 }>` |
| `skip_reason` | `Option<SkipReason>` |
| `renamed` | `Option<String>` |

`LossKind`: `Html`, `Embed`, `PluginBlock`, `Diagram`, `Math`, `SectionLink`, `BlockLink`,
`MissingTarget`, `AmbiguousTarget`. `SkipReason`: `TooLarge`, `NotUtf8`, `OtherFile`,
`ExistsSkipped`.

## ImportJournal

`.typsidian/imports/<id>.json`: `{ version, source_hint, destination, done: [{ source, destination }] }`.

## LinkSite (001, изменение)

`+ label: Option<String>` — текст после `|`.

## Уточнения после реализации ядра (2026-10-15)

- `ImportJob` + `started_ms: f64` (время запуска для журнала и экрана C4) и `texts: ReportTexts` (тексты
  заметки отчёта из переводов окна).
- `ImportState`: `id`, `destination`, `phase: Running | Done`, `next`, `total`, `clean`, `lossy`, `skipped`,
  `resumed`, `current`, `recent` (до 20 последних проблемных `ReportEntry`). План и полный отчёт лежат в ядре
  (research.md §11).
- `LossKind` + `Properties`, `Syntax`; `SkipReason` + `Unreadable`, `WriteFailed`; `ReportEntry.outcome` —
  `Clean | Lossy | Skipped`.
- `ImportJournal` + `started_ms`, `total`, `attachments`, `skipped`.
- `ImportError`: `NoSource | SessionLost | AccessDenied | Io`.
