# Data Model: Библиография vault

Все типы — Rust в `packages/core/src/sources/` (и два поля в `structure/`), наружу через
`Tsify`; в UI — только `import type` (принцип IV). Места в тексте заметки — `TextPlace`
фичи 015 (`specs/015-note-outline/data-model.md`): `line`/`column` с единицы, `from`/`to` в
кодовых единицах UTF-16. Замены — `elements::edit::TextEdit`. Фрагменты — `search::HitLine`.

## Файл источников

```text
SourceFormat   = BibTex | Hayagriva
SourceCandidate { path, size, modified_ms }          // обход, только метаданные
SourceFileInfo  { path, format, source_count, problem_count,
                  recognized_by: Extension | Content | Referenced | Registered }
```

- Кандидат: расширение `bib`/`yml`/`yaml`, `classify(path) == Other`, вне скрытых папок.
- `.bib` — источник всегда; YAML — по правилам research.md §3.
- Отпечаток `{size, modified_ms}` решает, перечитывать ли файл.

## Источник

```text
Source {
  key: String,
  authors: Vec<String>,        // «Фамилия И.» в порядке записи; пусто — авторов нет
  title: String,               // пусто — названия нет
  year: Option<i32>,
  kind: String,                // идентификатор вида записи Hayagriva: "article", "book", …
  container: Option<String>,   // журнал, сборник — название родительской записи
  file: String,                // путь файла источников в vault
  line: u32,                   // строка записи в файле
}
```

Тождество источника — пара `(file, key)`. Один ключ в двух файлах — два источника
(Edge Cases). Повтор ключа внутри одного файла — первая запись источник, вторая — ошибка.

## Ошибка чтения

```text
SourceProblem {
  file: String,
  line: u32, column: u32,      // 0 — место неизвестно
  key: Option<String>,         // None — ошибка уровня файла
  kind: SourceProblemKind,
}
SourceProblemKind =
    Syntax { detail }          // разбор записи
  | Field { field, detail }    // негодное значение поля
  | DuplicateKey
  | NoEntryType                // запись Hayagriva без вида
  | FileUnreadable { reason }  // чтение файла отказало (ReadError)
  | FileSyntax { detail }      // YAML не разобрался целиком
  | TooMany { hidden: u32 }    // свёрнутый хвост сверх 200
```

`detail` — текст крейта (английский, технический): показывается как подробность под
переведённой причиной, как сообщения компилятора Typst в предпросмотре.

## Библиотека источников

```text
SourceLibrary { files: Map<path, LoadedFile{stamp, info, sources, problems}>,
                haystack }                       // подготовленные строки поиска
RefreshStep   { changed: bool, pending: u32 }    // pending > 0 → вызвать ещё раз
LibrarySummary{ files: Vec<SourceFileInfo>, source_count, problems: Vec<SourceProblem>,
                yaml_candidates: Vec<String> }   // неопознанные YAML — для «указать»
SourceHit     { source: Source, cited: Option<u32> }   // None — индекс не построен
SourceHits    { hits: Vec<SourceHit>, total_matches: u32 }
```

Переходы `refresh`: сверка кандидатов с `files` → удалённые убираются, новые и изменившиеся
встают в очередь → за вызов читается один файл → `pending` убывает до 0.

## Цитата

```text
Citation      { key, place: TextPlace, form: Short | Call }
CitingNote    { path, name, lines: Vec<HitLine>, places: Vec<CitationPlace> }
CitationPlace { line, column }                    // для перехода (revealPlace, 013)
MissingKey    { key, notes: u32, first: CitationPlace, first_path }
```

## Индекс цитат

```text
CitationIndex { notes: Map<path, NoteCitations{stamp, citations, bibliography_paths}>,
                by_key: Map<key, Set<path>>, queue, phase }
ScanPhase = Idle | Running | Done | Cancelled
ScanView  { phase, processed: u32, total: u32 }
```

| Из | Событие | В |
|---|---|---|
| Idle, Done, Cancelled | `scan_start(notes)` | Running (заметки с прежним отпечатком в очередь не идут) |
| Running | `scan_step`, очередь не пуста | Running |
| Running | `scan_step`, очередь пуста | Done |
| Running | `scan_cancel` | Cancelled (собранное сохранено) |
| любое | `note_changed` / `note_removed` / `note_renamed` | то же (правка одной заметки) |

«Не цитируется» выводится только в `Done`.

## Вставка

```text
CitationPlan       { edits: Vec<TextEdit>, caret: usize }
CitationPlanError  = NotInsertable { where_: String | Comment } | UnknownKey
BibliographyOffer  = None
                   | Add    { file, edits: Vec<TextEdit> }
                   | Extend { file, edits: Vec<TextEdit> }
```

## Строение текста — расширение `structure/` (015)

```text
CiteSite         { key: String, place: TextPlace }            // вызов cite(…) целиком
BibliographySite { paths: Vec<String>, literal: bool,
                   place: TextPlace, arg: TextPlace }         // arg — место аргумента с путями
TextMode         = Markup | Math | Code | String | Comment
NoteStructure   += cites: Vec<CiteSite>, bibliographies: Vec<BibliographySite>
```

`literal = false` — аргумент не строка и не массив строк; `paths` тогда пуст.

## Отметки в редакторе

```text
CitationMark { place: TextPlace, kind: CitationMarkKind }      // sources/check.rs
CitationMarkKind = MissingSource { key, ambiguous: bool }
NoteCheck { marks: Vec<CitationMark>, citations: u32, broken: u32,
            has_bibliography: bool }
EditorMark { from, to, tone: "error" | "warning", message }    // ui/src/editor/marks.ts, тип UI
```

`ambiguous` — запись `@k`, которая могла быть и ссылкой на метку (research.md §10): от него
зависит текст причины. `EditorMark` — не тип ядра: это вход общего механизма подачи, куда
017 и 018 кладут уже переведённые причины.

## Настройки и события

- `VaultOverrides.source_files: Vec<String>` — YAML, указанные пользователем;
  `VaultSettingsError::InvalidSourceFile { problem }`.
- `VaultEvent::SourceFilesChanged` — кандидат создан, изменён, удалён или переименован.
- `CreateSourceFileError = AlreadyExists | AccessDenied | Io`.
- Состояние устройства (004, `Settings.note_panel` фичи 013): вариант
  `NotePanelMode::Sources` и поле `sources_uncited_only: bool` (по умолчанию `false`).
- Команды: `CommandId::InsertCitation`, `CommandId::ToggleSources` (research.md §15).
