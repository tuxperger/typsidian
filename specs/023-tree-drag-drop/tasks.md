# Tasks: Перетаскивание в дереве файлов

**Input**: Design documents from `/specs/023-tree-drag-drop/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для логики ядра на обоих таргетах (принципы I и II конституции):
каждый сценарий — в `packages/core/tests/common/relocate_cases.rs` и вызывается из обоих
раннеров, `relocate_native.rs` и `relocate_wasm.rs`. Для UI JS-тестов в репозитории нет —
сценарий headless-браузера `scripts/dev/scenarios/tree-move.mjs` и проверка по quickstart.md.

**Макет**: временный, https://claude.ai/code/artifact/f749a41c-2f82-40ef-a46f-3c016e30239f —
вёрстка идёт по нему (contracts/frontend-tree-move.md).

## Phase 1: Setup

- [X] T001 Завести `packages/core/src/vault/relocate.rs` с `pub mod relocate;` и реэкспортом в `packages/core/src/vault/mod.rs`; завести `packages/core/tests/common/relocate_cases.rs` (с `pub mod relocate_cases;` в `packages/core/tests/common/mod.rs`) и раннеры `packages/core/tests/relocate_native.rs`, `packages/core/tests/relocate_wasm.rs` по образцу `vault_native.rs` / `vault_wasm.rs`

## Phase 2: Foundational

**Purpose**: операция переноса в ядре, её биндинги и клиент — без них ни одна история не начинается.

- [X] T002 Типы `MoveRequest`, `MoveTarget`, `PathChange`, `MoveImpact`, `MoveRefusal`, `MoveOutcome`, `MoveError`, трейт `MoveControl` и пустая реализация `NoControl` (serde + `Tsify` под wasm32, `#[serde(tag = "kind")]` у перечислений) по data-model.md в `packages/core/src/vault/relocate.rs`
- [X] T003 Чистые функции `check_move` (отказы `SamePlace`, `IntoItself`, `ServiceFolder`; возвращает новое место) и `path_changes` (пары по заметкам/доскам и по папкам для элемента-файла и элемента-папки) по contracts/core-move-api.md в `packages/core/src/vault/relocate.rs` per FR-003, FR-005
- [X] T004 Чистая функция `plan_steps(from, to, kind, existing_snapshot_folders)` — шаги верхнего уровня: сам элемент и, для одиночной заметки, её собственные папки снимков (`<заметка>.assets` и `<папка>/assets/<заметка>/`), пути берутся из `packages/core/src/boards/placement.rs` и `packages/core/src/boards/snapshot.rs` — в `packages/core/src/vault/relocate.rs` per FR-018a, research.md §2
- [X] T005 [P] `NoteIndex::rename_prefix(from, to) -> Vec<PathChange>` (одна запись при файле, все записи под папкой при папке; имя записи не меняется) в `packages/core/src/vault/index.rs` с модульными тестами
- [X] T006 [P] `ConflictRegistry::rename_prefix(from, to)` — переезд буферов, конфликтов и «удалено с правками» в `packages/core/src/vault/conflict.rs` с модульными тестами per FR-020
- [X] T007 Переезд таблицы ссылок: для каждой пары `PathChange` — `rename_note`, затем переразбор исходящих ссылок перенесённой заметки из её текста под новым путём; обе реализации кэша в `packages/core/src/vault/cache.rs` per research.md §5
- [X] T008 Общая часть `apply_move<B: VaultBackend>` после успешного переезда файлов: индекс (T005), правки (T006), кэш (T007), `move_sections`, события `NoteRenamed` на каждую заметку и один `LinksInvalidated`; сборка `MoveOutcome::Done` с `undo` — в `packages/core/src/vault/relocate.rs` per FR-008, FR-018, FR-020, research.md §7
- [X] T009 Объявить `move_targets`, `move_impact`, `move_entry` в трейте `VaultBackend` в `packages/core/src/vault/backend.rs` по contracts/core-move-api.md
- [X] T010 `NativeBackend::move_entry`: отказы (`AccessDenied` как в `rename`, `check_move`, `NotFound`, `TargetNotFound`, `NameConflict` по диску для каждого шага), `std::fs::rename` на шаг с откатом первого шага при сбое второго, затем `apply_move` в порядке замков index → cache — в `packages/core/src/vault/native.rs` per FR-001, FR-002, FR-004, FR-006, research.md §5
- [X] T011 `WebBackend::move_entry`: те же отказы; копирование байтами (`read_bytes_any_size`, `ensure_folder`, `write_bytes`) с `control.progress` и проверкой `control.cancelled` между файлами; сбой или отмена на копировании — удалить скопированное, исход `Cancelled` либо `Io`; удаление источника, при сбое — `Partial { left }` и перечитывание индекса; затем `apply_move` — в `packages/core/src/vault/web.rs` per FR-002, FR-007, research.md §6, принцип X
- [X] T012 Биндинги wasm `moveEntry(request, onProgress, cancel)` (реализация `MoveControl` поверх `js_sys::Function` и объекта-флага) в `packages/core/src/vault/web.rs` и команда Tauri `move_entry` в `packages/shell/src/lib.rs` (с регистрацией в `invoke_handler`)
- [X] T013 Добавить `moveEntry(request, control?)` в интерфейс `ui/src/vault-client.ts` и реализации `ui/src/vault-client-web.ts` (через `wasm-call-queue`, `AbortSignal` → флаг отмены) и `ui/src/vault-client-tauri.ts`; типы — `import type` из `wasm-core`
- [X] T014 [P] Ключи `tree.move.*` по таблице contracts/frontend-tree-move.md в `ui/src/i18n/locales/en/tree.ts` (эталон) и `ui/src/i18n/locales/ru/tree.ts` per FR-027
- [X] T015 Поднять раскрытость папок из `useState` в `FolderRow` в общее множество путей (`expanded`, `toggleExpanded`, `expand`, `remapExpanded(folders: PathChange[])`) в `useVaultTreeActions` в `ui/src/vault-tree/VaultTreeActions.tsx`; поведение по умолчанию (верхний уровень раскрыт, раскрытие при создании) сохранить per research.md §9
- [X] T016 `applyPathChanges(moved, folders)` в `ui/src/workspace/Workspace.tsx`: открытая заметка или доска получает новый путь без перечитывания файла и без пересоздания редактора (буфер, каретка, прокрутка, регистрация `registerFlusher` остаются), запись идёт в новое место; вызвать `remapExpanded` per FR-020
- [X] T017 `ui/src/vault-tree/useMoveEntry.ts`: `requestMove(request, { confirm })` — шаги 3–4 contracts/frontend-tree-move.md (перенос, `applyPathChanges`, `refresh`, состояние итога и ошибки по причинам `MoveRefusal`/`Io`, занятость `pendingPath`); подключить в `ui/src/vault-tree/Sidebar.tsx`

**Checkpoint**: `client.moveEntry` переносит заметку и папку на обоих таргетах, открытый файл следует за путём — истории могут начинаться.

## Phase 3: User Story 1 — Перетащить заметку в другую папку (P1) 🎯 MVP

**Goal**: заметка или доска переносится жестом указателя на папку или в корень; цель подсвечена; открытая заметка остаётся открытой.

**Independent Test**: перетащить заметку из одной папки в другую и на свободное место дерева — файл на диске в новом месте, дерево это показывает, открытая заметка открыта.

### Tests for User Story 1

- [X] T018 [P] [US1] Сценарии: перенос заметки и доски в папку и в корень (побайтное равенство, источника нет, индекс под новым путём, имя прежнее); буфер и конфликт остаются за новым путём; собственная папка снимков едет с заметкой в обеих раскладках, общая папка корня — нет; после переноса и прохода сверки нет событий `NoteDeleted`/`NoteDeletedWithUnsavedEdits`/`BulkChange*`, есть `NoteRenamed`; текст заметок со ссылками на перенесённую побайтно прежний (FR-019) — в `packages/core/tests/common/relocate_cases.rs`, вызовы в `packages/core/tests/relocate_native.rs` и `packages/core/tests/relocate_wasm.rs` per FR-001, FR-008, FR-018a, FR-020
- [X] T019 [P] [US1] Сценарии `move_targets` без фильтра: нет папки-родителя элемента, служебной папки; корень первым, порядок по пути; признак раздела — в `packages/core/tests/common/relocate_cases.rs` и обоих раннерах per FR-012

### Implementation for User Story 1

- [X] T020 [US1] `move_targets(notes, sections, root_name, from, query)` (пока без фильтра: `query` пуст) в `packages/core/src/vault/relocate.rs`; реализация метода трейта в `packages/core/src/vault/native.rs` и `packages/core/src/vault/web.rs`; биндинг `moveTargets` в `packages/core/src/vault/web.rs`, команда `move_targets` в `packages/shell/src/lib.rs`; `moveTargets` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts`
- [X] T021 [US1] `ui/src/vault-tree/useTreeDrag.ts`: машина состояний `idle → armed → dragging → dropping` на Pointer Events с захватом указателя, порог 4 px для мыши и пера, запрос `moveTargets(from, "")` на захвате, определение цели через `elementFromPoint` и `data-drop-folder` (папка → она, заметка → её папка, пустое место → корень, вне множества → нет цели), отмена по Escape / отпусканию вне цели / `blur` окна / `pointercancel` с возвратом фокуса, бросок → `requestMove` per FR-009, FR-011, FR-012, FR-014, research.md §8
- [X] T022 [US1] Раскрытие свёрнутой папки после 600 мс под указателем (через `expand` из T015) и прокрутка дерева в полосе 28 px у краёв на `requestAnimationFrame` в `ui/src/vault-tree/useTreeDrag.ts` per FR-013
- [X] T023 [US1] Подключить жест в `ui/src/vault-tree/VaultTreeActions.tsx`: `data-drop-folder` на строках папок и заметок, обработчики на строках заметок и досок, классы `ts-tree__row--dragging`, `ts-tree__folder--drop`, `ts-tree--drop-root`; клик без сдвига, кнопка «⋯», поле переименования и контекстное меню жест не начинают; захват во время переименования его отменяет; при `pendingPath` жест недоступен per FR-015
- [X] T024 [P] [US1] `ui/src/vault-tree/TreeDragGhost.tsx` — ярлык у указателя (иконка вида, имя; состояние «нельзя») и область `aria-live="polite"` с объявлениями `tree.move.a11y.*` per FR-010, FR-026
- [X] T025 [US1] Стили по макету (экран «Перетаскивание указателем») в `ui/src/styles/app.css`: `.ts-tree--drop-root`, `.ts-tree__folder--drop`, `.ts-tree__row--dragging`, `.ts-tree-drag`, `.ts-tree-drag--denied`; только токены, все селекторы с `.ts-tree`; `prefers-reduced-motion`

**Checkpoint**: заметка переносится жестом в web и desktop.

## Phase 4: User Story 2 — Перенести папку целиком (P1)

**Goal**: папка переносится жестом со всем содержимым, пометкой раздела и раскрытостью.

**Independent Test**: перенести папку с вложенной папкой, тремя заметками и картинкой — содержимое и структура те же, на прежнем месте ничего нет.

### Tests for User Story 2

- [X] T026 [P] [US2] Сценарии: перенос папки с вложенными папками, заметками, доской и невидимыми файлами (картинка, `*.assets`) — побайтное равенство и структура; пометка раздела самой папки и вложенной едет следом; буфер заметки внутри папки остаётся за новым путём; отказ `IntoItself` для самой папки и потомка любой глубины, диск не изменён; `move_targets` для папки не содержит её саму и потомков; на web — отмена на середине копирования оставляет vault прежним, сбой удаления источника даёт `Partial { left }` — в `packages/core/tests/common/relocate_cases.rs` и обоих раннерах per FR-002, FR-003, FR-007, FR-018

### Implementation for User Story 2

- [X] T027 [US2] Обработчики жеста на строках папок и число переезжающих файлов в ярлыке (`TreeDragGhost`) в `ui/src/vault-tree/VaultTreeActions.tsx` и `ui/src/vault-tree/TreeDragGhost.tsx`; раскрытость перенесённой папки и вложенных сохраняется через `remapExpanded` per US2 AS5
- [X] T028 [US2] Прогресс переноса в нижней строке панели (`tree.move.progress`, полоса) из `control.onProgress` в `ui/src/vault-tree/Sidebar.tsx` и `ui/src/vault-tree/useMoveEntry.ts`; стили в `ui/src/styles/app.css` по макету (артборд «Большая папка едет»); на время переноса жест и действия дерева недоступны per Edge cases, SC-005

**Checkpoint**: папка переносится жестом; US1 по-прежнему работает.

## Phase 5: User Story 3 — Не потерять и не сломать при переносе (P1)

**Goal**: занятое имя и недоступный vault дают понятный отказ; перенос, разрывающий ссылки, требует подтверждения.

**Independent Test**: перенести заметку в папку с одноимённой заметкой; перенести заметку, на которую ссылаются две другие, — без согласия на диске ничего не меняется.

### Tests for User Story 3

- [X] T029 [P] [US3] Сценарии `broken_by_move` и `move_impact`: входящая относительная и корневая ссылка на перенесённую заметку; собственные относительные ссылки перенесённой заметки на соседей; ссылки между заметками одной переносимой папки не разрываются; уже битая ссылка в список не попадает; перенос папки даёт один общий список без повторов; пустой список, когда ничего не рвётся — в `packages/core/tests/common/relocate_cases.rs` и обоих раннерах per FR-016, FR-017
- [X] T030 [P] [US3] Сценарии отказов: `SamePlace` (FR-005); `NameConflict` для файла, для папки, для папки снимков, для имён, различающихся регистром (по правилу 001 §FR-012); `AccessDenied` при недоступном и только-для-чтения vault; `NotFound`, `TargetNotFound`; во всех случаях файлы vault побайтно прежние — в `packages/core/tests/common/relocate_cases.rs` и обоих раннерах per FR-004, FR-006, FR-007

### Implementation for User Story 3

- [X] T031 [US3] `broken_by_move` и `move_impact` (кандидаты: переезжающие заметки и источники обратных ссылок на них; чтение текста только кандидатов) в `packages/core/src/vault/relocate.rs`; реализация метода трейта в `packages/core/src/vault/native.rs` и `packages/core/src/vault/web.rs`; биндинг `moveImpact`, команда `move_impact` в `packages/shell/src/lib.rs`; `moveImpact` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts` per research.md §4
- [X] T032 [US3] Шаги 1–2 `requestMove` в `ui/src/vault-tree/useMoveEntry.ts`: `moveImpact` перед переносом, `SamePlace` — молча, непустой `broken` — `BacklinksConfirmDialog` с текстами `tree.move.confirm.*` (вариант для переноса в `ui/src/vault-tree/BacklinksConfirmDialog.tsx`, фокус на «Отмена»), отказ пользователя — ничего не делать per FR-016, FR-017
- [X] T033 [US3] Сообщения об отказах и частичном переносе по причинам (`tree.move.refused.*`, `tree.move.partial`) в `ts-notices` через существующий `ErrorBanner`/путь ошибок дерева в `ui/src/vault-tree/useMoveEntry.ts` и `ui/src/vault-tree/Sidebar.tsx`; при недоступном или только-для-чтения vault жест не начинается, причина показана per FR-004, FR-006, FR-007

**Checkpoint**: все три истории P1 готовы — перенос жестом безопасен.

## Phase 6: User Story 4 — Перенести без перетаскивания (P2)

**Goal**: действие «Переместить в…» с выбором папки и фильтром, полностью с клавиатуры.

**Independent Test**: не касаясь мыши, перенести заметку в папку третьего уровня.

### Tests for User Story 4

- [X] T034 [P] [US4] Сценарии фильтра `move_targets`: подстрока без учёта регистра по имени (с `name_match`) и по пути (без него), кириллица, пустой результат, корень находится по имени vault — в `packages/core/tests/common/relocate_cases.rs` и обоих раннерах per FR-022

### Implementation for User Story 4

- [X] T035 [US4] Фильтр `query` и `name_match` в `move_targets` в `packages/core/src/vault/relocate.rs`
- [X] T036 [US4] `ui/src/vault-tree/MoveToDialog.tsx` по макету (экран «Без перетаскивания»): разметка и классы палитры, заголовок с именем элемента, запрос `moveTargets(from, query)` на каждое изменение, ↑/↓/Enter/Escape, пустое состояние, возврат фокуса на строку дерева; Enter → `requestMove` per FR-022, FR-023
- [X] T037 [US4] Пункт «Переместить в…» в меню «⋯» и контекстном меню заметки, доски и папки в `ui/src/vault-tree/VaultTreeActions.tsx`; состояние открытого окна в `useVaultTreeActions` per FR-021
- [X] T038 [US4] Команда «Переместить в…» для открытого файла в `ui/src/workspace/CommandPalette.tsx` и `ui/src/workspace/Workspace.tsx`: тем способом, каким палитра показывает действия без `CommandId`; если такого нет — `CommandId::MoveEntry` без сочетания по умолчанию в `packages/core/src/settings/keybindings.rs` с обновлением его тестов (research.md §10) per FR-021

## Phase 7: User Story 5 — Перетащить пальцем (P2)

**Goal**: на сенсорном экране перенос начинается удержанием, прокрутка и касание работают как раньше.

**Independent Test**: на телефоне прокрутить дерево, открыть заметку касанием, перенести её долгим нажатием.

- [X] T039 [US5] Ветка `pointerType === "touch"` в `ui/src/vault-tree/useTreeDrag.ts`: удержание 400 мс без сдвига больше 8 px начинает жест, сдвиг раньше — обычная прокрутка; `touch-action: pan-y` на дереве и `none` после начала жеста; подавление системного контекстного меню и выделения текста на время жеста per FR-009, SC-008
- [X] T040 [US5] Стиль «поднятой» строки (`.ts-tree__row--lifted`) по макету (экран «Сенсорный экран») в `ui/src/styles/app.css` и перенос строки под пальцем вместо ярлыка в `ui/src/vault-tree/TreeDragGhost.tsx`

## Phase 8: User Story 6 — Вернуть случайный перенос (P3)

**Goal**: сообщение «Перенесено в …» с действием «Отменить» для последнего переноса.

**Independent Test**: перенести папку, нажать «Отменить» — папка на прежнем месте с тем же содержимым.

- [X] T041 [P] [US6] Сценарии отмены: `move_entry(outcome.undo)` возвращает файлы, индекс, раздел и буфер в исходное состояние для заметки (с папкой снимков) и для папки; занятое прежнее место — `NameConflict`, ничего не затёрто — в `packages/core/tests/common/relocate_cases.rs` и обоих раннерах per FR-024, FR-025
- [X] T042 [US6] `ui/src/vault-tree/MoveNotice.tsx` по макету (экран «После броска»): «Перенесено в {folder}» и «Отменить», 8 с или до следующего переноса; отмена — `requestMove(outcome.undo, { confirm: false })`, затем «Перенос отменён»; отказ отмены — `tree.move.undoRefused`; подключить в `ui/src/vault-tree/Sidebar.tsx` и `ui/src/vault-tree/useMoveEntry.ts`; стили в `ui/src/styles/app.css` per FR-024, FR-025

## Phase 9: Polish & Cross-Cutting

- [X] T043 Сценарий `scripts/dev/scenarios/tree-move.mjs` по quickstart.md (шаги 1–8) по образцу `scripts/dev/scenarios/rules-editor.mjs`; прогнать с `--opfs-vault`, сверить снимки шагов 1, 4, 6, 7 с артбордами макета, исправить расхождения; отдельным сценарием `scripts/dev/scenarios/tree-move-scale.mjs` — vault с 5 000 заметок: замер времени от движения указателя до смены подсвеченной цели, не больше одного кадра (SC-006)
- [X] T044 Проверить достижимость из UI: грепом по `moveTargets`, `moveImpact`, `moveEntry` убедиться в цепочке ядро → wasm-биндинг и команда Tauri → `VaultClient` → вызов в `ui/src/vault-tree/`
- [X] T045 Гейты: `.specify/scripts/typsidian/gates.sh all`, `scripts/check-i18n.sh`; сборка ядра под оба таргета, `relocate_native` и `relocate_wasm` зелёные
- [ ] T046 Ручная проверка desktop по quickstart.md: папка в 1 000 файлов ≤5 с (SC-005), нет «изменено снаружи» после переноса, перенос открытой заметки с несохранёнными правками
- [ ] T047 Ручная проверка Android по quickstart.md: прокрутка не начинает жест, удержание поднимает строку, «Переместить в…» из меню
- [X] T048 Обновить `README.md` (перенос в дереве) и поставить `**Status**: Implemented` в `specs/023-tree-drag-drop/spec.md`

## Dependencies

- Phase 2 блокирует все истории. Внутри неё: T002 → T003, T004 → T008 → T010, T011 → T012 → T013 → T017; T005, T006, T014 параллельны; T015 → T016 → T017.
- US1 (T018–T025) — после Phase 2. US2 — после US1 (тот же жест). US3 — после Phase 2, не зависит от US1/US2 по ядру; T032–T033 правят `useMoveEntry` из T017.
- US4 — после Phase 2 и T020; от жеста не зависит. US5 — после US1. US6 — после Phase 2.
- Polish — после всех историй; T046, T047 — вручную.

## Parallel Example

```text
Phase 2:  T005 ║ T006 ║ T014           (разные файлы)
US1:      T018 ║ T019 ║ T024           (тесты и ярлык, пока пишется T021)
US3:      T029 ║ T030
```

## Implementation Strategy

MVP — Phase 1–2 и US1–US3 (все P1): перенос жестом заметок и папок с гарантиями. Затем
US4 (клавиатура), US5 (сенсорный экран), US6 (отмена). После каждой истории — её
Independent Test; `[X]` ставится, когда задача сделана на всех таргетах.

## Phase 10: Convergence

- [X] T049 CRITICAL: дать прервать идущий перенос папки на web — `AbortController` в `ui/src/vault-tree/useMoveEntry.ts`, его `signal` в `client.moveEntry`, кнопка прерывания в строке прогресса `ui/src/vault-tree/Sidebar.tsx` (ключ `tree.move.cancel` в `ui/src/i18n/locales/{en,ru}/tree.ts`, стиль в `ui/src/styles/app.css`); после прерывания vault прежний, сообщений нет per Constitution X, research.md §6 (partial)
