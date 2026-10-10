# Contract: поверхность ядра для вкладок

Модуль `packages/core/src/workspace/`. Типы — [data-model.md](../data-model.md).

## Переходы и показ (чистые)

```rust
pub fn apply(state: WorkspaceState, action: &WorkspaceAction, facts: &Facts) -> Transition;
pub fn view(state: &WorkspaceState, facts: &Facts) -> WorkspaceView;
pub fn snapshot(state: &WorkspaceState) -> SavedWorkspace;
pub fn restore(saved: &SavedWorkspace, notes: &[Note]) -> WorkspaceState;

pub const SPLIT_MIN_WIDTH_PX: u32 = 900;
pub const COMPACT_MAX_WIDTH_PX: u32 = 720;
pub const HISTORY_LIMIT: usize = 100;
```

Обязательства, каждое — тестом на обоих таргетах:

1. `apply` не паникует ни на каком сочетании состояния и действия: неизвестная вкладка,
   отсутствующая группа, действие в пустом состоянии дают `rejected`, состояние прежнее.
2. Инварианты `WorkspaceState` (data-model.md) выполняются после любого перехода.
3. Отказ закрытия решается только по `Facts`: вкладка с `unsaved` или `conflict` не
   закрывается, если её путь не открыт в другой группе (FR-005).
4. `view` не меняет состояние; при ширине меньше порога возвращает слитую подачу, а две
   группы в состоянии остаются (FR-020).
5. Набор текста модель не вызывает: ни одного действия «текст изменился» нет (FR-027).

## Факты

```rust
pub async fn facts_of<B: VaultBackend>(
    backend: &B,
    state: &WorkspaceState,
    action: Option<&WorkspaceAction>,
    width_px: u32,
) -> Facts;
```

Берёт из индекса бэкенда записи путей, названных состоянием (вкладки, история) и действием
(`Open.path`, `Renamed.to`): вид, `has_unsaved_edits`; `conflict` — `pending_conflict(path)`
либо путь среди `unresolved_edit_paths()` при отсутствующем файле. Пути без записи в индексе
в `Facts` не попадают.

## Обёртки с бэкендом (их и зовут биндинги)

```rust
pub async fn apply_with<B: VaultBackend>(backend: &B, state: WorkspaceState,
    action: WorkspaceAction, width_px: u32) -> Transition;
pub async fn view_with<B: VaultBackend>(backend: &B, state: &WorkspaceState,
    width_px: u32) -> WorkspaceView;
pub async fn restore_with<B: VaultBackend>(backend: &B, saved: &SavedWorkspace) -> WorkspaceState;
```

## Настройки устройства

- `Settings.workspaces: Vec<VaultWorkspaceEntry>` и `Settings::workspace_for(&VaultId) ->
  Option<SavedWorkspace>` — `packages/core/src/settings/mod.rs`; терпимый разбор.
- `SettingsChange.workspace: Option<VaultWorkspaceEntry>` — `apply.rs`; `route` отправляет
  его в `ChangeTarget::Device` при любом раскладе (FR-022).
- `format.rs` пишет ключ `workspaces`, пустой список ключ убирает.
- Удаление записи vault из списка известных убирает его набор.
- Набор изменений из одного `workspace` не порождает записи журнала.

## Клавиатура

`CommandId::{TabClose, TabNext, TabPrevious, SplitEditor, TabMoveToOtherGroup, NavigateBack,
NavigateForward}` — в конец `COMMANDS`; умолчания — research.md §12. `Shortcut::code()`
возвращает имя физической клавиши и для знаков препинания.

## Правки существующей поверхности

`VaultBackend::discard_edit` получает метод клиента и команду Tauri (биндинг wasm
`discardEdit` уже есть).

## Биндинги (принцип II: одна поверхность)

| Операция | web (wasm-bindgen, `workspace/web.rs`) | desktop/Android (Tauri, `packages/shell/src/lib.rs`) |
|---|---|---|
| переход | `workspaceApply(backend, state, action, widthPx)` | `workspace_apply` |
| показ | `workspaceView(backend, state, widthPx)` | `workspace_view` |
| снимок набора | `workspaceSnapshot(state)` | `workspace_snapshot` |
| восстановление | `workspaceRestore(backend, saved)` | `workspace_restore` |
| пустое состояние | `workspaceEmpty()` | `workspace_empty` |
| снять буфер | `WebBackend.discardEdit(path)` (есть) | `discard_edit` |

На web вызовы с бэкендом идут через `wasm-call-queue`, как остальные методы `VaultClient`.
