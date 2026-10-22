# Tasks: Поиск с заменой по vault

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [contracts/replace.md](contracts/replace.md)

## Phase 1: Foundational

- [X] T001 Вынести сопоставитель поиска в общий `Matcher` для поиска и замены в `packages/core/src/search/mod.rs` (research.md §1)

## Phase 2: User Story 1 — заменить с предпросмотром (P1)

- [X] T002 [US1] Тесты ядра: предпросмотр перечисляет каждое совпадение с кусками «было/станет», группы `$1`, буквальный `$` в текстовом режиме, предел 10 000, неверное выражение — в `packages/core/tests/search_native.rs` и `packages/core/tests/search_wasm.rs` (FR-001, FR-002, FR-008)
- [X] T003 [US1] Тесты ядра: замена в заметке меняет только отмеченные, расхождение отпечатка — `Stale` без записи, открытая заметка — одна правка для редактора без записи на диск — там же (FR-003, FR-004, FR-007)
- [X] T004 [US1] `packages/core/src/search/replace.rs`: `replace_preview`, `replace_in_note` (research.md §2, §3, §5)
- [X] T005 [US1] Привязки: wasm-методы `WebBackend` в `packages/core/src/vault/web.rs`, команды Tauri в `packages/shell/src/lib.rs`, методы клиента в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts`
- [X] T006 [US1] UI режима замены: поле «Заменить на», выдача с отметками (`ui/src/search/ReplacePreview.tsx`), отметки заметки целиком, «Заменить отмеченные» через `ui/src/search/useReplace.ts`, открытые заметки — через `OpenNoteEditors` 013; правки в `ui/src/search/SearchOverlay.tsx`, стили в `ui/src/styles/app.css`, тексты в `ui/src/i18n/locales/{ru,en}/search.ts` (макет R1, R3)
- [X] T007 [US1] Команда `ReplaceInVault` (`Mod+Shift+H`) в `packages/core/src/settings/keybindings.rs` с тестом свободного сочетания, открытие поиска в режиме замены в `ui/src/workspace/Workspace.tsx`, `Mod+Enter` в режиме замены (research.md §6)

## Phase 3: User Story 2 — отмена пакета (P1)

- [X] T008 [US2] Тесты ядра: отмена возвращает текст при совпадении отпечатка, иначе `ChangedSince`; открытая заметка — правка для редактора — в `packages/core/tests/search_{native,wasm}.rs` (FR-005)
- [X] T009 [US2] `replace_undo` в `packages/core/src/search/replace.rs`, привязки как в T005, «Отменить замену» и итог отмены в `ui/src/search/useReplace.ts` (макет R2)

## Phase 4: User Story 3 — большой vault (P2)

- [X] T010 [US3] Порции применения с ходом и остановкой между заметками, итог после остановки — `ui/src/search/useReplace.ts` (FR-006, макет R2)
- [X] T011 [US3] Замер SC-001 и SC-004: бенч предпросмотра на 2 000 заметок в `packages/core/benches/replace_scale.rs` (запись в `Cargo.toml`), задержка ввода во время применения — в сценарии

## Phase 5: Polish

- [X] T012 Сценарий `scripts/dev/scenarios/replace.mjs`: предпросмотр, снятие отметок, применение, открытая заметка, отмена, состояния R3, телефон R4; сверка снимков с макетом
- [X] T013 README: строка о замене; `**Status**: Implemented`
