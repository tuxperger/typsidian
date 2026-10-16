# Tasks: Режим показа слайдов

**Input**: Design documents from `/specs/019-slides-mode/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для логики ядра на обоих таргетах (принципы I и II конституции):
каждый сценарий — в `packages/core/tests/common/slides_cases.rs` и вызывается из обоих
раннеров, `slides_native.rs` и `slides_wasm.rs`. Для UI JS-тестов в репозитории нет —
сценарий headless-браузера `scripts/dev/scenarios/slides.mjs` и проверка по quickstart.md.

## Phase 1: Setup

- [X] T001 Объявить `pub mod slides;` в `packages/core/src/lib.rs` и завести `packages/core/src/slides/mod.rs`, `collect.rs`, `deck.rs`, `keys.rs`, `web.rs` (последний под `cfg(target_arch = "wasm32")`); завести `packages/core/tests/common/slides_cases.rs` (с `pub mod slides_cases;` в `packages/core/tests/common/mod.rs`) и раннеры `packages/core/tests/slides_native.rs`, `packages/core/tests/slides_wasm.rs` по образцу `structure_native.rs` / `structure_wasm.rs`
- [X] T002 [P] Завести каталог `ui/src/slides/` с пустыми модулями из plan.md § Project Structure и область переводов `ui/src/i18n/locales/en/slides.ts` (эталон) и `ui/src/i18n/locales/ru/slides.ts`, подключённые в `ui/src/i18n/locales/en.ts` и `ui/src/i18n/locales/ru.ts`

## Phase 2: Foundational

**Purpose**: сведения о страницах в сборке, типы колоды, команда и канал — без них ни одна история не начинается.

- [X] T003 Типы `PageSlideInfo`, `SlidePlace`, `ShowState`, `ShowAction`, `SlideLabel`, `ShowKeymap`, `SlidesError` (serde + `Tsify` под wasm32) по data-model.md в `packages/core/src/slides/mod.rs`; `SlidesError::EmptyDeck` → `AppMessage` с переводом в `ui/src/i18n/locales/{en,ru}/slides.ts`
- [X] T004 Сбор `collect(document, world, main, source) -> Vec<PageSlideInfo>` по research.md §1–§2: обход кадров страницы (вложенные группы с трансформациями не нужны — только спаны), спаны глифов `TextItem` главного файла → минимальный байтовый `start` через `world.range` → `SlidePlace` (UTF-16 через `crate::text_offsets`, строка и колонка через `typst::syntax::Lines` по тексту главного файла); `introspector.query` по метке `<pdfpc>` и `MetadataElem`, страница из `introspector.position(location).page`, словари `t: "Note"` (строка или содержимое → `plain_text()`, несколько — через пустую строку) и `t: "LogicalSlide"`; прочее пропускается без паники — в `packages/core/src/slides/collect.rs`
- [X] T005 Поле `slides: Vec<PageSlideInfo>` (`#[serde(default)]`) в `CompiledDocument` и вызов `slides::collect` в `compile_with` при `Render::Pages` (при `Render::Pdf` — пусто, у неудачной сборки — пусто) в `packages/core/src/preview/compile.rs`; проверить, что все места, где `CompiledDocument` создаётся руками (тесты 005, 009), компилируются
- [X] T006 [P] Тесты сбора на обоих путях по contracts/core-slides-api.md § Тесты: три страницы — `source_start` возрастает; страница-картинка и страница с `#include` → `None`; `Note` строкой и содержимым, две заметки на странице; `LogicalSlide` по страницам; документ без `<pdfpc>`; кириллица и символы вне BMP в `source_start` (UTF-16); `slides.len() == pages.len()`; сбор не меняет `pages` и `digest` — в `packages/core/tests/common/slides_cases.rs`, вызовы в `packages/core/tests/slides_native.rs` и `packages/core/tests/slides_wasm.rs` per FR-001, FR-005
- [X] T007 Добавить `CommandId::StartShow` (`"start-show"`, `Mod+Alt+P`) в перечисление, `COMMANDS`, `as_str`, `parse` и `default_shortcut` с комментарием о выборе клавиши (research.md §12) в `packages/core/src/settings/keybindings.rs`; тест уникальности умолчаний проходит; случай на обоих путях (имя ↔ команда, умолчание) — в `packages/core/tests/common/vault_settings_cases.rs` per FR-001
- [X] T008 Интерфейс `ShowHost`, `ShowViewer`, `ShowRole` и тип-объединение сообщений `HostMessage` / `ViewerMessage` по contracts/show-channel.md (поля — `import type` из `wasm-core`) в `ui/src/slides/show-channel.ts`; реализация «в памяти» `createInMemoryShowHost()` (пара хозяин–зритель в одном окне) там же

**Checkpoint**: сборка отдаёт `slides`, команда есть, канал описан.

## Phase 3: User Story 1 — Показать заметку слайдами (P1) 🎯 MVP

**Goal**: показ открытой заметки с места каретки в отдельном окне (web, рабочий стол) или поверх приложения (Android), листание, переход по номеру, выход к месту слайда, ошибки сборки.

**Independent Test**: заметка из 10 слайдов → показ, вперёд до конца и назад, выход → каретка на последнем показанном слайде (quickstart.md §2).

### Tests for User Story 1

- [X] T009 [P] [US1] Тесты колоды на обоих путях: `start` с кареткой во второй странице, до первого текста, на пошаговом слайде (первый шаг), без каретки → 0, пустая колода → `EmptyDeck`; `navigate` — `Next`/`Previous` на границах, `First`, `Last`, `GoToSlide` по логическим номерам и по страницам, `GoToSlide(0)` и сверх числа, `GoToPage`; `caret_at_page` у страницы без текста → предыдущая, у первой без текста → `None`; `slide_label` со шагами и без, `next_page` на последней → `None`; `show_keymap` содержит `PageDown`/`PageUp`/стрелки/`Home`/`End`/пробел — в `packages/core/tests/common/slides_cases.rs` и обоих раннерах per FR-001, FR-003, FR-004

### Implementation for User Story 1

- [X] T010 [US1] `start`, `navigate`, `caret_at_page`, `slide_label` по research.md §1, §4 и data-model.md в `packages/core/src/slides/deck.rs`; `show_keymap` по research.md §6 в `packages/core/src/slides/keys.rs`
- [X] T011 [US1] wasm-биндинги `slidesStart`, `slidesNavigate`, `slidesCaret`, `slidesLabel`, `slidesKeymap` в `packages/core/src/slides/web.rs`
- [X] T012 [US1] Команды Tauri `slides_start`, `slides_navigate`, `slides_caret`, `slides_label`, `slides_keymap` без состояния оболочки и их регистрация в `packages/shell/src/lib.rs`
- [X] T013 [US1] Методы `slides.start`, `slides.navigate`, `slides.caret`, `slides.label`, `slides.keymap` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts`; типы — `import type` из `wasm-core`
- [X] T014 [P] [US1] `SlideImage.tsx`: SVG страницы, вписанный в рамку по `min(w/W, h/H)`, поле фоном показа, без текстового слоя; подмена SVG по отпечатку без мигания — в `ui/src/slides/SlideImage.tsx`
- [X] T015 [US1] Хук окна-зрителя `ui/src/slides/useShowWindow.ts`: `ready` с `have`, приём `hello`/`deck`/`state`/`end`, кэш SVG по отпечатку с выбросом отсутствующих в последнем `deck` (research.md §10), клавиши по присланной `ShowKeymap`, набор номера с таймаутом 3 с, `action`/`exit`/`closed` per FR-003
- [X] T016 [US1] `ShowView.tsx` по макету A2–A6: слайд, панель (назад, счётчик `SlideLabel`, вперёд, «Во весь экран», выход) с автоскрытием через 2 с, поле номера (A3), плашка устаревшей сборки с «К ошибке» (A4), экран «Показывать нечего» с текстом первой ошибки (A5), подсказка о полноэкранном режиме (A6, 6 с); стили `.ts-show*` только на токенах в `ui/src/styles/app.css`; тексты — `slides.ts` per FR-002, FR-003, FR-008, FR-010
- [X] T017 [US1] Хук окна редактора `ui/src/slides/useShowSession.ts` по contracts/frontend-show.md: `start(from, withPresenter)` — `slides.start` по `shown.slides` и каретке, открытие окна через `ShowHost`, ответы на `ready` (`hello`, `deck` только с новыми SVG, `state` со `stale`/`failure` из `useCompiledDocument`), `action` → `slides.navigate` → `state`, `exit`/`closed`/`revealError` → `stop()`: `slides.caret` → `revealPlace` строкой и колонкой `SlidePlace` и фокус в редактор; сессия привязана к заметке старта (research.md §7) per FR-001, FR-004, FR-008
- [X] T018 [US1] Web-реализация `ShowHost`/`ShowViewer` в `ui/src/slides/show-channel-web.ts`: `window.open('/show?session=…&role=…')`, `BroadcastChannel('typsidian-show-<id>')`, `false` при заблокированном окне, `closed` из `pagehide`; маршрут `apps/web/app/show/page.tsx`, монтирующий `ShowView` (и позже `PresenterView`) по `role` без рабочей области и ядра; `requestFullscreen` по кнопке и первому жесту, отказ → A6 per FR-009
- [ ] T019 [US1] Реализация для рабочего стола в `ui/src/slides/show-channel-tauri.ts`: `new WebviewWindow('show', { url: 'show.html?…' })`, `emitTo`/`listen`, при нескольких мониторах (`availableMonitors`) окно показа — на мониторе, отличном от текущего, `setFullscreen(true)`; `onCloseRequested` → `closed`; вторая точка входа `apps/desktop/show.html` + `apps/desktop/src/show.tsx` и вход в `apps/desktop/vite.config.ts` (`build.rollupOptions.input`); окна `show`, `presenter` и права из research.md §8 в `apps/desktop/src-tauri/capabilities/default.json`
- [X] T020 [US1] Кнопка «Показ» с меню (A1) в шапке `ui/src/workspace/Workspace.tsx` — только для открытой заметки `.typ`; пункты «С текущего слайда», «С начала», «Сразу с видом докладчика» (последний — при `host.supports.presenter`); те же пункты в палитре команд; сочетание `start-show` через `useShortcuts`; тост `slides.blocked` с «Открыть ещё раз», если окно не открылось; `ShowHost` приходит пропом `WorkspaceProps.showHost` из оболочек `apps/web/app/VaultWorkspace.tsx`, `apps/desktop/src/DesktopWorkspace.tsx`, `apps/mobile/src/MobileWorkspace.tsx`; название команды — в `ui/src/settings/ShortcutsSection.tsx` per FR-001
- [ ] T021 [US1] Android: `ShowOverlay.tsx` по макету A8 — слой на весь экран поверх рабочей области с хостом «в памяти» (T008), зоны касания 30/40/30 %, свайп, полоса «Выйти» со счётчиком на 3 с по касанию середины, системная «Назад» — выход через запись в истории при старте и `popstate` (research.md §9), `requestFullscreen` и `screen.orientation.lock('landscape')` по возможности без ошибки при отказе; `supports.presenter = false` — в `ui/src/slides/ShowOverlay.tsx` и `apps/mobile/src/MobileWorkspace.tsx` per FR-009
- [X] T022 [US1] Сценарий `scripts/dev/scenarios/slides.mjs` для `scripts/dev/browser.mjs --opfs-vault` по quickstart.md §2: образец без пакетов, каретка на втором слайде, `Ctrl+Alt+P`, счётчик, листание, `End`/`Home`, набор номера, `Esc` и положение каретки; замер от команды до первого слайда и смены слайда; снимок окна показа сверить с A2/A3 и поправить расхождения в `ui/src/styles/app.css` per SC-001, SC-002

**Checkpoint**: показ работает во всех оболочках без вида докладчика.

## Phase 4: User Story 2 — Вид докладчика (P2)

**Goal**: второе окно с текущим и следующим слайдом, заметками, часами и временем; листание синхронно.

**Independent Test**: показ с видом докладчика — заметки только у докладчика, листание в любом окне листает оба (quickstart.md §3.1).

- [X] T023 [US2] `PresenterView.tsx` по макету A7: часы, время с начала от `started_at_ms` (обновление раз в секунду), счётчик и шаг, «Завершить показ», текущий слайд, «Дальше» по `label.next_page` (на последнем — пусто), заметки `notes` текущей страницы (пусто без сообщений), лента страниц с `GoToPage`; клавиши — те же через `useShowWindow`; стили `.ts-presenter*` в `ui/src/styles/app.css` per FR-005, FR-006
- [ ] T024 [US2] Роль `presenter` в маршрутах `apps/web/app/show/page.tsx` и `apps/desktop/src/show.tsx`; на рабочем столе окно докладчика — на текущем мониторе; `togglePresenter` из окна показа и пункт «Сразу с видом докладчика» в `ui/src/slides/useShowSession.ts`; `closed` окна докладчика закрывает только его; кнопка «Докладчик» в `ShowView` — только при `supports.presenter` per FR-005, FR-009
- [X] T025 [US2] Проверить FR-006: `ShowView` не получает и не рисует `notes` (поле не передаётся в его пропсы) — в `ui/src/slides/ShowView.tsx`; дополнить сценарий `scripts/dev/scenarios/slides.mjs` открытием вида докладчика и проверкой синхронного листания

**Checkpoint**: US1 + US2 работают независимо.

## Phase 5: User Story 3 — Показ следует за правками (P3)

**Goal**: пересборка заметки обновляет показ, номер слайда сохраняется.

**Independent Test**: правка текущего слайда видна в показе ≤2 с, номер тот же; страниц стало меньше — последний (quickstart.md §3.2).

- [X] T026 [P] [US3] Тесты `carry` на обоих путях: число страниц то же, больше, меньше текущего номера, ноль → `EmptyDeck` — в `packages/core/tests/common/slides_cases.rs` и обоих раннерах per FR-007
- [X] T027 [US3] `carry` по research.md §5 в `packages/core/src/slides/deck.rs`; биндинг `slidesCarry` в `packages/core/src/slides/web.rs`; команда `slides_carry` в `packages/shell/src/lib.rs`; метод `slides.carry` в трёх файлах `ui/src/vault-client*.ts`
- [X] T028 [US3] В `ui/src/slides/useShowSession.ts`: новая удачная сборка заметки показа → `slides.carry` → `deck` (только новые SVG) и `state`; `EmptyDeck` после правки → `state` с `failure` (A5) без закрытия окна; пересборка другой заметки показ не трогает per FR-007, SC-004
- [X] T029 [US3] Дополнить `scripts/dev/scenarios/slides.mjs`: правка текста при открытом показе, замер до обновлённого слайда, удаление страниц при показе последней per SC-004

**Checkpoint**: все истории работают.

## Phase 6: Polish & Cross-Cutting

- [X] T030 [P] Замер сбора `slides` на презентации из 100 страниц против сборки без сбора (бенч `packages/core/benches/slides_scale.rs` либо тест под `test-support`); результат и решение по порогу 5 % — Decision в `specs/019-slides-mode/research.md` §11 (первый замер: 97,8 % — ошибка сбора; после исправления `collect` — 3,3 %, порог выполнен; оба Decision — в §11)
- [ ] T031 [P] Ручная проверка с настоящими touying и polylux по quickstart.md §4; расхождения разметки `<pdfpc>` — правка `packages/core/src/slides/collect.rs` и случай в `packages/core/tests/common/slides_cases.rs`
- [ ] T032 [P] Ручная проверка рабочего стола (quickstart.md §5) и Android (quickstart.md §6); итог — в `specs/019-slides-mode/quickstart.md`
- [X] T033 Гейты `.specify/scripts/typsidian/gates.sh all` (fmt, wasm32 и нативная сборка, `cargo test`, `wasm-bindgen-test`, `pnpm typecheck`, `scripts/check-i18n.sh`)
- [ ] T034 README: раздел о показе слайдов, сочетании `Mod+Alt+P` и разметке заметок докладчика в `README.md`; `**Status**: Implemented` в `specs/019-slides-mode/spec.md`

## Dependencies & Execution Order

- Setup (T001–T002) → Foundational (T003–T008) → истории.
- US1 (T009–T022) — MVP; внутри: T010 → T011, T012 → T013 → T017; T014, T015 → T016; T008 → T018, T019, T021; T017 → T020.
- US2 (T023–T025) зависит от T015–T019.
- US3 (T026–T029) зависит от T017; от US2 не зависит.
- Polish — после нужных историй.

### Parallel Opportunities

- T002 параллельно T001; T006 параллельно T007, T008.
- В US1: T009 и T014 параллельно с T010–T013; T018 и T019 параллельно после T008.
- T026 параллельно с US2.

## Implementation Strategy

1. Setup + Foundational → сборка отдаёт `slides`.
2. US1 → показ во всех оболочках (MVP): можно выступать.
3. US2 → вид докладчика.
4. US3 → репетиция с правками.
5. Polish → замеры, проверка с настоящими пакетами, гейты, README.
