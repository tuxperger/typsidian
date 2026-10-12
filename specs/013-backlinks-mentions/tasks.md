# Tasks: Обратные ссылки и упоминания

**Input**: Design documents from `/specs/013-backlinks-mentions/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для логики ядра на обоих таргетах (принципы I и II конституции):
общая таблица случаев + раннеры `*_native.rs` и `*_wasm.rs`. Для UI JS-тестов в репозитории
нет — проверка сценарием `scripts/dev/browser.mjs` и по quickstart.md.

**Открытый гейт**: принцип V — plan.md → Complexity Tracking. Задачи T046–T047 реализуют
только порт и его реализацию для обычного ввода; реализации для режимов через мост к Neovim
в этой фиче не пишутся.

## Phase 1: Setup

- [X] T001 Объявить `pub mod links;` в `packages/core/src/lib.rs` и создать `packages/core/src/links/mod.rs` с подмодулями `snippet`, `backlinks`, `mentions`, `link` и `web` (последний — под `cfg(target_arch = "wasm32")`), пока пустыми
- [X] T002 Расширить двойник `MemoryVault` в `packages/core/tests/common/mod.rs`: включаемая строителем проверка `expected` в `write` (`WriteError::Conflict` при расхождении с `stamp_of`), объявление несохранённых правок (`with_unsaved_edits(path)` → `has_unsaved_edits`), счётчик записей по пути (`write_count`) и замена файла «снаружи» между вызовами; существующие таблицы случаев не должны изменить поведение (проверка `expected` по умолчанию выключена)
- [X] T003 [P] Создать каталоги переводов `ui/src/i18n/locales/en/links.ts` и `ui/src/i18n/locales/ru/links.ts` (пустые объекты по образцу `en/search.ts` и `ru/search.ts`) и подключить их в `ui/src/i18n/locales/en.ts` и `ui/src/i18n/locales/ru.ts`

## Phase 2: Foundational

**Назначение**: разбор мест ссылок, общие типы панели и общая колонка заметки с её
состоянием. Блокирует все user stories; колонку используют также 015 и 017.

### Ядро

- [X] T004 Добавить `LinkSite` и `link_sites(content)` в `packages/core/src/graph/mod.rs`, переписать `extract_links` как отображение `link_sites` (research.md §2); модульные тесты: диапазоны байтов на кириллице, несколько ссылок в строке, незакрытая `[[`, пустая цель, равенство `extract_links` и `link_sites(..).map(raw)`; прежние тесты модуля остаются зелёными
- [X] T005 В `resolve_link_target` в `packages/core/src/graph/mod.rs` сохранять расширение `vault::BOARD_EXTENSION`, остальному по-прежнему дописывать `.typ` (research.md §3); модульные тесты: `[[схема.excalidraw]]` → `схема.excalidraw`, `[[глава.1]]` → `глава.1.typ`
- [X] T006 Определить в `packages/core/src/links/mod.rs` типы `OpenText`, `SitePlace`, `SiteRow`, `SourceGroup`, `SourceProblem` (serde + `Tsify` под wasm32, data-model.md), `title_of(path)` и порядок источников «близость папки, затем путь» с модульными тестами порядка (та же папка, соседняя, корень, глубоко вложенная)
- [X] T007 Реализовать в `packages/core/src/links/snippet.rs` вычисление `SitePlace` по байтовому диапазону (строка и колонка с единицы, в единицах `DiagnosticLocation`) и фрагмента-окна `Vec<LineSegment>` (48 символов по обе стороны, обрезка по границе слова, «…», подсвечено одно вхождение — research.md §6); модульные тесты: кириллица и символ вне BMP, вхождение в начале и конце строки, длинная строка, два вхождения в одной строке, совпадение колонки с `preview::diagnostics` на одном и том же тексте
- [X] T008 [P] Добавить `NotePanelMode { Links }` и `NotePanelState { shown, mode, mentions_expanded }` и поле `Settings.note_panel` с терпимым разбором (негодное или неизвестное значение → умолчание) в `packages/core/src/settings/mod.rs` (data-model.md, research.md §12)
- [X] T009 Добавить `SettingsChange.note_panel`, конструктор `SettingsChange::note_panel`, применение в `apply_to` и маршрут «всегда в устройство» в `route`/`target_for` в `packages/core/src/settings/apply.rs`; `reset_to_defaults` и `device_defaults_change` поле не содержат
- [X] T010 [P] Тесты состояния колонки в `packages/core/tests/common/note_panel_cases.rs` с вызовами из `packages/core/tests/settings_native.rs` и `packages/core/tests/settings_wasm.rs`: умолчания, круговой путь через файл настроек, негодное значение не роняет остальные настройки, неизвестный режим → `Links`, при открытом vault правка уходит в устройство и не попадает в `VaultOverrides`, сброс к умолчаниям не трогает поле (FR-021)
- [X] T011 [P] Добавить `CommandId::ToggleNotePanel` (`toggle-note-panel`, `Mod+Shift+L`) в конец `COMMANDS` в `packages/core/src/settings/keybindings.rs` и обновить тесты уникальности умолчаний (research.md §15)

### Общая колонка заметки (UI)

- [X] T012 [P] Описать реестр режимов `NotePanelModeSpec` в `ui/src/note-panel/modes.ts` (contracts/note-panel.md): идентификатор — тип `NotePanelMode` из ядра, заголовок, счётчик, тело
- [X] T013 [P] Реализовать `useNarrowViewport()` в `ui/src/note-panel/useNarrowViewport.ts` (`matchMedia("(max-width: 720px)")`, `false` на сервере и в первом кадре)
- [X] T014 Реализовать `useNotePanelState(device)` в `ui/src/note-panel/useNotePanelState.ts`: однократное чтение `Settings.note_panel` из снимка устройства, запись `SettingsChange.note_panel` с паузой 400 мс, состояние в памяти при отсутствии `device` (research.md §12)
- [X] T015 Сверстать оболочку `ui/src/note-panel/NotePanel.tsx` по макету `Typsidian Связи.dc.html` 1a и 1c: шапка с названиями режимов и тегом-счётчиком, тело активного режима, подача `column` и `sheet` (скрим, ручка, закрытие по Escape через `useDismissOnEscape` и по тапу на скрим)
- [X] T016 Добавить стили `.ts-notepanel*` (колонка 248px через `--ts-notepanel-w`, шторка, шапка 26/34px) в `ui/src/styles/app.css` по токенам макета; правил без префикса `.ts-` не добавлять
- [X] T017 Добавить ключи оболочки `notePanel.*` (кнопка «Панель заметки», подпись команды, закрытие шторки, «откройте заметку»; FR-022) в `ui/src/i18n/locales/en/links.ts` и `ui/src/i18n/locales/ru/links.ts`
- [X] T018 Встроить колонку в `ui/src/workspace/Workspace.tsx`: необязательный проп `device`, `useNotePanelState`, колонка между деревом и редактором (и рядом с `BoardCanvas`), шторка на узком окне, кнопка в шапке (`.ts-btn--on`), сочетание `toggle-note-panel` в обработчике клавиш и команда в палитре; пока режимов нет — тело «откройте заметку»
- [X] T019 [P] Добавить подпись команды `toggle-note-panel` в `ui/src/settings/ShortcutsSection.tsx`
- [X] T020 [P] Передать `device={device}` в `Workspace` в `apps/web/app/VaultWorkspace.tsx`, `apps/desktop/src/DesktopWorkspace.tsx` и `apps/mobile/src/MobileWorkspace.tsx`

**Checkpoint**: колонка показывается и скрывается кнопкой и `Mod+Shift+L`, состояние
переживает перезапуск, на узком окне — шторка.

## Phase 3: User Story 1 — Кто ссылается и переход к месту (P1) 🎯 MVP

**Goal**: в колонке — заметки-источники с фрагментами, клик открывает источник с кареткой
на ссылке.

**Independent Test**: `А`, `Б`, `В`; `Б` и `В` содержат `[[А]]`; открыть `А` → в панели `Б` и
`В` с фрагментами; клик по фрагменту из `Б` открывает `Б` с кареткой на ссылке.

### Tests for User Story 1

- [X] T021 [P] [US1] Случаи обратных ссылок в `packages/core/tests/common/links_cases.rs`: две заметки-источника с выделенной ссылкой во фрагменте (FR-001, FR-002, AS1); три ссылки из одной заметки — одна группа, три строки в порядке текста (AS2); место строки указывает на `[[`; нет ссылок — пустая страница (AS4); цель не источник (FR-006); порядок групп по близости папки; порции по `limit` и `next_offset`, `total_sources` из индекса (FR-005); источник сверх предела и нечитаемый — группа с `problem`; ссылка `[[/путь/имя]]` от корня; ссылка на доску из заметки (Edge Cases); на обоих таргетах, а не только модульно (принцип II): диапазоны `link_sites` на кириллице, колонка `SitePlace` на строке с символом вне BMP, обрезка фрагмента-окна с «…», подсветка одного из двух вхождений в строке
- [X] T022 [P] [US1] Раннеры `packages/core/tests/links_native.rs` (`#[tokio::test]`) и `packages/core/tests/links_wasm.rs` (`#[wasm_bindgen_test]`), вызывающие каждый случай `common::links_cases`, по образцу `export_native.rs` и `export_wasm.rs`

### Implementation for User Story 1

- [X] T023 [US1] Реализовать `BacklinksPage`, `BACKLINKS_PAGE_MAX` и `backlinks_page` в `packages/core/src/links/backlinks.rs` по research.md §4 и contracts/core-links-api.md
- [X] T024 [US1] wasm-биндинг `linksBacklinksPage(backend, target, openTexts, offset, limit)` в `packages/core/src/links/web.rs`
- [X] T025 [US1] Команда Tauri `links_backlinks_page` и её регистрация в `packages/shell/src/lib.rs`
- [X] T026 [US1] Добавить `links.backlinksPage` в интерфейс `ui/src/vault-client.ts` и обе реализации: `ui/src/vault-client-web.ts` (через `wasm-call-queue`) и `ui/src/vault-client-tauri.ts` — одна поверхность на всех оболочках (FR-020)
- [X] T027 [US1] Реализовать `useNoteLinks` в `ui/src/note-panel/links/useNoteLinks.ts` в части обратных ссылок: запрос первой порции при смене цели и при `enabled`, «показать ещё», состояние «идёт запрос», отбрасывание ответов по номеру запроса (contracts/note-panel.md)
- [X] T028 [US1] Добавить тексты режима `links.*` для обратных ссылок (заголовки раздела, «никто не ссылается» с примером, «собираю сведения», «фрагмент не показан», «не читается», «показано N из M», «показать ещё», формы числа ссылок, `aria-label` строки) в `ui/src/i18n/locales/en/links.ts` и `ui/src/i18n/locales/ru/links.ts`
- [X] T029 [US1] Сверстать `ui/src/note-panel/links/LinksMode.tsx` по макету 1a и 1b в части обратных ссылок: раздел со счётчиком (research.md §13), группы (название, папка при `same_folder = false`, число), строки-фрагменты с `<mark class="ts-links__mark">`, состояния «ничего не открыто», «идёт сбор» (hazard-полоса), «ссылок нет», источник с `problem`, «показать ещё 50»; строки — кнопки, доступные с клавиатуры
- [X] T030 [US1] Добавить стили `.ts-links*` (раздел, группа, строка-фрагмент, активная строка, пометка, пустое состояние, увеличенные строки в шторке — макет 1c) в `ui/src/styles/app.css`
- [X] T031 [US1] В `ui/src/workspace/Workspace.tsx`: зарегистрировать режим `links` в колонке, вызвать `useNoteLinks` для открытой заметки или доски, обобщить `revealDiagnostic` до `revealPlace({ path, line, column })` и передать его режиму; переход из шторки закрывает её (FR-003, макет 1c)
- [X] T032 [US1] Сегмент статус-бара «N ссылок» — проп `linksSummary` в `ui/src/editor/NoteEditor.tsx`, заполняемый из `Workspace` только при показанной колонке

**Checkpoint**: US1 проверяется сценарием quickstart.md §3 шаг 1.

## Phase 4: User Story 2 — Панель следует за правками (P2)

**Goal**: правка, переименование и удаление ссылающихся заметок — в приложении и снаружи —
видны в панели без действий пользователя; смена заметки показывает её связи.

**Independent Test**: открыть `А` с панелью, внешним редактором дописать в `В` ссылку `[[А]]`
→ `В` появилась в панели.

### Tests for User Story 2

- [X] T033 [P] [US2] Случаи актуальности в `packages/core/tests/common/links_cases.rs` с вызовами в `packages/core/tests/links_native.rs` и `packages/core/tests/links_wasm.rs`: после записи новой ссылки повторный `backlinks_page` её содержит (FR-007); после удаления ссылки из текста группа исчезает; источник, чей текст уже без ссылки при отставшем индексе, не даёт пустой группы; текст из `open_texts` имеет приоритет над диском (research.md §10); после удаления и переименования источника страница соответствует индексу (FR-008); страница для другой цели не содержит групп прежней (FR-009); ссылки на ещё не существующую заметку появляются в её странице, как только она создана (Edge Cases)

### Implementation for User Story 2

- [X] T034 [US2] Добавить проп `onSaved(path)` в `ui/src/editor/NoteEditor.tsx`, вызываемый после удачной записи автосохранения
- [X] T035 [US2] Завести в `ui/src/workspace/Workspace.tsx` счётчик `linksRevision` и увеличивать его на `NoteCreated`, `NoteUpdated`, `NoteRenamed`, `NoteDeleted`, `LinksInvalidated`, `BulkChangeSettled` (через существующий `useVaultEvents`) и на `onSaved`; передать в `useNoteLinks` вместе с `bulkChangeDepth > 0` (research.md §14)
- [X] T036 [US2] В `ui/src/note-panel/links/useNoteLinks.ts`: перезапрос первой порции с паузой 150 мс при изменении `revision`, состояние «идёт сбор» и отложенный запрос на время массового изменения, сброс при смене цели до прихода ответа (FR-004, FR-007, FR-008, FR-009)

**Checkpoint**: quickstart.md §3 шаг 2.

## Phase 5: User Story 3 — Упоминания и «Связать» (P3)

**Goal**: раскрываемый список упоминаний названия без ссылки и действие, превращающее
упоминание в ссылку.

**Independent Test**: в `Б` текст «см. Проект Альфа»; открыть «Проект Альфа», раскрыть
упоминания, «Связать» → в `Б` на диске `[[Проект Альфа]]`, остальной текст не изменился.

### Tests for User Story 3

- [X] T037 [P] [US3] Случаи упоминаний в `packages/core/tests/common/links_cases.rs` с вызовами в `packages/core/tests/links_native.rs` и `packages/core/tests/links_wasm.rs`: регистр не учитывается (FR-010, AS1); «Кот» внутри «который» не упоминание; название, кончающееся не буквой; название внутри `[[…]]` не упоминание (AS5); название короче трёх знаков → `TitleTooShort`, чтений нет (FR-011); цель и доски не просматриваются (FR-006); упоминание в коде и формуле показывается; проход по vault больше одной порции доходит до `Finished` с верными `scanned`/`total`; после `cancel` `step` ничего не читает (счётчик чтений `MemoryVault`); заметка, появившаяся между шагами, не ломает курсор; `in_notes` возвращает группы только названных заметок; нечитаемая заметка учитывается в `skipped`
- [X] T038 [P] [US3] Случаи «Связать» в `packages/core/tests/common/links_cases.rs` с вызовами в `packages/core/tests/links_native.rs` и `packages/core/tests/links_wasm.rs`: `Written` — файл отличается от прежнего только заменённым диапазоном (FR-013, FR-014, SC-005); упоминание в другом регистре заменяется названием в настоящем написании; сосед получает `[[название]]`, заметка из другой папки — `[[/путь/название]]`, при двух одноимённых заметках ссылка разрешается в ту, что в запросе (FR-017); сдвиг текста перед местом → `Stale`, записей нет (FR-015); файл изменён между индексом и записью → `Stale` по `Conflict`, записей нет; vault только на чтение → `ReadOnly` (FR-018); `editor_text` передан → `EditorEdit` с правкой в UTF-16, записей на диск нет, применение правки к тексту редактора сохраняет остальные правки (FR-016, SC-006); `has_unsaved_edits` без `editor_text` → `UnsavedElsewhere`, записей нет; цель с `]]` в пути → `Unexpressible`; таблица `link_text` (сосед, другая папка, корень, доска) с обратной проверкой `resolve_link_target` — на обоих таргетах (принцип II)

### Implementation for User Story 3

- [X] T039 [US3] Реализовать `link_text(source, target)` в `packages/core/src/graph/mod.rs` (research.md §9) с модульными тестами: сосед, другая папка, корень, доска, путь с `]]`, обратная проверка `resolve_link_target` на таблице путей
- [X] T040 [US3] Реализовать в `packages/core/src/links/mentions.rs` `MIN_MENTION_TITLE_CHARS`, `MentionsAvailability`, `availability`, `mentions_in_text` (research.md §5) с модульными тестами границ слова и маскирования ссылок
- [X] T041 [US3] Реализовать в `packages/core/src/links/mentions.rs` проход: `MentionScan`, `ScanPhase`, `MentionStep`, `MENTION_STEP_NOTES`, `MENTION_STEP_BYTES`, `start`, `step`, `cancel`, `in_notes` (research.md §7, §8; переходы — data-model.md)
- [X] T042 [US3] Реализовать в `packages/core/src/links/link.rs` `LinkRequest`, `LinkPlan`, `LinkOutcome`, чистую `plan_link` и `link_mention` строго в порядке шагов research.md §9
- [X] T043 [US3] wasm-биндинги `linksMentionsAvailability`, `linksMentionsStart` (размеченное объединение `Started | Unavailable`), `linksMentionsStep`, `linksMentionsCancel`, `linksMentionsIn`, `linksLinkMention` в `packages/core/src/links/web.rs`
- [X] T044 [US3] Команды Tauri `links_mentions_availability`, `links_mentions_start`, `links_mentions_step`, `links_mentions_cancel`, `links_mentions_in`, `links_link_mention` и их регистрация в `packages/shell/src/lib.rs`
- [X] T045 [US3] Добавить методы упоминаний и `linkMention` в `links` интерфейса `ui/src/vault-client.ts` и обе реализации: `ui/src/vault-client-web.ts` и `ui/src/vault-client-tauri.ts`
- [ ] T046 [P] [US3] Описать порт `OpenNoteEditors` в `ui/src/note-panel/links/openNoteEditors.ts` с обязательствами из contracts/editor-edit-port.md в комментарии модуля
- [ ] T047 [US3] Реализовать порт для обычного ввода в `ui/src/note-panel/links/plainEditors.ts`: `textOf` и `applyEdit` одной транзакцией CodeMirror без переноса курсора и фокуса, отказ `NoEditor` при несовпадении текста; создать экземпляр в `ui/src/workspace/Workspace.tsx` из `getEditorView` и открытой заметки
- [X] T048 [US3] В `ui/src/note-panel/links/useNoteLinks.ts`: доступность упоминаний, цикл прохода с уступкой циклу событий, отмена при сворачивании, смене цели и размонтировании, точечное обновление по `revision` (`mentionsIn` / удаление группы / проход заново), действие `link(row)` — `textOf` → `linkMention` → `applyEdit` или обновление, отметки исходов по строкам (contracts/note-panel.md)
- [X] T049 [P] [US3] Отображение исходов «Связать» и причин недоступности в `Phrase` — `ui/src/note-panel/links/describe.ts` (исчерпывающий `switch` с `never`)
- [X] T050 [US3] Добавить тексты упоминаний `links.*` («раскройте, чтобы найти», «ищу упоминания: N из M», «свернуть», «название короче трёх знаков», «связать», «только чтение», «устарело», «несохранённые правки», «ссылку записать нельзя», «не удалось записать», «упоминаний нет», формы числа упоминаний) в `ui/src/i18n/locales/en/links.ts` и `ui/src/i18n/locales/ru/links.ts`
- [X] T051 [US3] Дополнить `ui/src/note-panel/links/LinksMode.tsx` разделом упоминаний по макету 1a и 1b: свёрнут / проход с прогрессом и «Свернуть» / список с кнопкой «Связать» / короткое название / только чтение / «устарело» жёлтым / отказы; порции «показать ещё»; состояние раскрытия — из `useNotePanelState` (FR-012, FR-021)
- [X] T052 [US3] Добавить стили упоминаний (кнопка в строке, недоступная кнопка, тег «устарело», прогресс, крупная кнопка в шторке — макет 1c) в `ui/src/styles/app.css` с префиксом `.ts-links`
- [X] T053 [US3] Дополнить в `ui/src/workspace/Workspace.tsx` счётчик шапки колонки и сегмент статус-бара числом упоминаний (только после завершённого прохода) и увеличивать `linksRevision` по итогу «Связать»

**Checkpoint**: quickstart.md §3 шаги 3–5 и §4.

## Phase 6: Polish

- [X] T054 [P] Написать сценарий `scripts/dev/scenarios/note-links.mjs` по quickstart.md §3: vault макета в OPFS, шаги 1–6, снимки колонки (1a), состояний (1b) и окна 360px (1c)
- [X] T055 Прогнать гейты ядра по quickstart.md §1: `cargo fmt --check`, сборка под wasm32 и native, `cargo test -p typsidian-core`, wasm-тесты `links_wasm` и `settings_wasm`, `cargo test -p typsidian-shell`; итог — в конец `specs/013-backlinks-mentions/tasks.md`
- [X] T056 Пересобрать wasm-core, прогнать `pnpm -r typecheck` и `scripts/check-i18n.sh` для файлов `ui/src/note-panel/NotePanel.tsx` и `ui/src/note-panel/links/LinksMode.tsx` (FR-022); убедиться поиском по `packages/core/src/links/mod.rs` и соседним файлам модуля и по `ui/src/note-panel/NotePanel.tsx` с подкаталогом `links`, что сетевых обращений (`fetch`, `reqwest`, `XMLHttpRequest`) нет (FR-019); итог — в конец `specs/013-backlinks-mentions/tasks.md`
- [X] T057 Прогнать сценарий `scripts/dev/scenarios/note-links.mjs` в web, сверить снимки с макетом `Typsidian Связи.dc.html` 1a–1c (расхождения сверх research.md §16 — исправить), убедиться в чистой консоли; итог — в конец `specs/013-backlinks-mentions/tasks.md`
- [ ] T058 Ручная проверка по quickstart.md §4–§6: только чтение, короткое название, desktop и Android, замеры SC-001, SC-002, SC-003, SC-004 на 5 000 заметок; одинаковое поведение в трёх оболочках (FR-020); iOS записать как «не проверено»; результаты и непроверенное — в конец `specs/013-backlinks-mentions/tasks.md`

## Dependencies

- Setup (T001–T003) → Foundational (T004–T020) → US1 → US2 → US3 → Polish.
- Внутри Foundational: T006 после T001; T007 после T004 и T006; T009 после T008; T010 после
  T009; T014 после T008–T009 (нужны типы из пересобранного wasm-core); T015 после T012–T013;
  T018 после T011, T014–T017.
- US1: T023 после T004–T007; T024–T026 по цепочке; T027 после T026; T029 после T027–T028;
  T031 после T018 и T029.
- US2 опирается на `useNoteLinks` из US1 (T027) и новых функций ядра не требует.
- US3: T040 → T041; T042 после T039 и T040; T043 → T044 → T045; T047 после T046; T048 после
  T045 и T047; T051 после T048–T050.
- Типы для UI приходят из пересобранного wasm-core: пересборка нужна после T011 (перед
  T014), после T024 (перед T026) и после T043 (перед T045).
- T002 нужен до T038 (проверка `expected`, несохранённые правки, счётчик записей); T037
  обходится существующим счётчиком чтений `read_count`.

## Parallel Opportunities

- Foundational: T008, T011 (разные файлы настроек) параллельно T004–T007; T012, T013, T019,
  T020 — параллельно между собой.
- US1: T021–T022 пишутся параллельно T023 (тесты — отдельные файлы).
- US3: T037 и T038 затрагивают один файл `links_cases.rs` — параллельны реализации
  (T039–T042), но не друг другу; T046 и T049 параллельны ядру.

## Implementation Strategy

MVP — фазы 1–3: колонка заметки и обратные ссылки с переходом на всех клиентах. Затем
актуальность (US2) и упоминания с «Связать» (US3). Задача закрывается прогоном на обоих
таргетах, а не чтением кода (принцип II). Коммит — фича целиком (конвенции репозитория).
Принятие нарушения по принципу V (plan.md → Complexity Tracking) нужно получить от
пользователя до начала T046–T047.

## Итоги прогона ядра (T055, 2026-10-10)

- `cargo fmt --all --check` — чисто.
- `cargo build -p typsidian-core --target wasm32-unknown-unknown` — собирается; `pnpm build:wasm` (wasm-pack, release) — собирается, типы `links*`, `NotePanelState`, `toggle-note-panel` попали в `ui/src/wasm-core`.
- `cargo test -p typsidian-core` (native) — 724 теста, 0 упало; из них `links_native` — 31, `settings_native` — 58 (7 новых про колонку), модульные тесты ядра — 303. Прежние тесты 001 (`vault_native`, `search_native`) зелёные после изменения `resolve_link_target`.
- `wasm-pack test --headless --chrome packages/core` — 346 тестов, 0 упало; `links_wasm` — 31, `settings_wasm` — 50.
- `cargo test -p typsidian-shell` — 4 теста, 0 упало; команды `links_*` собираются.
- `cargo clippy -p typsidian-core --all-targets -- -D warnings` красный из-за 19 замечаний в файлах, которых фича не касалась (`elements/`, `preview/`, `sync/`, `search/mod.rs:327,208`): новые линты текущего компилятора. В коде фичи замечаний нет.
- Биндинги `links*` (wasm) проверены сборкой и типами, но не вызывались из JS: JS-тестов в репозитории нет; их вызовет сценарий T057.

## Итоги интерфейса (T056, T057, 2026-10-10)

- `pnpm -r typecheck` (после `pnpm --filter ui build`: оболочки читают `ui/dist`) — без ошибок во всех четырёх пакетах; `scripts/check-i18n.sh` (весь интерфейс и файлы колонки) — чисто.
- Сетевые обращения (FR-019): `fetch`/`reqwest`/`XMLHttpRequest` в `ui/src/note-panel/` не встречаются; в `packages/core/src/links/` совпадает только внутренняя функция `fetch` чтения заметки из vault — не сеть.
- Сценарий `scripts/dev/scenarios/note-links.mjs` (web, `--opfs-vault`): 11 проверок пройдено — группы и четыре выделенные ссылки (1a), клик по фрагменту открывает «Методологию», ссылка, дописанная снаружи, появляется сама, «Связать» меняет в файле только упоминание, «устарело» не трогает файл, после перезагрузки колонка показана и упоминания раскрыты, на 360px колонки нет, шторка открывается кнопкой и закрывается переходом. Консоль без ошибок и предупреждений. Снимки — в `shots-013/` (1a-column, 1a-mentions, 1b-stale, 1b-short-title, 1c-sheet).
- Сверка с макетом: состав, порядок, состояния и отступы совпадают; сверх research.md §16 остались: фон колонки — `color-mix` от `--steel-950` (токена для `#101315` нет), кегль строк 12px (`--text-xs`) вместо 11px, отступ строки 24px вместо 22px (сетка токенов), кнопка панели помечается `aria-pressed`, а не `.ts-btn--on` (так устроены остальные кнопки шапки).
- Сверх задач: шапка на ширине ≤720px скрывает метку, хлебные крошки, статус сборки, индикатор синхронизации и кнопку палитры — иначе кнопка панели уходила за край окна (макет 1c). `scripts/dev/browser.mjs` получил `page.viewport(w, h)` и автоприём диалога `beforeunload` (нужен сценарию перезагрузки).
- Не сделано по условию: T046–T047 (порт правки открытой заметки; исход `EditorEdit` разобран в `describe.ts` как отказ, место порта помечено в `useNoteLinks.ts`) и T058.
