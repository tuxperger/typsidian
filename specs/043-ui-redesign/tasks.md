# Tasks: Оболочка интерфейса по дизайну Typsidian.pdf

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md),
[contracts/shell-props.md](contracts/shell-props.md)

Ядро не меняется: тестов `cargo test`/`wasm-bindgen-test` у фичи нет, проверка — сценарий в
браузере и гейт `ui`.

## Phase 1: Foundational

- [X] T001 Поле вида `view: "text" | "side" | "document"` в `ui/src/workspace/usePaneLayout.ts` с сохранением (research.md, Decision 3)
- [X] T002 [P] Тексты шапки, вида, меню vault, низа дерева, полосы предпросмотра, «Истории», стартового экрана, телефона в `ui/src/i18n/locales/{ru,en}/` (FR-010)
- [X] T003 [P] Вход `openKnownVault` в `Workspace` и его передача из `apps/web/app/VaultWorkspace.tsx`, `apps/desktop/src/DesktopWorkspace.tsx`, `apps/mobile/src/MobileWorkspace.tsx` (contracts/shell-props.md)

## Phase 2: User Story 1 — рабочая область (P1)

- [X] T004 [US1] `ui/src/workspace/ViewSwitch.tsx` и показ колонок по виду в `Workspace.tsx`; недоступен у доски (FR-002, US1 AS1, AS6)
- [X] T005 [US1] `ui/src/workspace/VaultSwitcher.tsx`: известные vault, текущий, недоступные с причиной, «Открыть папку…», «Vault в настройках» (FR-003, US1 AS2)
- [X] T006 [US1] Шапка в `Workspace.tsx`: знак, переключатель vault, путь, поле палитры с сочетанием, вид, синхронизация, «Экспорт PDF», ход экспорта/импорта; остальное из шапки убрать (FR-001, FR-005, US1 AS3)
- [X] T007 [US1] Низ дерева в `ui/src/vault-tree/Sidebar.tsx`: «+ Заметка», значки графа, оглавления, источников, настроек (FR-004, US1 AS5)
- [X] T008 [US1] Полоса предпросмотра в `ui/src/preview/PreviewPane.tsx`: «стр. N» видимой страницы, статус сборки, масштаб; без линеек и без кнопки PDF (FR-006, US1 AS4)
- [X] T009 [US1] «История» в строке состояния `ui/src/editor/NoteEditor.tsx` при включённой истории (FR-007)
- [X] T010 [US1] Стили шапки, вида, меню vault, низа дерева, полосы предпросмотра по PDF в `ui/src/styles/app.css`
- [X] T011 [US1] Проверка FR-005: команды палитры для сворачивания дерева, графа, оглавления, источников, колонки заметки, «Сегодня», показа, настроек в `Workspace.tsx`

## Phase 3: User Story 2 — стартовый экран (P2)

- [X] T012 [US2] `ui/src/workspace/WelcomeScreen.tsx`: знак, описание, «Открыть папку…», импорт/клон где есть, «Настройки приложения», недавние vault с недоступными и «Указать заново» (FR-008)
- [X] T013 [US2] Стили стартового экрана в `ui/src/styles/app.css`

## Phase 4: User Story 3 — телефон (P3)

- [X] T014 [US3] Экран дерева: шапка с vault, низ «Быстрая заметка» + граф + настройки; экран заметки: «Назад», название и путь, «Текст / Документ», строка «стр. N · статус» в `Workspace.tsx` (FR-009)
- [X] T015 [US3] Стили телефона в `ui/src/styles/app.css`

## Phase 5: Polish

- [X] T016 Сценарий `scripts/dev/scenarios/redesign.mjs`: снимки экранов 1–4, проверки quickstart.md; сверка с PDF
- [X] T017 Гейт `ui`; проверка в оболочке рабочего стола (WebKitGTK); README; `**Status**: Implemented`

## Dependencies

T001 → T004; T003 → T005, T012; T002 — до всех; US2 и US3 не зависят от US1, кроме T001–T003.
