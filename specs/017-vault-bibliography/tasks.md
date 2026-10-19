# Tasks: Библиография vault

**Input**: Design documents from `/specs/017-vault-bibliography/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для логики ядра на обоих таргетах (принципы I и II конституции):
каждая проверка живёт в общей таблице `packages/core/tests/common/sources_cases.rs` и
вызывается из двух раннеров — `sources_native.rs` (`cargo test`) и `sources_wasm.rs`
(`wasm-bindgen-test`). Для UI JS-тестов в репозитории нет — проверка сценарием со снимком и
по quickstart.md.

**Внешние предусловия**: фичи 013 (колонка заметки `ui/src/note-panel/`, `revealPlace`,
`Settings.note_panel`) и 015 (модуль `packages/core/src/structure/`) реализованы. Без 015
блокируются T006 и всё, что от неё зависит; без 013 — T040–T043.

## Phase 1: Setup

- [X] T001 Объявить `hayagriva = "0.10.1"`, `biblatex = "0.12.0"`, `serde_yaml = "0.9"` прямыми зависимостями в `packages/core/Cargo.toml` с комментарием «уже приезжают с `typst-library`, веса не добавляют» (research.md §1) и написать зонд-тест в `packages/core/tests/common/sources_cases.rs`, закрепляющий прочитанное об API (research.md §2): `RawBibliography::parse` + `Bibliography::from_raw` на библиотеке из одной записи с общими сокращениями; `TryFrom<&biblatex::Entry>` для `hayagriva::Entry` по одной записи; `serde_yaml::from_value::<hayagriva::Library>` на карте из одной записи; если зонд не проходит — перейти на запасной путь §2 и записать это в research.md
- [X] T002 Объявить `pub mod sources;` в `packages/core/src/lib.rs`, завести `packages/core/src/sources/mod.rs` с пустыми подмодулями `discover`, `bibtex`, `yaml`, `library`, `citations`, `cite`, `check`, `engine` и раннеры `packages/core/tests/sources_native.rs` и `packages/core/tests/sources_wasm.rs` по образцу `export_native.rs`/`export_wasm.rs`, вызывающие зонд T001 на обоих таргетах

## Phase 2: Foundational

**Purpose**: типы, обнаружение файлов, чтение без ошибок, библиотека и расширение `structure/` — без этого не начинается ни одна история.

- [X] T003 Типы `SourceFormat`, `SourceCandidate`, `SourceFileInfo`, `Source`, `SourceProblem`, `SourceProblemKind`, `FileReading` с `Serialize`/`Deserialize` и `Tsify` под wasm32 в `packages/core/src/sources/mod.rs` (data-model.md)
- [X] T004 [P] `source_format(path)` в `packages/core/src/sources/discover.rs`: кандидат — расширение `bib`/`yml`/`yaml` без учёта регистра и `vault::classify(path) == Other`; модульные тесты на служебное место, снимки, конфликтные копии, регистр расширения (contracts/core-sources-api.md, правило 1)
- [X] T005 Метод трейта `list_source_candidates()` в `packages/core/src/vault/backend.rs` с реализациями по образцу `list_fonts` в `packages/core/src/vault/native.rs` и `packages/core/src/vault/web.rs` (только метаданные, скрытые папки пропускаются) и в `MemoryVault` из `packages/core/tests/common/mod.rs`
- [X] T006 Расширить общий разбор 015 по её правилу расширения: поля `cites: Vec<CiteSite>` и `bibliographies: Vec<BibliographySite>` в `NoteStructure` и функция `mode_at(text, offset_utf16) -> TextMode` в `packages/core/src/structure/mod.rs`, новые ветки обхода (`cite(<k>)`, `cite(label("k"))`, `bibliography("…")`, `bibliography(("…", "…"))`, нелитеральный аргумент → `literal: false`) в `packages/core/src/structure/walk.rs`; существующие поля и тесты `structure_cases` не меняются (research.md §9)
- [X] T007 [P] Тесты расширения `structure/` в `packages/core/tests/common/sources_cases.rs` (оба раннера): `cite` в разметке и в коде, ключ с пробелом через `label("…")`, `bibliography` со строкой, массивом и переменной, места в UTF-16 на тексте с кириллицей, `mode_at` в разметке, математике, коде, строке, комментарии; метка в `cite(<k>)` не попадает в `labels`
- [X] T008 [P] Чтение BibTeX без ошибок — `read_bibtex(file, text)` в `packages/core/src/sources/bibtex.rs`: `RawBibliography::parse` → `Bibliography::from_raw` → `hayagriva::Entry` через `TryFrom`; заполнение `Source` (авторы «Фамилия И.», название, год, вид, контейнер из родительской записи, строка записи); TeX-записи и `@string` раскрыты
- [X] T009 [P] Чтение Hayagriva без ошибок — `read_hayagriva(file, text)` в `packages/core/src/sources/yaml.rs`: `hayagriva::io::from_yaml_str`, то же заполнение `Source`, строка записи — строка ключа верхнего уровня; `has_entries` для опознания YAML (research.md §3)
- [X] T010 `SourceLibrary` в `packages/core/src/sources/library.rs`: `refresh(backend, registered, referenced)` со сверкой отпечатков и чтением одного файла за вызов (`RefreshStep`), правила опознания YAML (содержимое, указан, назван в `bibliography`), `FileUnreadable` при отказе чтения, `summary()` с `yaml_candidates` (contracts/core-sources-api.md § Движок)
- [X] T011 `SourceEngine::new`, `refresh`, `summary` в `packages/core/src/sources/engine.rs`; wasm-класс `SourceEngine` с `refresh(backend, registered)` и `summary()` в `packages/core/src/sources/web.rs`; состояние `Sources(Mutex<SourceEngine>)` со сбросом при смене vault и команды `sources_refresh`, `sources_summary` в `packages/shell/src/lib.rs`
- [X] T012 [P] Тесты чтения и библиотеки в `packages/core/tests/common/sources_cases.rs` (оба раннера): `.bib` и `.yml` дают одинаковые `Source` для одной и той же записи; раскрытие `{\"o}` и `@string`; посторонний `.yml` не источник, а указанный в `registered` — источник; `refresh` перечитывает только изменившийся файл (счётчик `MemoryVault.reads`); удалённый файл уходит из сводки; два файла с одним ключом — два источника (FR-001, Edge Cases)
- [X] T013 Раздел `sources` в интерфейсе `ui/src/vault-client.ts` (все методы contracts/frontend-sources.md, пока реализованы `refresh` и `summary`) и обе реализации в `ui/src/vault-client-web.ts` (через `wasm-call-queue`) и `ui/src/vault-client-tauri.ts`; остальные методы добавляются задачами историй в те же три файла
- [X] T014 Хук `ui/src/sources/useSources.ts`: цикл `refresh` до `pending == 0` с уступкой циклу событий при открытии vault и при изменении `source_files`, `summary`, счётчик `revision` (contracts/frontend-sources.md § useSources; проход индекса и события добавляют T039 и T053)
- [X] T015 [P] Каталоги текстов `ui/src/i18n/locales/en/sources.ts` и `ui/src/i18n/locales/ru/sources.ts` со всеми ключами из contracts/frontend-sources.md § Тексты (команды, режим, панель, палитра, причины ошибок и отказов, обе формулировки отметки, виды записей, формы числа) и подключение в `ui/src/i18n/locales/en.ts` и `ui/src/i18n/locales/ru.ts` (FR-010)

**Checkpoint**: ядро находит и читает файлы источников на обоих таргетах; UI получает сводку.

## Phase 3: User Story 1 — Вставить цитату поиском (P1) 🎯 MVP

**Goal**: команда «Вставить цитату» открывает палитру у каретки; выбранный источник вставляется цитатой с ключом; предлагается список литературы; без файлов источников — создать или указать.

**Independent Test**: vault с `.bib` на 200 записей → `Ctrl+Alt+B`, «кнут», Enter → в тексте цитата с ключом, предпросмотр собирается со ссылкой.

### Tests for User Story 1

- [X] T016 [P] [US1] Тесты поиска в `packages/core/tests/common/sources_cases.rs` (оба раннера): совпадение по автору, названию, году и ключу; несколько слов — все обязаны встретиться; регистр и кириллица; порядок (начало ключа → начало фамилии → прочее); `limit` и `total_matches`; пустой запрос — все по автору и году; 10 000 источников отвечают без деградации порядка (FR-002, research.md §14)
- [X] T017 [P] [US1] Тесты записи цитаты в `packages/core/tests/common/sources_cases.rs` (оба раннера): `@k` для простого ключа; `#cite(<k>)` при ключе с `.`/`:` на конце и когда следующий знак продолжил бы имя или это `[`; `#cite(label("…"))` для ключа с пробелом, кавычкой и обратной чертой; формы в математике и коде; отказ `NotInsertable` в строке и комментарии; `UnknownKey`; **каждый** результат после применения замен разбирается `typst::syntax::parse` без узлов-ошибок и даёт ровно одну цитату с исходным ключом (FR-003, research.md §12)
- [X] T018 [P] [US1] Тесты предложения списка литературы в `packages/core/tests/common/sources_cases.rs` (оба раннера): нет вызова → `Add` в конец заметки с путём от корня vault; есть вызов без этого файла → `Extend` (строка → массив, массив → массив + элемент); файл уже назван относительным или корневым путём → `None`; нелитеральный аргумент → `None` (FR-004, research.md §13)
- [X] T019 [P] [US1] Тесты создания файла и настройки в `packages/core/tests/common/sources_cases.rs` и `packages/core/tests/common/vault_settings_cases.rs` (оба раннера): `create_source_file` создаёт пустой `references.bib`; занятое имя → `AlreadyExists`; vault только на чтение → `AccessDenied`; `validate_source_file` отклоняет `..`, путь вне vault и чужое расширение; `write_vault_settings` с негодным `source_files` не трогает файл

### Implementation for User Story 1

- [X] T020 [US1] Поиск — `SourceLibrary::search(query, limit, uncited_only)` с подготовленными строками и типами `SourceHit`, `SourceHits` в `packages/core/src/sources/library.rs` (research.md §14; `cited` и `uncited_only` подключает T036)
- [X] T021 [US1] `citation_text`, `plan_citation` (`CitationPlan`, `CitationPlanError`) и `bibliography_offer` (`BibliographyOffer`) в `packages/core/src/sources/cite.rs`: режим — `structure::mode_at`, допустимость знаков — функцией `typst::syntax`, замены — `elements::edit::TextEdit` (research.md §12, §13)
- [X] T022 [US1] `create_source_file` в `packages/core/src/sources/mod.rs` и `validate_source_file` (`SourceFileProblem`) в `packages/core/src/sources/discover.rs` (research.md §5, §11)
- [X] T023 [US1] Поле `source_files: Vec<String>` в `VaultOverrides` и проверка через `validate_source_file` с вариантом `VaultSettingsError::InvalidSourceFile` в `packages/core/src/vault/vault_settings.rs`
- [X] T024 [US1] `CommandId::InsertCitation` (`insert-citation`, `Mod+Alt+B`) и `CommandId::ToggleSources` (`toggle-sources`, `Mod+Alt+R`) в конец `COMMANDS` в `packages/core/src/settings/keybindings.rs`, обновить тесты уникальности умолчаний (research.md §15)
- [X] T025 [US1] Методы `search`, `sources_for_completion`, `plan_citation`, `bibliography_offer` в `packages/core/src/sources/engine.rs`, их wasm-биндинги и `createSourceFile(backend)` в `packages/core/src/sources/web.rs`
- [X] T026 [US1] Команды Tauri `sources_search`, `sources_for_completion`, `sources_plan_citation`, `sources_bibliography_offer`, `sources_create_file` и их регистрация в `packages/shell/src/lib.rs`
- [X] T027 [US1] Методы `search`, `forCompletion`, `planCitation`, `bibliographyOffer`, `createFile` раздела `sources` в `ui/src/vault-client-web.ts` и `ui/src/vault-client-tauri.ts`; действия `createFile()` и `registerFile(path)` (запись `source_files` через `useVaultSettings`) в `ui/src/sources/useSources.ts`; после удачного `createFile` и `registerFile` хук сам запускает цикл `refresh`, не дожидаясь события vault
- [X] T028 [US1] Палитра `ui/src/sources/CitePalette.tsx` по макету `Typsidian Библиография.dc.html` 1b и 1c и образцу `ui/src/editor/InsertMenu.tsx`: поле запроса, строки «Авторы · год / название / ключ / файл при совпадении ключей», одно выделение на мышь и ↑↓, подвал без «⇥», «показаны 50 из N», отбрасывание устаревших ответов, пустые состояния («Создать references.bib», «Указать существующий…» со списком `yaml_candidates`, vault только на чтение, файлы без записей, подсказка «положите файл .bib или .yml в папку заметок» — Edge Cases: файл вне vault); переводы имён и причин — `ui/src/sources/describe.ts`
- [X] T029 [US1] Запрос `ui/src/sources/BibliographyPrompt.tsx`: формулировки для `Add` и `Extend`, «Добавить» применяет замены, «Не сейчас» запоминает путь заметки до конца сеанса; без нажатия текст не меняется (FR-004)
- [X] T030 [US1] В `ui/src/editor/NoteEditor.tsx`: открытие палитры у каретки по счётчику-запросу из `Workspace`, вставка `planCitation` → существующий `applyEdits` → каретка в `plan.caret`, показ причины отказа, затем `bibliographyOffer` на тексте после вставки и `BibliographyPrompt`
- [X] T031 [US1] В `ui/src/workspace/Workspace.tsx`: монтирование `useSources`, сочетание `insert-citation` через `useShortcuts`, команда палитры команд (только при открытой заметке Typst); строки обеих новых команд в `ui/src/settings/ShortcutsSection.tsx`
- [X] T032 [US1] Стили `.ts-cite*` (палитра, строки, подвал, пустые состояния, запрос списка литературы) в `ui/src/styles/app.css` по макету 1b и 1c

**Checkpoint**: US1 работает без панели и без отметок.

## Phase 4: User Story 2 — Видеть, где источник процитирован (P2)

**Goal**: режим «Источники» в колонке заметки: список с поиском, карточка источника с цитирующими заметками и переходом, показ непроцитированных.

**Independent Test**: один источник процитирован в двух заметках → в панели обе заметки с фрагментом, клик ведёт к цитате.

### Tests for User Story 2

- [X] T033 [P] [US2] Тесты индекса цитат в `packages/core/tests/common/sources_cases.rs` (оба раннера): все переходы data-model.md «Индекс цитат»; `scan_step` читает не больше `max_notes`; отмена сохраняет собранное; повторный `scan_start` пропускает заметки с прежним отпечатком; нечитаемая заметка пропущена и посчитана; `note_changed`, `note_removed`, `note_renamed`; `@имя`, совпавшее с меткой заметки, не цитата; вызов `scan_step` вне `Running` не ошибка
- [X] T034 [P] [US2] Тесты выдачи в `packages/core/tests/common/sources_cases.rs` (оба раннера): `citing(key)` — заметки по пути, строка-фрагмент с подсвеченной цитатой, место для перехода; `missing_keys()`; `SourceHit.cited` равен `None` до `Done` и числу после; `uncited_only` пуст до `Done` и даёт непроцитированные после; один ключ в двух файлах — цитирующие у обоих (FR-005, research.md §14)

### Implementation for User Story 2

- [X] T035 [US2] `CitationIndex` в `packages/core/src/sources/citations.rs`: `scan_start`, `scan_step`, `scan_cancel`, `scan_view`, правка одной заметки, `citing`, `missing_keys`; разбор — `structure::parse` (`refs`, `cites`, `labels`, `bibliographies`); фрагменты — `search::HitLine` (research.md §8, §9)
- [X] T036 [US2] Подключить индекс к движку в `packages/core/src/sources/engine.rs`: методы прохода и правки, `citing`, `missing_keys`, заполнение `SourceHit.cited` и `uncited_only` в поиске, передача путей `bibliography(…)` из индекса в `refresh` как `referenced`
- [X] T037 [US2] wasm-биндинги `scanStart`, `scanStep`, `scanCancel`, `scanView`, `noteChanged`, `noteRemoved`, `noteRenamed`, `citing`, `missingKeys` в `packages/core/src/sources/web.rs`
- [X] T038 [US2] Команды Tauri `sources_scan_start` (список заметок берёт у бэкенда), `sources_scan_step`, `sources_scan_cancel`, `sources_scan_view`, `sources_note_changed`, `sources_note_removed`, `sources_note_renamed`, `sources_citing`, `sources_missing_keys` в `packages/shell/src/lib.rs`
- [X] T039 [US2] Методы прохода, правки индекса, `citing`, `missingKeys` в `ui/src/vault-client-web.ts` и `ui/src/vault-client-tauri.ts`; в `ui/src/sources/useSources.ts` — цикл прохода при первом показе режима (порции по 50, уступка циклу событий, `scanCancel` при смене vault и размонтировании) и правка индекса по событиям `NoteCreated`/`NoteUpdated`/`NoteDeleted`/`NoteRenamed`
- [X] T040 [US2] Вариант `NotePanelMode::Sources` и поле `sources_uncited_only` в состоянии колонки заметки 013 в `packages/core/src/settings/mod.rs` (и маршрут изменения в `packages/core/src/settings/apply.rs`), проверки в `packages/core/tests/common/note_panel_cases.rs`: значение сохраняется и читается, негодное приводится к умолчанию
- [X] T041 [US2] Панель `ui/src/sources/SourcesPanel.tsx` по макету `Typsidian Библиография.dc.html` 1a (ширина — общая колонка 248px) и contracts/frontend-sources.md § Панель (FR-005): поле поиска, переключатель «Не цитируются», строка прогресса прохода с отменой и продолжением, строки источников с тегом ключа, карточка выбранного с метой и кнопкой «Вставить @ключ», цитирующие заметки подачей строк-фрагментов макета `Typsidian Связи.dc.html` 1a с переходом через `revealPlace`, группа «Нет в файлах источников», «показаны 200 из N», пустые состояния
- [X] T042 [US2] Зарегистрировать режим «Источники» (заголовок, тег «N записей») в `ui/src/note-panel/modes.ts`
- [X] T043 [US2] В `ui/src/workspace/Workspace.tsx`: кнопка «Источники» в шапке, сочетание `toggle-sources` и команда палитры (показать колонку в режиме «Источники», повторно — скрыть), передача `revision`, `scan` и `revealPlace` панели, «Вставить @ключ» из карточки — тем же путём, что выбор в палитре (счётчик-запрос в `NoteEditor`)
- [X] T044 [US2] Стили `.ts-sources*` (строки, карточка, теги «не цитируется» и «битая», прогресс, группы) для колонки и шторки в `ui/src/styles/app.css` по макету 1a

**Checkpoint**: US1 и US2 работают; на узком окне панель — шторка снизу.

## Phase 5: User Story 3 — Не терять цитаты при правке источников (P3)

**Goal**: изменение файлов источников замечается само; цитаты с ненайденным ключом отмечены в редакторе с причиной; ошибка в одной записи не прячет остальные.

**Independent Test**: удалить из `.bib` запись, процитированную в открытой заметке → цитата отмечена красным, причина названа.

### Tests for User Story 3

- [X] T045 [P] [US3] Тесты чтения с ошибками в `packages/core/tests/common/sources_cases.rs` (оба раннера): `.bib` с незакрытой скобкой в одной записи, с негодным полем, с повтором ключа, с неизвестным сокращением — остальные записи на месте, у ошибочной строка и ключ; `.yml` с записью без вида и с негодным полем — то же; `.yml` со сломанным синтаксисом — `FileSyntax` с местом, другие файлы библиотеки целы; больше 200 ошибок → `TooMany`; чтение не паникует на произвольных байтах (FR-008, research.md §2)
- [X] T046 [P] [US3] Тесты отметок в `packages/core/tests/common/sources_cases.rs` (оба раннера): вся таблица research.md §10 построчно, включая `ambiguous`, `known_labels` и случай «файлов источников нет»; места в UTF-16 на тексте с кириллицей и эмодзи; `citations`, `broken`, `has_bibliography`; после `refresh` с удалённой записью та же заметка даёт отметку (FR-006, SC-003)
- [X] T047 [P] [US3] Тесты обнаружения изменений: в `packages/core/tests/sources_native.rs` — создание, правка, удаление и переименование `.bib` на настоящем диске дают `VaultEvent::SourceFilesChanged`, а правка постороннего `.txt` — нет; в `packages/core/tests/sources_wasm.rs` — то же через `poll_for_changes`; в общей таблице `packages/core/tests/common/sources_cases.rs` — `write_bytes` в путь-кандидат кладёт событие (FR-007, research.md §7)

### Implementation for User Story 3

- [X] T048 [US3] Чтение по записям при ошибке: куски по началам записей и `from_raw` по одной записи в `packages/core/src/sources/bibtex.rs`; `Mapping` и `from_value` по одной записи в `packages/core/src/sources/yaml.rs`; перевод смещений в строку и колонку, предел 200 (research.md §2)
- [X] T049 [US3] `VaultEvent::SourceFilesChanged` в `packages/core/src/vault/events.rs`; на native — ветка пути-кандидата в `dispatch_debounced_event` и событие при `write_bytes` в `packages/core/src/vault/native.rs`; на web — снимок кандидатов в `poll_for_changes` и событие при `write_bytes` в `packages/core/src/vault/web.rs`; обработать новый вариант во всех `match`/`switch` по `VaultEvent` (ядро, `packages/shell/src/lib.rs`, `ui/src/workspace/Workspace.tsx`)
- [X] T050 [US3] `check_note` (`CitationMark`, `CitationMarkKind`, `NoteCheck`) в `packages/core/src/sources/check.rs`, метод в `packages/core/src/sources/engine.rs`, биндинг `checkNote` в `packages/core/src/sources/web.rs`, команда `sources_check_note` в `packages/shell/src/lib.rs`, метод `checkNote` в `ui/src/vault-client-web.ts` и `ui/src/vault-client-tauri.ts`
- [X] T051 [US3] Общий механизм отметок `ui/src/editor/marks.ts`: `EditorMark`, расширение `editorMarks` (поле состояния с `Decoration.mark` по поставщикам, перенос диапазонов через правки, отбрасывание диапазона за пределами текста, `hoverTooltip` с причиной), `setMarks(view, provider, marks)`; подключить расширение в `ui/src/editor/CodeEditor.tsx`; стили `.ts-mark--error` (красный текст и красный пунктир, макет 1a) и `.ts-mark--warning` (жёлтый пунктир, для 018) в `ui/src/styles/app.css`
- [X] T052 [US3] Хук `ui/src/sources/useCitationMarks.ts`: пауза 300 мс после правки текста, немедленный пересчёт при смене `revision`, отбрасывание ответа на устаревший текст, перевод причины (обе формулировки) и `setMarks(view, "sources", …)`; сегмент статус-бара «N цитат · M битых» в `ui/src/editor/NoteEditor.tsx`
- [X] T053 [US3] В `ui/src/sources/useSources.ts`: событие `SourceFilesChanged` → цикл `refresh` → `revision++`; в `ui/src/sources/SourcesPanel.tsx` — группа «Ошибки в файлах» (`файл · строка · ключ — причина`, `detail` под ней, тег с числом ошибок в шапке) стилем карточки отчёта макета 1c

**Checkpoint**: все три истории работают независимо.

## Phase 6: Polish & Cross-Cutting

- [X] T054 [P] Бенчмарк `packages/core/benches/sources_scale.rs` и его объявление в `packages/core/Cargo.toml`: разбор `.bib` на 10 000 записей, поиск по 10 000 источников (SC-002, бюджет 200 мс), выборка цитирующих и `missing_keys` на индексе 5 000 заметок (SC-004, бюджет 1 с); результаты записать в quickstart.md
- [X] T055 [P] Сценарий `scripts/dev/scenarios/sources.mjs` по образцу `rules-editor.mjs`: vault в OPFS с `references.bib` (три записи, одна негодная) и двумя заметками с цитатами; панель «Источники», палитра, вставка, согласие на список литературы, отметка битой цитаты; снимки колонки, палитры и пустого состояния
- [ ] T056 Сверить снимки T055 с макетом `Typsidian Библиография.dc.html` 1a–1c и перечнем расхождений research.md §16; поправить вёрстку в `ui/src/sources/SourcesPanel.tsx` и `ui/src/sources/CitePalette.tsx`; проверить шторку на окне уже 720px
- [ ] T057 Прогнать гейты quickstart.md (сборка ядра под оба таргета, `cargo test`, `wasm-pack test`, `pnpm typecheck`, `scripts/check-i18n.sh` для файлов `ui/src/sources/SourcesPanel.tsx`, `ui/src/sources/CitePalette.tsx`, `ui/src/sources/BibliographyPrompt.tsx`, `ui/src/editor/marks.ts`) и ручную проверку шагов 1–6 на web и desktop (FR-010; шаг 2 — с секундомером, SC-001: не дольше 10 с); на Android — шторка и фиксированная папка; iOS записать «не проверено» в `specs/017-vault-bibliography/quickstart.md`
- [X] T058 [P] Проверка офлайна и локальности (FR-009): тест в `packages/core/tests/common/sources_cases.rs`, что весь путь «refresh → search → plan_citation → scan → check_note» проходит на `MemoryVault` без сетевого клиента, и просмотр `packages/core/src/sources/` на отсутствие обращений к `reqwest`

## Dependencies

- Phase 1 → Phase 2 → истории. T006 требует модуля `structure/` фичи 015.
- US1 (Phase 3) зависит только от Phase 2. Внутри: T020–T023 → T025 → T026 → T027 → T028–T031; T024 до T031.
- US2 (Phase 4) зависит от Phase 2 и от колонки заметки 013 (T040–T043). От US1 берёт только кнопку «Вставить @ключ» (T043 использует путь T030); без US1 кнопка скрыта, остальное работает.
- US3 (Phase 5) зависит от Phase 2. T050 → T052; T051 → T052; T049 → T053. Группа ошибок в панели (вторая половина T053) требует T041.
- Polish: T055–T056 после всех историй; T054 после T020 и T035.

## Parallel Example

```text
Phase 2:  T004 ║ T008 ║ T009 ║ T015        (разные файлы)
          T007 ║ T012                       после T006 и T010
US1:      T016 ║ T017 ║ T018 ║ T019        тесты до реализации
US2:      T033 ║ T034
US3:      T045 ║ T046 ║ T047
```

Задачи, правящие общие файлы (`sources_cases.rs`, `engine.rs`, `web.rs`, `packages/shell/src/lib.rs`, `Workspace.tsx`, `app.css`), между собой не параллельны, даже если помечены [P] в разных историях.

## Implementation Strategy

1. **MVP** — Phase 1–3: вставка цитаты поиском. Проверяется шагами 1–2 quickstart.md.
2. US2: панель и индекс цитат — после готовности 013.
3. US3: отметки и терпимое чтение; механизм `ui/src/editor/marks.ts` после этого готов для 018.
4. Polish: бенчмарк, сценарий, сверка с макетом, гейты.

Задача считается готовой только с прогоном её проверок на обоих таргетах (принцип II); отметку `[X]` ставит прогон, а не чтение кода.
