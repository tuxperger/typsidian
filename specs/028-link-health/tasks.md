# Tasks: Битые ссылки и заметки-сироты

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md)

## Phase 1: User Story 1 — найти и починить (P1)

- [X] T001 [US1] Тесты ядра: группы битых целей с числом ссылок и заметок, виды заметка/доска/вложение, строки-примеры, открытые заметки по тексту редактора — `packages/core/tests/common/links_cases.rs`, вызовы в `links_native.rs` и `links_wasm.rs` (FR-001, FR-006)
- [X] T002 [US1] Тесты ядра: перенаправление с сохранением подписи, «убрать ссылку» оставляет текст, одна запись по `only`, вставка убирается целиком, устаревший отпечаток — `Stale`, открытая заметка — правка редактора, отмена через `replace_undo` — там же (FR-002, FR-003)
- [X] T003 [US1] `packages/core/src/links/health.rs`: `link_health`, `fix_links` (research.md §1–§3); экспорт в `links/mod.rs`
- [X] T004 [US1] Привязки: wasm в `packages/core/src/links/web.rs`, Tauri в `packages/shell/src/lib.rs`, клиент в `ui/src/vault-client{,-web,-tauri}.ts`
- [X] T005 [US1] UI: наложение «Состояние ссылок» с вкладкой «Битые ссылки», выбор заметки для перенаправления, «Создать», «Убрать», итог с «Отменить» — `ui/src/link-health/LinkHealth.tsx`, `ui/src/link-health/useLinkHealth.ts`, открытие из палитры в `ui/src/workspace/Workspace.tsx`, стили, тексты `ui/src/i18n/locales/{ru,en}/links.ts` (макет L1)

## Phase 2: User Story 2 — сироты (P2)

- [X] T006 [US2] Тесты ядра: сироты без входящих ссылок и вставок, входы (входящие, дневник) помечены, «намеренно» исключает и переезжает при переименовании — `links_cases.rs`, `vault_settings_cases.rs` (FR-004)
- [X] T007 [US2] `links.intentional_orphans` в `packages/core/src/vault/vault_settings.rs`, перенос в `packages/core/src/vault/relocate.rs`, отметка из UI через настройки vault; вкладка «Без входящих» с фильтром входов (макет L2)

## Phase 3: User Story 3 — признак (P3)

- [X] T008 [US3] Счётчик битых ссылок в подвале дерева, обновление при изменениях vault, ноль не показывается — `ui/src/workspace/Workspace.tsx` и подвал дерева (FR-005, макет L3)

## Phase 4: Polish

- [X] T009 Бенч `packages/core/benches/link_health_scale.rs` на 5 000 заметках (SC-001), запись в `Cargo.toml`
- [X] T010 Сценарий `scripts/dev/scenarios/link-health.mjs` по quickstart, сверка снимков с макетом L1–L3, README, `**Status**: Implemented`
