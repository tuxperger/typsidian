# Tasks: Вкладки и разделение редактора

**Input**: Design documents from `/specs/014-editor-tabs-splits/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для логики ядра на обоих таргетах (принципы I и II конституции):
каждый случай — в общей таблице `packages/core/tests/common/workspace_cases.rs` и в обоих
раннерах. Для UI JS-тестов в репозитории нет — проверка сценарием `scripts/dev/browser.mjs`
и по quickstart.md.

**Макет**: `Typsidian Вкладки.dc.html`, экраны 1a–1c (скилл `design-typsidian`).

**Порядок**: фича реализуется после 013; задачи T051 и T069 опираются на её колонку
`ui/src/note-panel/`.

## Phase 1: Setup

- [X] T001 Объявить `pub mod workspace;` в `packages/core/src/lib.rs` и завести `packages/core/src/workspace/mod.rs` с типами data-model.md (`TabId`, `GroupSlot`, `TextPlace`, `Tab`, `Group`, `History`, `HistoryEntry`, `WorkspaceState`, `WorkspaceAction`, `Facts`, `NoteFact`, `Transition`, `Focus`, `CloseRefusal`, `Rejection`, `WorkspaceView`, `GroupView`, `TabView`, `ActiveNote`, `Presentation`) с `Serialize`/`Deserialize`/`Tsify`, константами `SPLIT_MIN_WIDTH_PX`, `COMPACT_MAX_WIDTH_PX`, `HISTORY_LIMIT` и `WorkspaceState::empty()`
- [X] T002 Завести общую таблицу тестов `packages/core/tests/common/workspace_cases.rs` (помощники: состояние из списка путей, `Facts` из списка заметок с признаками) с первым случаем «пустое состояние — одна пустая группа», подключить её в `packages/core/tests/common/mod.rs` и создать раннеры `packages/core/tests/workspace_native.rs` и `packages/core/tests/workspace_wasm.rs` по образцу `export_native.rs`/`export_wasm.rs`

## Phase 2: Foundational

**Назначение**: предусловия всех историй — сочетания со знаками, сбор фактов и документ на
путь. После фазы приложение работает как прежде, с одной заметкой.

- [X] T003 Расширить `Shortcut::code()` в `packages/core/src/settings/keybindings.rs` на знаки (`.`→`Period`, `,`→`Comma`, `\`→`Backslash`, `[`→`BracketLeft`, `]`→`BracketRight`, `;`→`Semicolon`, `'`→`Quote`, `/`→`Slash`, `-`→`Minus`, `=`→`Equal`, `` ` ``→`Backquote`) с модульными тестами: имя кода для каждого знака, круг `to_text`→`parse` для `Mod+Alt+.` и `Mod+Alt+Shift+\`, `problem()` их не отвергает (research.md §12)
- [X] T004 [P] Случаи «сочетание со знаком несёт имя физической клавиши в `ResolvedBinding.code`» в `packages/core/tests/common/vault_settings_cases.rs` с вызовами в `packages/core/tests/settings_native.rs` и `packages/core/tests/settings_wasm.rs`
- [X] T005 [P] Убрать привязку `Mod-Alt-\` (`indentSelection`) из раскладки редактора в `ui/src/editor/CodeEditor.tsx` (фильтр `defaultKeymap`) с комментарием о причине (research.md §12)
- [X] T006 Реализовать `facts_of`, `apply_with`, `view_with`, `restore_with` в `packages/core/src/workspace/facts.rs` по contracts/core-workspace-api.md → «Факты» (пути из вкладок, истории и действия; `unsaved` из индекса, `conflict` из `pending_conflict` и `unresolved_edit_paths`)
- [X] T007 [P] Тесты `facts_of` на `MemoryVault` в `packages/core/tests/common/workspace_cases.rs` с вызовами в `packages/core/tests/workspace_native.rs` и `packages/core/tests/workspace_wasm.rs`: отсутствующая заметка не попадает в факты, несохранённые правки и неразрешённый конфликт видны, вид доски определён
- [X] T008 Добавить команду Tauri `discard_edit` с регистрацией в `packages/shell/src/lib.rs` и метод `discardEdit(path)` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts` (биндинг `discardEdit` уже есть) и `ui/src/vault-client-tauri.ts`
- [X] T009 Реализовать документ на путь `ui/src/editor/noteDocument.ts` по contracts/frontend-workspace.md → `NoteDocument`: перенести из `ui/src/editor/useNoteAutoSave.ts` правила автосохранения без изменений (пауза 600 мс, молчание при `pendingConflict`, состояния `Clean/Dirty/Saving/Held/Failed`), из `ui/src/editor/NoteEditor.tsx` — чтение с диска и `beginEdit`; добавить `attach`/`edit`/`replace`/`reload`/`flush`/`subscribe`, единственный `registerFlusher` и `discardEdit`, когда буфер вернулся к сохранённому тексту; автосохранение не зависит от того, смонтирован ли вид (SC-004; research.md §3, §6)
- [X] T010 Реализовать реестр `ui/src/editor/noteDocuments.ts` (`acquire`, `release`, `rename`, выгрузка документа без видов и без несохранённых правок) и привязку к React `ui/src/editor/useNoteDocument.ts` (`useSyncExternalStore`)
- [X] T011 Перевести `ui/src/editor/CodeEditor.tsx` на `document: NoteDocument` вместо `value`/`onChange` и на создание из снимка (`initialState`, `initialScroll`, `onDispose`): правка уходит в `document.edit`, чужие правки приходят транзакцией; расширение пересылки — в новом `ui/src/editor/syncViews.ts` (аннотация «от соседа», `addToHistory.of(false)`)
- [X] T012 Перевести `ui/src/editor/NoteEditor.tsx` на пропы `document`, `tabId`, `active`, `place`: убрать собственные `content`, чтение, `useNoteAutoSave` и регистрацию flusher; вставка доски и принятая внешняя версия идут через `document.replace`; слушатели окна (`insert-menu`, `element-params`), `onEditorReady` и `paramsRequest` работают только при `active`; удалить `ui/src/editor/useNoteAutoSave.ts`
- [X] T013 В `ui/src/workspace/Workspace.tsx` выдать единственной открытой заметке документ из реестра и получать текст для предпросмотра, экспорта и «сохранить как заготовку» из него вместо `editorText`/`onContentChange` — поведение одной заметки прежнее; проверить по quickstart.md 001-сценарии правки, автосохранения и конфликта в `scripts/dev/browser.mjs`

**Checkpoint**: одна заметка открывается, правится, сохраняется и конфликтует как до фичи.

## Phase 3: User Story 1 — Держать открытыми несколько заметок (P1) 🎯 MVP

**Goal**: заметки и доски открываются вкладками одной группы; переключение, закрытие, порядок;
каретка и прокрутка вкладки сохраняются; предпросмотр следует за активной вкладкой.

**Independent Test**: открыть три заметки, вернуться на первую — каретка и прокрутка на
месте; закрыть среднюю — две другие остались (quickstart.md §2).

### Tests for User Story 1

- [X] T014 [P] [US1] Случаи вкладок в `packages/core/tests/common/workspace_cases.rs` с вызовами в `packages/core/tests/workspace_native.rs` и `packages/core/tests/workspace_wasm.rs`: `Open` нового пути (FR-001), повторный `Open` (FR-002), `Open` несуществующего → `NotFound`, `Activate`, `Cycle` по кругу в обе стороны (FR-003), `Move` внутри группы (FR-003), инварианты состояния после каждого перехода, неизвестная вкладка → `UnknownTab` без паники
- [X] T015 [P] [US1] Случаи закрытия в `packages/core/tests/common/workspace_cases.rs` с вызовами в обоих раннерах (`workspace_native.rs`, `workspace_wasm.rs`): после `Close` активна предыдущая по давности (FR-006), `Close` при `unsaved` → `refused: Unsaved`, при `conflict` → `refused: Conflict` и фокус на вкладке, `Close` доски с `board_unsaved = true` → `refused: Unsaved` (FR-005, SC-004), `CloseOthers` закрывает чистые и перечисляет отказы, закрытие последней вкладки даёт пустую группу, `closed` перечисляет закрытые
- [X] T016 [P] [US1] Случаи событий и показа в `packages/core/tests/common/workspace_cases.rs` с вызовами в обоих раннерах: `Renamed` меняет путь и сохраняет `TabId`, `Deleted` закрывает вкладку, а при `conflict` оставляет; `view` даёт `title` без расширения, `hint` — кратчайший отличающий хвост для одноимённых (FR-007), признаки `unsaved`/`conflict`/`kind`, `active` и `tab_count`; `view` состояние не меняет

### Implementation for User Story 1

- [X] T017 [US1] Реализовать в `packages/core/src/workspace/model.rs` функцию `apply` для `Open`, `Activate`, `Cycle`, `Close`, `CloseOthers`, `Move` (в пределах группы), `Renamed`, `Deleted` с ведением `recent` и заполнением `focus`/`refused`/`closed`/`rejected`; остальные действия пока возвращают состояние без изменений
- [X] T018 [US1] Реализовать `view` в `packages/core/src/workspace/view.rs`: подписи и `hint`, признаки из `Facts`, `ActiveNote`, `tab_count`; подача пока всегда `Single`
- [X] T019 [US1] Добавить `CommandId::TabClose` (`tab-close`, `Mod+Alt+W`), `TabNext` (`tab-next`, `Mod+Alt+.`), `TabPrevious` (`tab-previous`, `Mod+Alt+,`) в конец `COMMANDS` в `packages/core/src/settings/keybindings.rs`; существующие тесты уникальности и круга текста проходят
- [X] T020 [US1] wasm-биндинги `workspaceEmpty`, `workspaceApply(backend, state, action, widthPx)`, `workspaceView(backend, state, widthPx)` в `packages/core/src/workspace/web.rs`
- [X] T021 [US1] Команды Tauri `workspace_empty`, `workspace_apply`, `workspace_view` с регистрацией в `packages/shell/src/lib.rs`
- [X] T022 [US1] Методы `workspaceEmpty`, `workspaceApply`, `workspaceView` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts` (через `wasm-call-queue`) и `ui/src/vault-client-tauri.ts`
- [X] T023 [P] [US1] Реализовать `ui/src/editor/tabEditorStates.ts`: `take`/`store`/`caretOf`/`drop`/`copy`, не больше 20 полных снимков `EditorState`, вытесненные хранят `anchor`/`head`/`scrollTop` (FR-004, research.md §4)
- [X] T024 [US1] Хук `ui/src/workspace/tabs/useWorkspaceTabs.ts` по contracts/frontend-workspace.md: значение состояния, очередь действий, подстановка `leaving`, `open`/`close` (flush документа → `Close` → исполнение `refused`), перечитывание `view` по событиям vault и переходам документов «чисто ↔ несохранённое», `Renamed`/`Deleted`/`reload` по событиям vault, сброс при смене клиента, измерение ширины окна
- [X] T025 [P] [US1] Тексты `workspace.tabs.*` (закрыть, закрыть остальные, следующая, предыдущая, «доска», «конфликт», «не сохранено», «N вкладок», «Вкладка не закрыта» и причина, «Открытые заметки», пустое состояние) в `ui/src/i18n/locales/en/workspace.ts` и `ui/src/i18n/locales/ru/workspace.ts`; подписи трёх команд — в карте `ui/src/settings/ShortcutsSection.tsx`
- [X] T026 [US1] Полоса вкладок `ui/src/workspace/tabs/TabStrip.tsx` по макету 1a: название, `hint`, точка несохранённого, теги «конфликт» и «доска», ×, контекстное меню (закрыть, закрыть остальные, следующая, предыдущая — с подписями сочетаний), перестановка указателем (research.md §13), `role="tablist"` и клавиши, прокрутка активной вкладки в видимую область (FR-008)
- [X] T027 [P] [US1] Доска во вкладке `ui/src/boards/BoardTab.tsx`: перенести из `ui/src/workspace/Workspace.tsx` `useBoardDocument`, `registerFlusher`, предупреждение `beforeunload`, `viewModeEnabled`; дописывать доску при уходе с вкладки и перед закрытием; после дописывания сообщать наверх состояние сохранения доски — оно уходит в `Close.board_unsaved` (FR-005; research.md §6, §8)
- [X] T028 [US1] Группа `ui/src/workspace/tabs/EditorGroup.tsx`: `TabStrip` и содержимое активной вкладки — `NoteEditor` с документом и снимком из `tabEditorStates`, `BoardTab` либо пустое состояние макета 1c; панель отказа `ui/src/workspace/tabs/CloseRefusedNotice.tsx` (FR-005)
- [X] T029 [US1] Показывать диалог конфликта для пути активной вкладки: выбор конфликта по пути в `ui/src/editor/useConflictResolution.ts`; конфликт по пути без вкладки открывает вкладку (research.md §7)
- [X] T030 [US1] Перевести `ui/src/workspace/Workspace.tsx` на вкладки: убрать `openNote`/`editorText`/`lastChange`-зависимости от одной заметки; `openEntry`, `openPath`, `openBoard`, создание из дерева, палитра, поиск, граф идут в `tabs.open`; `PreviewPane`, `useExport`, `Breadcrumb`, `Sidebar.openPath`, `GraphView.currentPath`, `SearchOverlay.currentNote`, `FileHistory`, `insertLink`, порт вставки заготовок берут заметку из `view.active` и текст из её документа (FR-001, FR-009, US1 AS7); при активной доске и одной группе колонки предпросмотра нет (research.md §9); `reopenVault` и клонирование сбрасывают вкладки
- [X] T031 [US1] Команды и сочетания в `ui/src/workspace/Workspace.tsx`: `tab-close`, `tab-next`, `tab-previous` в слушателе клавиш и в палитре; «Закрыть остальные» и «Передвинуть вкладку влево/вправо» — команды палитры без сочетания (FR-003, FR-025)
- [X] T032 [US1] Счётчик вкладок в статус-баре `ui/src/editor/NoteEditor.tsx` (макет 1a; research.md §14)
- [X] T033 [US1] Телефон (FR-024): при `presentation = List` (реализовать в `packages/core/src/workspace/view.rs` порог `COMPACT_MAX_WIDTH_PX`, случай в `packages/core/tests/common/workspace_cases.rs` с вызовами в обоих раннерах) полосы нет, в шапке `ui/src/workspace/Workspace.tsx` — счётчик открытых заметок, открывающий шторку `ui/src/workspace/tabs/OpenNotesSheet.tsx` по макету 1c
- [X] T034 [US1] Стили `.ts-tabs*`, `.ts-tab*`, `.ts-open-notes*`, `.ts-tab-refused*` в `ui/src/styles/app.css` по макету 1a и 1c: высота 30 px, фон `#101315`, линия активной вкладки, токены дизайн-системы вместо литералов
- [X] T035 [US1] Сценарий `scripts/dev/scenarios/tabs.mjs` (шаги quickstart.md §2: три вкладки, возврат каретки и прокрутки, закрытие, одноимённые заметки) и сверка снимка `--shot` с экраном 1a макета; расхождения устранить либо записать в research.md §14

**Checkpoint**: US1 проходит quickstart.md §2 на web; ядро зелёное на обоих таргетах.

## Phase 4: User Story 2 — Назад и вперёд по истории переходов (P2)

**Goal**: «Назад» и «Вперёд» возвращают к заметке и месту, откуда ушли.

**Independent Test**: `А` → `Б` → `В` по ссылкам, дважды «Назад» — `А` с кареткой на ссылке,
«Вперёд» — `Б` (quickstart.md §3).

### Tests for User Story 2

- [X] T036 [P] [US2] Случаи истории в `packages/core/tests/common/workspace_cases.rs` с вызовами в `packages/core/tests/workspace_native.rs` и `packages/core/tests/workspace_wasm.rs`: `Back`/`Forward` с местом ухода в `focus.place` (FR-011), `Open` после `Back` отбрасывает ветку (FR-012), закрытая открывается заново и удалённая пропускается — и по событию `Deleted`, и по отсутствию в `Facts` (FR-013), `can_back`/`can_forward` и `NoHistory` (FR-014), записи добавляют `Activate`, `Cycle` и закрытие активной, `Open` с `target` в активной заметке добавляет запись, соседние одинаковые схлопываются, предел `HISTORY_LIMIT`, `Renamed` переписывает путь, «Назад» из пустой группы возвращает последнюю закрытую

### Implementation for User Story 2

- [X] T037 [US2] Реализовать историю группы в `packages/core/src/workspace/history.rs` (добавление с обновлением места ухода, движение указателя, чистка и переименование пути, предел) и подключить её в `apply` для `Open`, `Activate`, `Cycle`, `Close`, `Renamed`, `Deleted`, `Back`, `Forward` в `packages/core/src/workspace/model.rs`; `can_back`/`can_forward` — в `packages/core/src/workspace/view.rs`
- [X] T038 [US2] Добавить `CommandId::NavigateBack` (`navigate-back`, `Mod+Alt+[`) и `NavigateForward` (`navigate-forward`, `Mod+Alt+]`) в `packages/core/src/settings/keybindings.rs`
- [X] T039 [US2] В `ui/src/workspace/tabs/useWorkspaceTabs.ts` исполнять `focus.place`: после загрузки документа поставить каретку в строку и колонку и прокрутить к ней (логика нынешнего эффекта `reveal` из `Workspace.tsx`; FR-011, SC-005), передавать `leaving` из `tabEditorStates.caretOf`
- [X] T040 [US2] Провести `revealDiagnostic` в `ui/src/workspace/Workspace.tsx` через `tabs.open(path, target)` с прежней сигнатурой (ею пользуются 013, 015, 018) и убрать состояние `reveal` с его эффектом; добавить `navigate-back`/`navigate-forward` в слушатель клавиш и палитру
- [X] T041 [US2] Кнопки «‹ ›» в `ui/src/workspace/tabs/TabStrip.tsx` (макет 1a: недоступная приглушена и `disabled`, подсказка с сочетанием) и в шапке телефона в `ui/src/workspace/Workspace.tsx` (макет 1c)
- [X] T042 [P] [US2] Тексты «Назад», «Вперёд» и подписи двух команд в `ui/src/i18n/locales/en/workspace.ts`, `ui/src/i18n/locales/ru/workspace.ts` и карте `ui/src/settings/ShortcutsSection.tsx`
- [X] T043 [US2] Дополнить `scripts/dev/scenarios/tabs.mjs` шагами quickstart.md §3 (переход по ссылкам, назад на место ухода, отброшенная ветка) и проверить сочетания `Mod+Alt+[`/`]` при кириллической раскладке

**Checkpoint**: US2 проходит quickstart.md §3.

## Phase 5: User Story 3 — Две заметки рядом (P2)

**Goal**: редактор делится на две группы; вкладки переносятся; одна заметка в двух группах —
одно содержимое; предпросмотр и панели следуют за группой с кареткой.

**Independent Test**: разделить, перенести `Б` во вторую группу — обе видны, правка в `А` не
трогает `Б`, предпросмотр следует за кареткой (quickstart.md §4).

### Tests for User Story 3

- [X] T044 [P] [US3] Случаи групп в `packages/core/tests/common/workspace_cases.rs` с вызовами в `packages/core/tests/workspace_native.rs` и `packages/core/tests/workspace_wasm.rs`: `Split` дублирует заметку во вторую группу и переводит туда фокус (US3 AS1), доску переносит, единственная доска → `NothingToSplit`, узкое окно → `TooNarrow` (FR-020); `MoveToOtherGroup` и `Move` между группами сохраняют `TabId` (FR-016), при одной группе → `SingleGroup`, перенос на уже открытый путь активирует существующую; закрытие и перенос последней вкладки группы убирают разделение, вторая становится первой (FR-019); `Unsplit` сливает без дубликатов (FR-015); `FocusGroup` меняет `active` в `view` (FR-009); `SetRatio` подрезается (FR-018); `Close` вкладки, чей путь открыт в другой группе, проходит при `unsaved` (FR-017); доска не открывается во второй группе повторно; у каждой группы своя история (FR-010)
- [X] T045 [P] [US3] Случаи подачи в `packages/core/tests/common/workspace_cases.rs` с вызовами в обоих раннерах: две группы при `width_px ≥ 900` → `Split`; `< 900` → `Single` со вкладками обеих групп, заметка из обеих показана один раз, состояние не изменено, `can_split = false`; возврат ширины → снова `Split`; закрытие в слитой подаче закрывает обе вкладки пути (research.md §10)

### Implementation for User Story 3

- [X] T046 [US3] Реализовать в `packages/core/src/workspace/model.rs` действия `Split`, `Unsplit`, `MoveToOtherGroup`, `Move` между группами, `FocusGroup`, `SetRatio`, схлопывание опустевшей группы и уникальность пути доски; в `packages/core/src/workspace/view.rs` — подачу `Split`/`Single` по `SPLIT_MIN_WIDTH_PX`, слияние полос, `can_split`/`can_unsplit`/`can_move_to_other`
- [X] T047 [US3] Добавить `CommandId::SplitEditor` (`split-editor`, `Mod+Alt+\`) и `TabMoveToOtherGroup` (`tab-move-to-other-group`, `Mod+Alt+Shift+\`) в `packages/core/src/settings/keybindings.rs`
- [X] T048 [US3] Два вида одного документа: проверить и довести `ui/src/editor/syncViews.ts` и `ui/src/editor/noteDocument.ts` — правка из одной группы приходит в другую в том же такте, отмена в группе отменяет своё, один поток `beginEdit` и одна запись на путь (FR-017, SC-006); при `Split` и переносе каретка копируется через `tabEditorStates.copy` (US3 AS2)
- [X] T049 [US3] Компонент `ui/src/workspace/tabs/EditorGroups.tsx`: одна или две `EditorGroup`, между ними `PaneDivider` (перетаскивание, стрелки, двойной щелчок → `SetRatio`), `focusin` группы → `FocusGroup`; подключить в `ui/src/workspace/Workspace.tsx` вместо одиночной группы
- [X] T050 [US3] В `ui/src/workspace/tabs/TabStrip.tsx`: кнопка «◫» (разделить / убрать разделение, недоступна по `can_split`), пункты меню «Разделить редактор» и «Перенести в другую группу» (приглушён по `can_move_to_other`), перетаскивание вкладки на другую полосу → `Move`, серая линия активной вкладки неактивной группы (макет 1b)
- [X] T051 [US3] В `ui/src/workspace/Workspace.tsx`: `split-editor` и `tab-move-to-other-group` в слушателе клавиш и палитре, команда палитры «Перевести фокус в другую группу»; колонка заметки `ui/src/note-panel/` (013), предпросмотр и остальные потребители следуют за `view.active` при смене группы (FR-009, US3 AS4); при активной доске и двух группах колонка предпросмотра остаётся с пустым состоянием (research.md §9); в статус-баре «группа N из 2 · M вкладок»
- [X] T052 [P] [US3] Тексты разделения (разделить, убрать разделение, перенести в другую группу, граница групп, «группа N из 2», «У доски нет предпросмотра», фокус в другую группу) и подписи двух команд в `ui/src/i18n/locales/en/workspace.ts`, `ui/src/i18n/locales/ru/workspace.ts` и карте `ui/src/settings/ShortcutsSection.tsx`
- [X] T053 [US3] Стили `.ts-groups*` и неактивной группы в `ui/src/styles/app.css` по макету 1b; дополнить `scripts/dev/scenarios/tabs.mjs` шагами quickstart.md §4 (разделение, правка в двух группах, перенос, сужение до 880 px) и сверить снимок с экраном 1b

**Checkpoint**: US3 проходит quickstart.md §4.

## Phase 6: User Story 4 — Вкладки переживают перезапуск (P3)

**Goal**: набор вкладок, порядок, группы, активная вкладка и доля ширины восстанавливаются
для каждого vault на устройстве.

**Independent Test**: три заметки, разделить, перезапустить — вкладки и разделение на месте
(quickstart.md §5).

### Tests for User Story 4

- [X] T054 [P] [US4] Случаи набора в `packages/core/tests/common/workspace_cases.rs` с вызовами в `packages/core/tests/workspace_native.rs` и `packages/core/tests/workspace_wasm.rs`: `snapshot`→`restore` возвращает вкладки, порядок, группы, активные и долю (SC-003); отсутствующая заметка пропущена молча, опустевшая группа убрана, дубликаты сняты, лишние группы и негодные `active`/доля исправлены (FR-023); история после восстановления пуста
- [X] T055 [P] [US4] Случаи настроек в `packages/core/tests/common/vault_settings_cases.rs` с вызовами в `packages/core/tests/settings_native.rs` и `packages/core/tests/settings_wasm.rs`: `SettingsChange.workspace` заменяет запись своего vault и не трогает чужую (US4 AS3); `route` отправляет его в `Device` и при открытом vault (FR-022); ключ `workspaces` переживает круг записи, негодное значение читается как пустое и не мешает разбору остального файла (FR-023); набор из одного `workspace` не даёт записи журнала; забытый vault теряет свой набор

### Implementation for User Story 4

- [X] T056 [US4] Реализовать `SavedWorkspace`, `SavedGroup`, `snapshot`, `restore` в `packages/core/src/workspace/persist.rs` (FR-021, FR-023)
- [X] T057 [US4] Добавить `VaultWorkspaceEntry`, поле `Settings.workspaces` с терпимым разбором и `Settings::workspace_for` в `packages/core/src/settings/mod.rs`; поле `SettingsChange.workspace` и его маршрут в `packages/core/src/settings/apply.rs`; ключ `workspaces` в `packages/core/src/settings/format.rs`; исключение из журнала в `packages/core/src/settings/logging.rs`; удаление набора забытого vault в `packages/core/src/settings/vaults.rs`
- [X] T058 [US4] wasm-биндинги `workspaceSnapshot(state)` и `workspaceRestore(backend, saved)` в `packages/core/src/workspace/web.rs`; команды Tauri `workspace_snapshot` и `workspace_restore` в `packages/shell/src/lib.rs`; методы в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts`
- [X] T059 [P] [US4] Порт `ui/src/workspace/tabs/tabs-store.ts` (`WorkspaceTabsStore`) и реализация для браузера `ui/src/workspace/tabs/tabs-store-web.ts` поверх `deviceStore()` и `deviceStoreQueue`
- [X] T060 [P] [US4] Реализация порта для Tauri `ui/src/workspace/tabs/tabs-store-tauri.ts` поверх команд `device_settings` и `apply_device_settings` (образец — `ui/src/sync/sync-client-tauri.ts`)
- [X] T061 [US4] В `ui/src/workspace/tabs/useWorkspaceTabs.ts`: загрузка набора при открытии vault (`load` → `workspaceRestore`), запись после паузы 400 мс и сразу при смене vault и уходе страницы в фон, идентификатор vault фиксируется при загрузке; отказ или отсутствие порта — работа без сохранения и без сообщений (FR-023)
- [X] T062 [US4] Проп `tabsStore` в `ui/src/workspace/Workspace.tsx` и его передача из `apps/web/app/VaultWorkspace.tsx`, `apps/desktop/src/DesktopWorkspace.tsx`, `apps/mobile/src/MobileWorkspace.tsx`
- [X] T063 [US4] Дополнить `scripts/dev/scenarios/tabs.mjs` шагами quickstart.md §5 (перезагрузка страницы с пятью вкладками в двух группах, удалённая заметка, испорченный ключ `workspaces`)

**Checkpoint**: US4 проходит quickstart.md §5 на web и desktop.

## Phase 7: Polish & Cross-Cutting

- [X] T064 Замеры в `scripts/dev/scenarios/tabs.mjs` по quickstart.md §6: SC-001 (переключение ≤100 мс, в том числе во время сборки предпросмотра), SC-002 и FR-027 (50 вкладок, задержка ввода в пределах 10 %; набор текста не вызывает `workspaceApply`/`workspaceView`), SC-006; при провале SC-001 из-за очереди вызовов — мера из research.md §2
- [X] T065 [P] Проверить `scripts/check-i18n.sh` на файлах `ui/src/workspace/tabs/TabStrip.tsx`, `ui/src/workspace/tabs/OpenNotesSheet.tsx`, `ui/src/workspace/tabs/CloseRefusedNotice.tsx`, `ui/src/boards/BoardTab.tsx` и полноту ключей `workspace.tabs.*` в обеих локалях `ui/src/i18n/locales/` (FR-026)
- [X] T066 Сверка вёрстки с макетом: снимки экранов 1a, 1b, 1c через `scripts/dev/browser.mjs --shot` со сценарием `scripts/dev/scenarios/tabs.mjs` (ширины 1180 и 300 px), расхождения устранить в `ui/src/styles/app.css` либо записать в research.md §14
- [X] T067 Гейты по скиллу `quality-gates-typsidian`: `.specify/scripts/typsidian/gates.sh all` — сборка `packages/core` под `wasm32-unknown-unknown` и нативный таргет, `cargo test`, `wasm-bindgen-test`, `tsc` strict, `cargo fmt --check` (принцип II; вывод — в отчёт по `specs/014-editor-tabs-splits/quickstart.md` §0)
- [ ] T068 Ручной проход `specs/014-editor-tabs-splits/quickstart.md` §2–§5 на desktop и §2, §3, §5 на Android при ширине телефона; iOS записать как «не проверено» (оболочки нет), переключение vault на Android — как неприменимое (путь фиксирован)
- [X] T069 Сквозная проверка с 013 в `scripts/dev/scenarios/tabs.mjs`: колонка заметки `ui/src/note-panel/` и предпросмотр показывают заметку активной вкладки после переключения вкладки, смены группы, «Назад» и закрытия (FR-009); переход из «Связей» попадает в историю

## Dependencies & Execution Order

- **Setup (T001–T002)** → **Foundational (T003–T013)** → истории.
- **US1 (T014–T035)** — после Foundational; MVP.
- **US2 (T036–T043)** — после US1 (история опирается на `Open`/`Activate`/`Close`).
- **US3 (T044–T053)** — после US1; от US2 не зависит, кроме случая «своя история у группы»
  в T044 (выполняется после T037).
- **US4 (T054–T063)** — после US1; восстановление двух групп проверяется после US3.
- **Polish (T064–T069)** — после всех историй; T069 требует реализованной 013.

Внутри истории: тесты пишутся первыми и падают, затем ядро (`model.rs` → `view.rs` →
биндинги → shell), затем клиент, хук, компоненты, `Workspace.tsx`, стили, сценарий.

Задачи одной истории, правящие `packages/core/tests/common/workspace_cases.rs`, помечены
[P] относительно реализации, но между собой идут по очереди — файл общий.

### Parallel Opportunities

- Phase 2: T004, T005 и T007 — параллельно с T006/T008; T009–T013 — цепочка.
- US1: T014–T016 (тесты) параллельно с T023, T025, T027; T019 параллельно с T017–T018.
- US2: T036 и T042 параллельно с T037–T038.
- US3: T044–T045 и T052 параллельно с T047.
- US4: T054–T055 параллельно с T059–T060.

## Implementation Strategy

1. **MVP** — Phase 1–3: вкладки одной группы. Самая рискованная часть — Phase 2 (документ
   на путь) и T030 (перевод `Workspace.tsx`): после каждой прогоняются сценарии 001
   (правка, автосохранение, конфликт) в браузере.
2. **US2** — история: только ядро и две кнопки, малый объём.
3. **US3** — разделение: второй вид документа и подача по ширине.
4. **US4** — сохранение набора.
5. Каждая история закрывается своим Checkpoint; задача не считается готовой без пары тестов
   на обоих таргетах (принцип II).

## Phase 8: Convergence

- [X] T070 Полоса вкладок не перерисовывается на каждый набранный знак: сейчас правка перерисовывает `Workspace` целиком, а с ним `TabStrip` (61 отрисовка на 30 знаков), и при 50 вкладках задержка ввода растёт с 9,3 до 13,7 мс на рабочей сборке web; сделать колбэки полосы в `ui/src/workspace/tabs/EditorGroup.tsx` постоянными и `TabStrip` — `memo`, проверить замером `tabs.mjs` с `TABS_BUDGETS=1` per FR-027, SC-002 (partial)
- [X] T071 Быстрый переход каретки из группы в группу не теряется: `ui/src/workspace/tabs/EditorGroup.tsx` шлёт `FocusGroup` только при `!focused` по последней отрисовке, и щелчок во вторую группу, пока не применён `FocusGroup` первой, не отправляется — фокус, предпросмотр и открытие из дерева остаются в первой группе (сценарий `tabs.mjs` падает так примерно в трети прогонов на рабочей сборке); слать `FocusGroup` при каждом входе в группу при разделении — модель действия упорядочивает per FR-009, US3/AS4 (contradicts)
