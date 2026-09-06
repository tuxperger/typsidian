---

description: "Task list for Note Rendering and Live Preview"
---

# Tasks: Note Rendering and Live Preview

**Input**: Design documents from `/specs/005-live-preview/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md),
[data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: включены обязательно. Принцип I конституции требует, чтобы каждая единица логики
ядра проходила и `cargo test`, и `wasm-bindgen-test`; логика без тестов на обоих путях не
считается готовой. Это гейт проекта, а не выбор режима TDD.

**Organization**: задачи сгруппированы по user story, чтобы каждую можно было реализовать и
проверить независимо.

> **Ревизия после `/speckit-analyze` (2026-09-06).** Файл перенумерован целиком: на момент
> правки ни одна задача не была закрыта, поэтому история отметок не пострадала. Закрыты
> находки C1, C2, I1, G1, U1, U2, U3, U4 — см. раздел «Что изменил анализ» в конце файла.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: можно выполнять параллельно (разные файлы, нет незакрытых зависимостей)
- **[Story]**: к какой user story относится задача (US1, US2, US3, US4)
- В описании — точный путь к файлу

## Path Conventions

Структура из `plan.md` → Project Structure: ядро в `packages/core/src/preview/`, общий UI в
`ui/src/preview/`, web-клиент в `apps/web/`, тесты ядра в `packages/core/tests/`, бенчмарки в
`packages/core/benches/`.

---

## Phase 1: Setup (Shared Infrastructure)

- [X] T001 Добавить в `packages/core/Cargo.toml` зависимости `typst = "0.15.1"`,
      `typst-layout = "0.15.1"`, `typst-svg = "0.15.1"` и `typst-assets = "0.15.1"`
      **без** feature `fonts` (research.md §1, §6). Версии проверены в индексе crates.io;
      `0.16.0-rc.1` намеренно не берётся
      - **Отклонение от формулировки задачи.** `typst-assets` объявлять явно не стал: он
        приходит транзитивно из `typst-library` и уже без feature `fonts`, то есть цель
        задачи достигнута, а явная строка была бы мёртвой зависимостью. Решение и его причина
        записаны комментарием в `packages/core/Cargo.toml`, чтобы никто не включил feature
        позже по невнимательности.
- [X] T002 **Гейт риска, выполняется до любой логики фичи.** Собрать ядро с typst под оба
      таргета и убедиться, что модуль реально грузится в браузере: `cargo build -p
      typsidian-core`, затем `pnpm build:wasm`, затем открыть `apps/web` и подтвердить, что
      wasm-модуль инстанцируется. Зонд подтвердил только `cargo build --target
      wasm32-unknown-unknown`; связка `wasm-pack` + `wasm-bindgen` + модуль на ~25 МБ в
      браузере — отдельный риск. Пока модуль не инстанцировался, остальные задачи фичи
      выполнять нельзя (research.md §1, §11)
      - **Пройден 2026-09-06.** `cargo build -p typsidian-core` — 9 с; под
        `wasm32-unknown-unknown` — 2 мин 40 с; `pnpm build:wasm` — 6 мин 00 с, все успешно.
        Решающая проверка: `wasm-pack test --headless --chrome packages/core --test
        preview_wasm` — модуль с компилятором Typst **загрузился и отработал в настоящем
        Chromium**, 6/6 тестов зелёные. Это сильнее, чем открыть страницу глазами: тот же
        путь инстанцирования, но с проверяемым результатом.
      - **Вес доставки измерен после T025.** Пока экспорт не достигал `preview`, модуль весил
        0,6 МБ — линковщик выбрасывал typst целиком. С появлением `PreviewEngine`
        `ui/src/wasm-core/typsidian_core_bg.wasm` — **34,3 МБ сырых, 10,5 МБ brotli**, что
        сходится с оценкой зонда (research.md §6). Из них ≈4 МБ — временно вшитые шрифты,
        которые снимает T029. Замер на реальном канале остаётся за T115.
- [X] T003 [P] Создать скелет модуля `packages/core/src/preview/mod.rs` и подключить его
      в `packages/core/src/lib.rs` через `pub mod preview;`
- [X] T004 [P] Создать наборы тестов `packages/core/tests/preview_native.rs` (`cargo test`) и
      `packages/core/tests/preview_wasm.rs` (`wasm-bindgen-test`) с cfg-разметкой по образцу
      `boards_native.rs`/`boards_wasm.rs`
- [X] T005 [P] Создать скелет бенчмарка `packages/core/benches/preview_scale.rs` с
      `harness = false` и объявить его в `packages/core/Cargo.toml` по образцу
      `board_digest.rs`
- [X] T006 Замерить и зафиксировать в `research.md` §11 фактическое время полной сборки ядра
      с typst под каждый таргет — оно попадёт в бюджет CI

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: то, без чего не начинается ни одна user story. `World` над снимком, две фазы
сборки, исполнитель вне UI-потока и встроенные шрифты нужны уже для того, чтобы показать
первую страницу.

### Снимок зависимостей и `World`

- [X] T007 [P] Реализовать `DependencySnapshot` в
      `packages/core/src/preview/snapshot.rs`: `main`, `files`, `missing`, `iterations` —
      data-model.md «DependencySnapshot»
- [X] T008 [P] Тест: `World` над снимком читает только из `files` и **ни при каких условиях**
      не обращается к файловой системе, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs`
- [X] T009 Реализовать `World` над снимком в `packages/core/src/preview/world.rs`:
      `library`, `book`, `main`, `source`, `file`, `font`, `today`. Корень —
      `VirtualRoot::Project` = папка vault (research.md §8). Основа взята из зонда фазы
      исследования
- [X] T010 Реализовать запись недостающих `FileId` в `missing` при промахе `source`/`file` в
      `packages/core/src/preview/world.rs` — это единственный сигнал фазе резолва
      (research.md §2)
- [X] T011 [P] Тест: `World` остаётся `Send + Sync` на обоих таргетах (компиляционный тест,
      страхует от попадания браузерных хендлов в снимок) в
      `packages/core/tests/preview_wasm.rs` — research.md §11

### Две фазы сборки

- [ ] T012 [P] Тест: замыкание зависимостей сходится за несколько итераций резолв↔сборка;
      документ с двухуровневым импортом собирается, в `packages/core/tests/preview_native.rs`
      и `packages/core/tests/preview_wasm.rs`
- [ ] T013 [P] Тест: превышение предела итераций даёт `DependencyBudgetExhausted`, а не
      бесконечный цикл, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs`
- [ ] T014 Реализовать трейт `DependencyResolver` в
      `packages/core/src/preview/resolve.rs` поверх существующего `VaultBackend` фичи 001 —
      своего доступа к файлам не заводить (принцип I, contracts/compile-engine.md)
- [ ] T015 Реализовать итеративное чередование резолва и сборки в
      `packages/core/src/preview/compile.rs` с пределом 8 итераций (research.md §2)
- [ ] T016 Реализовать `ResolveError` (`NotFound`, `Unreadable`, `AccessDenied`,
      `OutsideVault`, `TooLarge`) в `packages/core/src/preview/resolve.rs` —
      contracts/compile-engine.md

### Исполнитель вне UI-потока

- [ ] T017 [P] Тест: `spawn` не блокирует вызывающий поток; во время сборки вызывающая
      сторона продолжает выполняться, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-007)
- [ ] T018 [P] Тест: после `cancel` результат этой сборки не приходит ни в одном событии, в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
- [ ] T019 [P] Тест: `cancel` и превышение предела времени не изменяют ни одного файла vault
      (сверка отпечатков до и после), в `packages/core/tests/preview_native.rs`
- [ ] T020 Определить трейт `CompileExecutor` и `CompileHandle` в
      `packages/core/src/preview/mod.rs` — единственный платформенный шов фичи
      (contracts/compile-engine.md)
- [ ] T021 Реализовать исполнителя на потоке в `packages/core/src/preview/native.rs`
      (`spawn_blocking`, результат каналом). Отмена — отбрасывание результата; поток
      дорабатывает (research.md §3, plan.md → Complexity Tracking)
- [ ] T022 Реализовать исполнителя на Web Worker в `packages/core/src/preview/web.rs`:
      отдельный экземпляр ядра, снимок передаётся сообщением, `terminate()` даёт настоящую
      отмену (research.md §3)
- [ ] T023 Реализовать предел времени сборки 10 секунд в
      `packages/core/src/preview/compile.rs` с исходом `TimedOut` (FR-026)

### Результат сборки

- [X] T024 [P] Тест: `outcome == Succeeded` **никогда** не сопровождается диагностикой
      серьёзности `Error` (запрещённое состояние из data-model.md), в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
- [X] T025 Реализовать `CompiledDocument`, `CompileOutcome`, `RenderedPage` в
      `packages/core/src/preview/mod.rs` с выводом типов в TS через существующий
      `tsify`-путь (принцип IV) — data-model.md
- [X] T026 Реализовать постраничный экспорт `typst_svg::svg(page, &opts)` в
      `packages/core/src/preview/pages.rs` — постранично, **не** `svg_merged`
      (research.md §4)
- [X] T027 [P] Тест: страница с неизменившимся содержимым даёт тот же `digest`, а
      изменившаяся — другой, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs`
- [X] T028 Реализовать хэш страницы из `Page: Hash` в
      `packages/core/src/preview/pages.rs` — своего хэша содержимого не изобретать
      (research.md §5)

### Встроенные шрифты

- [ ] T029 Реализовать загрузку встроенного набора шрифтов в
      `packages/core/src/preview/fonts.rs`: файлы поставляются рядом с wasm-модулем, **не**
      внутри него (research.md §6, FR-027)
- [ ] T030 Реализовать ленивую загрузку модуля компилятора в `ui/src/preview/`: модуль не
      скачивается, пока пользователь не открыл текстовую заметку. 6,8 МБ brotli без ленивой
      загрузки проваливают SC-001 на первом открытии (research.md §6, §11)

**Checkpoint**: ядро собирает документ из текста и отдаёт постраничный SVG. Ни одна user story
до этого места не завершается.

---

## Phase 3: User Story 1 — Свёрстанный вид открытой заметки (Priority: P1) 🎯 MVP

**Goal**: пользователь открывает заметку и видит рядом с текстом свёрстанный документ —
многостраничный, листаемый, с выделяемым текстом.

**Independent Test**: открыть заметку из нескольких страниц с заголовками и формулой;
документ показан целиком, листается, совпадает со сборкой сторонним `typst compile`, текст из
него копируется (quickstart.md, Сценарии 1 и 8).

### Текстовый слой — тесты

- [ ] T031 [P] [US1] Тест: текстовый прогон извлекается с ожидаемым текстом, позицией,
      кеглем и языком, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-031, FR-032)
- [ ] T032 [P] [US1] Тест: прогон внутри `FrameItem::Group` с трансформацией приводится к
      системе координат страницы (самый вероятный дефект слоя), в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-031, contracts/text-layer.md обязательство 2)
- [ ] T033 [P] [US1] Тест: порядок прогонов совпадает с порядком чтения на многоколоночной и
      многостраничной вёрстке, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-031)
- [ ] T034 [P] [US1] Тест: сгенерированное содержимое (номер страницы в колонтитуле) попадает
      в слой, хотя в исходнике заметки такого текста нет, в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-031, SC-011)

### Текстовый слой — реализация

- [ ] T035 [US1] Реализовать `TextRun` в `packages/core/src/preview/textlayer.rs` —
      data-model.md «TextRun», contracts/text-layer.md
- [ ] T036 [US1] Реализовать обход `Frame` с накоплением трансформаций групп и сбором
      `FrameItem::Text(TextItem)` в `packages/core/src/preview/textlayer.rs` (FR-031)
- [ ] T037 [US1] Реализовать вычисление ширины прогона из `glyphs[].x_advance` в
      `packages/core/src/preview/textlayer.rs` (FR-031)
- [ ] T038 [US1] Реализовать перенос `TextItem::lang` в `TextRun::lang` в
      `packages/core/src/preview/textlayer.rs` (FR-032)

### Поверхность ядра для UI

- [ ] T039 [P] [US1] Реализовать события `BuildStarted` и `BuildFinished` в
      `packages/core/src/preview/events.rs` — contracts/frontend-preview.md
- [ ] T040 [US1] Расширить `ui/src/vault-client.ts` интерфейсом `PreviewClient`
      (`requestBuild`, `cancelBuild`, `subscribePreview`) — contracts/frontend-preview.md
- [ ] T041 [US1] Реализовать web-часть `PreviewClient` в `ui/src/vault-client-web.ts`,
      проведя **все** вызовы экземпляра ядра через `ui/src/wasm-call-queue.ts`: без очереди
      сборка наложится на цикл опроса vault и даст `recursive use of an object detected`
- [ ] T042 [US1] Реализовать native-часть `PreviewClient` в
      `apps/desktop/src/tauriVaultClient.ts` командами Tauri

### Панель предпросмотра

- [ ] T043 [P] [US1] Реализовать `ui/src/preview/PreviewPage.tsx`: одна страница — SVG плюс
      текстовый слой поверх, с общим множителем масштаба
- [ ] T044 [P] [US1] Реализовать `ui/src/preview/TextLayer.tsx`: прозрачный выделяемый текст,
      позиционированный по `TextRun`, с объявленным `lang`. Слой **не** помечается
      декоративным и **не** скрывается от вспомогательных технологий (FR-032,
      contracts/text-layer.md обязательство 4)
- [X] T045 [US1] Реализовать `ui/src/preview/PreviewPane.tsx`: колонка предпросмотра, список
      страниц, различимые границы между ними (FR-001, FR-003)
- [ ] T046 [US1] Реализовать `ui/src/preview/useCompiledDocument.ts`: подписка на
      `PreviewEvent`, хранение `PreviewState` — data-model.md «PreviewState»
- [X] T047 [US1] Заменить заглушку предпросмотра в `ui/src/workspace/Workspace.tsx`
      (строки 317-326) на `PreviewPane` и убрать устаревший комментарий на строке 67 о том,
      что компилятора в ядре нет
- [X] T048 [US1] Убедиться, что открытие доски по-прежнему занимает рабочую область без
      колонки предпросмотра, в `ui/src/workspace/Workspace.tsx` (FR-005)
- [X] T049 [US1] Реализовать показ признака сборки, **не убирающий** уже показанный вид, в
      `ui/src/preview/PreviewPane.tsx` (FR-010)

### Проверка истории

- [ ] T050 [US1] Прогнать `quickstart.md` Сценарий 1 на web- и native-сборке; подтвердить
      паритет (принцип II)
- [ ] T051 [US1] Прогнать `quickstart.md` Сценарий 8 целиком: выделение, копирование через
      несколько абзацев, сгенерированный текст, поиск браузера, программа чтения с экрана,
      попадание слоя по буквам (FR-031, FR-032, SC-011)
- [ ] T052 [US1] Сверить вид со сборкой тем же файлом через сторонний `typst compile`;
      расхождения зафиксировать в `specs/005-live-preview/quickstart.md` → Сценарий 1
      (SC-008)
- [ ] T053 [US1] Замерить время появления вида после открытия заметки на документе в 50
      страниц и зафиксировать в `packages/core/benches/preview_scale.rs` (SC-001)

**Checkpoint**: US1 самодостаточна — предпросмотр работает, хотя ещё не обновляется по мере
набора. Это MVP.

---

## Phase 4: User Story 2 — Обновление по мере набора (Priority: P2)

**Goal**: вид догоняет правки сам, ввод при этом не подтормаживает, а место в документе не
уезжает из-под пользователя.

**Independent Test**: набрать несколько абзацев не сохраняя; вид догнал последнее состояние
текста, ввод ни разу не подтормозил, файл на диске не изменился (quickstart.md, Сценарий 2).

- [ ] T054 [P] [US2] Тест: результат сборки с `revision` меньше уже доставленного
      отбрасывается и наверх не попадает, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-009)
- [ ] T055 [P] [US2] Тест: при потоке правок собирается **последнее** состояние текста;
      промежуточные могут быть пропущены, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-008)
- [ ] T056 [P] [US2] Тест: сборка идёт из переданного текста, а файл на диске не читается и
      не изменяется (сверка отпечатка), в `packages/core/tests/preview_native.rs` (FR-002)
- [ ] T057 [P] [US2] Тест: закрытие заметки прекращает её сборку — результат не приходит, в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-011)
- [ ] T058 [US2] Реализовать версионирование результата по `revision` и отбрасывание
      устаревших в `packages/core/src/preview/compile.rs` (research.md §9)
- [ ] T059 [US2] Реализовать отмену сборки при закрытии заметки и при переходе к другой в
      `packages/core/src/preview/mod.rs` (FR-011)
- [ ] T060 [US2] Реализовать сохранение кэша `comemo` между пересборками и вызов
      `comemo::evict(max_age)` после каждой сборки в
      `packages/core/src/preview/compile.rs` (research.md §10)
- [ ] T061 [US2] Подобрать `max_age` замером в `packages/core/benches/preview_scale.rs`:
      без кэша SC-002 недостижим, без вытеснения долгая сессия течёт по памяти
- [X] T062 [US2] Реализовать дебаунс запроса сборки в
      `ui/src/preview/useCompiledDocument.ts` под порог SC-002 — дебаунс живёт в UI, потому
      что он про ритм ввода, а не про правила сборки (FR-006,
      contracts/frontend-preview.md)
- [X] T063 [US2] Реализовать передачу текста из буфера редактора в сборку в
      `ui/src/editor/NoteEditor.tsx`, не добавляя второго источника истины для содержимого
      (FR-006)
- [ ] T064 [US2] Пересмотреть вызов `client.beginEdit` на каждое нажатие в
      `ui/src/editor/NoteEditor.tsx` (строка 216): через очередь сериализации он ставит по
      операции на символ. Свести к одному вызову по дебаунсу вместе с запросом сборки

### Удержание места при пересборке

- [ ] T065 [P] [US2] Тест: якорь по тексту прогона находит то же место после переверстки,
      сместившей его на другую страницу (`exact: true`), в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-004, SC-005)
- [ ] T066 [P] [US2] Тест: якорь, чей текст исчез из документа, разрешается в ближайшее
      сохранившееся место с `exact: false`, а **не** в начало документа, в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-034, SC-005)
- [ ] T067 [US2] Реализовать `ScrollAnchor`, `ResolvedAnchor` и правило разрешения якоря в
      `packages/core/src/preview/anchor.rs` — правило над содержимым документа живёт в ядре
      (принцип I), contracts/frontend-preview.md
- [ ] T068 [US2] Реализовать `ui/src/preview/useScrollAnchor.ts`: снять якорь с видимого на
      экране, передать его ядру, прокрутить к тому, что вернуло ядро. Собственного поиска
      совпадений в UI быть не должно — это была бы вторая реализация правила (принцип I)
- [ ] T069 [US2] Реализовать перерисовку только изменившихся страниц по `digest` в
      `ui/src/preview/PreviewPane.tsx`: неизменившиеся узлы DOM не трогаются
      (research.md §5)
- [ ] T070 [US2] Прогнать `quickstart.md` Сценарий 2 целиком на web- и native-сборке,
      включая шаги 6 и 7 (удержание места и исчезнувший фрагмент)
- [ ] T071 [US2] Замерить задержку отклика на ввод во время сборки и время обновления после
      паузы; зафиксировать в `packages/core/benches/preview_scale.rs` (FR-007, SC-002,
      SC-003)
- [X] T072 [US2] Убедиться, что действия «обновить предпросмотр» в интерфейсе нет вовсе, в
      `ui/src/preview/PreviewPane.tsx` (FR-006, SC-010)

**Checkpoint**: US1 и US2 работают вместе — живой предпросмотр состоялся.

---

## Phase 5: User Story 3 — Ошибка названа и показана на месте (Priority: P3)

**Goal**: ошибка не гасит предпросмотр; названы причина и место, переход к нему — одно
действие, в том числе в чужом файле.

**Independent Test**: внести ошибку в собравшуюся заметку; названы причина и место, переход
работает, прежний вид остался с пометкой, исправление возвращает согласованность
(quickstart.md, Сценарий 3).

- [ ] T073 [P] [US3] Тест: диагностика несёт причину словами и место — файл, строку, колонку
      и байтовый диапазон, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-012, FR-013)
- [ ] T074 [P] [US3] Тест: несколько диагностик отсортированы по `(path, byte_range.start)`,
      в `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-016)
- [ ] T075 [P] [US3] Тест: предупреждения приходят с `severity: Warning` и **не** переводят
      сборку в `Failed`, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-018)
- [ ] T076 [P] [US3] Тест: диагностика в связанном файле несёт `is_open_note: false` и путь
      того файла, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-015)
- [X] T077 [P] [US3] Тест: диагностика по файлу, не открываемому в редакторе (шрифт,
      изображение), несёт `openable: false`, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-033)
- [X] T078 [US3] Реализовать `Diagnostic` и `DiagnosticLocation` в
      `packages/core/src/preview/diagnostics.rs` — data-model.md
- [X] T079 [US3] Реализовать перевод `SourceDiagnostic` в `Diagnostic` через
      `WorldExt::range(span)` в `packages/core/src/preview/diagnostics.rs`
      (research.md §7)
- [ ] T080 [US3] Реализовать перевод байтового диапазона в строку и колонку (1-based) в
      `packages/core/src/preview/diagnostics.rs`
- [X] T081 [US3] Реализовать вычисление `openable` по виду файла в
      `packages/core/src/preview/diagnostics.rs`, переиспользуя `FileKind` фичи 002 —
      второй классификации не заводить (принцип I)
- [ ] T082 [US3] Реализовать разделение ошибок и предупреждений из формы `Warned` в
      `packages/core/src/preview/compile.rs` — не разбором текста сообщения
- [ ] T083 [P] [US3] Реализовать `ui/src/preview/DiagnosticsList.tsx`: список диагностик,
      ошибки отличимы от предупреждений
- [ ] T084 [US3] Реализовать пометку устаревания показанного вида в
      `ui/src/preview/PreviewPane.tsx` при неуспешной сборке (FR-014)
- [ ] T085 [US3] Реализовать состояние «показывать нечего» с обязательным называнием причины
      в `ui/src/preview/PreviewPane.tsx`: пустая панель без объяснения запрещена
      (FR-014, SC-004)
- [ ] T086 [US3] Реализовать переход к месту ошибки в открытой заметке в
      `ui/src/preview/DiagnosticsList.tsx` (FR-015, SC-007)
- [ ] T087 [US3] Реализовать переход к месту ошибки в связанном файле в
      `ui/src/preview/DiagnosticsList.tsx` и `ui/src/workspace/Workspace.tsx`: файл
      открывается в редакторе, курсор встаёт на место; одновременный показ двух файлов не
      вводится (FR-015)
- [ ] T088 [US3] Реализовать отсутствие действия перехода при `openable: false` в
      `ui/src/preview/DiagnosticsList.tsx`: недействующую кнопку показывать запрещено
      (FR-033)
- [ ] T089 [US3] Реализовать снятие пометки и очистку диагностик при исправлении ошибки в
      `ui/src/preview/useCompiledDocument.ts` (FR-017)
- [ ] T090 [US3] Прогнать `quickstart.md` Сценарии 3 и 4 на web- и native-сборке, включая
      шаг 7 (заметка, не собравшаяся ни разу)

**Checkpoint**: US1-US3 работают вместе — живой предпросмотр пригоден для реальной работы.

---

## Phase 6: User Story 4 — Связанные файлы (Priority: P4)

**Goal**: заметка собирается вместе с подключёнными файлами, изображениями, снимками досок и
шрифтами; правка любого из них обновляет предпросмотр.

**Independent Test**: собрать заметку с подключённым файлом и снимком доски, изменить
подключённый файл сторонним редактором, убедиться в обновлении без переоткрытия
(quickstart.md, Сценарий 5).

- [ ] T091 [P] [US4] Тест: заметка с двухуровневым импортом и изображением собирается вместе
      с ними, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-019)
- [ ] T092 [P] [US4] Тест: отсутствующий связанный файл называется поимённо с причиной, а не
      общей ошибкой сборки, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-021)
- [ ] T093 [P] [US4] **Тест циклической связи** — поведение `typst` здесь зондом не
      проверялось (research.md §11): два файла, подключающие друг друга, дают диагностику, а
      не зависание, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-022). Если `typst` зависает — нужен
      собственный предохранитель до сборки
- [ ] T094 [P] [US4] Тест: ссылка за пределы папки vault отклоняется с `OutsideVault` **до**
      чтения, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-023)
- [ ] T095 [US4] Реализовать учёт связанных файлов сборки в
      `packages/core/src/preview/snapshot.rs`: какие файлы vault прочитаны для этой заметки
- [ ] T096 [US4] Реализовать событие `DependencyChanged` в
      `packages/core/src/preview/events.rs` — contracts/frontend-preview.md
- [ ] T097 [US4] Подписать пересборку на изменения связанных файлов через существующий поток
      `VaultEvent` фичи 001 в `packages/core/src/preview/mod.rs`: своего механизма слежения
      не вводить (spec.md → Assumptions, FR-020)
- [ ] T098 [US4] Реализовать отказ `OutsideVault` до чтения в
      `packages/core/src/preview/resolve.rs` (FR-023)
- [ ] T099 [US4] Реализовать обработку циклической связи в
      `packages/core/src/preview/compile.rs` по результату T093

### Шрифты

- [ ] T100 [P] [US4] Тест: шрифт из папки vault попадает в набор и применяется, в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-027)
- [ ] T101 [P] [US4] Тест: отсутствующий шрифт называется поимённо, а документ собирается с
      подстановкой и остаётся `Succeeded` — **не** `Failed`, в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-028)
- [ ] T102 [P] [US4] Тест: появление и удаление шрифта в vault обновляет набор по правилам
      связанного файла, в `packages/core/tests/preview_native.rs` (FR-029)
- [ ] T103 [P] [US4] Тест: шрифт, установленный в системе, но отсутствующий во встроенном
      наборе и в vault, **не** применяется, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-027)
- [ ] T104 [US4] Реализовать сбор шрифтов из папки vault в
      `packages/core/src/preview/fonts.rs` (FR-027)
- [ ] T105 [US4] Реализовать `FontSet::missing()` и предупреждение о недостающих шрифтах в
      `packages/core/src/preview/fonts.rs` (FR-028)
- [ ] T106 [US4] Реализовать реакцию на появление и удаление шрифта в vault в
      `packages/core/src/preview/fonts.rs` (FR-029)
- [ ] T107 [US4] Проверить, что шрифты vault не показываются в дереве заметок — правило
      фичи 001 и 002 действует без изменений (spec.md → Assumptions)
- [ ] T108 [US4] Замерить время от изменения связанного файла до обновления предпросмотра и
      зафиксировать порог в `packages/core/benches/preview_scale.rs` (SC-006) — рядом с
      замерами SC-001 и SC-002/SC-003
- [ ] T109 [US4] Прогнать `quickstart.md` Сценарии 5, 6 и 7 на web- и native-сборке

**Checkpoint**: все четыре user story реализованы.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T110 [P] Реализовать исход `VaultUnavailable` и сообщение пользователю в
      `packages/core/src/preview/compile.rs` и `ui/src/preview/PreviewPane.tsx`; текст в
      редакторе при этом сохраняется по правилам фичи 001 (FR-025)
- [ ] T111 [P] Реализовать наследование предела размера заметки фичи 001 в
      `packages/core/src/preview/resolve.rs`: заметку, которую нельзя открыть, не собирать
      (FR-030)
- [ ] T112 [P] Тест: все операции фичи выполняются при полностью отключённой сети без
      отличий в поведении, в `packages/core/tests/preview_native.rs` (FR-024, SC-009)
- [ ] T113 Убедиться, что пакеты Typst из сети не поддерживаются и попытка их использовать
      даёт понятный отказ, в `packages/core/src/preview/world.rs` (research.md §8,
      принцип IX)
- [ ] T114 [P] Проверить, что панель предпросмотра остаётся темочувствительной и использует
      токены из `ui/src/styles/tokens/`, а не собственные цвета
- [ ] T115 Прогнать `quickstart.md` Сценарии 9 и 10 (офлайн, границы, потолок гарантий) на
      web- и native-сборке
- [ ] T116 Прогнать **весь** `quickstart.md` (Сценарии 1-10) подряд на native- и
      wasm32-сборках; зафиксировать любое расхождение паритета (принцип II)
- [ ] T117 Зафиксировать фактический вес доставки wasm-модуля после `wasm-opt` и сжатия и
      время первого открытия на небыстром канале; сверить с бюджетом SC-001
      (research.md §11)
- [ ] T118 Прогнать `pnpm typecheck` по всем пакетам (`ui/`, `apps/web/`, `apps/desktop/`):
      `strict` без `any` (принцип IV)
- [ ] T119 Прогнать `cargo test -p typsidian-core` и
      `wasm-pack test --headless --chrome packages/core` — оба обязательны (принцип II)
- [ ] T120 Обновить раздел «Что готово» в `README.md`: фича 005 переходит из «не начато» в
      реализованное, а из списка «чего нет» уходит компиляция Typst и preview

---

## Dependencies

```text
Phase 1 (Setup)
    └─> Phase 2 (Foundational) ─── блокирует всё
            ├─> Phase 3 (US1, P1) ── MVP
            │       └─> Phase 4 (US2, P2) ── нужен показанный вид, чтобы его обновлять
            │               └─> Phase 5 (US3, P3) ── нужно то, что ломается
            │                       └─> Phase 6 (US4, P4)
            └─> Phase 7 (Polish) ── после всех историй
```

**Внутри Phase 2**: T007…T011 (снимок и `World`) → T012…T016 (две фазы) → T017…T023
(исполнитель) → T024…T028 (результат) → T029, T030 (шрифты и ленивая загрузка).
T002 блокирует вообще всё: пока модуль не инстанцировался в браузере, остальное бессмысленно.

**Между историями**: US2 опирается на показ из US1; US3 — на наличие сборки, которую можно
сломать; US4 — на резолв из Phase 2 и диагностики из US3 (FR-021 показывается механизмом
US3). Порядок соответствует приоритетам спеки.

## Parallel Execution Examples

**Phase 2**: T007, T008, T011 — разные файлы, нет общих зависимостей. Тесты T012, T013 и
T017, T018, T019 пишутся параллельно реализациям, к которым относятся.

**Phase 3 (US1)**: T031-T034 (тесты текстового слоя) — все параллельны; T043 и T044 —
разные компоненты UI; T039 независим от UI-части.

**Phase 4 (US2)**: T054-T057 параллельны; T065, T066 параллельны друг другу.

**Phase 5 (US3)**: T073-T077 — пять независимых тестов; T083 независим от логики ядра.

**Phase 6 (US4)**: T091-T094 параллельны; T100-T103 параллельны.

**Phase 7**: T110, T111, T112, T114 — разные файлы.

## Implementation Strategy

**MVP — Phase 1 + Phase 2 + Phase 3 (US1)**: T001-T053. По завершении пользователь открывает
заметку и видит настоящий свёрстанный документ с выделяемым текстом. Живого обновления ещё
нет, но главная дыра продукта — «пишешь вслепую» — закрыта.

**Инкремент 2 — Phase 4 (US2)**: живое обновление. Это то, ради чего фича заказана.

**Инкремент 3 — Phase 5 (US3)**: без него живое обновление раздражает — предпросмотр гаснет на
каждой недописанной строке.

**Инкремент 4 — Phase 6 (US4)**: связанные файлы и шрифты пользователя.

**Порядок внутри истории**: тесты пишутся до реализации той же единицы логики — принцип I
требует покрытия на обоих таргетах, и писать тесты после проще всего забыть.

**Два места, где стоит остановиться и пересмотреть план**:

1. **T002** — если модуль на ~25 МБ не инстанцируется в браузере или делает это неприемлемо
   долго, вся конструкция требует пересмотра до того, как в неё вложены сотни часов.
2. **T093** — если `typst` зависает на циклическом импорте вместо диагностики, нужен
   собственный предохранитель, и он должен появиться раньше, чем пользователь встретит
   зависание.

---

## Что изменил анализ

Ревизия по итогам `/speckit-analyze`. Задач стало 120 вместо 118, файл перенумерован сквозняком.

**C1 (CRITICAL, принцип I) — разрешение якоря прокрутки перенесено в ядро.** Было: логика в
`ui/src/preview/useScrollAnchor.ts`, а тесты на неё — в `packages/core/tests/`, что несовместимо
файлово. По существу правило «какое место документа считать тем же после переверстки» — логика
над содержимым, а SC-005 делает его проверяемой гарантией. Теперь T067 реализует его в
`packages/core/src/preview/anchor.rs`, а новая T068 оставляет UI только снятие якоря и
прокрутку. Правки разошлись в `plan.md` → Project Structure и
`contracts/frontend-preview.md` (добавлен `ResolvedAnchor` с признаком `exact`).

**C2 (CRITICAL, принцип I) — пять тестов ядра получили wasm-путь.** T013, T033, T073, T076 и
тест о неприменении системного шрифта проверяют чистую логику без файловой системы, поэтому
принцип I требует обоих таргетов. Четыре теста остались native обоснованно: T019 и T056 сверяют
отпечатки файлов, T102 зависит от появления файла в папке vault, T112 проверяет поведение без
сети.

**I1 (HIGH) — снято противоречие о составе `pages`.** `data-model.md` описывал полный список
страниц, а `contracts/compile-engine.md` — только изменившиеся; при этом T069 (бывш. T068)
делает пропуск на стороне UI, то есть предполагает полный список. Выбран полный: ядро отдаёт
все страницы с `digest`, UI не перерисовывает совпавшие. Так первая сборка и переоткрытие
заметки не требуют отдельного пути «отдай всё». Поправлены `research.md` §5, `data-model.md`
(два места) и `contracts/compile-engine.md`.

**G1 (MEDIUM) — добавлен замер SC-006.** Новая T108 фиксирует в
`packages/core/benches/preview_scale.rs` время от изменения связанного файла до обновления
предпросмотра. Раньше этот критерий проверялся только ручным прогоном без порога.

**U1, U2, U3 (MEDIUM/LOW) — проставлены ссылки на требования.** FR-006, FR-007, SC-005, SC-007
не цитировались ни одной задачей, хотя работа существовала; FR-031 цитировал только ручной
прогон, но не реализация. Покрытие по ссылкам: было 40/45, стало **45/45**.

**U4 (LOW) — убрано поле `Diagnostic.hints`** из `data-model.md`: оно не следовало ни из одного
требования спеки и не было покрыто ни одной задачей.

---

## Вертикальный срез US1 (2026-09-06)

Предпросмотр доведён до работающего состояния раньше, чем закрыта Phase 2 целиком: заметка
открывается и показывается свёрстанной. Сделано это осознанно — «фундамент без экрана» нечем
проверить, — но означает, что часть задач Phase 2 обойдена, а не выполнена. Что именно:

**Работает**: сборка заметки из буфера редактора, постраничный SVG с различимыми границами,
обновление по мере набора с дебаунсом 250 мс, отбрасывание устаревших результатов по
`revision`, ошибки с причиной и местом (файл:строка:колонка), сохранение последнего удачного
вида с пометкой «не соответствует тексту», отделение предупреждений от ошибок.

**Сознательно обойдено — и это открытые задачи, а не недоделки без хозяина**:

- **T029, шрифты.** Набор временно вшит в модуль через feature `typst-assets/fonts`: без
  единого шрифта Typst не верстает текст вовсе. Цена — 10,5 МБ brotli вместо 6,8 МБ. Комментарий
  с причиной стоит в `packages/core/Cargo.toml`, снимает это T029.
- **T020…T022, исполнитель.** Сборка идёт в потоке UI. Для короткого документа незаметно, но
  FR-007 в полном объёме закрывает только Web Worker.
- **T023, предел 10 секунд.** Не реализован: без воркера прервать синхронный `typst::compile`
  нечем.
- **T014…T016, резолв зависимостей.** Собирается только односоставная заметка. Импорты,
  изображения и снимки досок (US4, FR-019…FR-023) пока не подтягиваются.
- **T035…T038, T044, текстовый слой.** Не сделан: текст в предпросмотре **не выделяется** и
  недоступен программе чтения с экрана. FR-031, FR-032 и SC-011 не выполнены.
- **T067, T068, якорь прокрутки.** Не сделан: при пересборке вид возвращается к началу
  документа. FR-004, FR-034 и SC-005 не выполнены.
- **T042, desktop.** `PreviewClient` реализован только для web; на рабочем столе колонка
  предпросмотра сообщает, что сборка недоступна.

**Отступления от буквы задач**, чтобы не искать их потом по диффу: T043 (`PreviewPage.tsx`) и
T046 (`useCompiledDocument.ts`) отдельными файлами не заведены — их содержимое пока живёт
внутри `ui/src/preview/PreviewPane.tsx`; T062 (дебаунс) реализован там же, а не в
`useCompiledDocument.ts`. При выполнении T046 это надо разнести по файлам, как предписывает
plan.md → Project Structure.
