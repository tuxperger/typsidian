# Tasks: Свернуть и развернуть папки разом

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/tree-ui.md](contracts/tree-ui.md)

Ядро не меняется (research.md, Decision 1): тестов `cargo test`/`wasm-bindgen-test` у фичи
нет, проверка — сценарием в браузере (quickstart.md).

## Phase 1: Foundational

- [X] T001 `folderPaths(nodes)` — пути всех папок дерева или ветки в `ui/src/vault-tree/buildTree.ts`
- [X] T002 Действия `collapseAll`, `expandAll`, `setBranchCollapsed(path, collapsed)` рядом с `toggleExpanded` в `ui/src/vault-tree/VaultTreeActions.tsx` (data-model.md, FR-004, FR-006)
- [X] T003 [P] Тексты «Свернуть все папки», «Развернуть все папки», «Свернуть вложенные», «Развернуть вложенные» в `ui/src/i18n/locales/{ru,en}/tree.ts` (FR-008)

## Phase 2: User Story 1 — свернуть всё дерево (P1)

**Independent Test**: quickstart.md, шаги 1–3 и 5.

- [X] T004 [P] [US1] Значки `collapseAll`/`expandAll` (шевроны внутрь/наружу, макет экран 1) в `ui/src/icons/Icons.tsx`
- [X] T005 [US1] Кнопки в строке подписи «Vault» с `aria-label` и `title`, выключены без папок, в `ui/src/vault-tree/VaultTreeActions.tsx` (FR-001, FR-002, FR-005, FR-007)
- [X] T006 [US1] Строка подписи с кнопками по макету (22px, наведение, фокус, выключенное состояние) в `ui/src/styles/app.css`

## Phase 3: User Story 2 — ветка из меню папки (P2)

**Independent Test**: quickstart.md, шаг 4.

- [X] T007 [US2] Группа «Развернуть вложенные / Свернуть вложенные» в меню папки с подпапками, после пунктов создания, в `ui/src/vault-tree/VaultTreeActions.tsx` (FR-003, FR-005)

## Phase 4: Polish

- [X] T008 Сценарий `scripts/dev/scenarios/tree-collapse.mjs` по quickstart.md, плюс vault на 500 папок с замером «Свернуть все» ≤ 0,2 с и «Развернуть все» ≤ 0,5 с (SC-002); прогон в браузере и снимок, сверка с макетом
- [X] T009 Гейт `ui`; README (раздел дерева vault); `**Status**: Implemented` в spec.md

## Dependencies

T001 → T002 → T005, T007. T003, T004 — параллельно. US2 не зависит от US1, кроме T001–T003.

## Implementation Strategy

MVP — Phase 1 + US1. US2 добавляется к нему без переделок.

## Phase 5: Convergence

- [X] T010 Привести имена действий в `specs/042-tree-collapse-all/contracts/tree-ui.md` и `data-model.md` к коду (`setFoldersCollapsed(paths, collapse)`, `expandAll()`) per plan: contracts (contradicts)
