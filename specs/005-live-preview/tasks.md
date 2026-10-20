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

- [X] T012 [P] Тест: замыкание зависимостей сходится за несколько итераций резолв↔сборка;
      документ с двухуровневым импортом собирается, в `packages/core/tests/preview_native.rs`
      и `packages/core/tests/preview_wasm.rs`
- [X] T013 [P] Тест: превышение предела итераций даёт `DependencyBudgetExhausted`, а не
      бесконечный цикл, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs`
- [X] T014 Реализовать трейт `DependencyResolver` в
      `packages/core/src/preview/resolve.rs` поверх существующего `VaultBackend` фичи 001 —
      своего доступа к файлам не заводить (принцип I, contracts/compile-engine.md)
- [X] T015 Реализовать итеративное чередование резолва и сборки в
      `packages/core/src/preview/compile.rs` с пределом 8 итераций (research.md §2)
- [X] T016 Реализовать `ResolveError` (`NotFound`, `Unreadable`, `AccessDenied`,
      `OutsideVault`, `TooLarge`) в `packages/core/src/preview/resolve.rs` —
      contracts/compile-engine.md

### Исполнитель вне UI-потока

- [X] T017 [P] Тест: `spawn` не блокирует вызывающий поток; во время сборки вызывающая
      сторона продолжает выполняться, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-007)
- [X] T018 [P] Тест: после `cancel` результат этой сборки не приходит ни в одном событии, в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
- [X] T019 [P] Тест: `cancel` и превышение предела времени не изменяют ни одного файла vault
      (сверка отпечатков до и после), в `packages/core/tests/preview_native.rs`
- [X] T020 Определить трейт `CompileExecutor` и `CompileHandle` в
      `packages/core/src/preview/mod.rs` — единственный платформенный шов фичи
      (contracts/compile-engine.md)
- [X] T021 Реализовать исполнителя на потоке в `packages/core/src/preview/native.rs`
      (`spawn_blocking`, результат каналом). Отмена — отбрасывание результата; поток
      дорабатывает (research.md §3, plan.md → Complexity Tracking)
- [X] T022 Реализовать исполнителя на Web Worker в `packages/core/src/preview/web.rs`:
      отдельный экземпляр ядра, снимок передаётся сообщением, `terminate()` даёт настоящую
      отмену (research.md §3)
- [X] T023 Реализовать предел времени сборки 10 секунд в
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

- [X] T029 Реализовать загрузку встроенного набора шрифтов в
      `packages/core/src/preview/fonts.rs`: файлы поставляются рядом с wasm-модулем, **не**
      внутри него (research.md §6, FR-027)
- [X] T030 Реализовать ленивую загрузку модуля компилятора в `ui/src/preview/`: модуль не
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

- [X] T031 [P] [US1] Тест: текстовый прогон извлекается с ожидаемым текстом, позицией,
      кеглем и языком, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-031, FR-032)
- [X] T032 [P] [US1] Тест: прогон внутри `FrameItem::Group` с трансформацией приводится к
      системе координат страницы (самый вероятный дефект слоя), в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-031, contracts/text-layer.md обязательство 2)
- [X] T033 [P] [US1] Тест: порядок прогонов совпадает с порядком чтения на многоколоночной и
      многостраничной вёрстке, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-031)
- [X] T034 [P] [US1] Тест: сгенерированное содержимое (номер страницы в колонтитуле) попадает
      в слой, хотя в исходнике заметки такого текста нет, в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-031, SC-011)

### Текстовый слой — реализация

- [X] T035 [US1] Реализовать `TextRun` в `packages/core/src/preview/textlayer.rs` —
      data-model.md «TextRun», contracts/text-layer.md
- [X] T036 [US1] Реализовать обход `Frame` с накоплением трансформаций групп и сбором
      `FrameItem::Text(TextItem)` в `packages/core/src/preview/textlayer.rs` (FR-031)
- [X] T037 [US1] Реализовать вычисление ширины прогона из `glyphs[].x_advance` в
      `packages/core/src/preview/textlayer.rs` (FR-031)
- [X] T038 [US1] Реализовать перенос `TextItem::lang` в `TextRun::lang` в
      `packages/core/src/preview/textlayer.rs` (FR-032)

### Поверхность ядра для UI

- [X] T039 [P] [US1] Реализовать события `BuildStarted` и `BuildFinished` в
      `packages/core/src/preview/events.rs` — contracts/frontend-preview.md
- [X] T040 [US1] Расширить `ui/src/vault-client.ts` интерфейсом `PreviewClient`
      (`requestBuild`, `cancelBuild`, `subscribePreview`) — contracts/frontend-preview.md
- [X] T041 [US1] Реализовать web-часть `PreviewClient` в `ui/src/vault-client-web.ts`,
      проведя **все** вызовы экземпляра ядра через `ui/src/wasm-call-queue.ts`: без очереди
      сборка наложится на цикл опроса vault и даст `recursive use of an object detected`
- [X] T042 [US1] Реализовать native-часть `PreviewClient` в
      `ui/src/vault-client-tauri.ts` командами Tauri, а сами команды — в
      `packages/shell/src/lib.rs`
      - **Закрыт 2026-09-15.** Файл в тот же день переехал: был
        `apps/desktop/src/tauriVaultClient.ts`, стал общим для обеих нативных оболочек,
        поэтому одна реализация закрыла **и desktop, и Android**.
      - Переписывать в ядре не пришлось ничего: `compile_note_resolved`,
        `compile_note_with_templates`, `FontSet::builtin()` и `PackageCache` живут вне
        `cfg` — под `cfg(target_arch = "wasm32")` спрятана только обёртка
        wasm-bindgen `preview::web`. Из ядра понадобилось ровно два штриха:
        `SnapshotError` стал сериализуемым (он теперь пересекает границу как есть, а не
        строкой в `JsError`), и `compile_note_with_templates` добавлен в `pub use`.
      - Движок (`typsidian_shell::Preview`) — состояние окна, как `PreviewEngine` на web
        и по той же причине: предпросмотр не владеет vault, а получает его на вызов.
        Своего `Mutex` у него нет — `PackageCache` синхронизируется изнутри.
      - Очереди вызовов, обязательной на web, здесь нет намеренно:
        «recursive use of an object» — свойство границы wasm-bindgen, а не ядра.
      - Покрытие: `preview_native.rs` → `note_on_native_backend_compiles_into_pages`,
        `dependency_is_read_through_the_native_backend`. До этого предпросмотр
        проверялся только на `MemoryVault`, и пара «`NativeBackend` + сборка» — та
        самая, которой пользуются оболочки, — не была связана ни одним тестом.

### Панель предпросмотра

- [X] T043 [P] [US1] Реализовать `ui/src/preview/PreviewPage.tsx`: одна страница — SVG плюс
      текстовый слой поверх, с общим множителем масштаба
- [X] T044 [P] [US1] Реализовать `ui/src/preview/TextLayer.tsx`: прозрачный выделяемый текст,
      позиционированный по `TextRun`, с объявленным `lang`. Слой **не** помечается
      декоративным и **не** скрывается от вспомогательных технологий (FR-032,
      contracts/text-layer.md обязательство 4)
- [X] T045 [US1] Реализовать `ui/src/preview/PreviewPane.tsx`: колонка предпросмотра, список
      страниц, различимые границы между ними (FR-001, FR-003)
- [X] T046 [US1] Реализовать `ui/src/preview/useCompiledDocument.ts`: подписка на
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
- [X] T053 [US1] Замерить время появления вида после открытия заметки на документе в 50
      страниц и зафиксировать в `packages/core/benches/preview_scale.rs` (SC-001)

**Checkpoint**: US1 самодостаточна — предпросмотр работает, хотя ещё не обновляется по мере
набора. Это MVP.

---

## Phase 4: User Story 2 — Обновление по мере набора (Priority: P2)

**Goal**: вид догоняет правки сам, ввод при этом не подтормаживает, а место в документе не
уезжает из-под пользователя.

**Independent Test**: набрать несколько абзацев не сохраняя; вид догнал последнее состояние
текста, ввод ни разу не подтормозил, файл на диске не изменился (quickstart.md, Сценарий 2).

- [X] T054 [P] [US2] Тест: результат сборки с `revision` меньше уже доставленного
      отбрасывается и наверх не попадает, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-009)
- [X] T055 [P] [US2] Тест: при потоке правок собирается **последнее** состояние текста;
      промежуточные могут быть пропущены, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-008)
- [X] T056 [P] [US2] Тест: сборка идёт из переданного текста, а файл на диске не читается и
      не изменяется (сверка отпечатка), в `packages/core/tests/preview_native.rs` (FR-002)
- [X] T057 [P] [US2] Тест: закрытие заметки прекращает её сборку — результат не приходит, в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-011)
- [X] T058 [US2] Реализовать версионирование результата по `revision` и отбрасывание
      устаревших в `packages/core/src/preview/compile.rs` (research.md §9)
- [X] T059 [US2] Реализовать отмену сборки при закрытии заметки и при переходе к другой в
      `packages/core/src/preview/mod.rs` (FR-011)
- [X] T060 [US2] Реализовать сохранение кэша `comemo` между пересборками и вызов
      `comemo::evict(max_age)` после каждой сборки в
      `packages/core/src/preview/compile.rs` (research.md §10)
- [X] T061 [US2] Подобрать `max_age` замером в `packages/core/benches/preview_scale.rs`:
      без кэша SC-002 недостижим, без вытеснения долгая сессия течёт по памяти
- [X] T062 [US2] Реализовать дебаунс запроса сборки в
      `ui/src/preview/useCompiledDocument.ts` под порог SC-002 — дебаунс живёт в UI, потому
      что он про ритм ввода, а не про правила сборки (FR-006,
      contracts/frontend-preview.md)
- [X] T063 [US2] Реализовать передачу текста из буфера редактора в сборку в
      `ui/src/editor/NoteEditor.tsx`, не добавляя второго источника истины для содержимого
      (FR-006)
- [X] T064 [US2] Пересмотреть вызов `client.beginEdit` на каждое нажатие в
      `ui/src/editor/NoteEditor.tsx` (строка 216): через очередь сериализации он ставит по
      операции на символ. Свести к одному вызову по дебаунсу вместе с запросом сборки

### Удержание места при пересборке

- [X] T065 [P] [US2] Тест: якорь по тексту прогона находит то же место после переверстки,
      сместившей его на другую страницу (`exact: true`), в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-004, SC-005)
- [X] T066 [P] [US2] Тест: якорь, чей текст исчез из документа, разрешается в ближайшее
      сохранившееся место с `exact: false`, а **не** в начало документа, в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-034, SC-005)
- [X] T067 [US2] Реализовать `ScrollAnchor`, `ResolvedAnchor` и правило разрешения якоря в
      `packages/core/src/preview/anchor.rs` — правило над содержимым документа живёт в ядре
      (принцип I), contracts/frontend-preview.md
- [X] T068 [US2] Реализовать `ui/src/preview/useScrollAnchor.ts`: снять якорь с видимого на
      экране, передать его ядру, прокрутить к тому, что вернуло ядро. Собственного поиска
      совпадений в UI быть не должно — это была бы вторая реализация правила (принцип I)
- [X] T069 [US2] Реализовать перерисовку только изменившихся страниц по `digest` в
      `ui/src/preview/PreviewPane.tsx`: неизменившиеся узлы DOM не трогаются
      (research.md §5)
- [ ] T070 [US2] Прогнать `quickstart.md` Сценарий 2 целиком на web- и native-сборке,
      включая шаги 6 и 7 (удержание места и исчезнувший фрагмент)
- [X] T071 [US2] Замерить задержку отклика на ввод во время сборки и время обновления после
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

- [X] T073 [P] [US3] Тест: диагностика несёт причину словами и место — файл, строку, колонку
      и байтовый диапазон, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-012, FR-013)
- [X] T074 [P] [US3] Тест: несколько диагностик отсортированы по `(path, byte_range.start)`,
      в `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-016)
- [X] T075 [P] [US3] Тест: предупреждения приходят с `severity: Warning` и **не** переводят
      сборку в `Failed`, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-018)
- [X] T076 [P] [US3] Тест: диагностика в связанном файле несёт `is_open_note: false` и путь
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
- [X] T080 [US3] Реализовать перевод байтового диапазона в строку и колонку (1-based) в
      `packages/core/src/preview/diagnostics.rs`
- [X] T081 [US3] Реализовать вычисление `openable` по виду файла в
      `packages/core/src/preview/diagnostics.rs`, переиспользуя `FileKind` фичи 002 —
      второй классификации не заводить (принцип I)
- [X] T082 [US3] Реализовать разделение ошибок и предупреждений из формы `Warned` в
      `packages/core/src/preview/compile.rs` — не разбором текста сообщения
- [X] T083 [P] [US3] Реализовать `ui/src/preview/DiagnosticsList.tsx`: список диагностик,
      ошибки отличимы от предупреждений
- [X] T084 [US3] Реализовать пометку устаревания показанного вида в
      `ui/src/preview/PreviewPane.tsx` при неуспешной сборке (FR-014)
- [X] T085 [US3] Реализовать состояние «показывать нечего» с обязательным называнием причины
      в `ui/src/preview/PreviewPane.tsx`: пустая панель без объяснения запрещена
      (FR-014, SC-004)
- [X] T086 [US3] Реализовать переход к месту ошибки в открытой заметке в
      `ui/src/preview/DiagnosticsList.tsx` (FR-015, SC-007)
- [X] T087 [US3] Реализовать переход к месту ошибки в связанном файле в
      `ui/src/preview/DiagnosticsList.tsx` и `ui/src/workspace/Workspace.tsx`: файл
      открывается в редакторе, курсор встаёт на место; одновременный показ двух файлов не
      вводится (FR-015)
- [X] T088 [US3] Реализовать отсутствие действия перехода при `openable: false` в
      `ui/src/preview/DiagnosticsList.tsx`: недействующую кнопку показывать запрещено
      (FR-033)
- [X] T089 [US3] Реализовать снятие пометки и очистку диагностик при исправлении ошибки в
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

- [X] T091 [P] [US4] Тест: заметка с двухуровневым импортом и изображением собирается вместе
      с ними, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-019)
- [X] T092 [P] [US4] Тест: отсутствующий связанный файл называется поимённо с причиной, а не
      общей ошибкой сборки, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-021)
- [X] T093 [P] [US4] **Тест циклической связи** — поведение `typst` здесь зондом не
      проверялось (research.md §11): два файла, подключающие друг друга, дают диагностику, а
      не зависание, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-022). Если `typst` зависает — нужен
      собственный предохранитель до сборки
- [X] T094 [P] [US4] Тест: ссылка за пределы папки vault отклоняется с `OutsideVault` **до**
      чтения, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-023)
- [X] T095 [US4] Реализовать учёт связанных файлов сборки в
      `packages/core/src/preview/snapshot.rs`: какие файлы vault прочитаны для этой заметки
- [X] T096 [US4] Реализовать событие `DependencyChanged` в
      `packages/core/src/preview/events.rs` — contracts/frontend-preview.md
- [X] T097 [US4] Подписать пересборку на изменения связанных файлов через существующий поток
      `VaultEvent` фичи 001 в `packages/core/src/preview/mod.rs`: своего механизма слежения
      не вводить (spec.md → Assumptions, FR-020)
- [X] T098 [US4] Реализовать отказ `OutsideVault` до чтения в
      `packages/core/src/preview/resolve.rs` (FR-023)
- [X] T099 [US4] Реализовать обработку циклической связи в
      `packages/core/src/preview/compile.rs` по результату T093

### Шрифты

- [X] T100 [P] [US4] Тест: шрифт из папки vault попадает в набор и применяется, в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-027)
- [X] T101 [P] [US4] Тест: отсутствующий шрифт называется поимённо, а документ собирается с
      подстановкой и остаётся `Succeeded` — **не** `Failed`, в
      `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`
      (FR-028)
- [X] T102 [P] [US4] Тест: появление и удаление шрифта в vault обновляет набор по правилам
      связанного файла, в `packages/core/tests/preview_native.rs` (FR-029)
- [X] T103 [P] [US4] Тест: шрифт, установленный в системе, но отсутствующий во встроенном
      наборе и в vault, **не** применяется, в `packages/core/tests/preview_native.rs` и
      `packages/core/tests/preview_wasm.rs` (FR-027)
- [X] T104 [US4] Реализовать сбор шрифтов из папки vault в
      `packages/core/src/preview/fonts.rs` (FR-027)
- [X] T105 [US4] Реализовать `FontSet::missing()` и предупреждение о недостающих шрифтах в
      `packages/core/src/preview/fonts.rs` (FR-028)
- [X] T106 [US4] Реализовать реакцию на появление и удаление шрифта в vault в
      `packages/core/src/preview/fonts.rs` (FR-029)
- [X] T107 [US4] Проверить, что шрифты vault не показываются в дереве заметок — правило
      фичи 001 и 002 действует без изменений (spec.md → Assumptions)
- [X] T108 [US4] Замерить время от изменения связанного файла до обновления предпросмотра и
      зафиксировать порог в `packages/core/benches/preview_scale.rs` (SC-006) — рядом с
      замерами SC-001 и SC-002/SC-003
- [ ] T109 [US4] Прогнать `quickstart.md` Сценарии 5, 6 и 7 на web- и native-сборке

**Checkpoint**: все четыре user story реализованы.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T110 [P] Реализовать исход `VaultUnavailable` и сообщение пользователю в
      `packages/core/src/preview/compile.rs` и `ui/src/preview/PreviewPane.tsx`; текст в
      редакторе при этом сохраняется по правилам фичи 001 (FR-025)
- [X] T111 [P] Реализовать наследование предела размера заметки фичи 001 в
      `packages/core/src/preview/resolve.rs`: заметку, которую нельзя открыть, не собирать
      (FR-030)
- [X] T112 [P] Тест: все операции фичи выполняются при полностью отключённой сети без
      отличий в поведении, в `packages/core/tests/preview_native.rs` (FR-024, SC-009)
- [X] T113 Убедиться, что пакеты Typst из сети не поддерживаются и попытка их использовать
      даёт понятный отказ, в `packages/core/src/preview/world.rs` (research.md §8,
      принцип IX)
- [X] T114 [P] Проверить, что панель предпросмотра остаётся темочувствительной и использует
      токены из `ui/src/styles/tokens/`, а не собственные цвета
- [ ] T115 Прогнать `quickstart.md` Сценарии 9 и 10 (офлайн, границы, потолок гарантий) на
      web- и native-сборке
- [ ] T116 Прогнать **весь** `quickstart.md` (Сценарии 1-10) подряд на native- и
      wasm32-сборках; зафиксировать любое расхождение паритета (принцип II)
- [ ] T117 Зафиксировать фактический вес доставки wasm-модуля после `wasm-opt` и сжатия и
      время первого открытия на небыстром канале; сверить с бюджетом SC-001
      (research.md §11)
- [X] T118 Прогнать `pnpm typecheck` по всем пакетам (`ui/`, `apps/web/`, `apps/desktop/`):
      `strict` без `any` (принцип IV)
- [X] T119 Прогнать `cargo test -p typsidian-core` и
      `wasm-pack test --headless --chrome packages/core` — оба обязательны (принцип II)
- [X] T120 Обновить раздел «Что готово» в `README.md`: фича 005 переходит из «не начато» в
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
- ~~**T042, desktop.**~~ Закрыт 2026-09-15: команды `preview_compile` и
  `preview_compile_with_templates` в `packages/shell`, клиент — в
  `ui/src/vault-client-tauri.ts`, общий для desktop и Android. В окне не прогонялся.

**Отступления от буквы задач**, чтобы не искать их потом по диффу: T043 (`PreviewPage.tsx`) и
T046 (`useCompiledDocument.ts`) отдельными файлами не заведены — их содержимое пока живёт
внутри `ui/src/preview/PreviewPane.tsx`; T062 (дебаунс) реализован там же, а не в
`useCompiledDocument.ts`. При выполнении T046 это надо разнести по файлам, как предписывает
plan.md → Project Structure.

---

## Текстовый слой закрыт (2026-09-10)

Из «сознательно обойдённого» вертикального среза закрыт первый кусок: T031-T038
(ядро), T043 и T044 (интерфейс). Текст в предпросмотре теперь выделяется,
копируется и доступен программе чтения с экрана — FR-031, FR-032 и SC-011
выполнены в той части, которую проверяет автоматика.

**Ядро** — `packages/core/src/preview/textlayer.rs`: обход `Frame` с накоплением
трансформаций групп, `TextRun` с координатами страницы, ширина из
`TextItem::width()`, язык из `TextItem::lang`. Слой лежит в самом
`RenderedPage` рядом с картинкой, а не приезжает отдельным событием: два слоя
от разных сборок — запрещённое состояние (contracts/text-layer.md § 5).

Правило composition координат взято из `typst-svg-0.15.1/src/lib.rs`
(`render_frame`, `render_group`), а не подобрано: расхождение с ним и есть тот
самый дефект, при котором картинка правильная, а выделение съехало.

**Проверки** — общая таблица `packages/core/tests/common/textlayer_cases.rs`,
прогоняется обоими таргетами (по 4 теста в `preview_native.rs` и
`preview_wasm.rs`), все зелёные.

**Интерфейс** — `ui/src/preview/TextLayer.tsx` и `ui/src/preview/PreviewPage.tsx`,
вынесенный из `PreviewPane.tsx`, как и предписывал plan.md. Слой — `<svg>` с
`viewBox` в пунктах страницы: браузер пересчитывает и позиции, и кегль тем же
множителем, что применён к картинке, поэтому своего масштаба в JS нет и
разъехаться слою неоткуда. Картинка помечена `aria-hidden`, слой — **нет**
(contracts/text-layer.md § 4).

### Отметки, отставшие от кода

Ниже — задачи, чья реализация существует и покрыта, но чекбокс не переставлен.
Как и в `004/tasks.md` § Phase 8, отметки не двигаются без прогона; прогон от
2026-09-10 зелёный (383 теста `cargo test`, 147 `wasm-pack test`).

| Задача | Свидетельство |
|---|---|
| T012, T013 | `resolve_iterations_are_bounded` на обоих таргетах; `MAX_RESOLVE_ITERATIONS` в `preview/snapshot.rs` |
| T014-T016 | `preview/resolve.rs` (`ResolveError` со всеми вариантами), `compile_note_resolved` в `preview/compile.rs`. **Отступление**: T014 сделан свободной функцией `read_dependency`, а не трейтом `DependencyResolver` — по той же причине, что и в фиче 003, все операции асинхронны |
| T080 | `preview/diagnostics.rs`: строка и колонка 1-based из `byte_to_line_column` |
| T092, T094 | `missing_image_is_named_with_its_reason_and_does_not_spin_the_loop`, `path_escaping_the_vault_is_rejected_before_any_read` |
| T095, T098 | `dependencies_list_every_file_that_entered_the_build`; `ResolveError::OutsideVault` до чтения |
| T111 | `ResolveError::TooLarge` наследуется от `ReadError::TooLarge` фичи 001 |

### T113 требует переформулировки, а не выполнения

Задача просит «убедиться, что пакеты Typst из сети не поддерживаются». С тех пор
фичи 006 и 007 добавили ровно обратное: `preview/packages.rs::fetch_and_install`
скачивает внешний шаблон, ради чего в `Cargo.toml` стоят `reqwest`, `flate2` и
`tar`. Выполнять задачу в прежней формулировке значило бы откатить две фичи.

---

## Сверка с кодом и доделка (2026-10-09)

Отметки не двигались с 2026-09-15, хотя фичи 006–009 дописывали предпросмотр. Сверка прошла
по коду и тестам; прогон от 2026-10-09: `preview_native` — 42 теста, `preview_wasm` — 40,
оба зелёные; `pnpm typecheck` по четырём пакетам чист; сценарий
`scripts/dev/scenarios/preview.mjs` в headless-браузере на `next dev` проходит.

### Сделано в этот заход

| Задачи | Что и где |
|---|---|
| T060 | `typst::comemo::evict(MEMO_MAX_AGE)` после каждой сборки в `preview/compile.rs`. До этого кэш мемоизации только рос |
| T065–T068 | `preview/anchor.rs` (`ScrollAnchor`, `ResolvedAnchor`, `resolve`), `CompiledDocument::with_anchor`; якорь уезжает в ядро параметром сборки и возвращается в `CompiledDocument.anchor`; `ui/src/preview/useScrollAnchor.ts` только снимает и прокручивает |
| T069 | `PreviewPage` — `memo` по `digest` |
| T054–T056, T073–T076, T091, T093, T099 | общая таблица `tests/common/preview_cases.rs` на обоих таргетах |
| T095–T097 | `CompiledDocument.dependencies` (вошедшие файлы и те, что сборка не нашла); рабочая область пересобирает заметку по событию vault о таком файле |
| T086–T088 | место замечания — кнопка, когда файл открывается редактором (`canReveal`), иначе строка без действия; переход — `revealDiagnostic` рабочей области |

### Отметки, отставшие от кода

| Задачи | Свидетельство |
|---|---|
| T012–T016, T080, T092, T094, T098, T111 | таблица от 2026-09-10 выше; тесты зелёные |
| T040, T041 | `ui/src/preview-client.ts`, `previewClient` в `vault-client-web.ts` |
| T058 | `revision` в `CompiledDocument`, отбрасывание в `PreviewPane.tsx` |
| T082, T084, T085, T089 | разделение ошибок и предупреждений, пометка «не соответствует тексту», «показывать нечего» с причиной, очистка при удачной сборке — `PreviewPane.tsx` |
| T101, T105, T106 | `FontSet::missing`, `VaultFontCache`; `font_outside_vault_is_reported_unknown`, `vault_font_is_read_once_per_change_not_per_compile`, `a_warning_does_not_fail_the_build` |

### Отступления от буквы задач

- **T039, T096 — событий сборки нет.** Вместо `BuildStarted`/`BuildFinished`/`DependencyChanged`
  — запрос-ответ с `revision` и список `dependencies` в результате. Сборку запрашивает
  интерфейс, поэтому её начало и конец он знает сам; отдельный канал событий дублировал бы это.
- **T055** проверяет метку версии в ядре; «собирается последнее состояние» обеспечивает
  дебаунс в `PreviewPane.tsx`, отдельного теста на него нет (JS-тестов в репозитории нет).
- **T083** — `DiagnosticsList.tsx` не заведён: список — `DiagnosticItem` внутри `PreviewPane.tsx`.
- **T097 — изображения и файлы данных.** Vault сообщает событиями только о заметках и досках;
  остальные связанные файлы в индекс не входят. Для них пересборка идёт при возврате фокуса
  в окно приложения — картинку правят во внешней программе. Событийной пересборки для таких
  файлов нет; это остаток FR-020, см. T121.
- **T113 закрыта как устаревшая**: фичи 006 и 007 ввели загрузку внешних шаблонов, задача в
  прежней формулировке потребовала бы их откатить (см. раздел выше).

### Что остаётся

- **T018, T057, T059 — отмена сборки при закрытии заметки и переходе к другой.** Результат
  такой сборки отбрасывается по `revision` и пути заметки, но сама она дорабатывает: воркер
  убивается только по пределу времени. Убивать его на каждую смену заметки значило бы
  каждый раз заново поднимать экземпляр ядра и терять кэш мемоизации.
- **T029 — шрифты вне модуля.** Встроенный набор по-прежнему вшит в wasm (`typst-assets`,
  feature `fonts`); цена — вес доставки. Требует раздачи файлов шрифтов каждой оболочкой.
- **T046** — вынести состояние сборки из `PreviewPane.tsx` в `useCompiledDocument.ts`.
- **T061, T053, T071, T108, T117** — замеры; `MEMO_MAX_AGE = 10` взят как у `typst watch`,
  без замера.
- **T064, T102, T103, T107, T110, T112, T114** — не сверялись.
- **T050–T052, T070, T090, T109, T115, T116, T119, T120** — прогоны quickstart и итоговые шаги.

- [X] T121 [US4] Пересборка по изменению связанного файла, которого нет в индексе vault
      (изображение, файл данных): отметка версии таких зависимостей в ядре и сверка на такте
      опроса vault в `packages/core/src/preview/` и `ui/src/preview/PreviewPane.tsx`, вместо
      пересборки по возврату фокуса (FR-020, SC-006)

### Сборка вне потока интерфейса (2026-10-09, второй заход)

| Задачи | Что и где |
|---|---|
| T020 | `CompileExecutor`, `CompileJob`, `CompileOutput`, `ExecutorError`, `InlineExecutor` в `preview/compile.rs`; цикл резолва зовёт исполнителя на каждый проход (`every_compiler_pass_goes_through_the_executor`) |
| T021 | `ThreadExecutor` в `packages/shell/src/lib.rs`: `spawn_blocking` с `tokio::time::timeout`; по пределу результат отбрасывается, поток дорабатывает |
| T022, T030 | `preview/worker.rs` — форма сообщений и обе их стороны (`WorkerExecutor`, `runCompileJob`); `ui/src/preview/compile-worker.ts` — воркер со вторым экземпляром ядра, `compile-worker-client.ts` — его владелец. Воркер создаётся при первой сборке; если он не поднялся, ядро собирает на месте и пишет причину в консоль |
| T023, T019 | `BUILD_TIME_LIMIT_SECS = 10`; исход — документ `Failed` с `AppMessage::BuildTimedOut`; на web по пределу `worker.terminate()`. `a_build_over_the_time_limit_is_named_and_changes_nothing` на обоих таргетах |
| T017 | сценарий `scripts/dev/scenarios/preview-timeout.mjs`: во время десятисекундной сборки страница рисует ~60 кадров в секунду, затем показывает причину, прежний вид остаётся, после правки предпросмотр восстанавливается на новом воркере |

**Отступления.** Трейт называется как в задаче, но `CompileHandle` и `spawn` нет: исполнитель
— асинхронная функция «выполни проход», а не дескриптор задачи; отмена — отбрасывание
результата по `revision`. Предел времени живёт у исполнителя, а не в `compile.rs`: оборвать
синхронный `typst::compile` изнутри нечем. Экспорт в PDF (009) идёт через `InlineExecutor`,
в потоке вызывающего. **T030** выполнена для второго экземпляра ядра; основной по-прежнему
загружается при открытии vault — он нужен дереву и редактору, а не только предпросмотру.

### Итог захода 2026-10-09

Прогон после всех правок: `cargo test -p typsidian-core` — зелёный целиком, `preview_native`
— 45 тестов; `wasm-pack test --headless --chrome packages/core` — зелёный целиком,
`preview_wasm` — 43; `cargo check -p typsidian-shell` и `pnpm typecheck` чисты; сценарии
`preview.mjs` и `preview-timeout.mjs` в headless-браузере проходят. В окне рабочего стола
ничего из этого не прогонялось.

Дополнительно закрыто: T102 (`a_font_added_to_and_removed_from_the_vault_updates_the_set`),
T103 (`font_outside_vault_is_reported_unknown` — набор без системных шрифтов по построению),
T107 (`classify` относит шрифты к `Other`, в дерево они не попадают), T114 (цвет листа
вынесен в токен `--paper`; других собственных цветов у панели нет), T119, T120.

**Замер к T117** (задача остаётся открытой — время первого открытия не мерялось):
`typsidian_core_bg.wasm` после `wasm-opt` — 39,9 МБ, gzip -9 — 17,5 МБ. Это заметно больше
10,5 МБ brotli, названных в срезе от 2026-09-06: с тех пор в модуль вошли загрузка шаблонов,
экспорт в PDF и синхронизация. Модуль теперь загружается дважды — в окно и в воркер сборки
(второй раз из кэша браузера).

Открыто 22 задачи: отмена сборки (T018, T057, T059), шрифты вне модуля (T029), вынос
состояния сборки из панели (T046), `beginEdit` на каждое нажатие (T064 — меняет гарантию
001 FR-035, без решения не трогалась), `VaultUnavailable` (T110), офлайн-тест (T112),
событийная пересборка для изображений (T121), замеры (T053, T061, T071, T108, T117) и
ручные прогоны quickstart (T050–T052, T070, T090, T109, T115, T116).

### Третий заход (2026-10-09): отмена, связанные файлы вне индекса, остатки

| Задачи | Что и где |
|---|---|
| T018, T057, T059 | `ExecutorError::Cancelled`; `PreviewClient.cancel` — на web убивает воркер с идущей сборкой (`compile-worker-client.ts`), панель зовёт его при смене и закрытии заметки и не показывает результат, пришедший для другой заметки. Отменённая сборка не несёт ни страниц, ни замечаний (`a_cancelled_build_yields_nothing_to_show`). Сценарий `preview-timeout.mjs`: переход к другой заметке посреди долгой сборки показывает её вид меньше чем за секунду |
| T121 | `VaultBackend::file_mark` (размер и время изменения без чтения), `preview::dependency_mark`, привязки `dependencyMark` / `preview_dependency_mark`; панель раз в 2 секунды сверяет отметку связанных файлов и пересобирает заметку при расхождении. Пересборка по возврату фокуса убрана. `the_dependency_mark_follows_the_files`; сценарий `preview.mjs` — правка `#read("данные.txt")` на диске |
| T110 | `AppMessage::VaultUnavailable`: предупреждение, когда папка vault недоступна во время сборки; текст из буфера при этом собирается (`an_unavailable_vault_is_named_and_the_text_still_builds`) |
| T046 | `ui/src/preview/useCompiledDocument.ts` — сборка, её состояние, отмена, слежение за зависимостями и якорь прокрутки; `PreviewPane.tsx` показывает |
| T112 | покрыта тестами 007: `a_note_without_imports_never_reaches_for_the_network` и соседними в `install_cases.rs` на обоих таргетах; у предпросмотра без внешних шаблонов сетевого пути нет вовсе |

**Отступления.** На рабочем столе и Android отмены нет: поток с идущей сборкой прервать
нечем, её результат отбрасывается. `VaultUnavailable` — предупреждение, а не отказ: сам
текст заметки собирается из буфера редактора, недоступны только связанные файлы и шрифты.

### Замеры (T053, T061, T071, T108)

`packages/core/benches/preview_scale.rs` был заглушкой; теперь мерит и проверяет пороги как
условие прохождения. Прогон 2026-10-09, native, release, документ 50 страниц (65 КБ):

| Что | Измерено | Бюджет |
|---|---|---|
| Первая сборка (SC-001) | 165 мс | 2 с |
| Пересборка после правки в середине (SC-002) | медиана 139 мс, худшая 215 мс | 750 мс ядру + 250 мс дебаунса |
| Начало и конец сессии из 40 правок (T061) | 143 мс / 146 мс | конец не медленнее начала вдвое |
| Отметка 40 связанных файлов | 43 мкс | — |
| Пересборка после правки одного из 40 импортов (SC-006) | 695 мс | 750 мс ядру + 1 с опроса + 250 мс дебаунса |

`MEMO_MAX_AGE = 10` оставлен: за сессию пересборка не замедляется. Такт опроса связанных
файлов сокращён с 2 до 1 секунды — с двумя порог SC-006 в худшем случае не выдерживался.
SC-003 (отклик на ввод во время сборки) ядром не измерить; сценарий `preview-timeout.mjs`
показывает ~60 кадров в секунду во время десятисекундной сборки.

**Оговорки.** Замеры — на native; под wasm сборка медленнее, и там пороги не мерялись.
Пересборка заметки с 40 импортами укладывается в бюджет с запасом в 55 мс: каждый импорт —
отдельный проход цикла резолва, и содержимое связанных файлов между сборками не кэшируется
(кэшируются только файлы шаблонов). На заметке с сотней связанных файлов SC-006 не
выдержится — см. T122.

- [X] T122 [US4] Кэш содержимого связанных файлов vault между сборками одной заметки (по
      отметке `file_mark`), чтобы пересборка не поднимала каждый импорт отдельным проходом
      компилятора, в `packages/core/src/preview/cache.rs` и `compile.rs`; порог — в
      `benches/preview_scale.rs` на 100 связанных файлах (SC-002, SC-006)

Открыто 12 задач: шрифты вне модуля (T029), `beginEdit` на каждое нажатие (T064 — меняет
гарантию 001 FR-035, нужно решение), кэш связанных файлов (T122), вес и время первого
открытия (T117) и ручные прогоны quickstart (T050–T052, T070, T090, T109, T115, T116).

### T122: кэш связанных файлов (2026-10-09)

`PackageCache::vault_files_of` / `remember_vault_files` держат содержимое связанных файлов
последней сборки заметки вместе с их отметкой (`file_mark`). Следующая сборка той же заметки
подкладывает в снимок те, чья отметка на диске не изменилась, — и компилятору не приходится
запрашивать каждый импорт отдельным проходом. В кэше остаются только файлы, которые
компилятор действительно запрашивал (`DependencySnapshot::record_access`; из воркера список
приезжает в ответе), так что убранный из текста импорт перестаёт быть зависимостью через
одну сборку. Кэш держит одну заметку.

Замер тем же бенчмарком: пересборка заметки с 40 импортами после правки одного из них —
133 мс вместо 695. Тест `a_linked_file_is_read_once_until_it_changes` на обоих таргетах;
`preview_native` — 49, `preview_wasm` — 47.
