# Tasks: Сессия урока с ИИ-учителем

**Input**: `specs/051-ai-teaching-session/` (spec, plan, research, data-model, contracts)

**Tests**: тесты ядра — в `packages/core/tests/common/lesson_cases.rs`, на обоих путях
(`ai_native.rs`, `ai_wasm.rs`), с поддельным потоковым транспортом.

## Phase 1: Основа

- [X] T001 `FileKind::Lesson` по расширению `lesson`, виден в дереве, в `packages/core/src/vault/mod.rs`; вкладка урока одна на путь, как доска, в `packages/core/src/workspace/{model,view,persist}.rs`
- [X] T002 Поля `lessons_folder` и `context_budget` в `AiSettings`, отказы `LessonChanged`, `LessonUnreadable`, `GoalEmpty` в `packages/core/src/ai/{settings,mod}.rs`; `lesson-partial/` в `derived_paths()` в `packages/core/src/vault/service_dir.rs`
- [X] T003 Фичи `web-sys` `ReadableStream`, `ReadableStreamDefaultReader`, `ReadableStreamReadResult` в `packages/core/Cargo.toml`

## Phase 2: Фундамент ядра

- [X] T004 Формат файла урока: шапка, события, чтение с пропуском битых строк, дописывание, `LessonView`, `LessonSummary` в `packages/core/src/ai/lesson/file.rs` (data-model.md)
- [X] T005 Инкрементальный разбор потока Anthropic и совместимого протокола (текст, вызовы инструментов, объёмы) в `packages/core/src/ai/lesson/stream.rs` (research §3)
- [X] T006 Потоковая отправка `AiTransport::send_stream` с пределами 30 с до начала, 60 с тишины, 8 МиБ (research §4): native — `reqwest`, wasm — `ReadableStream`, в `packages/core/src/ai/transport.rs`; буфер прогресса по `vault_id/request_id` в `packages/core/src/ai/lesson/partial.rs`
- [X] T007 Инструкции учителя на ru/en и описания инструментов `ask_question`, `read_note` в `packages/core/src/ai/lesson/prompt.rs` (research §2, §7)
- [X] T008 Тесты формата и разбора потока на обоих путях в `packages/core/tests/common/lesson_cases.rs`, `packages/core/tests/ai_{native,wasm}.rs`

## Phase 3: US1 — начать урок и пройти шаг (P1)

**Goal**: урок создаётся, учитель задаёт вопросы, ответ с уверенностью и рассуждением, текст потоком, остановка.
**Independent Test**: поддельный поток: создать урок, получить вопрос с вариантами, ответить, получить объяснение.

- [X] T009 [US1] `create_lesson` (выключено → `Disabled`, пустая цель → `GoalEmpty`, имя из цели, уникальное, в `lessons_folder`) и `open_lesson` в `packages/core/src/ai/lesson/mod.rs`
- [X] T010 [US1] Ход: `begin_turn` (ответ в файл до журнала, проверка `expected`, сборка запроса, журнал 050), `send_turn` (поток в буфер, отмена), `finish_turn` (реплика, итог журнала, `Next`) с пределом 4 автопродолжений в `packages/core/src/ai/lesson/turn.rs`
- [X] T011 [US1] Тесты: ответ на диске до сети; выключено — 0 запросов; вопрос с вариантами и без; текст без инструментов; `LessonChanged`; отмена — в `packages/core/tests/common/lesson_cases.rs`
- [X] T012 [US1] Команды `ai_lesson_{list,create,open,answer,progress,set_state,set_language}` в `packages/host/src/commands.rs`, перегенерация `dispatch.rs`; обёртки в `packages/shell/src/{commands,lib}.rs`
- [X] T013 [US1] Сервер: команды в `apps/server/src/allowed.rs`, `ai_lesson_answer` в `AI_NETWORK_COMMANDS` в `apps/server/src/invoke.rs`, тест изоляции в `apps/server/tests/api.rs`
- [X] T014 [US1] wasm-биндинги `lessonList/Create/Open/Begin/Send/Finish/Progress/SetState/SetLanguage` в `packages/core/src/ai/lesson/web.rs`
- [X] T015 [US1] `LessonClient` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts` (цикл по `Next`), `ui/src/vault-client-tauri.ts`
- [X] T016 [US1] Локали `ui/src/i18n/locales/{ru,en}/lessons.ts`
- [X] T017 [US1] Вкладка урока по макету A–C, E, I: шапка, лента, вопрос, уверенность, «думаю вслух», `Mod+Enter`, поток и «Остановить» (`Esc`), язык, «Завершить» в `ui/src/lessons/{LessonTab,LessonQuestion}.tsx`; подключение в `ui/src/workspace/tabs/EditorGroup.tsx`; удалённый файл урока закрывает вкладку, как у заметки
- [X] T018 [US1] Диалог «Новый урок» по макету D, E и команда палитры в `ui/src/lessons/NewLessonDialog.tsx`, `ui/src/workspace/{Workspace,CommandPalette}.tsx`; поле «Папка уроков» в `ui/src/settings/AiSection.tsx` (FR-006)

## Phase 4: US2 — продолжить урок позже (P1)

**Goal**: список уроков, восстановление после перезапуска и обрыва, сжатие.
**Independent Test**: оборвать поток посреди текста, открыть урок заново, увидеть оборванную реплику и повторить.

- [X] T019 [US2] Промежуточный файл ответа: `checkpoint`, восстановление при открытии, удаление по итогу (research §5) в `packages/core/src/ai/lesson/partial.rs`; команда `ai_lesson_checkpoint` (host, shell, сервер, wasm)
- [X] T020 [US2] Сжатие: оценка объёма, ход `Compact`, событие `Summary`, сборка запроса после изложения (research §6) в `packages/core/src/ai/lesson/turn.rs`
- [X] T021 [US2] Тесты: обрыв сохраняет полученное; восстановление из промежуточного файла; повтор без нового ответа; сжатие и запрос после него; список с состояниями — в `packages/core/tests/common/lesson_cases.rs`
- [X] T022 [US2] Список уроков по макету G, H, оборванная реплика и «Запросить ответ снова», отказы и отметка сжатия по макету F в `ui/src/lessons/{LessonsScreen,LessonTab}.tsx`; вход в список из палитры

## Phase 5: US3 — учитель опирается на заметки (P2)

**Goal**: прикрепление заметок и инструмент `read_note` через проверку доступа 050.
**Independent Test**: прикреплённая заметка в журнале; заметка из запрещённой папки — отказ в уроке.

- [X] T023 [US3] Прикрепление при создании и `read_note`: `check_access_in`, обрезка 100 тыс. символов, событие `NoteRead`, результат инструмента в следующем запросе (research §12) в `packages/core/src/ai/lesson/turn.rs`
- [X] T024 [US3] Тесты: прикреплённая заметка в теле запроса и журнале; запрещённая папка и путь вне vault — `Denied`, текста в запросе нет; несуществующая — `Missing`
- [X] T025 [US3] Строки чтения и отказа со ссылкой на заметку (макет A, F), выбор заметок в диалоге (макет D) в `ui/src/lessons/{LessonTab,NewLessonDialog}.tsx`

## Phase 6: US4 — стоимость урока (P2)

**Goal**: счётчик примерной стоимости и объёма.
**Independent Test**: сумма счётчика равна сумме записей журнала урока.

- [X] T026 [US4] Объём и цена хода из потока, сумма в `LessonView` (`None` при неизвестной цене) в `packages/core/src/ai/lesson/{turn,file}.rs`; тест
- [X] T027 [US4] Счётчик в шапке вкладки (макет A, I) в `ui/src/lessons/LessonTab.tsx`

## Phase 7: Завершение

- [X] T028 Сценарий `scripts/dev/scenarios/ai-lesson.mjs`, снимок и сверка с макетом A, B, G
- [X] T029 Гейты `gates.sh all`
- [X] T030 Проверка `security-review-agent` (конституция, принцип IX), правка находок
- [X] T031 `/speckit-converge`, цепочка ядро → биндинг → `ui/src`, README, `**Status**: Implemented`

## Phase 8: Находки ревью безопасности (T030, research §14)

- [X] T032 [US1] H1: проверенный снимок подключения (провайдер, адрес, ключ) передаётся в каждый ход цикла; `send_turn` отказывает при расхождении с запросом, в `packages/core/src/ai/lesson/turn.rs`, `packages/host/src/commands.rs`; тест смены `ai.json` между ходами
- [X] T033 [US1] M1: запись урока не идёт через символические ссылки, свободное имя — по существованию, в `packages/core/src/ai/lesson/{mod,file}.rs`; тест
- [X] T034 [US1] M2: нормализация `lessons_folder` и `context_budget` при чтении в `packages/core/src/ai/settings.rs`; тест
- [X] T035 [US3] M3: `read_note` и прикрепление — только заметки и текст, предел размера до чтения; предел размера файла урока, в `packages/core/src/ai/lesson/{turn,file,mod}.rs`; тесты
- [X] T036 [US3] M4: предел числа и объёма прикреплённых заметок, нормализация путей, `by_user` только из начальных событий, в `packages/core/src/ai/lesson/turn.rs`; тест
- [X] T037 [US1] M5: линейный разбор потока, предел строки SSE, дельта в буфер, общий предел потока 10 мин в `packages/core/src/ai/lesson/{stream,partial}.rs`, `packages/core/src/ai/transport.rs`; предел запросов на аккаунт в `apps/server/src/invoke.rs`; тесты
- [X] T038 [US2] M6: цикл хода на хосте доживает до итога при обрыве соединения либо снимает буфер и слот отмены; уборка забытых буферов, в `packages/host/src/commands.rs`, `packages/core/src/ai/lesson/partial.rs`
- [X] T039 [US1] L1, L2: слот отмены до буфера и записи ответа; отмена на весь цикл, в `packages/core/src/ai/{cancel,lesson/turn}.rs`; тесты
- [X] T040 [US2] L3: `checkpoint` берёт путь из буфера, пути нормализуются, в `packages/core/src/ai/lesson/partial.rs`; тест
- [X] T041 [US2] L5: атомарное дописывание урока на native, уроки не разбираются как Typst для ссылок, в `packages/core/src/ai/lesson/file.rs`, `packages/core/src/vault/native.rs`
- [X] T042 [US1] L6, L7: команды принимают только пути вида `Lesson` вне служебного места; имя из цели без зарезервированных имён Windows и символов форматирования; тесты
- [X] T043 [US1] L8: до 5 вариантов, пределы длины вопроса и вариантов в `packages/core/src/ai/lesson/turn.rs`; тест
- [X] T044 [US3] L9: `check_access` закрывает `.stversions/` в `packages/core/src/ai/access.rs`; тест
- [X] T045 L4: проверить, что отчёт «неполные правила» `.stignore` включает `lesson-partial` через `derived_paths`
- [X] T046 [US2] `waiting` в `LessonSummary` в `packages/core/src/ai/lesson/mod.rs`; список уроков показывает «ждёт ответа» и «ответ оборван» по макету G в `ui/src/lessons/LessonsScreen.tsx`

## Dependencies

Phase 1 → Phase 2 → US1 → US2; US3 и US4 — после US1, независимо друг от друга. T012–T014
можно вести параллельно после T010; T016 — параллельно с ядром.

## Implementation Strategy

MVP — US1 (урок с вопросами и потоком) на всех таргетах; затем US2 (надёжность), US3, US4.
