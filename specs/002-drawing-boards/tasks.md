---

description: "Task list for Drawing Boards"
---

# Tasks: Drawing Boards

**Input**: Design documents from `/specs/002-drawing-boards/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md),
[data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: включены обязательно. Принцип I конституции требует, чтобы каждая единица логики
ядра проходила и `cargo test`, и `wasm-bindgen-test`; логика без тестов на обоих путях не
считается готовой. Это гейт проекта, а не выбор режима TDD.

**Organization**: задачи сгруппированы по user story, чтобы каждую можно было реализовать и
проверить независимо.

> **Ревизия после `/speckit-analyze`.** Файл перенумерован целиком: на момент правки ни одна
> задача не была закрыта, поэтому история отметок не пострадала. Закрыты находки G1, G2, G3,
> D1, G4, G5, U1, U2, A1, F1 — см. раздел «Что изменил анализ» в конце файла.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: можно выполнять параллельно (разные файлы, нет незакрытых зависимостей)
- **[Story]**: к какой user story относится задача (US1, US2, US3)
- В описании — точный путь к файлу

## Path Conventions

Структура из `plan.md` → Project Structure: ядро в `packages/core/src/`, общий UI в
`ui/src/`, web-клиент в `apps/web/`, тесты ядра в `packages/core/tests/`.

---

## Phase 1: Setup (Shared Infrastructure)

- [X] T001 Добавить `@excalidraw/excalidraw` версии `0.18.1` в зависимости `ui/package.json`
      и установить через `nix develop --command pnpm install`. Версия и лицензия проверены
      (MIT, peer `react: ^17 || ^18 || ^19`), см. research.md §1
- [X] T002 **Гейт риска, выполняется до любой логики.** Смонтировать движок в
      `ui/src/boards/BoardCanvas.tsx` минимальным компонентом и открыть его в браузере через
      `apps/web`: Excalidraw требует `window` и не рендерится на сервере, поэтому подключение
      идёт динамическим импортом с `ssr: false`. Пока полотно не появилось на экране,
      остальные задачи фичи выполнять нельзя — research.md §1, contracts/board-engine-bridge.md
      - **Сделано наполовину 2026-08-09.** `ui/src/boards/BoardCanvas.tsx` написан:
        `lazy()` + флаг `isMounted` вместо `next/dynamic`, чтобы компонент оставался
        framework-agnostic и жил в общем `ui/`; CSS движка импортируется там же
        (`@excalidraw/excalidraw/index.css`, объявление в `ui/src/types/css.d.ts`).
        Временный маршрут `apps/web/app/board-gate/` смонтирован для проверки.
      - **Пройдено**: `next build` собирает `/board-gate` и **пререндерит его статически** —
        значит модуль движка и его CSS резолвятся, а на сервере движок не выполняется
        (иначе сборка упала бы на отсутствии `window`). `tsc --noEmit` чист.
      - **Закрыт 2026-08-09**: пользователь открыл `/board-gate` в браузере и подтвердил,
        что полотно Excalidraw рисуется. Временный маршрут `apps/web/app/board-gate/`
        удалён, `ui/src/boards/BoardCanvas.tsx` остаётся рабочим компонентом.
- [X] T003 [P] Создать скелет модуля ядра `packages/core/src/boards/mod.rs` и подключить его
      в `packages/core/src/lib.rs`
- [X] T004 [P] Создать наборы тестов `packages/core/tests/boards_native.rs` (`cargo test`) и
      `packages/core/tests/boards_wasm.rs` (`wasm-bindgen-test`) с cfg-разметкой по образцу
      `vault_native.rs`/`vault_wasm.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: то, без чего ни одна user story не начинается. Классификация файлов и состав
дерева нужны US1 и US2; хэш — US1 (конфликты) и US3 (устаревание); общая обёртка экспорта —
US1 (превью конфликта) и US2 (снимок).

- [X] T005 Ввести `FileKind { Note, Board, Snapshot, Other }` в
      `packages/core/src/vault/mod.rs` с классификацией по пути, без чтения содержимого —
      data-model.md «FileKind», research.md §3
- [X] T006 Переключить `packages/core/src/vault/native.rs` на `FileKind`, удалив локальный
      `is_note(path: &Path)` со строки 27 и все его вызовы
- [X] T007 Переключить `packages/core/src/vault/web.rs` на `FileKind`, удалив локальный
      `is_note(name: &str)` со строки 21 и его вызов на строке 74
- [X] T008 Включить доски в состав дерева: `list()` в `packages/core/src/vault/native.rs` и
      `packages/core/src/vault/web.rs` возвращает записи с `FileKind::Note` **и**
      `FileKind::Board`, исключая `Snapshot` и `Other`. Без этой правки созданная доска в
      дереве не появится, и US1 не выполнима per FR-004, FR-021
- [X] T009 [P] Тест классификации на обоих таргетах: `.typ` → `Note`, `.excalidraw` →
      `Board`, файл в `<имя заметки>.assets/` → `Snapshot`, произвольный `.svg` в vault →
      `Other`; оба backend дают одинаковый результат на одинаковом пути — в
      `packages/core/tests/vault_native.rs` и `vault_wasm.rs` per Constitution II
- [X] T010 [P] Тест состава дерева на обоих таргетах: доска присутствует в результате
      `list()`, снимок и произвольный файл отсутствуют — в
      `packages/core/tests/vault_native.rs` и `vault_wasm.rs` per FR-004, FR-021
- [X] T011 Реализовать нормализацию содержимого доски и `BoardDigest` (BLAKE3) в
      `packages/core/src/boards/digest.rs`: в хэш входят порядок элементов, геометрия, стиль,
      текст и привязки; не входят `version`, `versionNonce`, `updated`, `seed`, `appState` и
      порядок ключей JSON — data-model.md «BoardDigest», research.md §5
- [X] T012 [P] Тест нормализации на обоих таргетах: одинаковый вход даёт одинаковый
      `BoardDigest` на native и в WASM; изменение только полей ревизий движка хэш **не**
      меняет — в `boards_native.rs` и `boards_wasm.rs` per Constitution II, FR-026
- [X] T013 Объявить типы `Board`, `BoardContent`, `BoardDigest`, `BoardSaveState`,
      `EmbedState` в `packages/core/src/boards/mod.rs` с разметкой `tsify` для границы
      WASM/JS; ручных копий на TypeScript не создавать per Constitution IV
- [X] T014 [P] Объявить `BoardEvent` в `packages/core/src/boards/events.rs` четырьмя
      вариантами `SaveStateChanged`/`SaveFailed`/`ConflictDetected`/`EmbedsChanged` —
      contracts/frontend-board-events.md
- [X] T015 Реализовать общую обёртку экспорта `ui/src/boards/exportSvg.ts` поверх
      `exportToSvg` движка. Её переиспользуют обе истории: превью конфликта в US1 и снимок в
      US2. Отдельный вызов `exportToSvg` где-либо ещё запрещён — иначе поведение при отказе
      экспорта разойдётся между двумя местами per contracts/board-engine-bridge.md

**Checkpoint**: ядро отличает доску от заметки и от снимка, доски видны в дереве, хэш
устойчив к полям ревизий, экспорт вызывается из одного места.

---

## Phase 3: User Story 1 - Создать доску и рисовать на ней (Priority: P1) 🎯 MVP

**Goal**: пользователь создаёт доску в vault, рисует на полотне вместо панели
«редактор + preview», и нарисованное само попадает в файл.

**Independent Test**: создать доску в открытом vault, нарисовать, перезагрузить приложение —
рисунок на месте, файл доски виден в папке vault сторонним файловым менеджером.

### Tests for User Story 1 ⚠️

- [X] T016 [P] [US1] Тест: `create_board` создаёт файл, который классифицируется как
      `FileKind::Board` и присутствует в результате `list()` — в `boards_native.rs` и
      `boards_wasm.rs` per FR-001, FR-004
- [X] T017 [P] [US1] Тест: переименование, перемещение и удаление доски проходят теми же
      вызовами `VaultBackend`, что и для заметки, с теми же отказами; отдельных операций для
      доски не появилось — в `boards_native.rs` и `boards_wasm.rs` per FR-003
- [X] T018 [P] [US1] Тест: создание, изменение, переименование и удаление файла доски **вне
      приложения** отражаются в составе дерева без перезапуска, тем же потоком `VaultEvent`,
      что и для заметок — в `boards_native.rs` и `boards_wasm.rs` per FR-005, Constitution II
- [X] T019 [P] [US1] Тест: `write_board` с несовпадающим `expected: FileStamp` отклоняется
      как `Conflicted` и **не касается файла** — в `boards_native.rs` и `boards_wasm.rs`
      per FR-014, data-model.md инвариант 1
- [X] T020 [P] [US1] Тест: пока по доске есть неразрешённый конфликт, запись отклоняется, в
      том числе принудительным сбросом дебаунса — в `boards_native.rs` и `boards_wasm.rs`
      per FR-014
- [X] T021 [P] [US1] Тест: `read_board` на повреждённом JSON возвращает `Unreadable`, файл
      после этого **не перезаписан** — в `boards_native.rs` и `boards_wasm.rs` per FR-017
- [X] T022 [P] [US1] Тест: на профиле без права записи доски `write_board` отклоняется как
      `ReadOnlyProfile`; путь записи отсутствует, а не заблокирован флагом в UI — в
      `boards_native.rs` и `boards_wasm.rs` per FR-035, research.md §8
- [X] T023 [P] [US1] Тест: переходы `BoardSaveState` — `Clean → Dirty → Saving → Clean`,
      `Saving → Failed`, `Failed → Dirty`; из `Failed` нет автоперехода в `Clean` — в
      `boards_native.rs` и `boards_wasm.rs` per FR-009, FR-011
- [X] T024 [P] [US1] Тест: при пропаже доступа к vault во время рисования несохранённые
      изменения доски остаются в памяти сессии и не теряются — тем же правилом, что для
      заметок в 001 (FR-035 фичи 001) — в `boards_native.rs` и `boards_wasm.rs`
      per spec.md Edge Cases

### Implementation for User Story 1

- [X] T025 [US1] Реализовать `create_board`, `read_board`, `write_board` и `digest` трейта
      `BoardStore` в `packages/core/src/boards/mod.rs` поверх `VaultBackend` из 001;
      содержимое доски пересекает границу непрозрачными байтами. Файл доски записывается как
      обычный JSON движка, без обёртки приложения — contracts/board-store-trait.md,
      per FR-002, research.md §2
- [X] T026 [US1] Реализовать гейты записи `Conflicted` / `ReadOnlyProfile` и наследование
      отказов `VaultBackend` в `packages/core/src/boards/mod.rs` —
      contracts/board-store-trait.md § Гейты записи
- [X] T027 [US1] Реализовать машину `BoardSaveState` с переходами из data-model.md в
      `packages/core/src/boards/mod.rs`; отказ записи не теряет содержимое буфера per FR-010
- [X] T028 [US1] Довести `ui/src/boards/BoardCanvas.tsx` до рабочего полотна: загрузка
      содержимого в движок, `viewModeEnabled` как проп, оформление **только контейнера** —
      рамка, заголовок, индикатор сохранения. Внутренний DOM движка не стилизуется ни
      селекторами, ни из JS per Constitution VIII, contracts/board-engine-bridge.md
- [X] T029 [US1] Реализовать `ui/src/boards/useBoardDocument.ts`: дебаунс 800 мс от
      последнего изменения, принудительный сброс перед переключением заметки, закрытием окна
      и построением снимка; сериализация и хэширование выполняются вне главного потока (Web
      Worker на web, задача `tokio` нативно) per FR-008, SC-004, SC-005, research.md §6
- [X] T030 [US1] Переключать рабочую область между «редактор + preview» и полотном в
      `apps/web/app/VaultWorkspace.tsx` — в том же окне, без открытия отдельного per FR-006, FR-007
- [X] T031 [P] [US1] Показать признак доски в дереве в `ui/src/vault-tree/VaultTree.tsx` и
      `ui/src/vault-tree/VaultTreeActions.tsx`: отличие от текстовой заметки видно с одного
      взгляда, без открытия файла per FR-004, SC-001
- [X] T032 [US1] Расширить `ui/src/editor/useWindowCloseGuard.ts` на несохранённые изменения
      доски: предупреждение перед закрытием окна per FR-011
- [X] T033 [US1] Реализовать `ui/src/boards/BoardConflictDialog.tsx`: две превью рядом —
      своя версия и внешняя — **через общую обёртку `ui/src/boards/exportSvg.ts`** (T015),
      явный выбор одной из них per FR-015, SC-006
- [X] T034 [US1] Обработать отказ построения превью в `ui/src/boards/BoardConflictDialog.tsx`:
      на месте недоступной превью сказано, что показать её не удалось, выбор остаётся
      доступен, в файл доски до выбора не пишется ничего per FR-016
- [ ] T035 [US1] Прогнать `specs/002-drawing-boards/quickstart.md` Сценарии 1, 2 и 3 на
      web-сборке. Дополнительно зафиксировать два измерения, которых в сценариях пока нет:
      число действий от открытого vault до начала рисования (SC-002) и поведение при той же
      доске, открытой во втором окне (spec.md Edge Cases). Сценарий 1 при этом закрывает
      восстановление содержимого при повторном открытии (шаг 7) и работу при полностью
      отключённой сети (шаг 8) — per FR-012, FR-013, SC-010. Native-половину зафиксировать как
      не проверенную до появления `apps/desktop` (`001-vault-manager/tasks.md` → T112)
      per Constitution II

**Checkpoint**: US1 работает независимо — доска создаётся, видна в дереве, рисуется,
сохраняется, конфликты разрешаются выбором.

---

## Phase 4: User Story 2 - Вставить доску в текстовую заметку (Priority: P2)

**Goal**: пользователь вставляет доску в Typst-заметку, и preview показывает рисунок; заметка
остаётся обычным Typst.

**Independent Test**: создать доску с рисунком, открыть текстовую заметку, вставить доску и
убедиться, что preview показывает нарисованное, а `typst compile` собирает заметку без
Typsidian.

**Depends on**: US1 (доску нужно уметь создать и прочитать).

### Tests for User Story 2 ⚠️

- [X] T036 [P] [US2] Тест: `plan_embed` выдаёт разные `EmbedId` для двух вставок одной доски
      в одну заметку и для вставок в разные заметки — в `boards_native.rs` и `boards_wasm.rs`
      per FR-020, FR-023
- [X] T037 [P] [US2] Тест: `embeds_of` на тексте с двумя вставками возвращает две записи с
      корректными диапазонами; вызов `#image`, ведущий не в `*.assets/`, вставкой не считается
      — в `boards_native.rs` и `boards_wasm.rs` per FR-026, research.md §11
- [X] T038 [P] [US2] Тест: `embed_markup` даёт обычный вызов `#image(...)` без каких-либо
      маркеров приложения — в `boards_native.rs` и `boards_wasm.rs` per FR-022
- [X] T039 [P] [US2] Тест: `snapshot_metadata` → `read_snapshot_metadata` возвращает тот же
      `board_path` и `board_digest` (round-trip) — в `boards_native.rs` и `boards_wasm.rs`
      per data-model.md «Snapshot»
- [X] T040 [P] [US2] Тест: удаление вставки из текста заметки не изменяет ни файл доски, ни
      файл снимка — в `boards_native.rs` и `boards_wasm.rs` per FR-024, инвариант 4

### Implementation for User Story 2

- [X] T041 [US2] Реализовать `packages/core/src/boards/snapshot.rs`: путь
      `<папка заметки>/<имя заметки>.assets/<id>.svg`, закрепление имени за вставкой,
      формирование и чтение метаданных `data-typsidian-board` / `data-typsidian-board-hash`
      per FR-019, FR-020, research.md §4, §5
- [X] T042 [US2] Реализовать `packages/core/src/boards/embed.rs`: разбор вызовов `#image` в
      тексте заметки и опознание вставки по адресу, а не по маркеру per FR-026, research.md §11
- [X] T043 [US2] Реализовать `plan_embed`, `embed_markup`, `snapshot_metadata` и
      `read_snapshot_metadata` в `packages/core/src/boards/mod.rs` — contracts/board-store-trait.md
- [X] T044 [US2] Реализовать `ui/src/boards/exportSnapshot.ts`: вызов общей обёртки
      `ui/src/boards/exportSvg.ts` (T015), вложение метаданных из ядра в `<desc>` и запись
      файла через `VaultBackend`; перед экспортом принудительно сбрасывается дебаунс
      автосохранения per FR-019, research.md §6
- [X] T045 [US2] Реализовать `ui/src/boards/InsertBoardDialog.tsx`: выбор доски среди досок
      vault, вставка не более чем в три действия от открытой заметки per FR-018, SC-007
- [X] T046 [P] [US2] Обработать пустой список досок в `ui/src/boards/InsertBoardDialog.tsx`:
      понятное сообщение и возможность создать доску, без ошибки per US2 AS2
- [X] T047 [P] [US2] Реализовать переход от вставки в тексте к её доске на полотне в
      `ui/src/boards/EmbedLink.tsx` per FR-025
- [ ] T048 [US2] Прогнать `specs/002-drawing-boards/quickstart.md` Сценарии 4 и 6, включая
      сборку заметки сторонним `typst compile` после копирования папки vault per FR-022, SC-011

**Checkpoint**: US1 и US2 работают независимо — рисунок становится частью документа, и
документ собирается без Typsidian.

---

## Phase 5: User Story 3 - Обновить устаревшую вставку (Priority: P3)

**Goal**: пользователь видит, что снимок разошёлся с доской, и приводит его в соответствие
одним действием.

**Independent Test**: вставить доску в заметку, изменить доску, вернуться к заметке — виден
индикатор устаревания; обновить — снимок совпал с доской, индикатор исчез.

**Depends on**: US2 (вставка должна существовать).

### Tests for User Story 3 ⚠️

- [X] T049 [P] [US3] Тест: расхождение хэшей даёт `EmbedState::Stale` — в `boards_native.rs`
      и `boards_wasm.rs` per FR-026, SC-008
- [X] T050 [P] [US3] **Ключевой тест нормализации**: открытие доски без единой правки
      оставляет вставку в `UpToDate`; поля ревизий движка на состояние не влияют — в
      `boards_native.rs` и `boards_wasm.rs` per research.md §5
- [X] T051 [P] [US3] Тест: снимок без метаданных или с нечитаемыми метаданными даёт
      `Undeterminable { reason }`, а **не** `UpToDate` — в `boards_native.rs` и
      `boards_wasm.rs` per SC-008, spec.md Edge Cases
- [X] T052 [P] [US3] Тест: отсутствующая доска даёт `BoardMissing`, отсутствующий файл
      снимка при живой доске — `SnapshotMissing`; варианты не схлопываются — в
      `boards_native.rs` и `boards_wasm.rs` per FR-031, FR-032
- [X] T053 [P] [US3] Тест: обновление вставки затрагивает ровно один файл снимка — тот, что
      закреплён за её `EmbedId`; снимки других вставок той же доски не меняются — в
      `boards_native.rs` и `boards_wasm.rs` per FR-029, инвариант 3
- [X] T054 [P] [US3] Тест: без явного действия пользователя снимок не переписывается ни при
      каком переходе `EmbedState` — в `boards_native.rs` и `boards_wasm.rs` per FR-030,
      инвариант 2

### Implementation for User Story 3

- [X] T055 [US3] Реализовать вычисление `EmbedState` всеми пятью вариантами в
      `packages/core/src/boards/embed.rs` — data-model.md «EmbedState»
- [X] T056 [US3] Реализовать событие `EmbedsChanged` в `packages/core/src/boards/events.rs`,
      возникающее и при изменении доски, а не только текста заметки —
      contracts/frontend-board-events.md
- [X] T057 [US3] Реализовать `ui/src/boards/EmbedStaleBadge.tsx`: пять состояний показываются
      по-разному, `Undeterminable` не схлопывается в `UpToDate`, `SnapshotMissing` не
      схлопывается в `BoardMissing` per FR-027, FR-031, FR-032, SC-008
- [X] T058 [US3] Реализовать обновление вставки одним действием от видимого индикатора:
      экспорт текущего состояния доски в тот же файл снимка per FR-028, SC-009
- [X] T059 [P] [US3] Реализовать восстановление отсутствующего снимка из доски одним
      действием в `ui/src/boards/EmbedStaleBadge.tsx` per FR-032
- [ ] T060 [US3] Прогнать `specs/002-drawing-boards/quickstart.md` Сценарий 5 целиком,
      включая шаг 3 (открытие доски без правок не зажигает индикатор) и шаг 5 (обновление
      одной вставки не меняет вторую) per FR-029, SC-008

**Checkpoint**: все три user story работают независимо; молчаливого расхождения снимка с
доской не остаётся.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T061 [P] Реализовать бенчмарк нормализации и хэша на доске в 2 000 элементов в
      `packages/core/benches/board_digest.rs` per SC-005, research.md §10
- [ ] T062 Прогнать `specs/002-drawing-boards/quickstart.md` Сценарий 7: открытие ≤2 с,
      отклик ≤100 мс в том числе во время автосохранения, запись ≤2 с после последнего штриха
      per SC-003, SC-004, SC-005, FR-037
- [X] T063 Ревью на соответствие принципу VIII: в `ui/src/boards/` и во всех CSS проекта нет
      ни одного селектора, попадающего внутрь DOM движка, и ни одной правки его узлов из JS;
      `exportToSvg` вызывается только из `ui/src/boards/exportSvg.ts`
      per Constitution VIII, contracts/board-engine-bridge.md § Запреты
- [X] T064 [P] Прогнать `nix develop --command pnpm -r typecheck`: TypeScript `strict` без
      `any`, без `@ts-ignore`; типы границы взяты из сгенерированных биндингов per Constitution IV
- [X] T065 [P] Прогнать `nix develop --command cargo test -p typsidian-core` и
      `nix develop --command wasm-pack test --headless --chrome packages/core` целиком per
      Constitution II
- [X] T066 [P] Разрешить открытый вопрос формата снимка в `specs/002-drawing-boards/research.md`
      §4: либо задать проверяемый критерий перехода на PNG, либо убрать запасной вариант.
      Сейчас PNG назван запасным «если `exportToSvg` окажется несовместим», но условие не
      определено и задачи под него нет
- [ ] T067 Реализовать мобильный просмотр в `ui/src/boards/BoardCanvas.tsx`:
      `viewModeEnabled`, объявленное пользователю ограничение, отсутствие пути записи на
      уровне ядра per FR-034, FR-035, research.md §8 (blocked-by: `apps/mobile`,
      `001-vault-manager/tasks.md` → T116)
- [ ] T068 Прогнать `specs/002-drawing-boards/quickstart.md` Сценарий 8 на мобильной сборке:
      сверка отпечатка файла доски до и после просмотра, **и отдельно** — что вставка доски в
      заметку и обновление устаревшей вставки работают там так же, как на web
      per FR-035, FR-036, SC-012 (blocked-by: T067)
- [ ] T069 Прогнать все сценарии `quickstart.md` на desktop-сборке и подтвердить паритет с
      web per FR-033, Constitution II (blocked-by: `apps/desktop`,
      `001-vault-manager/tasks.md` → T112)

---

## Что изменил анализ

`/speckit-analyze` от 2026-08-09 нашёл 12 расхождений. Здесь закрыты все, кроме двух LOW,
оставленных намеренно.

| Находка | Было | Стало |
|---|---|---|
| G1 HIGH | правки `list()` не было ни в одной задаче, хотя тест на «доска появляется в дереве» был | T008 включает `Board` в состав дерева на обоих backend; T010 это проверяет |
| G2 HIGH | FR-003 (переименование, перемещение, удаление доски) без задач | T017 |
| G3 HIGH | FR-005 (внешние изменения доски) без задач | T018 |
| D1 MEDIUM | `exportToSvg` вызывался бы дважды — в превью конфликта (US1) и в снимке (US2) | T015 выносит обёртку в Phase 2; T033 и T044 её переиспользуют; T063 проверяет, что других вызовов нет |
| G4 MEDIUM | FR-036 (вставка и обновление на мобильном) без задач | добавлено в T068 |
| G5 MEDIUM | SC-002 (≤3 действия до начала рисования) не измерялся нигде | добавлено в T035 |
| U1 MEDIUM | краевой случай «vault пропал во время рисования» без покрытия | T024 |
| U2 MEDIUM | краевой случай «та же доска в двух окнах» без покрытия | добавлено в T035 |
| A1 MEDIUM | PNG как запасной формат без критерия переключения | T066 закрывает вопрос в одну или другую сторону |
| F1 LOW | T025 и T041 прежней нумерации указывали каталог вместо файла | T031 и T047 указывают файлы |
| D2 LOW | FR-014 и FR-016 частично перекрываются | оставлено: второе — намеренное усиление краевого случая |
| I1 LOW | классификация снимка при удалённой заметке-владельце | оставлено: файл в обоих прочтениях остаётся скрытым, поведение не меняется |

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)** → блокирует всё. T002 — гейт риска: пока движок не смонтирован в
  браузере, остальное строить не на чем.
- **Phase 2 (Foundational)** → блокирует все user story. T005-T008 меняют оба backend и
  выполняются последовательно; T011 нужен и US1 (конфликты), и US3 (устаревание); T015 нужен
  и US1 (превью конфликта), и US2 (снимок).
- **Phase 3 (US1)** → зависит от Phase 2.
- **Phase 4 (US2)** → зависит от US1: вставлять нечего, пока доску нельзя создать.
- **Phase 5 (US3)** → зависит от US2: устаревать нечему, пока нет вставки.
- **Phase 6 (Polish)** → зависит от историй, кроме T061, T064 и T066, выполнимых раньше.

### User Story Dependencies

Истории выстроены цепочкой, а не независимо, и это свойство самой фичи: US2 работает с
объектом, который создаёт US1, а US3 следит за связью, которую заводит US2. Каждая при этом
остаётся самостоятельно проверяемой — свой Independent Test и свои сценарии `quickstart.md`.

### Within Each User Story

Тесты пишутся до реализации и падают на пустом модуле; затем ядро; затем UI; затем прогон
сценариев `quickstart.md`.

### Parallel Opportunities

- Phase 1: T003 и T004 параллельны друг другу.
- Phase 2: T009 и T010 параллельны; T012 параллелен им; T014 параллелен T013. T008 — нет:
  он правит те же файлы, что T006 и T007.
- US1: девять тестов T016-T024 параллельны — разные проверки в одних и тех же двух файлах,
  поэтому распараллеливаются по авторам, а не по процессам. T031 параллелен реализации ядра:
  он живёт в `ui/src/vault-tree/`.
- US2: T036-T040 параллельны; T046 и T047 параллельны друг другу.
- US3: T049-T054 параллельны; T059 параллелен T058.
- Polish: T061, T064, T065, T066 параллельны.

## Parallel Example: User Story 1

```text
Одновременно (разные проверки, независимые входы):
  T016 create_board → Board, есть в list()
  T017 переименование/перемещение/удаление доски
  T018 внешние изменения доски без перезапуска
  T019 write_board с чужим FileStamp → Conflicted
  T020 запись при активном конфликте → отклонена
  T021 повреждённый файл → Unreadable, файл цел
  T022 профиль без записи → ReadOnlyProfile
  T023 переходы BoardSaveState
  T024 буфер переживает пропажу vault

Затем последовательно (один файл mod.rs):
  T025 → T026 → T027

Параллельно с ними:
  T031 признак доски в дереве (ui/src/vault-tree/)
```

## Implementation Strategy

### MVP First (только User Story 1)

Phase 1 → Phase 2 → Phase 3, то есть T001-T035. На этом месте пользователь уже получает
работающий инструмент: доска создаётся, видна в дереве наравне с заметками, рисуется,
сохраняется в его vault обычным файлом и открывается на excalidraw.com. Ценность
самостоятельна и не зависит от связки с текстом.

### Incremental Delivery

1. **MVP** — US1: доска как самостоятельный тип файла vault.
2. **+US2** — рисунок становится частью документа; заметка собирается сторонним Typst.
3. **+US3** — расхождение снимка с доской перестаёт быть молчаливым.

Каждый шаг заканчивается прогоном своих сценариев `quickstart.md` и остаётся пригодным для
остановки: следующая история ничего не ломает в предыдущей.

### Порядок с оглядкой на риск

T002 стоит вторым не по важности, а по риску: интеграция с чужим движком в Next.js — самое
вероятное место, где план разойдётся с реальностью. Если движок не монтируется, дешевле
узнать это до того, как написаны девять тестов и три модуля ядра.

T008 — второй по значимости и самый неочевидный: без него доска создаётся, пишется и
тестируется, но в дереве не появляется, и US1 упирается в тупик на этапе, когда почти всё уже
написано. Ровно эту дыру нашёл `/speckit-analyze`.

Третий по риску — T011 (нормализация): ошибка в ней не ломает сборку, а проявляется как
индикатор устаревания, загорающийся от простого открытия доски. Поэтому T050 вынесен в
отдельный «ключевой» тест, а не растворён в проверках US3.

## Notes

- **[P]** — разные файлы, нет зависимостей между задачами
- Каждая задача ядра требует прохождения **и** `cargo test`, **и** `wasm-bindgen-test`
  (принцип II, NON-NEGOTIABLE)
- Экспорт SVG и рисование тестами ядра не покрываются принципиально: это работа движка, у
  которой нет Rust-стороны (research.md §9). Они проверяются сценариями `quickstart.md`
- Ядро не принимает и не возвращает модель Excalidraw: содержимое доски пересекает границу
  непрозрачными байтами плюс хэшем (contracts/board-store-trait.md)
- Доска подчиняется правилам 001 без исключений: отдельных путей записи, переименования и
  удаления для неё не заводится
- T067-T069 заблокированы отсутствием `apps/mobile` и `apps/desktop` — это блокировка
  приёмки фичи, а не её реализации
- Коммит после каждой задачи или логической группы
- На любом checkpoint можно остановиться и проверить историю независимо

---

## Phase 7: Ревизия по ходу реализации (2026-08-09)

**Purpose**: зафиксировать то, что обнаружилось при выполнении Phase 2 и US1 и чего в списке
не было. Существующие ID не переиспользуются.

- [X] T070 Пробросить операции с досками в WASM-биндинг `WebBackend`
      (`packages/core/src/vault/web.rs`): `createBoard`, `readBoard`, `writeBoard`,
      `boardDigest`. **Обнаружено при реализации**: `create_board`/`read_board`/`write_board`
      обобщены по `VaultBackend` и работают, но в сгенерированном
      `ui/src/wasm-core/typsidian_core.d.ts` слово «board» не встречалось ни разу — UI
      физически не мог создать доску, а задачи T028-T030 это молча предполагали. Тот же
      класс дыры, что закрыт в 004 задачами T108-T109 per Constitution I, IV
- [X] T071 Починить расхождение таргетов в `packages/core/src/vault/native.rs`:
      `VaultBackend::read` схлопывал все ошибки в `ReadError::Io`, включая отсутствие файла,
      тогда как `WebBackend::read` возвращает `NotFound` (`web.rs:269`). Из-за этого
      формулировка «заметку удалили или переименовали снаружи» в
      `apps/web/app/NoteEditor.tsx` была недостижима на native. **Дефект фичи 001**,
      найденный тестом фичи 002; регрессионный тест добавлен в `vault_native.rs`
      per Constitution II (NON-NEGOTIABLE), 001 FR-008, FR-014

---

## Phase 8: Ревизия по ходу US2 и US3 (2026-08-10)

**Purpose**: то же, что у Phase 7 — зафиксировать обнаруженное при реализации, чего в списке
не было. Существующие ID не переиспользуются.

Четыре из семи задач ниже (T075-T078) — дыры «логика в ядре есть, из приложения
недостижима», то есть тот же класс, что T070. Две (T072, T074) — дефекты фичи 001, которые
проявились только на путях, введённых 002. Одна (T073) — новая операция ядра, без которой
снимок некуда положить.

- [X] T072 Починить разрешение путей в `packages/core/src/vault/web.rs`: `find_handle`
      запрашивал **каждый** компонент пути через `getFileHandle`, поэтому ни один вложенный
      путь не разрешался вовсе, а `getFileHandle` без `{create:true}` отказывает на
      отсутствующем файле — значит `write` в новый файл и `create` (который делает
      `write(path, "")`) на web не работали ни разу. **Дефект фичи 001**, найденный
      реализацией 002: снимок вставки всегда лежит во вложенной папке `<заметка>.assets/`.
      Введены `resolve_handle(path, create_file, create_folders)` и `find_folder_handle`
      per Constitution II, 001 FR-014
- [X] T073 Ввести `VaultBackend::ensure_folder` (`packages/core/src/vault/backend.rs`,
      реализации в `native.rs` и `web.rs`) и вызвать её из `write_snapshot`. Отдельная
      операция, а не `create_dir_all` внутри `write`: «запись в несуществующую папку» обязана
      остаться отказом — на этом стоит тест 001 `write_to_missing_parent_directory_...`,
      который сначала упал именно из-за такой правки per FR-019, research.md §4
- [X] T074 Закрыть попадание снимков в дерево: `write` в обоих backend безусловно вставлял
      записанный путь в `Index` и разбирал его содержимое как Typst (`record_links`).
      Первая же вставка добавляла бы свой `.svg` в дерево вопреки FR-021 — поймано тестом
      T039 на файлах. Обе реализации теперь гейтят вставку в индекс по
      `classify(path).is_listed()` per FR-021, Constitution II
- [X] T075 Дать пользователю создать доску: `createBoard` был проброшен в WASM (T070), но в
      `ui/` его не вызывал никто — `grep createBoard ui/src apps/web` не находил ни одного
      вызова, то есть FR-001 из приложения был недостижим, а Сценарий 1 `quickstart.md`
      начинается именно с создания доски. Добавлены кнопка «+ Новая доска» в
      `ui/src/vault-tree/VaultTreeActions.tsx` и выбор вызова по виду файла в `performCreate`
      (пустой файл доски ядро читает как `Unreadable`, поэтому `create` для доски не годится)
      per FR-001, FR-017. Созданная доска сразу открывается на полотне
      (`onCreated` → `openPath`): её содержимое видно только там, а Сценарий 1 `quickstart.md`
      ожидает открытое полотно на шаге 2
- [X] T076 Пробросить в WASM операции вставок (`packages/core/src/vault/web.rs`):
      `planEmbed`, `embedMarkup`, `writeSnapshot`, `embedsOf`, `embedsInText`,
      `embedsChanged`. Без них US2 и US3 существовали бы только в тестах ядра — тот же класс
      дыры, что T070 per Constitution I, IV
- [X] T077 Довести конфликт доски до достижимого состояния: `useBoardDocument` не объявлял
      правки ядру через `beginEdit`, а без буфера в `ConflictRegistry` конвейер обнаружения
      конфликт не поднимает — диалог из T033 не мог появиться ни при каком сценарии.
      Добавлены `beginEdit` на каждое изменение полотна, `replace`/`contentRevision` для
      замены версии после выбора (движок принимает сцену только при монтировании,
      contracts/board-engine-bridge.md), `ui/src/boards/useBoardConflict.ts` (обе версии
      берутся из ядра) и связка в `apps/web/app/VaultWorkspace.tsx`, где доска получает свой
      диалог, а заметка — свой per FR-014, FR-015
- [X] T078 Довести вставки до видимого состояния: `ui/src/boards/useNoteEmbeds.ts` (состав
      вставок и их состояние, пересчёт по правилу ядра `embedsChanged`) и список вставок под
      редактором в `apps/web/app/NoteEditor.tsx`, где живут `EmbedLink` (T047) и
      `EmbedStaleBadge` (T057). Кнопка «Вставить доску» и сама вставка тоже здесь: позиция
      курсора известна только самому `textarea`. Пересчёт состояния запускается по изменению
      **состава** вставок, а не на каждый набранный символ — иначе чтение снимков шло бы на
      каждое нажатие клавиши per FR-018, FR-027, SC-007

**Про парность тестов.** Проверки состояния вставки (T049-T054) читают файлы vault, поэтому
живут только в `boards_native.rs`: смонтировать vault в headless-браузере нечем — то же
ограничение, по которому native-только были сделаны T016-T024. Платформенно нейтральная
часть (нормализация и хэш, разбор `#image`, метаданные снимка, правило пересчёта) продублирована
в `boards_wasm.rs` дословно, и именно она и могла бы разойтись между таргетами.

**Что осталось незакрытым и почему.**

- **T035, T048, T060, T062** — прогоны сценариев `quickstart.md` в браузере. Их нельзя
  закрыть из терминала: они требуют выбора папки через `showDirectoryPicker` (жест
  пользователя), рисования на полотне и измерений отклика глазами и секундомером.
  Реализация под них готова, `pnpm build` собирает `apps/web` статически.
- **T048 дополнительно ограничен** тем, что колонка предпросмотра в
  `apps/web/app/VaultWorkspace.tsx` — заглушка: компилятора Typst в `packages/core` ещё нет.
  Шаги 1, 3-10 Сценария 4 проверяемы полностью, шаг 2 в части «preview показывает рисунок»
  проверить нечем до появления компилятора. Сценарий 6 (сторонний `typst compile`) от этого
  не зависит и проверяем.
- **T069** — блокировка снята 2026-08-10: `apps/desktop` существует и собирается
  (`001-vault-manager/tasks.md` → T112-T115 закрыты). Сам прогон остаётся ручным: нужен
  дисплей и собранное окно. Доски на desktop работают тем же кодом, что на web, —
  `Workspace` там один и тот же, а операции досок проброшены командами Tauri.
- **T067-T068** — заблокированы отсутствием `apps/mobile`. Решение по нему принято в
  `001-vault-manager/plan.md` → Project Structure (T116, 2026-08-10): мобильная оболочка
  отложена до появления Android SDK/Xcode в `flake.nix`, потому что собрать и прогнать её
  здесь нечем.
