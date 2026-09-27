---

description: "Task list for Element Parameters"
---

# Tasks: Element Parameters

**Input**: Design documents from `/specs/008-element-parameters/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md),
[data-model.md](./data-model.md), [contracts/element-params.md](./contracts/element-params.md),
[quickstart.md](./quickstart.md)

**Tests**: включены обязательно. Принцип I конституции требует, чтобы каждая единица логики
ядра проходила и `cargo test`, и `wasm-bindgen-test`; логика без тестов на обоих путях не
считается готовой. Сценарии — таблица из contracts § «Проверяемые сценарии». JS-тестов в
репозитории нет: UI проверяется стендом (chromium headless + esbuild из `ui/dist`).

**Organization**: задачи сгруппированы по user story; каждую можно реализовать и проверить
отдельно.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: можно выполнять параллельно (разные файлы, нет незакрытых зависимостей)
- **[Story]**: к какой user story относится задача (US1…US5)

## Path Conventions

Структура из `plan.md` → Project Structure: ядро `packages/core/src/elements/`, тесты ядра
`packages/core/tests/`, команды Tauri `packages/shell/src/lib.rs`, общий UI `ui/src/`.

---

## Phase 1: Setup

- [X] T001 Завести модуль `packages/core/src/elements/mod.rs` с подмодулями `schema`, `locate`,
      `values`, `render`, `edit` (пустые) и подключить его в `packages/core/src/lib.rs`
- [X] T002 [P] Завести пары тестов `packages/core/tests/elements_native.rs`
      (`#![cfg(not(target_arch = "wasm32"))]`, tokio) и `packages/core/tests/elements_wasm.rs`
      (`wasm_bindgen_test_configure!(run_in_browser)`) над общей таблицей
      `packages/core/tests/common/element_cases.rs`, подключённой в
      `packages/core/tests/common/mod.rs` — по образцу `install_native.rs`/`install_wasm.rs`

---

## Phase 2: Foundational (блокирует все истории)

**Purpose**: схема параметров, её сверка с Typst, разбор значений и граница клиентов — без
них ни вставка, ни правка не работают.

- [X] T003 Описать типы схемы `ElementSpec`, `ElementSyntax` (`call`, `set`, `markup-heading`,
      `markup-raw`, `markup-equation`), `LabelSpec`, `ParamSpec`, `ParamKind` (data-model §
      «Схема параметров») с serde и `tsify` под wasm в `packages/core/src/elements/schema.rs`
- [X] T004 Загрузка схемы: `include_str!("schema.json")`, разбор один раз (`OnceLock`),
      `element_schema(id) -> Option<ElementSpec>` в `packages/core/src/elements/schema.rs`
- [X] T005 Функция Typst по пути (`figure`, `math.frac`, `polygon.regular`) из
      `<Library as LibraryExt>::default().global.scope()` и перевод `NativeParamInfo`
      (`CastInfo` → `ParamKind`, `default()` → запись Typst через `repr`) в
      `packages/core/src/elements/schema.rs` (research §1)
- [X] T006 Генератор машинной части схемы `packages/core/examples/element-schema.rs`: читает
      `schema.json`, для каждого элемента дописывает недостающие параметры из Typst с
      `title: ""`, `main: false`, сохраняя уже заполненные подписи; флаг `--check` только
      печатает расхождения
- [X] T007 Первая запись схемы: элементы `figure`, `image`, `table` — все параметры Typst,
      русские подписи, основные параметры по нынешнему диалогу (содержимое, подпись,
      размещение, ширина, колонки) в `packages/core/src/elements/schema.json`
- [X] T008 Тест схемы против Typst (SC-001a): для каждого `call`/`set`-элемента набор имён,
      `kind`, `default`, `positional`, `required`, `settable` совпадает с `Func::params()`;
      `title` непуст; у элемента с параметрами есть `main`; `id` уникальны — в
      `packages/core/src/elements/schema.rs` (`#[cfg(test)]`) и случаем в
      `packages/core/tests/common/element_cases.rs`
- [X] T009 Разбор значения узла дерева Typst в `ParamValue`: литерал по `ParamKind` либо
      выражение как есть (research §5), проверка значения поля формы → `FieldError`
      (FR-015) в `packages/core/src/elements/values.rs`
- [X] T010 [P] Случаи для `values.rs`: `2cm`, `80%`, `1fr`, `"str"` с экранированием,
      `[a (b) [c]]`, `center`, `true`, `auto`, `none` — литералы; `100% - 2cm`, `luma(240)`,
      `n * 2` — выражения; неверные значения полей — ошибки — в
      `packages/core/tests/common/element_cases.rs` + пары
- [X] T011 Смещения UTF-16 ↔ байты для текста заметки: переиспользовать или вынести
      общий помощник из `packages/core/src/templates/mod.rs` в
      `packages/core/src/elements/mod.rs`
- [X] T012 Объявить четыре функции контракта на wasm-границе статическими методами
      `WebBackend`: `elementSchema`, `elementAt`, `renderElement`, `planElementEdit` в
      `packages/core/src/vault/web.rs` (пока `element_at`/`render`/`plan` возвращают
      заглушки `None`/ошибку — реализуются в историях)
- [X] T013 [P] Те же четыре команды Tauri `element_schema`, `element_at`, `render_element`,
      `plan_element_edit` и их регистрация в `invoke_handler` в `packages/shell/src/lib.rs`
- [X] T014 [P] Методы `elementSchema`, `elementAt`, `renderElement`, `planElementEdit` в
      интерфейсе `ui/src/vault-client.ts` и реализациях `ui/src/vault-client-web.ts`,
      `ui/src/vault-client-tauri.ts` (типы — из `ui/src/wasm-core/typsidian_core.js`)

**Checkpoint**: схема читается, сверена с Typst на обоих таргетах, функции видны всем
клиентам.

---

## Phase 3: US1 — Вставка элемента с параметрами через форму (P1) 🎯 MVP

**Goal**: элемент с параметрами открывает форму с именованными полями; «Будет вставлено»
пересобирается на каждое изменение; вставляется ровно показанное.

**Independent Test**: выбрать элемент с параметрами, заполнить два-три поля, текст в заметке
совпадает с «Будет вставлено» и собирается компилятором.

### Tests for US1

- [X] T015 [P] [US1] Случаи `render_element`: умолчания не пишутся (FR-013), обязательное
      пустое → `FieldError`, метка `<fig:…>` дописывается, занятая метка → ошибка,
      `caret` на первом пустом месте, результат собирается `compile_note` без ошибок
      синтаксиса (SC-007) — в `packages/core/tests/common/element_cases.rs` + пары

### Implementation for US1

- [X] T016 [US1] `render_element(id, values, label)` для `call` и `set`: позиционные в
      порядке схемы, именованные в порядке схемы, тело `[…]` последним, метка после
      элемента в `packages/core/src/elements/render.rs`
- [X] T017 [US1] `render_element` для `markup-heading`, `markup-raw`, `markup-equation`
      (профиль разметки, research §7) в `packages/core/src/elements/render.rs`
- [X] T018 [US1] Реализовать `renderElement`/`render_element` на границах вместо заглушек в
      `packages/core/src/vault/web.rs` и `packages/shell/src/lib.rs`
- [X] T019 [US1] Общая форма `ui/src/editor/ParamForm.tsx`: поля по `ParamSpec.kind`
      (выбор, переключатель, число, число+единица, строка, текст, «авто/нет»-обёртки, поле
      «как есть» для `expression`), основные сверху, «Все параметры» свёрнуты (FR-012b),
      ошибки по полям и запертое «Применить» (FR-015), «Будет вставлено» через
      `renderElement` на каждое изменение (FR-004)
- [X] T020 [P] [US1] Стили формы (поля, блок «Все параметры», «Будет вставлено», ошибки) в
      `ui/src/styles/app.css` — только классы `.ts-*`
- [X] T021 [US1] Вставка через форму в `ui/src/editor/NoteEditor.tsx`: элемент со схемой
      (`opensFormOnInsert(schema)` из contracts § «Когда форма») открывает `ParamForm` в режиме `insert`, применение
      вставляет `RenderedElement.text` и ставит каретку в `caret`; без схемы — сниппет, как
      сейчас (FR-003); здесь же удалить `ui/src/editor/BlockParamsDialog.tsx` и все его
      вызовы — рисунок и таблица идут через `ParamForm` (иначе UI не соберётся после T022)
- [X] T022 [US1] Маршрут выбора в `ui/src/editor/InsertMenu.tsx` и `ui/src/editor/InsertPanel.tsx`:
      заменить `opensParams` на запрос схемы; удалить из `ui/src/editor/typstElements.ts`
      тип `BlockParamsKind` и поле `opensParams`
- [X] T023 [US1] Наполнить схему: заголовки, `outline`, `title`, `divider`, списки, `enum`/`list`
      `#set`, `terms` в `packages/core/src/elements/schema.json` (все параметры, подписи,
      основные)
- [X] T024 [US1] Наполнить схему: математика — `math.equation` `#set`, `frac`, `sqrt`, `sum`,
      `integral`, `mat`, `vec`, `cases`, `binom`, `attach`, `accent`, `underbrace`, `cancel`,
      `lr`, `op`, `bb` в `packages/core/src/elements/schema.json`
- [X] T025 [US1] Наполнить схему: кодовый блок (`raw`), `quote`, `block`, `columns`, `grid`,
      `stack`, `align`, `pad`, `place`, `box`, `h`, `v`, `repeat`, `hide`, `move`, `rotate`,
      `scale`, `colbreak`, `pagebreak`, `parbreak` в `packages/core/src/elements/schema.json`
- [X] T026 [US1] Наполнить схему: фигуры — `rect`, `square`, `circle`, `ellipse`, `line`,
      `polygon.regular`, `image`; `#set`-элементы `page`, `text`, `par`, `heading`,
      `document` в `packages/core/src/elements/schema.json`
- [X] T027 [US1] Круг «вставка → разбор» для каждого элемента схемы:
      `render_element` с умолчаниями и с заполненными основными → `compile_note` без
      синтаксических ошибок — случай в `packages/core/tests/common/element_cases.rs` + пары

**Checkpoint**: любой элемент с параметрами вставляется через форму; рисунок и таблица —
через ту же форму.

---

## Phase 4: US2 — Правка параметров вставленного элемента (P1)

**Goal**: каретка внутри элемента → «Параметры» → форма заполнена значениями из текста →
применение переписывает элемент на месте одним шагом отмены.

**Independent Test**: поставить каретку в ранее вставленный элемент, изменить параметр,
текст переписан на месте, одна отмена возвращает прежний вид.

### Tests for US2

- [X] T028 [P] [US2] Случаи `element_at`: элемент найден по каретке внутри, на `#` и после
      `)`/`]`/метки; значения прочитаны; незаполненные подстановки старого сниппета читаются
      как обычные значения (research §11); кириллица до элемента — смещения UTF-16 верны;
      каретка вне элементов → `None` — в `packages/core/tests/common/element_cases.rs` + пары
- [X] T029 [P] [US2] Случаи `plan_element_edit`: без изменений → `[]` для **каждого**
      элемента схемы (FR-009, SC-003); смена значения → одна замена его диапазона;
      новый параметр → вставка в конец аргументов; сброс к умолчанию → удаление с
      запятой; метка добавлена/изменена/снята; текст элемента изменился → `Stale`
      (FR-020) — в `packages/core/tests/common/element_cases.rs` + пары

### Implementation for US2

- [X] T030 [US2] `element_at(text, offset)`: `typst::syntax::parse`, `LinkedNode::leaf_at`,
      подъём до ближайшего `FuncCall`/`SetRule`/`Heading`/`Raw`/`Equation`, известного
      схеме; `ParsedElement` с `values`, `label`, `taken_labels`, `revision` в
      `packages/core/src/elements/locate.rs`
- [X] T031 [US2] `plan_element_edit`: сверка `revision` (BLAKE3 текста диапазона `from..to`), точечные `TextEdit` по
      `ParamValue.span`, вставка/удаление именованных аргументов с запятыми, правка
      метки в `packages/core/src/elements/edit.rs`
- [X] T032 [US2] Реализовать `elementAt`/`planElementEdit` на границах вместо заглушек в
      `packages/core/src/vault/web.rs` и `packages/shell/src/lib.rs`
- [X] T033 [US2] Хук доступности `ui/src/editor/useElementAtCaret.ts`: `elementAt` после
      паузы каретки 150 мс (research §3), результат `None | Found | Unavailable`
- [X] T034 [US2] Режим правки `ParamForm` в `ui/src/editor/NoteEditor.tsx`: форма
      заполнена `ParsedElement.values` (выражения — полем «как есть»), «Все параметры»
      раскрыт, если задан не-основной (FR-012b); применение — `planElementEdit` и все
      замены одной транзакцией CodeMirror (FR-010); пустой список — форма закрывается без
      транзакции (FR-009)
- [X] T035 [US2] Запреты записи (FR-019): «Применить» заперто при недоступном на запись vault
      и неразрешённом конфликте, причина названа; `Stale` — расхождение названо, текст не
      тронут (FR-020) в `ui/src/editor/ParamForm.tsx`
- [X] T036 [US2] Кнопка «Параметры» на панели вставки по `useElementAtCaret` вместо нынешней
      правки блока в `ui/src/editor/InsertPanel.tsx`
- [X] T037 [US2] Пункт «Параметры…» контекстного меню редактора — через `elementAt` по
      позиции клика; недоступен — с причиной в подписи в `ui/src/editor/editorContextMenu.ts`
      и `ui/src/editor/NoteEditor.tsx`
- [X] T038 [US2] Удалить регулярный разбор из UI — `BlockParams`, `blockAtOffset`,
      `blockTemplate`, `labelsInText`, `suggestedLabel` из `ui/src/editor/typstElements.ts`;
      правка блока на панели вставки уже идёт через `elementAt` (T036) (принцип I, research §10)

**Checkpoint**: US1 + US2 — полный цикл «вставить — поправить позже» на всех клиентах.

---

## Phase 5: US3 — Отказ от правки вместо молчаливой потери (P1)

**Goal**: непонятое сохраняется символ в символ; неразбираемое — правка не предлагается,
причина названа.

**Independent Test**: дописать в элемент незнакомый аргумент и выражение — после правки другого
поля оба на месте; повторить параметр — «Параметры» недоступны с причиной.

### Tests for US3

- [X] T039 [P] [US3] Случаи: незнакомый аргумент `foo: 1` сохраняется при правке соседнего
      (FR-012); выражение `n * 2` нетронутым не даёт замены, заменённое значением — даёт
      (FR-012a); повтор параметра → `Unavailable(DuplicateParam)`; незакрытая скобка/строка →
      `Unavailable(Unclosed)`; `..args` → `Unavailable(SpreadArgs)`; лишний позиционный →
      `Unavailable(UnknownPositional)`; комментарий внутри аргументов не тронут — в
      `packages/core/tests/common/element_cases.rs` + пары

### Implementation for US3

- [X] T040 [US3] Причины отказа `UnavailableReason` и их распознавание по дереву
      (`SyntaxKind::Error` в поддереве, повтор `Named`, `Spread`, лишние позиционные) в
      `packages/core/src/elements/locate.rs`
- [X] T041 [US3] Гарантия «незнакомое не адресуется»: утверждение в `plan_element_edit`, что
      ни одна замена не пересекает незнакомый аргумент или комментарий, в
      `packages/core/src/elements/edit.rs`
- [X] T042 [US3] Причина недоступности словами во всех местах вызова (панель, контекстное
      меню, палитра) — общий `describeUnavailable` в `ui/src/editor/useElementAtCaret.ts`

---

## Phase 6: US4 — Форма целиком с клавиатуры (P2)

**Goal**: вызвать, пройти, применить и отменить без мыши.

**Independent Test**: пройти сценарии US1 и US2, не касаясь мыши.

- [X] T043 [US4] Команда `CommandId::ElementParams` («Параметры элемента») с умолчанием
      `Mod+Shift+E` (research §9) и тест на отсутствие конфликта умолчаний в
      `packages/core/src/settings/keybindings.rs`
- [X] T044 [US4] Обработка сочетания и команда палитры «Параметры элемента» с причиной
      недоступности в `ui/src/workspace/Workspace.tsx` и `ui/src/editor/NoteEditor.tsx`
- [X] T045 [US4] Клавиатура в `ui/src/editor/ParamForm.tsx`: фокус в первом поле при
      открытии, ⌘↵ — применить, Escape — закрыть без изменений и вернуть каретку на место
      (FR-017, FR-018)

---

## Phase 7: US5 — Вложенные элементы (P3)

**Goal**: параметры открываются у элемента, непосредственно содержащего каретку.

**Independent Test**: вложить элемент в элемент, проверить обе позиции каретки.

- [X] T046 [P] [US5] Случаи: `#columns(2)[ #figure(image("a.png")) ]` — каретка в `image(…)`
      → `image`, в `#figure(` → `figure`, в тексте колонок → `columns`; вложенная правка
      внутреннего не меняет внешний — в `packages/core/tests/common/element_cases.rs` + пары
- [X] T047 [US5] Выбор ближайшего предка и правило границы для вложенных элементов в
      `packages/core/src/elements/locate.rs`

---

## Phase 8: Polish

- [X] T048 Пересборки гейта IV: `pnpm build:wasm`, `pnpm -C ui build`, `pnpm typecheck` —
      зелёные на всех пакетах
- [X] T049 [P] Стенд UI (chromium headless, `ui/dist`): вставка через форму, правка, отказ с
      причиной, клавиатура — по сценариям `specs/008-element-parameters/quickstart.md`;
      обход всех записей `TYPST_ELEMENTS`: у записей-функций Typst `elementSchema(id)` не `null`
      (SC-001)
- [X] T050 [P] Производительность SC-005: `element_at` на заметке 10 000 строк под wasm
      (release) — временный замер, результат в `specs/008-element-parameters/research.md` §3
- [ ] T051 Прогон quickstart руками на web и desktop, отметка результата в
      `specs/008-element-parameters/checklists/requirements.md`
- [X] T052 [P] README: раздел о форме параметров и о том, где зафиксирована схема
      (`packages/core/src/elements/schema.json`) и как её обновлять генератором

---

## Dependencies

- **Phase 2** блокирует все истории: схема, сверка, разбор значений, граница.
- **US1** (Phase 3) — MVP. Не зависит от других историй.
- **US2** зависит от US1 только формой `ParamForm` (T019) — режим правки добавляется к ней.
- **US3** опирается на `locate.rs`/`edit.rs` из US2 (T030, T031).
- **US4** — поверх US2 (есть что вызывать).
- **US5** — уточнение `locate.rs` из US2.
- T023–T026 (наполнение схемы) параллельны друг другу и US2 — один файл, но разные записи;
  выполнять последовательно во избежание конфликтов правки.

## Parallel Execution Examples

- Phase 2: T010, T013, T014 — разные файлы, после T003–T005.
- US1: T015 (тесты) и T020 (стили) параллельно с T016–T017.
- US2: T028 и T029 (тесты) параллельно, до T030–T031.
- Polish: T049, T050, T052.

## Implementation Strategy

1. Phase 1–2: схема для рисунка, изображения и таблицы + сверка с Typst на обоих таргетах.
2. US1 для этих трёх элементов → замена `BlockParamsDialog` без потери нынешнего
   поведения → **MVP**.
3. US2 + US3 — правка на месте с гарантиями; только после них удаляется регулярный разбор
   в UI (T038).
4. Наполнение схемы на весь каталог (T023–T026) — каждое наполнение сразу проверяется
   тестом сверки и кругом «вставка → разбор».
5. US4, US5, Polish.

## Пересмотр 2026-09-26: схема из Typst без файла

Решение владельца проекта (research §1, FR-001, FR-001b, FR-001d, FR-001e, FR-012b,
SC-001a). Задачи T004, T006, T023–T026 выше описывают прежний файл схемы и сохранены
как история.

- [X] T060 `schema.rs`: `ElementSpec` из `Func::params()` по ключу «синтаксис + путь»,
      кэш на процесс; `schema.json` и `examples/element-schema.rs` удалены
- [X] T061 `element_schema_for(template)`: элемент и начальные значения — разбором
      шаблона каталога; граница wasm (`elementSchemaFor`) и Tauri (`element_schema_for`)
- [X] T062 `locate.rs`: элемент — любой вызов встроенной функции Typst, `set`, заголовок,
      блок кода, формула; вложенный элемент рисунка — из текста
- [X] T063 Подпись поля — имя Typst, подсказка — `docs`; основные — по правилу FR-012b
- [X] T064 Тесты: набор параметров против `Func::params()`, круг по всем шаблонам
      `ui/src/editor/typstElements.ts` на обоих таргетах
- [X] T065 Прогон `wasm-pack test --headless --chrome` для `elements_wasm.rs` (27 из 27)
