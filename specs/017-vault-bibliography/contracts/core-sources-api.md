# Contract: поверхность ядра — источники и цитаты

Модуль `packages/core/src/sources/`; два поля и одна функция — в общем
`packages/core/src/structure/` (015). Типы — [data-model.md](../data-model.md), обоснования —
[research.md](../research.md).

## Обнаружение и чтение файлов

```rust
// sources::discover
pub fn source_format(path: &Path) -> Option<SourceFormat>;     // None — не кандидат

// vault::backend::VaultBackend — новый метод, обе реализации и MemoryVault в тестах
async fn list_source_candidates(&self) -> Vec<SourceCandidate>;

// sources::bibtex, sources::yaml — чистые, без файловой системы
pub fn read_bibtex(file: &str, text: &str) -> FileReading;
pub fn read_hayagriva(file: &str, text: &str) -> FileReading;
// FileReading { sources: Vec<Source>, problems: Vec<SourceProblem>, has_entries: bool }
```

Правила, и каждое проверяется тестом:
1. Кандидат — расширение `bib`/`yml`/`yaml` без учёта регистра и `classify(path) == Other`;
   служебное место, снимки, конфликтные копии и скрытые папки не кандидаты.
2. Чтение не паникует ни на каком тексте и не возвращает `Result`: всё, что не прочиталось, —
   `SourceProblem`.
3. Одна негодная запись не убирает остальные; у неё есть строка и, если он читается, ключ
   (FR-008, research.md §2).
4. Повтор ключа в одном файле: первая запись — источник, вторая — `DuplicateKey`.
5. Больше 200 ошибок в файле → 200 названных и одна `TooMany { hidden }`.
6. TeX-записи и сокращения `@string` раскрыты в `authors`, `title`, `container`.

## Движок — состояние по обе стороны границы

```rust
// sources::engine
pub struct SourceEngine { /* SourceLibrary + CitationIndex */ }

impl SourceEngine {
    pub fn new() -> Self;

    // библиотека
    pub async fn refresh<B: VaultBackend>(&mut self, backend: &B, registered: &[String]) -> RefreshStep;
    pub fn summary(&self) -> LibrarySummary;
    pub fn search(&self, query: &str, limit: u32, uncited_only: bool) -> SourceHits;
    pub fn sources_for_completion(&self, prefix: &str, limit: u32) -> Vec<SourceHit>;   // для 018

    // индекс цитат
    pub fn scan_start(&mut self, notes: &[Note]) -> ScanView;
    pub async fn scan_step<B: VaultBackend>(&mut self, backend: &B, max_notes: u32) -> ScanView;
    pub fn scan_cancel(&mut self) -> ScanView;
    pub fn scan_view(&self) -> ScanView;
    pub async fn note_changed<B: VaultBackend>(&mut self, backend: &B, path: &str);
    pub fn note_removed(&mut self, path: &str);
    pub fn note_renamed(&mut self, from: &str, to: &str);
    pub fn citing(&self, key: &str) -> Vec<CitingNote>;
    pub fn missing_keys(&self) -> Vec<MissingKey>;

    // открытая заметка
    pub fn check_note(&self, text: &str, known_labels: &[String]) -> NoteCheck;
    pub fn plan_citation(&self, text: &str, caret: usize, file: &str, key: &str)
        -> Result<CitationPlan, CitationPlanError>;
    pub fn bibliography_offer(&self, text: &str, note_path: &str, file: &str) -> BibliographyOffer;
}
```

- `refresh`: сверяет кандидатов с загруженным по отпечатку; за вызов читает и разбирает
  **один** файл; `pending > 0` — вызвать ещё раз. YAML опознаётся по research.md §3
  (содержимое, `registered`, пути `bibliography(…)` из индекса). Файл, чтение которого
  отказало, остаётся в списке с `FileUnreadable`.
- `search`: research.md §14. Пустой запрос — все источники в порядке «первый автор, год».
  `uncited_only` до конца прохода (`ScanPhase != Done`) даёт пустую выдачу — признака ещё нет.
  `SourceHit.cited` — `None`, пока проход не завершён.
- `scan_*`: переходы — data-model.md «Индекс цитат». `scan_step` читает не больше
  `max_notes` заметок через `backend.read`; нечитаемая заметка пропускается и считается
  обработанной. Вызов вне `Running` — не ошибка, возвращает текущий вид.
- `citing`: заметки в порядке пути; в `lines` — строка с цитатой, цитата подсвечена
  сегментом (`HitLine`, как у поиска).
- `check_note`: таблица research.md §10. Ключи — объединение всех файлов источников.
- `plan_citation`: запись — research.md §12; режим — `structure::mode_at`; `caret` и замены —
  UTF-16.
- `bibliography_offer`: research.md §13; путь в заменах — от корня vault с ведущей `/`.

## Создание файла и настройка

```rust
pub async fn create_source_file<B: VaultBackend>(backend: &mut B) -> Result<String, CreateSourceFileError>;
pub fn validate_source_file(raw: &str) -> Result<String, SourceFileProblem>;   // нормализованный путь
```

`create_source_file` пишет пустой `references.bib` в корень через `write_bytes`; занятое имя —
`AlreadyExists`, vault только на чтение — `AccessDenied`. `write_vault_settings` вызывает
`validate_source_file` для каждого пути `source_files` до записи; отказ —
`VaultSettingsError::InvalidSourceFile`, файл настроек не тронут.

## Событие

`VaultEvent::SourceFilesChanged` — native: вотчер, путь-кандидат; web: `poll_for_changes`,
расхождение снимка кандидатов; оба: `write_bytes` в путь-кандидат (research.md §7).

## Расширение `structure/` (по правилу 015)

```rust
// NoteStructure += cites: Vec<CiteSite>, bibliographies: Vec<BibliographySite>
pub fn mode_at(text: &str, offset_utf16: usize) -> TextMode;
```

Новые ветки в `structure/walk.rs`; существующие поля и тесты `structure_cases` не меняются.

## Команды клавиатуры

`CommandId::InsertCitation` (`"insert-citation"`, `Mod+Alt+B`) и `CommandId::ToggleSources`
(`"toggle-sources"`, `Mod+Alt+R`) — в конец `COMMANDS` (research.md §15).

## Биндинги (принцип II: одинаковая поверхность)

| Операция | web (wasm-bindgen, `sources/web.rs`) | desktop/Android (Tauri, `packages/shell/src/lib.rs`) |
|---|---|---|
| движок | класс `SourceEngine` | состояние `Sources(Mutex<SourceEngine>)`, сбрасывается при смене vault |
| обновить | `refresh(backend, registered)` | `sources_refresh` |
| сводка, поиск | `summary()`, `search(…)`, `sourcesForCompletion(…)` | `sources_summary`, `sources_search`, `sources_for_completion` |
| проход | `scanStart(notes)`, `scanStep(backend, n)`, `scanCancel()`, `scanView()` | `sources_scan_start` (заметки берёт у бэкенда), `sources_scan_step`, `sources_scan_cancel`, `sources_scan_view` |
| правка индекса | `noteChanged(backend, path)`, `noteRemoved`, `noteRenamed` | `sources_note_changed`, `sources_note_removed`, `sources_note_renamed` |
| цитирующие | `citing(key)`, `missingKeys()` | `sources_citing`, `sources_missing_keys` |
| открытая заметка | `checkNote`, `planCitation`, `bibliographyOffer` | `sources_check_note`, `sources_plan_citation`, `sources_bibliography_offer` |
| создать файл | `createSourceFile(backend)` | `sources_create_file` |

На web вызовы с `backend` идут через `wasm-call-queue`, как `search` и `exportPdf`.
