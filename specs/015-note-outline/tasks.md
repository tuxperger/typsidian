# Tasks: Оглавление заметки

**Input**: Design documents from `/specs/015-note-outline/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для логики ядра на обоих таргетах (принципы I и II конституции):
каждый сценарий — в `packages/core/tests/common/structure_cases.rs` и вызывается из обоих
раннеров, `structure_native.rs` и `structure_wasm.rs`. Для UI JS-тестов в репозитории нет —
сценарий headless-браузера и проверка по quickstart.md.

**Порядок реализации фич P2**: 013 → 014 → **015** → 016 → 017 → 018. Колонку заметки
`ui/src/note-panel/` заводит 013; команды `COMMANDS` дополняют 013 и 014 раньше.

## Phase 1: Setup

- [X] T001 Объявить `pub mod structure;` в `packages/core/src/lib.rs` и завести пустые `packages/core/src/structure/mod.rs`, `walk.rs`, `plain.rs`, `outline.rs`, `view.rs`, `web.rs` (последний под `cfg(target_arch = "wasm32")`); завести `packages/core/tests/common/structure_cases.rs` (с `pub mod structure_cases;` в `packages/core/tests/common/mod.rs`) и раннеры `packages/core/tests/structure_native.rs`, `packages/core/tests/structure_wasm.rs` по образцу `elements_native.rs` / `elements_wasm.rs`

## Phase 2: Foundational

**Purpose**: общий разбор текста (его же берут 018 и 017), команда и тексты — без них ни одна история не начинается.

- [X] T002 Сверить с реализованными 013 и 014 и записать результат в раздел «Сверка с 013 и 014» в конце `specs/015-note-outline/tasks.md`: имя компонента колонки и её переключателя режимов в `ui/src/note-panel/`, хук и место хранения её состояния, компонент шторки снизу для телефона, вид вызова перехода (`reveal` в `ui/src/workspace/Workspace.tsx` или обёртка 014), положение новых команд в `COMMANDS`; расхождения с plan.md и contracts/frontend-outline.md — перечислить, пути в задачах ниже читать с этой поправкой
- [X] T003 Типы `TextPlace`, `NoteStructure`, `HeadingSite`, `FigureSite`, `FigureKind`, `LabelSite`, `LabelOwner`, `RefSite` (serde + `Tsify` под wasm32) по data-model.md в `packages/core/src/structure/mod.rs`
- [X] T004 Читаемый текст разметки без оформления по research.md §6 (текст, пробелы, экранирование, сокращения, кавычки, `Strong`/`Emph`, `Raw`, `Link`, `Ref`, формула исходной записью, блоки содержимого вызовов; запасной вариант — начало исходной записи до 60 знаков с признаком `text_is_source`) в `packages/core/src/structure/plain.rs`
- [X] T005 Один обход дерева `typst::syntax::parse` и `pub fn parse(text) -> NoteStructure` по правилам 1–9 contracts/core-structure-api.md: заголовки `=` и `heading(…)` с уровнем по research.md §4, пропуск `Closure`/`ForLoop`/`WhileLoop`/`ShowRule`/`SetRule`/`ModuleImport`/`ModuleInclude`, `figure(…)` с видом и подписью, определения меток в разметке против употреблений в коде, привязка метки к предыдущему соседу, ссылки `Ref`, места через `typst::syntax::Lines` — в `packages/core/src/structure/walk.rs`, реэкспорт в `packages/core/src/structure/mod.rs`
- [X] T006 [P] Тесты разбора на обоих путях: `=` и `#heading(level: 2)[…]`, `#heading[…]` без уровня, уровни не подряд; `=` в блоке кода, комментарии и строке; заголовки в цикле, замыкании, `#show`, рядом с `#include`; заголовок в блоке содержимого аргумента и в ветке `#if`; читаемый текст (оформление, ссылка, формула, пустой остаток); рисунок и таблица с подписью и без, `kind: table`; метка при заголовке, при рисунке, отдельная; `<ключ>` в `#cite(<ключ>)` — не определение; ссылки `@имя` с дополнением; места на тексте с кириллицей и символами вне BMP; незакрытая скобка и обрыв записи без паники — в `packages/core/tests/common/structure_cases.rs`, вызовы в `packages/core/tests/structure_native.rs` и `packages/core/tests/structure_wasm.rs` per FR-002, FR-005
- [X] T007 Добавить `CommandId::ToggleOutline` (`"toggle-outline"`, `Mod+Shift+O`) в перечисление, `COMMANDS`, `as_str` и `default_shortcut` с комментарием о выборе клавиши (research.md §12) и обновить тесты уникальности умолчаний в `packages/core/src/settings/keybindings.rs`; случай на обоих путях (имя ↔ команда, умолчание, `problem()` его не отвергает) — в `packages/core/tests/common/vault_settings_cases.rs` per FR-013
- [X] T008 [P] Завести область переводов `ui/src/i18n/locales/en/outline.ts` (эталон) и `ui/src/i18n/locales/ru/outline.ts` и подключить в `ui/src/i18n/locales/en.ts` и `ui/src/i18n/locales/ru.ts`: название режима «Структура», название команды и кнопки «Оглавление», подпись и подсказка поля фильтра, пустые состояния (нет заголовков, ничего не найдено, доска, заметка слишком велика), приставки «рис.» / «табл.», пункты меню видов, подвал шторки, подписи для чтения с экрана per FR-016

**Checkpoint**: `structure::parse` готов и покрыт на обоих таргетах — 018 и 017 могут на него опираться.

## Phase 3: User Story 1 — Видеть строение заметки и переходить к разделу (P1) 🎯 MVP

**Goal**: в колонке заметки режим «Структура» показывает заголовки по уровням; выбор пункта ставит каретку на заголовок и доводит предпросмотр.

**Independent Test**: открыть заметку с заголовками трёх уровней — панель повторяет порядок и вложенность; клик по последнему — редактор показывает его, фокус в редакторе.

### Tests for User Story 1

- [X] T009 [P] [US1] Тесты оглавления: порядок и уровни, родитель при уровнях не подряд, уникальность ключей у одинаковых заголовков, место пункта — начало записи заголовка; вид: только заголовки по умолчанию, счётчики, `NoHeadings` на заметке без заголовков, оглавление по тексту с ошибкой разбора — в `packages/core/tests/common/structure_cases.rs` и обоих раннерах `packages/core/tests/structure_native.rs`, `packages/core/tests/structure_wasm.rs` per FR-001, FR-005, FR-012
- [X] T010 [P] [US1] Тест соответствия собранному документу: набор заметок с заголовками, записанными в тексте (оба вида записи, оформление, блок содержимого), собирается настоящим компилятором, уровни и тексты заголовков собранного документа сравниваются с `outline(text)`; заголовки документа тестам отдаёт новая функция под существующей feature `test-support` в `packages/core/src/preview/compile.rs` (запрос элементов заголовка у интроспектора; в обычную сборку не попадает) — сценарии в `packages/core/tests/common/structure_cases.rs` и обоих раннерах `packages/core/tests/structure_native.rs`, `packages/core/tests/structure_wasm.rs` per SC-005
- [X] T011 [P] [US1] Тесты `heading_probe` и `locate_heading`: один прогон, заголовок из нескольких прогонов (оформление, перенос строки), повтор текста в оглавлении документа обычным кеглем, два одинаковых заголовка и `occurrence`, отсутствие совпадения → `None`, заголовок с `text_is_source` → пробы нет; на прогонах, собранных руками, и на документе настоящего компилятора — в `packages/core/tests/common/structure_cases.rs` и обоих раннерах per FR-004

### Implementation for User Story 1

- [X] T012 [US1] `Outline`, `OutlineItem`, `OutlineKind`, `pub fn outline(text)` (пункты-заголовки, ключи, `parent`, `level`) и `pub fn heading_probe(outline, key)` по data-model.md и research.md §7 в `packages/core/src/structure/outline.rs`
- [X] T013 [US1] `OutlineKinds`, `OutlineRequest`, `OutlineView`, `OutlineRow`, `OutlineCounts`, `OutlineState` и `pub fn outline_view` в объёме шагов 1 и 6 контракта (заголовки, счётчики, `Ready` / `NoHeadings`; поля свёрнутости, фильтра и каретки приняты и пока не влияют) в `packages/core/src/structure/view.rs`
- [X] T014 [US1] `HeadingProbe` и `pub fn locate_heading(runs, probe)` по research.md §10 рядом с существующей `resolve` (её не менять) в `packages/core/src/preview/anchor.rs`
- [X] T015 [US1] wasm-биндинги `noteOutline(text)` и `noteOutlineView(outline, request)` в `packages/core/src/structure/web.rs` и `previewLocateHeading(runs, outline, key)` в `packages/core/src/preview/web.rs`
- [X] T016 [US1] Команды Tauri `structure_outline`, `structure_outline_view`, `preview_locate_heading` без состояния оболочки и их регистрация в `packages/shell/src/lib.rs`
- [X] T017 [US1] Методы `outline`, `outlineView` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts` и `locateHeading` в `ui/src/preview-client.ts` с обеими реализациями; типы — `import type` из `wasm-core`
- [X] T018 [US1] Хук `ui/src/note-panel/useOutline.ts` в объёме US1: `outline(text)` сразу при открытии и смене заметки, `outlineView`, отбрасывание ответа на другую заметку, ни одного вызова при выключенном режиме или скрытой колонке (contracts/frontend-outline.md § useOutline) per SC-001
- [X] T019 [US1] Строка пункта `ui/src/note-panel/OutlineRow.tsx` и режим `ui/src/note-panel/OutlineMode.tsx` по макету `Typsidian Оглавление.dc.html` 1a и 1c: дерево `role="tree"` со строками `treeitem` и `aria-level`, отступ по уровню без номеров разделов, обрезка длинного текста с полным в `title` и по удержанию, моноширинный вид для `text_is_source`, счётчик в шапке, пустые состояния (нет заголовков, доска, заметка слишком велика), клавиши ↑ ↓ Home End Enter Esc; стили `.ts-outline*` в `ui/src/styles/app.css` per FR-001, FR-012, FR-014
- [X] T020 [US1] Подключить режим «Структура» в переключатель колонки заметки 013 и в её шторку снизу для телефона (файл колонки в `ui/src/note-panel/` — по записи T002) и в `ui/src/workspace/Workspace.tsx`: сочетание `toggle-outline` через `useShortcuts` с поведением research.md §12, пункт палитры (только для открытой заметки Typst), кнопка «Оглавление» в шапке с `aria-pressed`; название команды — в `ui/src/settings/ShortcutsSection.tsx`; показана ли колонка и режим «Структура» переживают перезапуск через запись состояния колонки 013 per FR-013, FR-015
- [X] T021 [US1] Переход к пункту в `ui/src/workspace/Workspace.tsx`: `onJump(row)` ставит каретку механизмом `reveal` по `row.place.line` / `row.place.column`, показывает место и возвращает фокус в редактор, не дожидаясь предпросмотра; на телефоне закрывает шторку per FR-003, SC-002
- [X] T022 [US1] Запрос «показать заголовок» из `onJump` в `ui/src/workspace/Workspace.tsx` пропом в `ui/src/preview/PreviewPane.tsx`: при показанном предпросмотре с собранным документом — `locateHeading(pages.map(p => p.text_runs), outline, key)` и прокрутка существующим `useScrollAnchor.restore`; `null` — предпросмотр не двигается per FR-004
- [X] T023 [US1] Сценарий `scripts/dev/scenarios/outline.mjs` для `scripts/dev/browser.mjs --opfs-vault`: заметка с заголовками трёх уровней, открытие оглавления сочетанием `Ctrl+Shift+O` (проверка, что браузер отдаёт сочетание странице), текст колонки, клик по последнему пункту и положение каретки, заметка без заголовков, заметка с ошибкой сборки; снимок `--shot` сверить с макетом 1a и 1c, расхождения вёрстки исправить в `ui/src/styles/app.css`

**Checkpoint**: US1 работает самостоятельно — оглавление есть, переход есть.

## Phase 4: User Story 2 — Оглавление следует за текстом и кареткой (P2)

**Goal**: правка заголовков отражается без сохранения; раздел под кареткой подсвечен и виден в панели.

**Independent Test**: дописать заголовок — он появляется в панели; каретку в середину третьего раздела — подсвечен третий пункт.

### Tests for User Story 2

- [X] T024 [P] [US2] Тесты текущего раздела: каретка выше первого заголовка → нет, в начале строки заголовка → он, в середине раздела, в конце текста, за концом текста; `caret: None` → нет; ответ `outline_active` совпадает с `outline_view(...).active` — в `packages/core/tests/common/structure_cases.rs` и обоих раннерах `packages/core/tests/structure_native.rs`, `packages/core/tests/structure_wasm.rs` per FR-007

### Implementation for User Story 2

- [X] T025 [US2] `pub fn outline_active(outline, request)` и поле `active` в `outline_view` по research.md §8 в `packages/core/src/structure/view.rs`
- [X] T026 [US2] Биндинг `noteOutlineActive` в `packages/core/src/structure/web.rs`, команда `structure_outline_active` в `packages/shell/src/lib.rs`, метод `outlineActive` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts`
- [X] T027 [US2] Отдать положение каретки (UTF-16) из `ui/src/editor/NoteEditor.tsx` наружу пропом-обработчиком и довести до колонки в `ui/src/workspace/Workspace.tsx`; слушатель обновления редактора только запоминает значение
- [X] T028 [US2] В `ui/src/note-panel/useOutline.ts`: разбор через 250 мс после последней правки, `outlineActive` не чаще раза в 100 мс по заднему фронту, запуск только из таймера (не из обработчика нажатия), отбрасывание ответа на устаревший текст по номеру версии per FR-006, SC-003, SC-004
- [X] T029 [US2] В `ui/src/note-panel/OutlineMode.tsx` и `ui/src/note-panel/OutlineRow.tsx`: подсветка строки по макету 1a (фон steel-900, текст steel-100, оранжевая линия слева 2px), `aria-current`, прокрутка подсвеченной строки в видимую область при смене подсветки; смена подсветки перерисовывает только две строки (мемоизация по ключу) per FR-007
- [X] T030 [US2] Дополнить `scripts/dev/scenarios/outline.mjs`: дописать заголовок и сменить его уровень — колонка меняется без сохранения не позже 0,5 с; каретку в третий раздел — строка с `aria-current`; каретку выше первого заголовка — подсветки нет; переход к другой заметке — оглавление её

**Checkpoint**: US1 и US2 работают вместе.

## Phase 5: User Story 3 — Свернуть разделы и отфильтровать (P3)

**Goal**: ветки сворачиваются и переживают правки; фильтр оставляет подходящие пункты с родителями; в узком окне колонка подаётся шторкой с фильтром.

**Independent Test**: в заметке со 100 заголовками свернуть первый раздел — подзаголовки скрыты; набрать «введ» — остались пункты со словом «Введение».

### Tests for User Story 3

- [X] T031 [P] [US3] Тесты свёрнутости и фильтра: потомки свёрнутого скрыты, неизвестный ключ игнорируется; фильтр без учёта регистра на кириллице, подходящий пункт с цепочкой предков, предок `dimmed`, разметка совпадения в `segments`, фильтр из пробелов = пустой, свёрнутость не действует при фильтре и возвращается после очистки, `NoMatches`; `parent` строки — ближайший видимый предок; текущий раздел в свёрнутой ветке и под фильтром — ближайший видимый предок либо нет — в `packages/core/tests/common/structure_cases.rs` и обоих раннерах `packages/core/tests/structure_native.rs`, `packages/core/tests/structure_wasm.rs` per FR-008, FR-009
- [X] T032 [P] [US3] Тесты `carry_collapsed`: правка текста внутри раздела, переименование свёрнутого заголовка, смена его уровня, вставка и удаление заголовка, результат без чужих ключей и повторов — в `packages/core/tests/common/structure_cases.rs` и обоих раннерах per FR-008

### Implementation for User Story 3

- [X] T033 [US3] Шаги 2–5 контракта `outline_view` (свёрнутость, фильтр с предками, `dimmed`, `segments` на `search::LineSegment`, `has_children`, `collapsed`, видимый `parent`, `NoMatches`) и учёт скрытых строк в `outline_active` в `packages/core/src/structure/view.rs`
- [X] T034 [US3] `pub fn carry_collapsed(prev, next, collapsed)` по research.md §7 в `packages/core/src/structure/outline.rs`
- [X] T035 [US3] Биндинг `noteOutlineCarry` в `packages/core/src/structure/web.rs`, команда `structure_outline_carry` в `packages/shell/src/lib.rs`, метод `outlineCarry` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts`
- [X] T036 [US3] В `ui/src/note-panel/useOutline.ts`: действия `toggle` и `setFilter`, свёрнутые ключи и текст фильтра в памяти окна по пути заметки, пока она открыта; `outlineCarry` между прежним и новым оглавлением при каждой правке текста per FR-008
- [X] T037 [US3] В `ui/src/note-panel/OutlineMode.tsx` и `ui/src/note-panel/OutlineRow.tsx`: шеврон и `aria-expanded` у строк с вложенными, поле фильтра под шапкой (Esc очищает, на пустом — фокус в редактор; ↓ — к первой строке; Enter — переход к первой подходящей), подсветка совпадения `rgba(255,106,0,.35)` и приглушение `dimmed` по макету 1b, клавиши ← → по `row.parent`, состояние «ничего не найдено»; стили в `ui/src/styles/app.css` per FR-009, FR-014
- [X] T038 [US3] Боковая шторка колонки `ui/src/note-panel/NotePanelDrawer.tsx` по макету 1b для ширины `.ts-body` от 720px до 1100px: панель 280px справа на скриме, шапка с названием режима и меткой `esc`, поле фильтра, подвал с подсказками клавиш, фокус в фильтре при открытии, закрытие по Esc, скриму и после перехода с возвратом фокуса в редактор (`ui/src/workspace/useDismissOnEscape.ts`); подключение в `ui/src/workspace/Workspace.tsx`, стили `.ts-notepanel-drawer*` в `ui/src/styles/app.css` (research.md §11)
- [X] T039 [US3] Дополнить `scripts/dev/scenarios/outline.mjs`: свернуть раздел, дописать текст — свёрнутость на месте; каретку в свёрнутый подраздел — подсвечен родитель; фильтр «введ»; узкое окно — шторка, переход закрывает её; снимок сверить с макетом 1b

**Checkpoint**: US1–US3 работают вместе.

## Phase 6: User Story 4 — Рисунки, таблицы и метки в оглавлении (P3)

**Goal**: по желанию под разделами показываются рисунки и таблицы по подписи и метки по имени; выбор сохраняется между запусками.

**Independent Test**: в заметке с двумя рисунками с подписями и меткой `<итог>` включить показ — в оглавлении три дополнительных пункта на своих местах.

### Tests for User Story 4

- [X] T040 [P] [US4] Тесты видов: по умолчанию только заголовки; рисунок и таблица с подписью под своим разделом с уровнем раздела + 1, до первого заголовка — без родителя; рисунок без подписи пунктом не становится, а его метка — пункт-метка; метка сразу за заголовком или рисунком показана при нём и отдельным пунктом не идёт; отдельная метка — пункт; `label` у строки только при включённых метках; `has_children` и счётчики по включённым видам; рисунки и метки не бывают текущим разделом; фильтр по имени метки — в `packages/core/tests/common/structure_cases.rs` и обоих раннерах `packages/core/tests/structure_native.rs`, `packages/core/tests/structure_wasm.rs` per FR-010, FR-011

### Implementation for User Story 4

- [X] T041 [US4] Пункты `Figure`, `Table`, `Label` в `outline()` по правилам data-model.md § OutlineItem в `packages/core/src/structure/outline.rs` и отбор по `OutlineKinds`, показ `label`, `has_children` и счётчики по включённым видам в `packages/core/src/structure/view.rs`
- [X] T042 [US4] Поля `figures` и `labels` (по умолчанию выключены) в записи состояния колонки заметки 013 (файл хука состояния в `ui/src/note-panel/` — по записи T002) и их передача в `ui/src/note-panel/useOutline.ts`; выбор переживает перезапуск per FR-010
- [X] T043 [US4] В `ui/src/note-panel/OutlineMode.tsx` и `ui/src/note-panel/OutlineRow.tsx`: кнопка-меню видов в шапке режима на `ui/src/menu/ContextMenu.tsx` («Рисунки и таблицы», «Метки»), моноширинные строки рисунка, таблицы и метки с приставкой из переводов и `<меткой>`, метка при заголовке, счётчик в шапке по включённым видам; стили в `ui/src/styles/app.css` per FR-011
- [X] T044 [US4] Дополнить `scripts/dev/scenarios/outline.mjs`: два рисунка с подписями и метка `<итог>`, включение видов через меню — три дополнительных пункта под своими разделами, перезагрузка страницы — выбор сохранён, выключение — только заголовки

**Checkpoint**: все четыре истории работают.

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T045 [P] Бенчмарк `packages/core/benches/structure_scale.rs` (с записью `[[bench]]` в `packages/core/Cargo.toml`, `harness = false`): `outline`, `outline_view`, `outline_active` на заметке в 10 000 строк с 2 000 заголовков; бюджеты research.md §9 (≤20 мс, ≤15 мс, ≤8 мс native)
- [X] T046 Замеры в браузере в `scripts/dev/scenarios/outline.mjs`: время от открытия заметки в 10 000 строк до оглавления (SC-001, ≤200 мс), от клика до каретки на месте (SC-002, ≤200 мс), от остановки набора до правки в колонке (SC-003, ≤0,5 с), время `noteOutline` и `noteOutlineActive` в wasm при 2 000 заголовков (≤50 мс и ≤8 мс), задержка отклика на нажатие с открытым оглавлением и без него (SC-004, разница ≤10 %; метод — время от отправки нажатия до следующего кадра, медиана по 200 нажатиям подряд с интервалом 50 мс); результаты — в раздел «Результаты проверки» в конце `specs/015-note-outline/tasks.md`; превышение порога — условие пересмотра из plan.md § Complexity Tracking, решение остаётся за пользователем
- [ ] T047 Гейты качества: `.specify/scripts/typsidian/gates.sh all` и `scripts/check-i18n.sh` — fmt, оба таргета ядра, `cargo test`, `wasm-bindgen-test`, `cargo test -p typsidian-shell`, типы UI; результаты — в «Результаты проверки» в `specs/015-note-outline/tasks.md` per FR-015, Definition of done конституции
- [ ] T048 Итоговая сверка вёрстки снимками `scripts/dev/browser.mjs --shot` с макетом `Typsidian Оглавление.dc.html` 1a, 1b, 1c с учётом расхождений research.md §13–§19; правки — в `ui/src/styles/app.css`
- [ ] T049 Ручная проверка по `specs/015-note-outline/quickstart.md` на web, в desktop-сборке и на Android (шторка снизу, закрытие после перехода); iOS записать как «не проверено — оболочки нет»; результаты — в «Результаты проверки» в `specs/015-note-outline/tasks.md`

## Dependencies

- Setup (T001) → Foundational (T002–T008) → US1 → US2 → US3 → US4 → Polish.
- **Снаружи**: 013 реализована (колонка `ui/src/note-panel/`, её состояние, шторка снизу) —
  без неё T020, T038, T042 начать нельзя; T002 это фиксирует. 014 меняет `Workspace.tsx` и
  может обернуть `reveal` — T021 идёт через обёртку, если она есть.
- T005 зависит от T003 и T004. T012 и T013 — от T005. T014 — от T012 (`HeadingProbe`).
- US2 зависит от US1 (T013, T018, T019). US3 — от US2 (T025: `outline_active` учитывает
  скрытые строки). US4 зависит от US1 и не зависит от US2 и US3 по ядру, но правит те же
  `view.rs` и `OutlineMode.tsx` — параллельно с ними не идёт.
- Сторонние потребители: 018 и 017 зависят только от Foundational (T003–T006).

## Parallel Opportunities

- Foundational: T006 (тесты) параллелен T007 и T008 — разные файлы.
- US1: T009, T010, T011 — один файл сценариев, пишутся подряд, но параллельны реализации
  T015–T022; T014 параллелен T013.
- US3: T031 и T032 параллельны T035–T038.
- Polish: T045 параллелен T046–T049.

## Implementation Strategy

MVP — фазы 1–3: оглавление заголовков с переходом на всех клиентах. Дальше по одной истории:
US2 (следование за текстом и кареткой), US3 (свёрнутость, фильтр, шторка узкого окна), US4
(рисунки, таблицы, метки). Коммит — фича целиком (конвенции репозитория). Тестовые задачи
каждой истории пишутся до её реализации и до неё не проходят.

## Сверка с 013 и 014

Записано по T002 (2026-10-18), по коду на момент реализации UI.

- **Колонка** — `ui/src/note-panel/NotePanel.tsx` (`presentation: "column" | "sheet"`), режимы —
  реестр `NotePanelModeSpec[]` (`ui/src/note-panel/modes.ts`), собирает его `Workspace`
  (`noteModes`). «Структура» — вариант `NotePanelMode::Outline` ядра (`"outline"`), он уже был.
- **Состояние колонки** — `ui/src/note-panel/useNotePanelState.ts`, хранится в настройках
  устройства (`Settings.note_panel`). Поле `outline_kinds` в ядре было, а хук его не читал и
  при записи терял — исправлено в T042.
- **Шторка снизу** — та же `NotePanel` с `presentation="sheet"` при `max-width: 720px`
  (`useNarrowViewport`); открыта ли она — память окна.
- **Переход** — `revealPlace` в `Workspace.tsx` (открывает вкладку 014 с местом, закрывает
  шторку). Обёртки истории 014 поверх него нет — переход идёт через `revealPlace`.
- **Каретка наружу** — проп `onCaretOffset` у `NoteEditor` (только активная вкладка), через
  `SharedEditorProps` группы вкладок 014.
- **Команда** — `ToggleOutline` уже была в ядре и в `ShortcutsSection` (название —
  `notePanel.outlineCommand`, отдельного ключа `outline.command` нет).

Расхождения с plan.md и contracts/frontend-outline.md:

- Боковая шторка (`NotePanelDrawer`) меряет ширину `.ts-body` наблюдателем размера, а не
  медиазапросом окна: ширину колонке оставляют боковая панель и предпросмотр.
- Кнопка меню видов стоит в строке фильтра, а не в шапке колонки: шапка 013 общая для всех
  режимов и своих кнопок у режима не принимает.
- Отмечаемые пункты меню видов — новое поле `checked` у `ContextMenuItem`
  (`role="menuitemcheckbox"`).
- В шапке колонки четыре режима не помещались в 248px и наезжали на счётчик — переключатель
  режимов прокручивается вбок.

## Результаты проверки

**2026-10-18, web (headless chromium, `scripts/dev/scenarios/outline.mjs --opfs-vault`)** —
22 проверки сценария зелёные: сочетание `Ctrl+Shift+O` браузер отдаёт странице; порядок и
уровни; переход и фокус в редакторе; подсветка текущего раздела; правка без сохранения;
свёрнутость переживает правку; фильтр с родителями и подсветкой; Esc очищает фильтр; рисунки,
таблицы и метки под своими разделами; виды и режим переживают перезагрузку; пустые состояния;
оглавление при ошибке сборки; боковая шторка на 1000px с фокусом в фильтре, закрытие после
перехода.

**Замеры (T046, `OUTLINE_PERF=1`, 10 000 строк, 2 000 заголовков, в странице):**

| Что | Итог | Порог |
|---|---|---|
| SC-001 оглавление после открытия | 141 мс | ≤200 мс |
| SC-002 каретка на месте после щелчка | 113 мс | ≤200 мс |
| SC-003 правка видна после остановки набора | 479 мс | ≤500 мс |
| SC-004 задержка нажатия с оглавлением / без | 10,7 / 10,0 мс (×1,07) | ≤10 % |
| `noteOutline` в wasm (медиана / макс.) | 6,1 / 8,8 мс | ≤50 мс |
| `noteOutlineView` в wasm | 6,1 / 9,2 мс | — |
| `noteOutlineActive` в wasm | 3,1 / 4,2 мс | ≤8 мс |
| FR-004 предпросмотр прокручен к «Раздел 1500» | 0 → 48 699 px | — |

Native-бенчмарк (T045): `outline` 5,42 мс, `outline_view` 1,01 мс, `outline_active` 1,00 мс.
Условия пересмотра из plan.md § Complexity Tracking не сработали. Запас SC-003 мал: из 479 мс
250 — пауза, разбор в wasm — около 6 мс, остальное — отрисовка 2 000 строк в React.

**Ядро** (сверка агентом): `structure_native` и `structure_wasm` — 27 сценариев зелёные;
`settings_native`/`settings_wasm` 63; `cargo test -p typsidian-core` и
`cargo test -p typsidian-shell` зелёные; `cargo fmt --all --check` чисто. Типы UI
(`pnpm --filter @typsidian/ui typecheck`) и `scripts/check-i18n.sh` — чисто.

**Открыто:**

- T047 — полный `gates.sh all` не прогнан: сборка wasm в гейте упала в Oops ядра (cargo умер
  в `d_splice_alias_ops`, держа блокировку каталога), следующий cargo завис в ядре
  неубиваемым до перезагрузки. Отдельно — `cargo clippy -D warnings` красный от 21 замечания
  в чужих файлах после обновления до clippy 1.99 (`structure/*` среди них нет).
- T048 — Claude Design в сессии недоступен (403), локальной копии макета нет: вёрстка
  сделана по числам contracts/frontend-outline.md, сверки снимков с 1a–1c не было.
- T049 — desktop и Android не проверялись (окно рабочего стола в сессии не открыть,
  устройства нет); iOS — не проверено, оболочки нет.
