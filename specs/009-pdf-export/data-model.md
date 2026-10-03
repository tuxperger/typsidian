# Data Model: PDF Export

Все типы — в `packages/core/src/export/`, с `Serialize/Deserialize` и `Tsify` под wasm32
(принцип IV: TS-типы порождаются из Rust).

## ExportFolder

Не отдельный тип, а поле `VaultOverrides::export_folder: Option<String>` (`None` → `"export"`).

Правила (`export::folder::validate`), нарушение → `ExportFolderProblem`:

| Правило | Проблема |
|---|---|
| не пустое после обрезки пробелов и `/` | `Empty` |
| относительное: без корня, без префикса диска | `Absolute` |
| без компонентов `..` и `.` | `LeavesVault` |
| не `.typsidian` и не внутри неё | `ServiceFolder` |

Разделитель в хранимом значении — `/`, а завершающий `/` отбрасывается.

## ExportCandidate

Строка Центра экспорта (FR-010, FR-011).

| Поле | Тип | Смысл |
|---|---|---|
| `path` | `String` | путь в vault |
| `folder` | `String` | родительская папка (`""` — корень), для группировки |
| `name` | `String` | имя файла |
| `exportable` | `bool` | можно ли отметить |
| `reason` | `Option<NotExportable>` | `Board` · `InsideExportFolder` |

Порядок — по пути, как в дереве. Диапазон Shift-выбора (FR-012) строится в этом порядке.

## ExportReport

Результат экспорта одной заметки (`export::export_note_pdf`).

| Поле | Тип | Смысл |
|---|---|---|
| `note_path` | `String` | |
| `pdf_path` | `String` | путь результата в vault (FR-027) |
| `outcome` | `ExportOutcome` | см. ниже |
| `diagnostics` | `Vec<Diagnostic>` | те же, что у предпросмотра (FR-008, FR-016) |

`ExportOutcome`:
- `Written { bytes: u64 }` — файл записан; предупреждения, если есть, лежат в `diagnostics`.
- `CompileFailed` — среди диагностик есть ошибка, файл не тронут.
- `WriteFailed { reason: ExportWriteProblem }` — `AccessDenied` · `TargetIsFolder` · `Io`.
- `NoteMissing` — заметки нет (удалена или переименована во время пакета).

Инвариант: `Written` ⇒ в `diagnostics` нет ни одной ошибки.

## BatchState

Состояние пакета (FR-015 … FR-023). Меняется только переходами ядра.

| Поле | Тип |
|---|---|
| `jobs` | `Vec<BatchJob>` в порядке списка |
| `stop_on_error` | `bool` |
| `phase` | `BatchPhase`: `Running` · `Cancelled` · `StoppedOnError { note_path }` · `Finished` |
| `export_folder` | `String` — куда пишутся файлы, для итога (FR-023) |

`BatchJob`:

| Поле | Тип |
|---|---|
| `note_path` | `String` |
| `status` | `JobStatus` |
| `elapsed_ms` | `Option<u32>` |
| `report` | `Option<ExportReport>` |

`JobStatus`: `Queued` · `Exporting` · `Done` · `DoneWithWarnings` · `Failed` · `Skipped` · `Cancelled`.

Производные (методы, а не поля; сериализуются в `BatchView`):
- `progress` = число заданий не в `Queued`/`Exporting` из общего;
- `remaining_estimate_ms` = среднее `elapsed_ms` готовых × число `Queued` (+ текущее, если
  есть); `None`, пока нет ни одного завершённого (FR-017);
- `summary` = счётчики `Done+DoneWithWarnings`, `Failed`, `Skipped+Cancelled` (FR-023).

### Переходы

```text
start(paths, stop_on_error, folder) → всё Queued, phase Running
next(state)          → первый Queued → Exporting, возвращает его путь;
                        нет Queued → phase Finished, путь None
                        phase ≠ Running → путь None (отмена побеждает)
record(state, report, ms)   — путь берётся из report.note_path
                     → Exporting → Done | DoneWithWarnings | Failed
                        Failed && stop_on_error → phase StoppedOnError,
                                                   все Queued → Cancelled
skip(state, path)    → Queued → Skipped; иначе без изменений
cancel(state)        → phase Cancelled, все Queued → Cancelled;
                        Exporting остаётся до record (запись атомарна)
```

Запрещённые состояния, которые проверяются тестами:
- больше одного `Exporting`;
- `Exporting` при `phase = Finished`;
- `record` для пути не в `Exporting` меняет состояние (такой вызов игнорируется).
