# Tasks: Дополнение меток и ссылок

**Input**: Design documents from `/specs/018-label-completion/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для каждой единицы логики ядра на обоих таргетах (принципы I и II):
случаи — в общей таблице `packages/core/tests/common/completion_cases.rs`, раннеры —
`completion_native.rs` и `completion_wasm.rs`. Для UI JS-тестов в репозитории нет — проверка
`pnpm typecheck`, `scripts/check-i18n.sh`, сценарием со снимком и по quickstart.md.

**Предусловие всей фичи**: реализованы 013, 014, 015 и 017 — 018 идёт последней и читает их
код (plan.md «Зависимости от соседних фич»). Новых зависимостей нет: `pnpm-lock.yaml` и хэш
`pnpmDeps` в `nix/desktop.nix` не меняются, задач для `nix-env-agent` нет (research.md §9).

## Phase 1: Setup

- [X] T001 Объявить `pub mod completion;` в `packages/core/src/lib.rs` и завести `packages/core/src/completion/mod.rs` с пустыми подмодулями `context`, `linked`, `labels`, `notes`, `definition`, `marks` и `web` (под `cfg(target_arch = "wasm32")`)
- [X] T002 Сверить с фактическим кодом соседей то, на что опирается план, и дописать итог разделом «§17 Сверка с соседями» в `specs/018-label-completion/research.md`: имена и сигнатуры `structure` (015) для меток, ссылок и корня разбора; места вхождений `[[…]]` и функция кратчайшей записи в `packages/core/src/graph/mod.rs` (013); обёртка перехода и команда «Назад» (014); механизм отметок редактора, перечень ключей источников и показ источника в панели (017). Расхождение с планом — поправить пути и имена в задачах ниже до начала работы
- [X] T003 [P] Завести таблицу случаев `packages/core/tests/common/completion_cases.rs` (подключить в `packages/core/tests/common/mod.rs`) с общими заготовками vault и раннеры `packages/core/tests/completion_native.rs` и `packages/core/tests/completion_wasm.rs` по образцу `export_native.rs`/`export_wasm.rs`

## Phase 2: Foundational

**Purpose**: то, без чего не начинается ни одна история: типы границы, контекст каретки,
перечень меток связанных файлов, свидетельство сборки, команды и точки подключения в редакторе.

- [X] T004 Дополнить `packages/core/src/structure/mod.rs` тем, чего 018 не нашла при сверке T002 (research.md §1): текст помеченного элемента одной строкой, признак «разметка верхнего уровня» у метки, доступ к корню разбора; с модульными тестами там же. Если всё уже есть — закрыть задачу записью в research.md §17
- [X] T005 [P] Случаи для дополнений `structure` из T004 в таблице случаев фичи 015 `packages/core/tests/common/structure_cases.rs` с парными вызовами в её раннерах на обоих таргетах
- [X] T006 Описать типы границы из data-model.md (`LinkedIndex`, `LinkedFile`, `LinkedLabel`, `CompletionRequest`, `CompletionList`, `CompletionGroup`, `CompletionItem`, `ItemDetail`, `NoCompletionReason`, `SourceKey`, `DefinitionRequest`, `DefinitionOutcome`, `NoDefinition`, `MarksRequest`, `ReferenceMark`, `MarkKind`, `DuplicatePlace`) с `Serialize`/`Deserialize` и `Tsify` в `packages/core/src/completion/mod.rs`
- [X] T007 Случаи контекста каретки в `packages/core/tests/common/completion_cases.rs` с вызовами в обоих раннерах: `@` и `[[` в разметке и в блоке содержимого внутри кода — предлагать; в коде, строке, строчном и блочном комментарии, `raw`, формуле, адресе почты и автоссылке — нет, с причиной; каретка после кириллического текста и эмодзи (смещения UTF-16); ручной вызов без открывающего знака (FR-004, research.md §2)
- [X] T008 Реализовать `context::at(text, offset, manual)` по research.md §2 поверх корня разбора `structure` в `packages/core/src/completion/context.rs`; пересчёт UTF-16 ↔ байты — существующими `elements::byte_at`/`utf16_at`
- [X] T009 Завести `BuildEvidence` и `BuiltLabel` и сбор меток из `PagedDocument::introspector().query_labelled()` в `packages/core/src/preview/evidence.rs` (объявить в `packages/core/src/preview/mod.rs`)
- [X] T010 Добавить поле `evidence: Option<BuildEvidence>` (`serde(default, skip_serializing_if)`, `tsify(optional)`) в `CompiledDocument` и заполнять его у удачной сборки `Render::Pages` в `compile_with` — `labels` из документа, `references` из `structure` над текстом заметки — в `packages/core/src/preview/compile.rs`; у неудавшейся и оборванной сборки поле `None`
- [X] T011 [P] Случаи свидетельства в `packages/core/tests/common/preview_cases.rs` с вызовами в `packages/core/tests/preview_native.rs` и `packages/core/tests/preview_wasm.rs`: метка из цикла и из `#label(…)` есть в `evidence.labels`; повторная метка имеет `count == 2`; ссылки текста — в `references`; у сборки с ошибкой `evidence` нет
- [X] T012 Случаи `linked_index` в `packages/core/tests/common/completion_cases.rs` с вызовами в обоих раннерах: метки `#include`-файла с видом и местом; не-`.typ` и сама заметка пропущены; файл из кэша с совпавшей отметкой не читается повторно; отсутствующий и не-UTF-8 файл — в `unreadable`; недоступная папка vault не приводит к панике (принцип VII)
- [X] T013 Реализовать `linked_index(backend, packages, note_path, dependencies)` по contracts/core-completion-api.md в `packages/core/src/completion/linked.rs`
- [X] T014 Добавить `CommandId::GotoDefinition` (`goto-definition`, `Mod+Alt+D`) и `CommandId::TriggerCompletion` (`trigger-completion`, `Mod+Alt+P`) в `COMMANDS`, `as_str` и `default_shortcut` в `packages/core/src/settings/keybindings.rs`; обновить тесты уникальности умолчаний и добавить проверку, что оба сочетания проходят `Shortcut::problem()` и имеют `code()` (research.md §12)
- [X] T015 wasm-биндинг `PreviewEngine.completionLinkedIndex(backend, notePath, dependencies)` в `packages/core/src/preview/web.rs`
- [X] T016 Команда Tauri `completion_linked_index` на том же состоянии, что команда сборки, и её регистрация в `packages/shell/src/lib.rs`
- [X] T017 Метод `completionLinkedIndex` в `ui/src/preview-client.ts` и обеих реализациях: `ui/src/vault-client-web.ts` (через `wasm-call-queue`) и `ui/src/vault-client-tauri.ts`
- [X] T018 Хук `ui/src/completion/useLinkedIndex.ts` по contracts/frontend-completion.md: пересчёт при смене заметки, списка зависимостей и их отметки; сигнал о новой отметке зависимостей вывести наружу из `ui/src/preview/useCompiledDocument.ts` рядом с `onDependencies`
- [X] T019 [P] В `ui/src/editor/CodeEditor.tsx`: проп `onCompletionTrigger(offset)` (пользовательский ввод `@` или второго `[` подряд, по образцу `onSlashMenu`) и проп `extensions` в отдельном `Compartment`, перенастраиваемый без пересоздания редактора
- [X] T020 [P] Вынести `useMenuPosition` из `ui/src/editor/InsertMenu.tsx` в `ui/src/editor/useMenuPosition.ts` без изменения поведения меню вставки

**Checkpoint**: ядро отвечает на «где каретка» и «какие метки в связанных файлах», сборка
отдаёт свидетельство, редактор готов принять список и расширения.

## Phase 3: User Story 1 — Выбрать метку или заметку из списка при наборе (P1) 🎯 MVP

**Goal**: по `@` — метки заметки и связанных файлов и ключи источников, по `[[` — заметки
vault; список сужается, управляется с клавиатуры, выбор подставляет имя.

**Independent Test**: в заметке с `<введение>` и рисунком `<рис-схема>` набрать `@рис` —
предложена `рис-схема` с пометкой «рисунок», Enter подставляет её.

### Tests for User Story 1

- [X] T021 [US1] Случаи кандидатов-меток в `packages/core/tests/common/completion_cases.rs` с вызовами в обоих раннерах: вид и текст помеченного элемента (заголовок, рисунок, таблица, формула); метка связанного файла несёт имя файла; совпадение по любой части имени без учёта регистра и отрезки `matched`; ярусы совпадения; при пустом запросе ближайшие к каретке первыми; 500 меток → 50 строк и `total == 500`; повторное имя — одна строка с `duplicate`; ключи источников — отдельная группа после меток (FR-001, FR-003, US1 №1, №2, №5)
- [X] T022 [US1] Случаи кандидатов-заметок в `packages/core/tests/common/completion_cases.rs` с вызовами в обоих раннерах: заметки той же папки первыми, затем по числу шагов между папками; доски и сама открытая заметка не предлагаются; одноимённые заметки различаются папкой; `insert` каждой строки, пропущенный через `graph::resolve_link_target` из текущей заметки, даёт путь выбранной заметки; `]]` дописывается только при отсутствии (FR-002, US1 №3)
- [X] T023 [US1] Случаи сквозной `completions` в `packages/core/tests/common/completion_cases.rs` с вызовами в обоих раннерах: в коде и строке ответ пуст с причиной и `backend` не опрашивается; неподходящий набор даёт пустые группы (US1 №4); текст берётся из запроса, а не с диска (FR-008)
- [X] T024 [P] [US1] Парные тесты настройки в `packages/core/tests/settings_native.rs` и `packages/core/tests/settings_wasm.rs`: `editor_completion_auto` по умолчанию `true`, отсутствие поля в файле читается как `true`, значение переживает запись и чтение, изменение через `SettingsChange`; там же — пара для T014: умолчания `goto-definition` и `trigger-completion` разрешаются в `Mod+Alt+D` и `Mod+Alt+P` и не совпадают ни с одной другой командой (принцип II — на обоих таргетах)

### Implementation for User Story 1

- [X] T025 [US1] Реализовать `labels::candidates` (метки заметки из `structure`, метки `LinkedIndex`, `sources`; фильтр, ярусы, близость, предел 50, `matched`, `insert`, `preview`) по research.md §5 в `packages/core/src/completion/labels.rs`
- [X] T026 [US1] Реализовать `notes::candidates` (фильтр, порядок по папке, запись через функцию кратчайшей записи 013, дописывание `]]`) по research.md §6 в `packages/core/src/completion/notes.rs`
- [X] T027 [US1] Реализовать `completions(backend, request)` по порядку шагов contracts/core-completion-api.md в `packages/core/src/completion/mod.rs`
- [X] T028 [P] [US1] Добавить `editor_completion_auto: bool` (умолчание `true`) в `Settings` в `packages/core/src/settings/mod.rs` и поле-`Option` с применением в `SettingsChange` в `packages/core/src/settings/apply.rs`
- [X] T029 [US1] wasm-биндинг `WebBackend.completions(request)` в `packages/core/src/completion/web.rs`
- [X] T030 [US1] Команда Tauri `completions` и её регистрация в `packages/shell/src/lib.rs`
- [X] T031 [US1] Метод `completions` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts` и `ui/src/vault-client-tauri.ts`
- [X] T032 [P] [US1] Тексты списка и настройки на всех языках: ключи `editor.complete.*` в `ui/src/i18n/locales/ru/editor.ts` и `ui/src/i18n/locales/en/editor.ts`, подписи выключателя в `ui/src/i18n/locales/ru/settings.ts` и `ui/src/i18n/locales/en/settings.ts`, команда «Вызвать дополнение» в `ui/src/i18n/locales/ru/palette.ts` и `ui/src/i18n/locales/en/palette.ts`
- [X] T033 [US1] Расширение клавиш открытого списка (`ArrowUp`, `ArrowDown`, `Enter`, `Tab`, `Escape` с `Prec.highest`, только при открытом списке) в `ui/src/completion/completionKeys.ts`
- [X] T034 [US1] Хук сеанса списка `ui/src/completion/useCompletion.ts` по contracts/frontend-completion.md: автопоказ по `onCompletionTrigger` при включённой настройке, ручной вызов и при выключенной (FR-009), запрос на каждое изменение при открытом списке, отбрасывание опоздавших ответов, выбор одной транзакцией, закрытие
- [X] T035 [US1] Сверстать попап по макету `Typsidian Дополнение.dc.html`, экраны 1a и 1b, в `ui/src/completion/CompletionPopup.tsx` со стилями `.ts-complete*` в `ui/src/styles/app.css`: группы с заголовком и числом, подсветка совпадения, тег вида, текст элемента, имя файла или папка справа, подвал с подсказками и строкой «будет: …», роли `listbox`/`option`
- [X] T036 [US1] Подача для узкого экрана в `ui/src/completion/CompletionPopup.tsx`: список над строкой набора на ширину редактора, без подвала с клавишами, выбор касанием без потери фокуса редактора (Edge Cases «Телефон», research.md §14)
- [X] T037 [US1] Подключить список в `ui/src/editor/NoteEditor.tsx`: `useLinkedIndex`, `useCompletion`, `CompletionPopup`, расширение `completionKeys` через проп `extensions`, число предложений в строке состояния
- [X] T038 [US1] Передать в список ключи источников фичи 017 (`SourceKey` с готовой строкой авторов и года) из её клиента в `ui/src/editor/NoteEditor.tsx`; без библиографии — пустой список (US1 №5)
- [X] T039 [US1] В `ui/src/workspace/Workspace.tsx`: сочетание и команда палитры `trigger-completion`, проброс настройки `editor_completion_auto` в редактор
- [X] T040 [US1] Выключатель «Предлагать метки и заметки при наборе» с подписью о ручном вызове (сочетание — из `displayOf("trigger-completion")`) в `ui/src/settings/AppearanceSection.tsx` (FR-009, research.md §11)
- [X] T041 [US1] Сценарий `scripts/dev/scenarios/completion.mjs`: vault из quickstart.md, набор `@рис` и `[[пл`, снимки; сверить снимки с экранами 1a и 1b макета и записать оставшиеся расхождения в research.md §15

**Checkpoint**: US1 работает и проверяется отдельно — без перехода и отметок.

## Phase 4: User Story 2 — Перейти к определению (P2)

**Goal**: команда открывает место метки (в том числе в связанном файле) или заметку-цель;
обратно — «Назад» (014); при отсутствии цели названа причина.

**Independent Test**: каретка на `@введение`, `Mod+Alt+D` — каретка у `<введение>`, в том числе
когда метка в связанном файле.

### Tests for User Story 2

- [X] T042 [US2] Случаи `definition_at` в `packages/core/tests/common/completion_cases.rs` с вызовами в обоих раннерах: метка в заметке и в связанном файле → `Place` с путём, строкой, колонкой и байтовым диапазоном; `[[цель]]` → начало заметки; каретка сразу после ссылки считается на ней; ключ источника → `Source`; причины `NoReferenceAtCaret`, `LabelMissing`, `LabelFromBuild`, `NoteMissing`, `OutsideVault`; повторная метка → первое объявление (FR-005, US2 №1–№3)

### Implementation for User Story 2

- [X] T043 [US2] Реализовать `definition_at(backend, request)` по contracts/core-completion-api.md в `packages/core/src/completion/definition.rs`
- [X] T044 [US2] wasm-биндинг `WebBackend.definitionAt(request)` в `packages/core/src/completion/web.rs`
- [X] T045 [US2] Команда Tauri `definition_at` и её регистрация в `packages/shell/src/lib.rs`
- [X] T046 [US2] Метод `definitionAt` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts` и `ui/src/vault-client-tauri.ts`
- [X] T047 [P] [US2] Тексты перехода на всех языках: ключи `editor.goto.*` (название команды и каждая причина отказа) в `ui/src/i18n/locales/ru/editor.ts` и `ui/src/i18n/locales/en/editor.ts`, команда палитры в `ui/src/i18n/locales/ru/palette.ts` и `ui/src/i18n/locales/en/palette.ts`
- [X] T048 [US2] Хук `ui/src/completion/useGotoDefinition.ts`: `Place` → переход через `revealDiagnostic`, `Source` → показ источника в панели 017 (если 017 такого действия не даёт — строка состояния «это ключ источника», research.md §17), `Nothing` → текст причины до следующего движения каретки
- [X] T049 [US2] В `ui/src/workspace/Workspace.tsx`: сочетание и команда палитры `goto-definition`; переход идёт через обёртку истории и вкладок фичи 014, чтобы «Назад» возвращал к ссылке (FR-005)
- [X] T050 [P] [US2] Пункт «Перейти к определению» с подписью сочетания в `ui/src/editor/editorContextMenu.ts` (путь к команде на телефоне)
- [X] T051 [US2] Показ причины отказа и подсказки сочетания в строке состояния по макету 1c в `ui/src/editor/NoteEditor.tsx`

**Checkpoint**: US1 и US2 работают независимо друг от друга.

## Phase 5: User Story 3 — Увидеть ссылку в никуда до сборки (P2)

**Goal**: ссылки на отсутствующие метки и заметки и повторные метки отмечены в редакторе не
позже секунды после набора; ссылки, которые разрешаются при сборке, не отмечаются.

**Independent Test**: набрать `@нет-такой` — ссылка отмечена; объявить `<нет-такой>` — отметка
исчезла.

### Tests for User Story 3

- [X] T052 [US3] Случаи отметок о метках в `packages/core/tests/common/completion_cases.rs` с вызовами в обоих раннерах: отсутствующая метка отмечена без `evidence`; метка в связанном файле — не отмечена; ключ источника — не отмечен; диапазон из `claimed` — не отмечен; при нечитаемом связанном файле (`unreadable` не пуст) ни одной `MissingLabel` нет, а отметки о заметках остаются; места в UTF-16 на тексте с кириллицей и эмодзи (FR-006, FR-008, Edge Cases)
- [X] T053 [US3] Случаи «нет ложных отметок» в `packages/core/tests/common/completion_cases.rs` с вызовами в обоих раннерах: собрать настоящей сборкой документы с меткой из цикла, из функции связанного файла, из `#label(…)` и с правилом `show ref`, передать их `evidence` — ни одной `MissingLabel`; после правки текста без новой сборки подтверждённые ссылки остаются без отметки, а новая неизвестная — отмечена (FR-007, SC-002)
- [X] T054 [US3] Случаи удержания и повторов в `packages/core/tests/common/completion_cases.rs` с вызовами в обоих раннерах: при `held` новая `MissingLabel` не выдаётся, а имя из `previously_missing` сохраняется; две метки верхнего уровня — отмечены обе с местами друг друга; метка в ветках `if`/`else` — не повтор без `evidence` и повтор при `count ≥ 2`; повтор между заметкой и связанным файлом — только при `count ≥ 2` (US3 №3, research.md §4)
- [X] T055 [US3] Случаи отметок о заметках в `packages/core/tests/common/completion_cases.rs` с вызовами в обоих раннерах: `[[нет]]` → `MissingNote` с `create_path`, равным пути разрешения ссылки; `[[../наружу]]` → `LinkOutsideVault`; существующая заметка и ссылка на себя — без отметки; отметки отсортированы и не пересекаются (US3 №2)

### Implementation for User Story 3

- [X] T056 [US3] Реализовать чистую часть `marks::label_marks` (правило `MissingLabel`, удержание, `DuplicateLabel`) по data-model.md и research.md §4 в `packages/core/src/completion/marks.rs`
- [X] T057 [US3] Реализовать `reference_marks(backend, request)` — отметки о заметках по вхождениям `graph` (013) и индексу заметок, слияние с отметками о метках — в `packages/core/src/completion/marks.rs`
- [X] T058 [US3] wasm-биндинг `WebBackend.referenceMarks(request)` в `packages/core/src/completion/web.rs`
- [X] T059 [US3] Команда Tauri `reference_marks` и её регистрация в `packages/shell/src/lib.rs`
- [X] T060 [US3] Метод `referenceMarks` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts` и `ui/src/vault-client-tauri.ts`
- [X] T061 [P] [US3] Тексты отметок на всех языках: ключи `editor.marks.*` (причина и описание каждого вида, «Создать заметку», счётчик с формами числа) в `ui/src/i18n/locales/ru/editor.ts` и `ui/src/i18n/locales/en/editor.ts`
- [X] T062 [US3] Хук `ui/src/completion/useReferenceMarks.ts` по contracts/frontend-completion.md: пересчёт через 300 мс после правки и при смене `LinkedIndex`, свидетельства, ключей источников и по событию vault; `held` и повтор без удержания по приходу сборки или через 800 мс; `evidence` из `PreviewState.shown`; `claimed` — диапазоны отметок 017
- [X] T063 [US3] Добавить виды отметок 018 (жёлтый пунктир, класс `.ts-mark-note*` в `ui/src/styles/app.css`) в механизм отметок редактора фичи 017 — модуль по research.md §17, ожидаемо `ui/src/editor/marks.ts`; красная подача 017 не меняется, одна ссылка несёт одну отметку
- [X] T064 [US3] Пояснение отметки по макету 1c в `ui/src/completion/MarkExplanation.tsx`: причина и описание у каретки и по наведению, для `MissingNote` — кнопка «Создать заметку» (создание по `create_path` существующей операцией клиента, отказ — словами из `errors`), для `DuplicateLabel` — остальные места
- [X] T065 [US3] Подключить отметки в `ui/src/editor/NoteEditor.tsx`: `useReferenceMarks`, `MarkExplanation`, счётчик отметок в строке состояния рядом со счётчиками 015 и 017
- [X] T066 [US3] Дополнить `scripts/dev/scenarios/completion.mjs` экраном 1c (отсутствующая метка, отсутствующая заметка с пояснением, повторная метка, заметка с меткой из цикла без отметки); сверить снимок с макетом и записать расхождения в research.md §15

**Checkpoint**: все три истории работают независимо.

## Phase 6: Polish & Cross-Cutting

- [X] T067 [P] Бенчмарк `packages/core/benches/completion_scale.rs` с записью `[[bench]]` в `packages/core/Cargo.toml`: 10 000 строк и 500 меток — `completions` ≤ 20 мс, `reference_marks` ≤ 50 мс на нативном таргете (SC-001, FR-006)
- [X] T068 Проверить SC-004 и SC-001 в браузере: дополнить `scripts/dev/scenarios/completion.mjs` замером — при наборе текста без `@` и `[[` вызовов `completions` нет, а от набора `@` до показа списка в заметке с 500 метками проходит не больше 100 мс; результат записать в `specs/018-label-completion/quickstart.md`
- [X] T069 Проверить SC-003 в браузере: дополнить `scripts/dev/scenarios/completion.mjs` замером времени от команды до каретки у метки в связанном файле (не больше 1 с); результат записать в `specs/018-label-completion/quickstart.md`
- [X] T070 [P] Прогнать гейты по скиллу `quality-gates-typsidian` (fmt, оба таргета ядра, `cargo test`, `wasm-bindgen-test`, `pnpm typecheck`, `scripts/check-i18n.sh`; FR-010 — паритет таргетов и тексты только из переводов) и записать итог в `specs/018-label-completion/quickstart.md`
- [ ] T071 Пройти ручные пункты quickstart.md на desktop и Android, в том числе с отключённой сетью (FR-010), и записать итог в `specs/018-label-completion/quickstart.md`; iOS записать как «не проверено» — оболочки нет

## Dependencies

- Phase 1 → Phase 2 → истории. T002 раньше всего остального в Phase 2: от неё зависят пути и
  имена в T004, T026, T038, T048, T049, T057, T063.
- В Phase 2: T004 → T008, T010, T013; T006 → T008, T013; T009 → T010; T013 → T015 → T016 →
  T017 → T018. T014, T019, T020 независимы.
- US1: T021–T023 пишутся до T025–T027; T027 → T029 → T030 → T031 → T034; T028 → T040;
  T033, T035 → T037; T020 → T035; T019 → T034.
- US2 зависит только от Phase 2 (нужны `LinkedIndex` и `evidence`); от US1 не зависит.
- US3 зависит только от Phase 2; от US1 и US2 не зависит. T056 → T057 → T058 → T059 → T060 →
  T062 → T065; T063 → T064 → T065.
- Файлы, которые правят несколько историй последовательно: `completion_cases.rs`,
  `completion/web.rs`, `packages/shell/src/lib.rs`, `vault-client*.ts`, `NoteEditor.tsx`,
  `Workspace.tsx`, `locales/*/editor.ts`, `scenarios/completion.mjs`. Между историями эти
  задачи параллельно не выполняются.

## Parallel Example

```text
# Phase 2, после T006:
T011 (preview_cases)  ||  T014 (keybindings.rs)  ||  T019 (CodeEditor.tsx)  ||  T020 (useMenuPosition.ts)

# US1, после T027:
T028 (settings)  ||  T032 (i18n)  ||  T033 (completionKeys.ts)
```

## Implementation Strategy

1. **MVP** — Phase 1, Phase 2 и US1: список по `@` и `[[`. Уже полезно без остального.
2. US2 — переход к определению; проверяется отдельно сценарием из quickstart.md.
3. US3 — отметки; ключевой риск фичи (FR-007) закрыт тестами T053 до вёрстки.
4. Polish — замеры SC и прогон на оболочках.

Задача считается готовой только с парой тестов на обоих таргетах (принцип II); отметку `[X]`
даёт прогон, а не чтение кода.
