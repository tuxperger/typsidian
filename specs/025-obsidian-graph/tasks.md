# Tasks: Граф связей в духе Obsidian

**Input**: Design documents from `/specs/025-obsidian-graph/`

**Tests**: ядро — оба таргета (`tests/common/graph_cases.rs`, раннеры `graph_native.rs`,
`graph_wasm.rs`), бенч `benches/graph_view.rs`; UI — сценарий `scripts/dev/scenarios/graph.mjs`
с замером кадров.

## Phase 1: Setup

- [X] T001 Добавить `d3-force` и `@types/d3-force` в `ui/package.json` (`pnpm add` через nix); проверить, что nix-сборка зависимостей (хэш pnpm в `flake.nix`/`nix/`) не ломается, при необходимости — поправка хэша (агент `nix-env-agent`); при отказе — Decision в research.md и свой Barnes–Hut (сделано: d3-force 3.0.0 уже был в lock транзитивно, `nix build .#desktop.pnpmDeps --rebuild` сошёлся со старым хэшем — правка `nix/desktop.nix` не нужна)
- [X] T002 [P] Завести `packages/core/src/graph/{view,web}.rs` (`web.rs` под wasm32), `pub mod view` в `graph/mod.rs`, раннеры `tests/graph_native.rs`, `tests/graph_wasm.rs`, `tests/common/graph_cases.rs`
- [X] T003 [P] Переводы `ui/src/i18n/locales/{en,ru}/graph.ts` (дополнить существующую область `graph.*`): режимы «Весь vault» / «Вокруг заметки», глубина, панель (фильтр, показ, группы, вид, силы, сброс), пустые состояния G4, подписи для чтения с экрана

## Phase 2: Foundational

- [X] T004 Типы `GraphRequest`, `GraphGroup`, `GroupColor`, `LocalGraph`, `VaultGraph`, `GraphNode`, `GraphNodeKind`, `GraphEdge`, `GraphSettings` (serde + Tsify) по data-model.md в `packages/core/src/graph/view.rs`
- [X] T005 Вставки файлов в индексе (research.md §3): разбор `#image("…")` с литеральным путём и снимков досок (`boards::embeds_of`) при обновлении заметки, хранение рядом со ссылками в `IndexCache` (SQLite — таблица/столбец, `MemoryCache`) в `packages/core/src/vault/cache.rs` и `vault/{native,web}.rs`
- [X] T006 Ключ `graph` (`GraphSettings`) в `VaultOverrides` с разбором, умолчаниями и записью, не трогающей прочие ключи, в `packages/core/src/vault/vault_settings.rs`; `NotePanelMode::Graph` в `packages/core/src/settings/mod.rs`
- [X] T007 [P] Тесты настроек и режима колонки на обоих путях — `tests/common/graph_cases.rs`, `tests/common/note_panel_cases.rs`

## Phase 3: User Story 1 — Живой граф всего vault (P1) 🎯 MVP

- [X] T008 [P] [US1] Тесты `vault_graph` на обоих путях по contracts/core-graph-api.md (узлы, связи, петли, в никуда, `both_ways`, `[[путь|текст]]`, `degree`, `show_orphans`, переименование) per FR-001, FR-005, FR-010
- [X] T009 [US1] `vault_graph` (глобальный, без фильтра и групп) в `packages/core/src/graph/view.rs` по research.md §1–§2
- [X] T010 [US1] Метод `WebBackend.vaultGraph` в `packages/core/src/graph/web.rs`; команда `vault_graph` в `packages/shell/src/lib.rs`; метод `graph(request)` в `ui/src/vault-client*.ts`
- [X] T011 [US1] Бенч `packages/core/benches/graph_view.rs` (5 000 заметок, 15 000 связей; native), итог — Decision в research.md §9 per SC-001
- [X] T012 [US1] Воркер раскладки `ui/src/graph/layout.worker.ts` и мост `ui/src/graph/useGraphLayout.ts` по research.md §6, §8 (силы из настроек, перетаскивание `fx/fy`, `Float32Array` положений, перенос положений по `id`, сборка воркера во всех трёх оболочках — проверить `new Worker(new URL(...))` в Next и Vite) per FR-002, FR-003
- [X] T013 [US1] `ui/src/graph/GraphCanvas.tsx` по research.md §7 и макету G1: Canvas 2D, сдвиг, масштаб колесом и щипком, перетаскивание узла (долгое нажатие на сенсоре), подсветка соседей с приглушением, подписи по масштабу, размер по `degree`, форма узла по виду, щелчок — открыть per FR-003…FR-006, SC-003
- [X] T014 [US1] `ui/src/graph/useVaultGraph.ts` (запрос, прореживание 300 мс, обновление по событиям vault) и новый `ui/src/graph/GraphView.tsx` (верхняя полоса G1, счётчик, кнопки масштаба), подключение вместо прежнего в `ui/src/workspace/Workspace.tsx` per FR-009, SC-004
- [X] T015 [US1] `ui/src/graph/GraphA11yList.tsx` и Tab по узлам per FR-011
- [X] T016 [US1] Сценарий `scripts/dev/scenarios/graph.mjs` (quickstart.md §2 — часть US1, §3 — замер кадров на 5 000); снимки сверить с G1, G4 (замеры на 5 000: до первого движения ≈3,1–3,5 с (бюджет 2 с не выдержан: `vault_graph` в wasm ≈1,5–2 с), покой ≈8–9 с, кадры при перетаскивании ≈32/с в headless без GPU, подсветка ≤20 мс)

## Phase 4: User Story 2 — Локальный граф (P1)

- [X] T017 [P] [US2] Тесты локального графа на обоих путях (глубина 1/2/3, обе стороны, центр без связей, центр вне фильтра) per FR-001
- [X] T018 [US2] `local` в `vault_graph` (обход в ширину после фильтра) в `packages/core/src/graph/view.rs`
- [X] T019 [US2] Режим «Вокруг заметки» и глубина в `GraphView.tsx` (G2), перестройка при смене открытой заметки с сохранением положений; режим колонки «Граф» `ui/src/graph/LocalGraphMode.tsx` (G3) в `noteModes` `Workspace.tsx`, «Открыть весь граф»; G4 при отсутствии связей per FR-001, US2

## Phase 5: User Story 3 — Настройки (P2)

- [X] T020 [P] [US3] Тесты фильтра (текст, `тег:`, `папка:`), показа вложений и тегов, групп (первая подходящая) на обоих путях per FR-007
- [X] T021 [US3] Фильтр, показ видов, группы в `vault_graph` (`search::parse_conditions`, индекс свойств 020, вставки T005) в `packages/core/src/graph/view.rs`
- [X] T022 [US3] `ui/src/graph/GraphPanel.tsx` по G1 и шторка G5 на узком экране: фильтр, переключатели, группы (добавить, цвет из палитры токенов, порядок перетаскиванием, удалить), стрелки, подписи, размер узлов, силы, «Сбросить» с подтверждением в интерфейсе; запись в `vault.json` через настройки vault 006 с прореживанием; силы — сразу в воркер per FR-007, FR-008
- [X] T023 [US3] Стрелки направления в `GraphCanvas.tsx` (`both_ways` — две стрелки) per US3 сц. 6
- [X] T024 [US3] Дополнить `scripts/dev/scenarios/graph.mjs` (группа, фильтр, теги, силы, перезапуск); снимки — G1, G5

## Phase 6: Polish

- [X] T025 [P] Удалить прежнюю раскладку и N вызовов `backlinks` из графа; проверить, что `Mod+Shift+G` и кнопка «Граф» ведут на новый экран
- [X] T026 Гейты `.specify/scripts/typsidian/gates.sh all`
- [ ] T027 README: граф связей; `**Status**: Implemented` в `specs/025-obsidian-graph/spec.md` после ручных прогонов desktop/Android

## Dependencies

T001 до T012. T004 → T009 → T010 → (T012, T013, T014). T005 → T021. T018 → T019. US1 → US2 → US3.

## Phase 7: Convergence

- [X] T028 Ускорить тик раскладки на 5 000 узлах до бюджета 16 мс (сейчас ~29 мс в воркере, research.md §13): подобрать `theta`, `alphaDecay`, разреживание `forceLink` для больших графов, либо WebGL-рендер по Complexity Tracking; повторить замер кадров на устройстве с GPU per SC-002 (partial)
- [X] T029 Проверить гипотезу research.md §13: замерить в браузере время ожидания `vaultGraph` в очереди вызовов wasm и длительность `poll_for_changes` на vault из 5 000 файлов; если опрос держит очередь — отдельной задачей 001 (опрос без обхода всех файлов либо уступка очереди между файлами) и повторить SC-001 сразу после открытия vault per SC-001 (partial)
- [X] T030 Убрать из `ui/src/graph/GraphCanvas.tsx` диагностический зонд `tsProbe` и атрибуты `data-moving`/`data-draws`/`data-hoverMs` из продуктового кода или оставить их только для сценария за явным флагом per plan: «UI только рисует» (unrequested)
- [ ] T031 Замерить кадры при перетаскивании и сдвиге на 5 000 узлах на устройстве с GPU (desktop и Android): в headless chromium без GPU — 25–27 кадров/с при тике раскладки 13 мс (research.md §14), упор в Canvas 2D главного потока; если меньше 30 — WebGL-рендер по Complexity Tracking per SC-002 (partial)
