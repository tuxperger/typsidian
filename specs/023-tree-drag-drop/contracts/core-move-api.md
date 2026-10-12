# Contract: перенос элементов vault в ядре

Модуль `packages/core/src/vault/relocate.rs`. Типы — data-model.md.

## Чистые функции (без диска, оба таргета, покрыты общими сценариями)

```rust
/// Папки, которые могут принять `from`, в порядке показа; `query` — фильтр (пусто — все).
pub fn move_targets(notes: &[Note], sections: &[SectionMark], root_name: &str,
                    from: &Path, query: &str) -> Vec<MoveTarget>;

/// Проверки, не требующие диска: SamePlace, IntoItself, ServiceFolder.
pub fn check_move(from: &Path, to_folder: &Path) -> Result<PathBuf /* новое место */, MoveRefusal>;

/// Пары переезда по заметкам и папкам для элемента `from` → `to`.
pub fn path_changes(notes: &[Note], from: &Path, to: &Path) -> (Vec<PathChange>, Vec<PathChange>);

/// Какие источники потеряют рабочую ссылку. `sources` — (путь до переноса, текст).
pub fn broken_by_move(sources: &[(PathBuf, String)], existing: &HashSet<PathBuf>,
                      mapping: &[PathChange]) -> Vec<PathBuf>;
```

Гарантии:
- `move_targets` не содержит папки-родителя `from`, самой `from` и её потомков, служебной
  папки; корень — первым, если он допустим.
- `broken_by_move` считает ссылку разорванной, только если до переноса её цель была в
  `existing`, а после (с учётом переезда и источника, и цели) — нет. Ссылки между
  заметками одной переносимой папки, записанные относительно, не разрываются.

## Операции backend

```rust
// trait VaultBackend
async fn move_targets(&self, from: &Path, query: &str) -> Vec<MoveTarget>;
async fn move_impact(&self, request: &MoveRequest) -> Result<MoveImpact, MoveRefusal>;
async fn move_entry(&mut self, request: &MoveRequest, control: &dyn MoveControl)
    -> Result<MoveOutcome, MoveError>;
```

`move_entry`:

1. Отказы до любых изменений: `AccessDenied` (как `rename`), `check_move`, `NotFound`,
   `TargetNotFound`, `NameConflict` (по диску, для каждого шага плана). Одновременный
   второй перенос исключён `&mut self`; интерфейс не начинает его по `pendingPath`.
2. Исполнение шагов плана: native — `std::fs::rename` на шаг, откат при сбое второго
   шага; web — копирование байтами с `control.progress`, проверкой `control.cancelled`
   между файлами, затем удаление источника (research.md §5–§6).
3. После успеха, до возврата: индекс, `ConflictRegistry`, кэш ссылок, пометки разделов
   — по `PathChange`. Отказ записи настроек разделов перенос не отменяет (как в `rename`).
4. События: `NoteRenamed { from_path, to_path }` на каждую заметку и доску, один
   `LinksInvalidated` со всеми затронутыми путями. `NoteDeleted`,
   `NoteDeletedWithUnsavedEdits`, `ConflictDetected`, `BulkChange*` как следствие
   собственного переноса не публикуются (FR-008).

Инварианты, проверяемые тестами на обоих таргетах:
- при `Err(_)` и `Cancelled` набор файлов vault и их содержимое побайтно прежние;
- при `Done` содержимое перенесённого дерева побайтно равно исходному, источника нет;
- `has_unsaved_edits(новый путь)` истинно, если было истинно для старого; буфер тот же;
- `move_entry(outcome.undo)` возвращает исходное состояние файлов и индекса.

## Биндинги

| Ядро | wasm (`WebBackend`) | Tauri (`packages/shell`) | `VaultClient` |
|---|---|---|---|
| `move_targets` | `moveTargets(from, query)` | `move_targets` | `moveTargets(from, query): Promise<MoveTarget[]>` |
| `move_impact` | `moveImpact(request)` | `move_impact` | `moveImpact(request): Promise<MoveImpact>` |
| `move_entry` | `moveEntry(request, onProgress, cancel)` | `move_entry` | `moveEntry(request, control?): Promise<MoveOutcome>` |

`control` в `VaultClient`: `{ onProgress?(done, total): void; signal?: AbortSignal }`. На
desktop и Android перенос атомарен, `control` не используется. Ошибки приходят в форме
`{ kind: … }` существующим путём (`Failure<MoveError>`).
