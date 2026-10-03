# Contract: поверхность ядра для экспорта

Модуль `packages/core/src/export/`. Типы — см. [data-model.md](../data-model.md).

## Экспорт одной заметки

```rust
pub async fn export_note_pdf<B: VaultBackend>(
    backend: &mut B,
    packages: &PackageCache,
    note_path: &str,
    source_text: Option<&str>,   // None → читать заметку с диска (пакет, FR-022)
    fonts: FontSet,
    overrides: &VaultOverrides,  // папка экспорта + правила шаблонов
) -> ExportReport
```

Порядок, и он проверяется тестами:
1. `pdf_path = export_folder / note_path.with_extension("pdf")` (FR-027).
2. `source_text` не передан → `backend.read(note_path)`; `NotFound` → `NoteMissing`.
3. Сборка тем же циклом резолва и доустановки шаблонов, что у предпросмотра, с
   `Render::Pdf` (research.md §3). Есть ошибка → `CompileFailed`, запись не выполняется.
4. `typst_pdf::pdf` вернул ошибку → её диагностики, `CompileFailed`.
5. Путь результата занят папкой → `WriteFailed { TargetIsFolder }`.
6. `ensure_folder(parent)`, затем `write_bytes(pdf_path)`, атомарно (research.md §4).
7. Никаких иных записей в vault (FR-032), кроме установки шаблонов, как у предпросмотра.

Функция не паникует и не возвращает `Result`: каждый исход — вариант `ExportOutcome`.

## Кандидаты и папка

```rust
pub fn candidates(notes: &[Note], overrides: &VaultOverrides) -> Vec<ExportCandidate>;
pub fn validate_folder(raw: &str) -> Result<String, ExportFolderProblem>; // нормализованное
pub fn export_folder(overrides: &VaultOverrides) -> String;              // с умолчанием
```

`write_vault_settings` вызывает `validate_folder` до записи. При отказе возвращается
`VaultSettingsError::InvalidExportFolder { problem }`, файл не тронут.

## Пакет

```rust
// модуль export::batch
pub fn start(paths: Vec<String>, stop_on_error: bool, overrides: &VaultOverrides) -> BatchState;
pub fn step(state: BatchState) -> BatchStep;            // { state, next: Option<String> }
pub fn record(state: BatchState, report: ExportReport, elapsed_ms: u32) -> BatchState;
pub fn skip(state: BatchState, note_path: &str) -> BatchState;
pub fn cancel(state: BatchState) -> BatchState;
pub fn view(state: BatchState) -> BatchView;          // state + progress, estimate, summary, is_over
pub async fn candidates_of<B: VaultBackend>(backend: &B, overrides: &VaultOverrides) -> Vec<ExportCandidate>;
```

## Команды клавиатуры

`CommandId::ExportPdf` (`"export-pdf"`, `Mod+P`, «Экспорт в PDF») и `CommandId::ExportCenter`
(`"export-center"`, `Mod+Shift+P`, «Центр экспорта») — в конец `COMMANDS`.

## Биндинги (принцип II: одинаковая поверхность)

| Операция | web (wasm-bindgen) | desktop/Android (Tauri) |
|---|---|---|
| экспорт заметки | `PreviewEngine.exportPdf(backend, path, text?, overrides)` | `export_pdf` |
| кандидаты | `exportCandidates(backend, overrides)` — список берётся у бэкенда | `export_candidates` |
| пакет | `exportBatchStart/Next/Record/Skip/Cancel/View` | `export_batch_*` |

На web вызовы `exportPdf` идут через `wasm-call-queue`, как `compile`: заимствование
`WebBackend` на время сборки.
