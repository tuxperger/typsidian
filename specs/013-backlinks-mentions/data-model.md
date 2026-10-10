# Data Model: Обратные ссылки и упоминания

Все типы — в `packages/core`, с `Serialize`/`Deserialize` и `Tsify` под wasm32; в UI
приходят через `import type`. Ничего из этого не хранится на диске, кроме `NotePanelState`.

## Разбор ссылок (`graph`)

### LinkSite

| Поле | Тип | Смысл |
|---|---|---|
| `raw` | `String` | цель как написана, без скобок, обрезанная по пробелам |
| `byte_start`, `byte_end` | `usize` | диапазон всей записи `[[…]]` в тексте |

Инвариант: `extract_links(text) == link_sites(text).map(raw)`.

## Связи заметки (`links`)

### OpenText

`{ path: String, text: String }` — текст заметки, открытой в редакторе. Для неё места и
фрагменты считаются по этому тексту, а не по диску (research.md §10).

### SitePlace

| Поле | Тип | Смысл |
|---|---|---|
| `line` | `u32` | строка, с единицы |
| `column` | `u32` | колонка, с единицы — как у `DiagnosticLocation` |
| `byte_start`, `byte_end` | `u32` | диапазон вхождения в тексте заметки |

### SiteRow — строка панели

| Поле | Тип | Смысл |
|---|---|---|
| `place` | `SitePlace` | куда ставить каретку |
| `text` | `String` | текст вхождения дословно (`[[Проект Альфа]]` или `проект альфа`) |
| `snippet` | `Vec<search::LineSegment>` | фрагмент; подсвечено ровно это вхождение |

### SourceGroup — заметка-источник

| Поле | Тип | Смысл |
|---|---|---|
| `path` | `String` | путь в vault |
| `title` | `String` | имя файла без расширения |
| `folder` | `String` | папка относительно корня; пустая строка — корень |
| `same_folder` | `bool` | та же папка, что у цели (папку тогда не показывают) |
| `rows` | `Vec<SiteRow>` | вхождения в порядке следования в тексте |
| `problem` | `Option<SourceProblem>` | `TooLarge` \| `Unreadable`; тогда `rows` пуст |

Правила: цель не бывает источником (FR-006); группа без строк и без `problem` не
существует.

### BacklinksPage

| Поле | Тип | Смысл |
|---|---|---|
| `target` | `String` | для какой заметки ответ (отбрасывание устаревших ответов) |
| `total_sources` | `u32` | ссылающихся заметок по индексу |
| `groups` | `Vec<SourceGroup>` | порция, ближайшие по папке первыми |
| `next_offset` | `Option<u32>` | откуда просить следующую; `None` — всё показано |

### MentionsAvailability

`Available { title }` | `TitleTooShort { title }` (FR-011).

### MentionScan — состояние прохода

| Поле | Тип | Смысл |
|---|---|---|
| `target` | `String` | цель |
| `title` | `String` | что ищем |
| `after` | `Option<String>` | путь последней обработанной заметки |
| `scanned`, `total` | `u32` | прогресс («1 240 из 5 000») |
| `skipped` | `u32` | нечитаемые и сверх предела |
| `phase` | `ScanPhase` | `Running` \| `Cancelled` \| `Finished` |

Переходы:

```text
start ──► Running ──step──► Running ──step (очередь пуста)──► Finished
              │
              └──cancel──► Cancelled        (step после этого ничего не читает)
```

`step` в `Cancelled` и `Finished` возвращает состояние без изменений и пустой список.
`total` фиксируется при `start`; заметки, появившиеся позже, проход может обработать, и
тогда `scanned` прижимается к `total`.

### MentionStep

`{ state: MentionScan, found: Vec<SourceGroup> }` — группы только этой порции.

### LinkRequest

| Поле | Тип | Смысл |
|---|---|---|
| `target` | `String` | заметка, на которую ставится ссылка |
| `source` | `String` | заметка с упоминанием |
| `place` | `SitePlace` | место из `SiteRow` |
| `text` | `String` | текст упоминания из `SiteRow` |

### LinkOutcome

| Вариант | Файл изменён | Что видит пользователь |
|---|---|---|
| `Written` | да, только упоминание | строка переезжает в обратные ссылки |
| `EditorEdit { edit: TextEdit }` | нет | замена в тексте редактора, дальше — обычное автосохранение |
| `Stale` | нет | «упоминание устарело», список обновляется (FR-015) |
| `ReadOnly` | нет | причина названа (FR-018) |
| `UnsavedElsewhere` | нет | у заметки несохранённые правки вне редактора |
| `Unexpressible` | нет | ссылку на эту заметку записать нельзя |
| `Failed` | нет | запись не удалась |

`TextEdit` — существующий `elements::edit::TextEdit` (`from`, `to` в UTF-16, `insert`).

## Состояние колонки (`settings`)

### NotePanelState — поле `Settings.note_panel`

| Поле | Тип | Умолчание | Смысл |
|---|---|---|---|
| `shown` | `bool` | `false` | колонка показана (на шторку не действует) |
| `mode` | `NotePanelMode` | `Links` | активный режим |
| `mentions_expanded` | `bool` | `false` | раскрыт ли список упоминаний (FR-012) |

`NotePanelMode { Links }` — 015 добавляет `Outline`, 017 — `Sources`. Негодное или
неизвестное значение в файле настроек читается как умолчание и не роняет файл. Меняется
через `SettingsChange.note_panel`, всегда пишется в устройство, в `VaultOverrides` не
попадает, сбросом к умолчаниям не затрагивается.

## Состояние UI (не из ядра)

`useNoteLinks` держит: номер запроса, `BacklinksPage`-и по порядку, `MentionScan` и
накопленные группы, отметки «устарело» по строкам. Это кэш ответов ядра для отрисовки;
правил в нём нет.
