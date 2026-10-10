# Data Model: Дополнение меток и ссылок

Все типы — в ядре (`packages/core/src/completion/`, `preview/evidence.rs`), наружу — через
`Tsify`. Ничего из перечисленного не хранится на диске, кроме одного поля настроек.

Единицы мест: в тексте **редактора** — кодовые единицы UTF-16 от начала текста
(`from`/`to`), как у `elements` (008); в **файлах vault** — `DiagnosticLocation` (005).

## BuildEvidence (свидетельство сборки, `preview/evidence.rs`)

| Поле | Тип | Смысл |
|---|---|---|
| `labels` | `Vec<BuiltLabel>` | метки собранного документа, по имени |
| `references` | `Vec<String>` | имена ссылок `@имя` в тексте заметки, из которого собран документ; без повторов |

`BuiltLabel { name: String, count: u32 }` — `count ≥ 1`.

Живёт в `CompiledDocument.evidence: Option<BuildEvidence>`; `Some` ⇔ `outcome == Succeeded`.
Запрещённое состояние: `Some` у неудавшейся сборки.

## LinkedIndex (метки связанных файлов)

| Поле | Тип | Смысл |
|---|---|---|
| `note_path` | `String` | заметка, для которой собран перечень |
| `files` | `Vec<LinkedFile>` | связанные файлы `.typ` по алфавиту путей |
| `unreadable` | `Vec<String>` | пути, которые не прочитались |

`LinkedFile { path: String, labels: Vec<LinkedLabel> }`;
`LinkedLabel { name, kind: LabelKind, text: String, line: u32, column: u32, byte_start: u32,
byte_end: u32, top_level: bool }`.

`LabelKind` — перечисление фичи 015 (`structure`): заголовок, рисунок, таблица, формула,
прочее. 018 его не дублирует.

Правила: перечень для другой заметки функциями 018 не принимается (сверка `note_path`);
открытая заметка в `files` не входит.

## CompletionRequest

| Поле | Тип | Смысл |
|---|---|---|
| `note_path` | `String` | открытая заметка |
| `text` | `String` | буфер редактора (FR-008) |
| `offset` | `u32` | каретка, UTF-16 |
| `manual` | `bool` | вызов командой, а не набором |
| `linked` | `LinkedIndex` | метки связанных файлов |
| `sources` | `Vec<SourceKey>` | ключи источников 017; пусто, если библиографии нет |

`SourceKey { key: String, summary: String }` — `summary` (авторы и год одной строкой)
формирует 017; 018 показывает как есть.

## CompletionList

| Поле | Тип | Смысл |
|---|---|---|
| `target` | `CompletionTarget` | `Label`, `Note` или `None { reason }` |
| `from`, `to` | `u32` | диапазон набранной части, который заменит выбор (UTF-16) |
| `query` | `String` | набранная часть |
| `groups` | `Vec<CompletionGroup>` | группы по порядку показа |

`CompletionGroup { kind: GroupKind, total: u32, items: Vec<CompletionItem> }`, `GroupKind` —
`Labels`, `Sources`, `Notes`; `items.len() ≤ 50`, `total` — сколько подошло всего.

`CompletionItem`:

| Поле | Тип | Смысл |
|---|---|---|
| `label` | `String` | имя метки, ключ или имя заметки |
| `matched` | `Vec<(u32, u32)>` | отрезки совпадения в `label` (UTF-16) для подсветки |
| `detail` | `ItemDetail` | см. ниже |
| `insert` | `String` | что встанет на место `from..to` |
| `preview` | `String` | итоговая запись словами подвала: `@рис-схема`, `[[План]]` |

`ItemDetail`:
- `Label { kind: LabelKind, text: String, file: Option<String>, duplicate: bool }` — `file`
  есть у метки из связанного файла;
- `Source { summary: String }`;
- `Note { folder: String, same_folder: bool }`.

`NoCompletionReason` — `NoTrigger`, `InCode`, `InString`, `InComment`, `InRaw`, `InMath`,
`InAddress` (почта, автоссылка). Причина нужна тестам и ручному вызову; при наборе UI её не
показывает.

Инварианты: `target == None` ⇒ `groups` пусты; группа без строк не отдаётся; при `target ==
Note` группа одна — `Notes`.

## DefinitionRequest / DefinitionOutcome

`DefinitionRequest { note_path, text, offset, linked, evidence: Option<BuildEvidence>,
sources: Vec<String> }`.

`DefinitionOutcome`:
- `Place { location: DiagnosticLocation }`;
- `Source { key: String }`;
- `Nothing { reason: NoDefinition }`.

`NoDefinition` — `NoReferenceAtCaret`, `LabelMissing { name }`, `LabelFromBuild { name }`
(метка есть в собранном документе, но места в тексте не имеет), `NoteMissing { target }`,
`OutsideVault { target }`.

## MarksRequest / ReferenceMark

`MarksRequest`:

| Поле | Тип | Смысл |
|---|---|---|
| `note_path`, `text`, `linked` | — | как выше |
| `evidence` | `Option<BuildEvidence>` | последняя удачная сборка этой заметки |
| `sources` | `Vec<String>` | ключи источников 017 |
| `claimed` | `Vec<(u32, u32)>` | диапазоны, уже отмеченные 017 (UTF-16) |
| `held` | `bool` | сборка текущего текста ещё не вернулась |
| `previously_missing` | `Vec<String>` | имена, отмеченные «метки нет» в прошлом ответе |

`ReferenceMark { kind: MarkKind, from: u32, to: u32, line: u32 }`.

`MarkKind`:
- `MissingLabel { name }`;
- `MissingNote { target: String, create_path: String }` — `create_path` — путь в vault, по
  которому действие создаст заметку;
- `LinkOutsideVault { target }`;
- `DuplicateLabel { name, others: Vec<DuplicatePlace> }` — `DuplicatePlace { path:
  Option<String>, line: u32 }`, `path` — у объявления в связанном файле.

Правила (research.md §4, §8):
- `MissingLabel` ⇔ имя не найдено в тексте заметки и связанных файлов, не является ключом
  источника, не подтверждено `evidence` и диапазон не занят `claimed`;
- при непустом `linked.unreadable` `MissingLabel` не выдаётся вовсе (research.md §3);
- при `held` новые `MissingLabel` (имени нет в `previously_missing`) не выдаются;
- `DuplicateLabel` в пределах заметки — оба объявления `top_level`; между файлами и для
  объявлений не верхнего уровня — только при `evidence.labels[name].count ≥ 2`;
- отметки отсортированы по `from` и не пересекаются.

## Настройка

`Settings.editor_completion_auto: bool` (настройки устройства, 004), умолчание `true`;
отсутствующее поле в файле читается как `true`. Изменение — `SettingsChange` с тем же
полем-`Option`.

## Команды

`CommandId::GotoDefinition` (`goto-definition`, `Mod+Alt+D`),
`CommandId::TriggerCompletion` (`trigger-completion`, `Mod+Alt+P`) — в `COMMANDS` после
`ElementParams`.

## Состояние в UI (не логика)

| Что | Где | Время жизни |
|---|---|---|
| `LinkedIndex` открытой заметки | `useLinkedIndex` | до смены заметки или отметки зависимостей |
| открыт ли список, номер запроса, выбранная строка | `useCompletion` | сеанс списка |
| последние отметки и имена `previously_missing` | `useReferenceMarks` | до смены заметки |
| текст отказа перехода | `useGotoDefinition` | до следующего движения каретки |
