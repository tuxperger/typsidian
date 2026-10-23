# Tasks: Закладки

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/core-bookmarks-api.md](contracts/core-bookmarks-api.md)

## Phase 1: Основа

- [X] T001 Ключ `bookmarks` в `vault/vault_settings.rs` (`VaultOverrides::bookmarks`, мягкий разбор, запись) с тестом

## Phase 2: User Story 1 — закрепить и открыть (P1)

- [X] T002 [US1] Тесты ядра `packages/core/tests/common/bookmarks_cases.rs`, вызовы в `bookmarks_native.rs` и `bookmarks_wasm.rs`: добавить четыре вида, повтор без дубля, имя по умолчанию, порядок и перемещение, группы (создать, свернуть, переименовать, удалить — закладки остаются), переименование закладки, vault только для чтения (FR-001…FR-003)
- [X] T003 [US1] `packages/core/src/bookmarks/mod.rs`: модель, операции, вид; экспорт в `lib.rs`
- [X] T004 [US1] Привязки: `bookmarks/web.rs`, команды Tauri, `client.bookmarks` в `ui/src/vault-client{,-web,-tauri}.ts`
- [X] T005 [US1] UI: `ui/src/bookmarks/BookmarksPanel.tsx`, `useBookmarks.ts` над деревом; открытие по виду цели; перетаскивание и Alt+↑/↓; меню закладки и группы; «В закладки» в меню дерева, вкладки, оглавления, палитре и окне поиска; тексты `ui/src/i18n/locales/{ru,en}/bookmarks.ts`; стили (макет B1, B2)

## Phase 3: User Story 2 — переименование (P2)

- [X] T006 [US2] Тесты ядра: переименование заметки и перенос папки переписывают пути закладок; удалённая цель — `Missing`, пропавший заголовок — `SectionMissing` (FR-004, SC-002)
- [X] T007 [US2] `move_bookmarks` в `vault/{native,web}.rs` (`rename`) и `vault/relocate.rs`; доступность в виде

## Phase 4: Polish

- [X] T008 Бенч вида 200 закладок ≤ 50 мс — в `bookmarks_native.rs` как тест времени; сценарий `scripts/dev/scenarios/bookmarks.mjs` по quickstart, сверка с макетом, README, `**Status**: Implemented`
