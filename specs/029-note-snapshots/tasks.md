# Tasks: Локальные снимки заметок

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md)

## Phase 1: Основа

- [X] T001 `remove_service_file` в `VaultBackend` (`packages/core/src/vault/backend.rs`): native и web удаляют файл служебной папки мимо корзины, вне неё — отказ; обёртка «только чтение» отказывает; двойник `MemoryVault` в `tests/common/mod.rs` (research.md §3)
- [X] T002 Путь `.typsidian/snapshots` в `service_dir.rs` и `derived_paths()`; таблица `snapshots` (`keep_days`, `interval_minutes`) в `vault_settings.rs` с тестом разбора и записи

## Phase 2: User Story 1 — вернуть прошлую версию (P1)

- [X] T003 [US1] Тесты ядра `packages/core/tests/common/snapshots_cases.rs`, вызовы в `snapshots_native.rs` и `snapshots_wasm.rs`: снимок и пропуск `Same`/`TooSoon`, +/− строк, история новыми первыми, различия, возврат в редактор и в файл байт в байт со снимком «перед возвратом», срок хранения убирает записи и файлы, перенос истории при переименовании (FR-001…FR-003, FR-005, SC-002)
- [X] T004 [US1] `packages/core/src/snapshots/mod.rs`: опись, `snapshot_take`, `snapshot_history`, `snapshot_diff`, `snapshot_restore`, `move_snapshots`, срок (research.md §1–§4); экспорт в `lib.rs`
- [X] T005 [US1] Перенос истории при переименовании в `vault/{native,web}.rs` и переносе в `vault/relocate.rs`
- [X] T006 [US1] Привязки: `packages/core/src/snapshots/web.rs`, команды Tauri в `packages/shell/src/lib.rs`, клиент `ui/src/vault-client{,-web,-tauri}.ts`
- [X] T007 [US1] UI: снимки при открытии, сохранении, закрытии вкладки и внешнем изменении — `ui/src/workspace/Workspace.tsx`; окно «История заметки» с различиями, возвратом и версиями git 011 — `ui/src/snapshots/NoteHistory.tsx`, `useSnapshots.ts`; команда палитры; тексты `ui/src/i18n/locales/{ru,en}/snapshots.ts`; стили (макет S1, S4)

## Phase 3: User Story 2 — удалённая заметка (P2)

- [X] T008 [US2] Тесты ядра: удаление делает снимок и отметку, список удалённых, восстановление на прежний путь, занятый путь — `Occupied` с новым именем, восстановленная снова обычная — `snapshots_cases.rs` (FR-004)
- [X] T009 [US2] Снимок перед удалением в `vault/{native,web}.rs`; `deleted_notes`, `recover_note` в `snapshots`; окно «Недавно удалённые» — `ui/src/snapshots/RecentlyDeleted.tsx`, команда палитры (макет S2)

## Phase 4: User Story 3 — срок и объём (P3)

- [X] T010 [US3] `snapshot_usage`, `snapshots_clear` с тестами; раздел «Снимки заметок» в настройках vault: срок, интервал, занято, «Очистить» с подтверждением (макет S3)

## Phase 5: Polish

- [X] T011 Бенч `packages/core/benches/snapshots_scale.rs`: история из 200 снимков ≤ 500 мс (SC-003), снимок заметки на 10 000 строк (SC-001); запись в `Cargo.toml`
- [X] T012 Сценарий `scripts/dev/scenarios/snapshots.mjs` по quickstart, сверка снимков с макетом S1–S4, README, `**Status**: Implemented`

## Notes

- T007: отдельного `useSnapshots.ts` нет — окна `NoteHistory`/`RecentlyDeleted` зовут `client.snapshots` напрямую; формат времени — `ui/src/snapshots/when.ts`.
- T012: кнопка «Сравнить с предыдущим» из макета S1 не сделана — в спеке её нет. Снимок `Save` с интервалом в сценарии не проверяется (интервал ≥ 1 мин), он покрыт тестами ядра.
