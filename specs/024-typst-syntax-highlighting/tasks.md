# Tasks: Подсветка синтаксиса Typst

**Input**: Design documents from `/specs/024-typst-syntax-highlighting/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для логики ядра на обоих таргетах (принцип I конституции): одна
таблица `packages/core/tests/common/highlight_cases.rs`, прогон через
`highlight_native.rs` и `highlight_wasm.rs`. Для UI JS-тестов в репозитории нет — проверка
по quickstart.md и сценарием браузера.

## Phase 1: Setup

- [ ] T001 Вынести `utf16_at` и `byte_at` из `packages/core/src/elements/mod.rs` в новый `packages/core/src/text_offsets.rs` (`pub(crate) mod text_offsets;` в `packages/core/src/lib.rs`), добавить там `Utf16Index` — перевод байтовых смещений в UTF-16 одним проходом для возрастающих запросов (research.md §3); `elements` импортирует оттуда, модульные тесты переезжают вместе с функциями, включая кириллицу и суррогатную пару
- [ ] T002 Объявить `pub mod highlight;` в `packages/core/src/lib.rs` и создать `packages/core/src/highlight/mod.rs` с типами `SyntaxRole` (порядок и номера — data-model.md, `#[repr(u8)]`), `StyleFlags`, `HighlightWindow` (`Tsify`, `from`/`to` в UTF-16), `HighlightSpans { packed: Vec<u32> }` и заглушкой `pub fn highlight(text: &str, windows: &[HighlightWindow]) -> HighlightSpans`
- [ ] T003 [P] Создать каркас тестов: `packages/core/tests/common/highlight_cases.rs` (помощник `roles_of(text) -> Vec<(&str, SyntaxRole, StyleFlags)>` — фрагмент текста, роль, флаги — и помощник проверки инвариантов 1–3 data-model), `packages/core/tests/highlight_native.rs` и `packages/core/tests/highlight_wasm.rs` по образцу `structure_native.rs`/`structure_wasm.rs`; подключить модуль в `packages/core/tests/common/mod.rs`

## Phase 2: Foundational

**⚠️ CRITICAL**: без этой фазы ни одна история не видна в редакторе.

- [ ] T004 Реализовать обход листьев в `packages/core/src/highlight/walk.rs`: `typst::syntax::parse`, `LinkedNode`, флаги от предков `Strong`/`Emph`/`Heading`, пропуск поддеревьев вне окон, склейка соседних листьев с одинаковыми ролью и флагами, перевод смещений через `Utf16Index`, упаковка `[from, to, role | flags << 8]`; слияние и прижатие окон, пустой список окон — весь текст (data-model.md, contracts/core-highlight-api.md правила 2–5)
- [ ] T005 Реализовать базовое сопоставление `Tag` → `SyntaxRole` без уточнений в `packages/core/src/highlight/roles.rs` (`Error` → без роли, `Interpolated` → `Variable`, `MathGroupingParens` → `Punctuation`) и подключить его в `highlight()` в `packages/core/src/highlight/mod.rs`
- [ ] T006 [P] Тесты инвариантов в `packages/core/tests/common/highlight_cases.rs`: сортировка и непересечение, границы символов при эмодзи и кириллице перед фрагментом, фрагмент на краю окна отдаётся целиком, объединение ответов двух окон равно ответу их объединения, пустой текст и текст из одного `$` не паникуют; вызвать каждый случай из `highlight_native.rs` и `highlight_wasm.rs`
- [ ] T007 wasm-биндинг — класс `SyntaxSession` (`new(text)`, `update(edits, length, windows)`, `free`) в `packages/core/src/highlight/web.rs` (`#[cfg(target_arch = "wasm32")] mod web;` в `mod.rs`), своей памятью вне очереди вызовов `WebBackend` (пересмотрено по замеру T027, research.md §2)
- [ ] T008 [P] Команды Tauri `syntax_session_open`, `syntax_session_update`, `syntax_session_close` и состояние `HighlightSessions` в `packages/shell/src/lib.rs` рядом с `element_at`, регистрация в `generate_handler!` (contracts/core-highlight-api.md)
- [ ] T009 Метод `openHighlight(text): HighlightSession` в интерфейсе `ui/src/vault-client.ts` и реализации в `ui/src/vault-client-web.ts` (объект `SyntaxSession`, синхронный `updateNow`) и `ui/src/vault-client-tauri.ts` (`syntax_session_*`, ответ → `Uint32Array.from`) (contracts/core-highlight-api.md § VaultClient)
- [ ] T010 [P] Перечень ролей и флагов `as const` и функция «число → классы» в `ui/src/editor/highlight/roles.ts` (contracts/frontend-highlight.md § Классы и цвета)
- [ ] T011 [P] Нативный тест `roles_match_frontend_table` в `packages/core/tests/highlight_native.rs`: читает `ui/src/editor/highlight/roles.ts` и сверяет порядок имён ролей с `SyntaxRole`
- [ ] T012 Расширение `typstHighlight(client)` в `ui/src/editor/highlight/typstHighlight.ts`: `ViewPlugin` с `Decoration.mark`, окна из `view.visibleRanges` (± 50 строк, до границ строк), запрос не чаще раза за кадр, номер поколения, `decorations.map(changes)` до ответа, разовая запись ошибки в консоль, `destroy()` отменяет кадр (contracts/frontend-highlight.md § Расширение)
- [ ] T013 Убрать `typstLanguage`, `typstHighlight` (`HighlightStyle`), `KEYWORDS`, `TypstState` и `syntaxHighlighting` из `ui/src/editor/CodeEditor.tsx`; перенести `commentTokens` в `EditorState.languageData.of`; добавить проп `highlight?: Extension` через отдельный `Compartment`: в `stateFor` при создании состояния и `Compartment.reconfigure` в эффекте по пропу для смонтированного вида (contracts/frontend-highlight.md § Что уходит)
- [ ] T014 Передать `typstHighlight(client)` из `ui/src/editor/NoteEditor.tsx` в `CodeEditor` только для Typst-заметок (FR-012) — по виду файла от ядра (`kind === "Note"`), не по расширению в TypeScript (принцип I, `ui/src/vault-tree/fileKind.ts`); расширение создаётся один раз на клиента, а не на каждый рендер
- [ ] T015 Алиасы `--ts-syn-*` под `.ts-editor` и правила `.ts-editor .ts-syn-*` для всех ролей и флагов в `ui/src/styles/app.css` по таблице contracts/frontend-highlight.md; без литералов цвета, все селекторы с префиксом `.ts-editor`

**Checkpoint**: заметка раскрашена по базовым ролям Typst во всех оболочках; старой подсветки нет.

## Phase 3: User Story 1 — Структура заметки видна по цвету (P1) 🎯 MVP

**Goal**: элементы разметки различимы, обычный текст не перекрашивается случайными символами.

**Independent Test**: образец со всеми элементами разметки и кавычками в тексте; роли совпадают с таблицей образца (quickstart §2).

- [ ] T016 [P] [US1] Тесты разметки в `packages/core/tests/common/highlight_cases.rs` и вызовы в обоих файлах прогона: заголовки уровней 1–3 (`HeadingMarker` + флаг `heading` у текста), `*…*`/`_…_` с `MarkupMarker` и флагами, маркеры `-`/`+`/`1.`/`/ термин:` и `ListTerm`, `` ` `` и ```` ``` ```` с языком (`Raw`, ограничители — `MarkupMarker`, внутри без других ролей), `<рис:схема.1>`/`@рис:схема.1`, адрес с `*` внутри, `\#` и перенос `\`, AS 1.1–1.6
- [ ] T017 [P] [US1] Тесты «обычный текст остаётся текстом» в `packages/core/tests/common/highlight_cases.rs`: одиночная `"` в разметке, `$` в «450 $» без пары, `2*3` и `*` посреди слова, `=` не в начале строки, `#` перед пробелом и в конце строки (FR-005, Edge Cases); первые три строки артборда «Было · стало» (SC-004)
- [ ] T018 [US1] Уточнения ролей разметки в `packages/core/src/highlight/roles.rs`: `HeadingMarker`, `MarkupMarker` для `Star` в `Strong`, `Underscore` в `Emph` и `RawDelim` (research.md §4), пока тесты T016–T017 не зелёные на обоих таргетах
- [ ] T019 [US1] Сценарий `scripts/dev/scenarios/syntax-highlight.mjs` по образцу `scripts/dev/scenarios/rules-editor.mjs`: создаёт заметку с текстом артборда «Редактор» из макета, для каждого фрагмента образца проверяет классы `ts-syn-*` под ним, печатает расхождения и снимок (quickstart §2)
- [ ] T020 [US1] Проверка в браузере по quickstart §2 для разметки: снимок сравнить с артбордом «Редактор», консоль без исключений; расхождения исправить в `ui/src/styles/app.css` или `ui/src/editor/highlight/roles.ts`

**Checkpoint**: US1 проверяема отдельно — разметка раскрашена, текст не съезжает.

## Phase 4: User Story 2 — Код и формулы раскрашены по смыслу (P1)

**Goal**: ключевые слова, вызовы целиком, аргументы, литералы, формулы и их вложенность различимы.

**Independent Test**: образец с кодом и формулами и вложенностью трёх уровней; роли совпадают с таблицей (quickstart §2).

- [ ] T021 [P] [US2] Тесты кода в `packages/core/tests/common/highlight_cases.rs` и вызовы в обоих файлах прогона: ключевые слова AS 2.1, `#figure(image("a.png"), caption: [Схема])` (AS 2.2), пути `calc.pow`, `text.with`, `heading.where` целиком `Function` (AS 2.3), числа с единицами и `true`/`false`/`none`/`auto` как `Constant` (AS 2.4), строка с `\"` (AS 2.6), вложенный `/* /* */ */` (AS 2.7), вызов на нескольких строках (Edge Cases)
- [ ] T022 [P] [US2] Тесты формул в `packages/core/tests/common/highlight_cases.rs`: `$ a^2 + b^2 = c^2 $` и `$ sum_(i=1)^n #x $` — `MathDelimiter`, `MathIdent`, `MathOperator`, вставка `#x` как код (AS 2.5); цепочка разметка → код → разметка → формула → код из строки 30 артборда «Редактор»; две последние строки артборда «Было · стало» (SC-004)
- [ ] T023 [US2] Уточнения ролей кода в `packages/core/src/highlight/roles.rs`: `ArgName`, `Constant`, `MathIdent`, распространение `Function` на звенья `FieldAccess` перед вызовом, если `highlight_ident` красит не весь путь (research.md §4), пока T021–T022 не зелёные на обоих таргетах
- [ ] T024 [US2] Проверка в браузере по quickstart §2 для кода и формул: строки 2–5 и 22–31 артборда «Редактор» совпадают со снимком

**Checkpoint**: US1 и US2 вместе закрывают SC-001 и SC-004.

## Phase 5: User Story 3 — Раскраска поспевает за набором (P1)

**Goal**: ввод не ждёт ядра, незакрытая пара не перекрашивает заметку, после правки раскраска как при открытии заново.

**Independent Test**: заметка на 5 000 строк, набор в начале, середине и конце; набор `#figure(` и `/*` (quickstart §3, §4).

- [ ] T025 [P] [US3] Тесты устойчивости в `packages/core/tests/common/highlight_cases.rs`: незакрытые `(`, `"`, `$`, `*`, `_` в строке не меняют ролей следующего абзаца (AS 3.2, FR-006); незакрытый `/*` и raw-блок красят до конца текста (FR-006); после ошибки `#let x = (1, 2 ]` роли восстанавливаются со следующего абзаца (AS 3.4); строка в 100 000 символов не паникует и укладывается в один-два фрагмента
- [ ] T026 [US3] Режим `--perf` в `scripts/dev/scenarios/syntax-highlight.mjs`: заметка на 5 000 строк из повторённого образца, 200 нажатий в начале, середине и конце, 95-й перцентиль «нажатие → кадр с раскраской» и разница во времени открытия с подсветкой и без (quickstart §4)
- [ ] T027 [US3] Прогнать quickstart §3 и §4 в web; записать замеры в `specs/024-typst-syntax-highlighting/quickstart.md` (раздел «Результаты»); если SC-002 или SC-003 не выдержаны — сессия с `Source::edit` за той же формой ответа (research.md §2) в `packages/core/src/highlight/` отдельной задачей в этом файле
- [ ] T038 [US3] Сессия подсветки по итогам T027 (полный разбор на каждое нажатие — 20–25 мс, SC-002 не выдержан): `HighlightSession` на `Source::edit` и `HighlightEdit` в `packages/core/src/highlight/session.rs`, `walk::highlight_source`; тесты `session_matches_fresh_parse_after_edits` и `session_reports_desync` в `packages/core/tests/common/highlight_cases.rs` на обоих таргетах; синхронный `updateNow` на web в обновлении вида
- [ ] T028 [US3] Проверка на desktop по quickstart §4: сборка `tauri build --debug`, набор в середине заметки на 5 000 строк без видимой задержки и мигания; `Mod-/` ставит `// `

**Checkpoint**: SC-002, SC-003 и FR-007 подтверждены замером.

## Phase 6: User Story 4 — Одинаковые цвета в любой теме и оболочке (P2)

**Goal**: палитра из токенов читаема в любой теме; разделённый редактор раскрашен одинаково; одна раскраска во всех оболочках.

**Independent Test**: заметка-образец в обеих темах и в двух половинах разделённого редактора (quickstart §5).

- [ ] T029 [P] [US4] Тест паритета в `packages/core/tests/common/highlight_cases.rs`: упакованный ответ для текста артборда «Редактор» совпадает с эталонным массивом, записанным в тесте, на обоих таргетах (FR-010, SC-006)
- [ ] T030 [US4] Проверить контраст каждой роли по таблице research.md §7 скриптом сценария (вычисленный `getComputedStyle` цвет против `--steel-950` и `--steel-900`, порог SC-005) в `scripts/dev/scenarios/syntax-highlight.mjs`, ключ `--contrast`
- [ ] T031 [US4] Проверка quickstart §5 в web: смена темы при открытой заметке — раскраска и каретка на месте; разделённый редактор (014) с одной заметкой — обе половины раскрашены одинаково после правки в одной
- [ ] T032 [US4] Проверка quickstart §6: правила игнорирования (012) и не-Typst файлы без классов `ts-syn-*` (FR-012)

- [ ] T037 [US4] Проверка на Android по quickstart §4: APK (`nix develop -c pnpm --filter mobile tauri android build --debug`), набор в середине заметки на 5 000 строк без видимой задержки, раскраска образца совпадает с web (SC-002, SC-006); если устройства в сессии нет — задача остаётся открытой с причиной

**Checkpoint**: все истории проверены.

## Phase 7: Polish & Cross-Cutting

- [ ] T033 [P] Удалить из `ui/package.json` зависимость `@lezer/highlight`, если после T013 её больше никто не импортирует (`grep -rn "@lezer/highlight" ui/src`); иначе оставить
- [ ] T034 [P] Обновить doc-комментарий о раскраске в `ui/src/editor/CodeEditor.tsx` и раздел о редакторе в `README.md`: подсветка по правилам Typst из ядра
- [ ] T035 Гейты: `.specify/scripts/typsidian/gates.sh all` (fmt, сборка ядра под wasm32 и native, `cargo test`, `wasm-bindgen-test`, `pnpm typecheck`, `scripts/check-i18n.sh`)
- [ ] T036 Сверка цепочки ядро → биндинг → UI грепом: `SyntaxSession` создаётся в `vault-client-web.ts`, `syntax_session_open/update/close` вызываются из `vault-client-tauri.ts`, `typstHighlightFor(` — из `NoteEditor.tsx`

## Dependencies & Execution Order

- Phase 1 → Phase 2 → US1, US2, US3, US4.
- US1 и US2 правят один файл `roles.rs` (T018, T023) — последовательно; их тесты (T016, T017, T021, T022) пишутся параллельно.
- US3 опирается на расширение T012 и сценарий T019.
- US4 опирается на палитру T015 и сценарий T019.
- Polish — после всех историй.

## Parallel Example

```text
Phase 2:  T006 ∥ T008 ∥ T010 ∥ T011   (после T004–T005 и T007)
US1:      T016 ∥ T017                 → T018 → T019 → T020
US2:      T021 ∥ T022                 → T023 → T024
```

## Implementation Strategy

MVP — Phase 1, Phase 2 и US1: заметка раскрашена ядром, старой подсветки нет, разметка
различима. Затем US2 (код и формулы — основная часть ошибок старой подсветки), US3 (замер
и запасной вариант) и US4 (контраст, паритет, разделённый редактор).
