# Tasks: Задачи из заметок

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/core-tasks-api.md](contracts/core-tasks-api.md)

## Phase 1: Основа

- [X] T001 Путь `.typsidian/tasks.json` в `vault/service_dir.rs` и `derived_paths()` с тестом

## Phase 2: User Story 1 — все задачи в одном месте (P1)

- [X] T002 [US1] Тесты ядра `packages/core/tests/common/tasks_cases.rs`, вызовы в `tasks_native.rs` и `tasks_wasm.rs`: разбор (`[ ]`/`[x]`, `+`, срок трёх видов, вложенность, код и комментарий не считаются), вид из трёх заметок, индекс перечитывает только изменённые, отметка закрытой заметки — ровно одна строка, открытой — `EditorEdit`, строка сдвинулась — найдена по тексту, исчезла — `Stale`, только чтение — `ReadOnly` (FR-001, FR-002, FR-005, SC-002)
- [X] T003 [US1] `packages/core/src/tasks/mod.rs`: разбор, индекс, вид с группами, отметка; экспорт в `lib.rs`
- [X] T004 [US1] Привязки `tasks/web.rs`, команды Tauri, `client.tasks` в `ui/src/vault-client{,-web,-tauri}.ts`
- [X] T005 [US1] UI `ui/src/tasks/TasksScreen.tsx`, `useTasks.ts`: окно из палитры, группы, отметка (через редактор открытой заметки), переход на строку, клавиатура; тексты `ui/src/i18n/locales/{ru,en}/tasks.ts`; стили (макет T1, T2)

## Phase 3: User Story 2 — фильтры (P2)

- [X] T006 [US2] Тесты и ядро: состояние, срок (просрочено/сегодня/неделя/без срока), папка, условия свойств 020, группировка по сроку (FR-003); фильтры в окне

## Phase 4: User Story 3 — задача из любого места (P3)

- [X] T007 [US3] `task_toggle_at` с тестами; команда палитры «Переключить задачу»

## Phase 5: Polish

- [X] T008 Бенч `packages/core/benches/tasks_scale.rs`: 5 000 заметок, повторный показ ≤ 1 с (SC-001); запись в `Cargo.toml`
- [X] T009 Сценарий `scripts/dev/scenarios/tasks.mjs` по quickstart, сверка с макетом, README, `**Status**: Implemented`
