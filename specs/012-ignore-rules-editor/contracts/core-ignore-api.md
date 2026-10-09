# Contract: ядро — правила исключения

Типы — [data-model.md](../data-model.md). Все операции одинаковы на обоих таргетах.

## Чистые функции (`sync::ignore`)

| Функция | Вход | Выход | Требования |
| --- | --- | --- | --- |
| `check(file, text)` | `IgnoreFile`, `&str` | `IgnoreCheck` | FR-010, FR-013 |
| `restore_block(file, text)` | `IgnoreFile`, `&str` | `String` | FR-011 |

- `check(file, t).missing == report(file, Some(t)).missing` для любого `t`.
- `restore_block` = `apply` (011) для текста с `\n`: строки вне блока — символ в символ,
  `restore_block(restore_block(t)) == restore_block(t)`,
  `check(restore_block(t)).missing` пусто, если пользователь не отменяет правило дословно
  ниже блока.

## Операции сессии (`SyncSession`)

| Операция | Вход | Выход | Требования |
| --- | --- | --- | --- |
| `ignore_open(backend, file)` | `IgnoreFile` | `IgnoreDocument` | FR-001, FR-005, FR-007, FR-016 |
| `ignore_save(backend, ctx, request)` | `IgnoreSaveRequest` | `Result<IgnoreSaveOutcome, SyncError>` | FR-003, FR-004, FR-009, FR-012, FR-014…FR-018 |

`ignore_open` не отказывает: всё, что мешает правке, — в `editable`.

`ignore_save`:

- пишет ровно `request.text` с окончаниями `request.line_ending`, атомарно;
- не пишет ничего при любом исходе, кроме `Saved`;
- после `Saved` выпускает `SyncEvent::OverviewChanged` (FR-009);
- `SyncError::Io` — только сбой чтения или записи; текст запроса при этом не потерян
  (он у вызывающего).

## Привязки

| Ядро | wasm (`SyncSession` в `sync/web.rs`) | Tauri (`packages/shell/src/sync.rs`) |
| --- | --- | --- |
| `ignore_open` | `ignoreOpen(backend, file)` | `sync_ignore_open { file }` |
| `ignore::check` | свободная функция `ignoreCheck(file, text)` | `sync_ignore_check { file, text }` |
| `ignore::restore_block` | свободная функция `ignoreRestore(file, text)` | `sync_ignore_restore { file, text }` |
| `ignore_save` | `ignoreSave(backend, ctx, request)` | `sync_ignore_save { request }` |

## `VaultBackend`

Новый метод `is_symlink(&self, path) -> bool`, умолчание `false`; native — по
`symlink_metadata`. Других изменений контракта хранилища нет.
