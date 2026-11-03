# Tasks: Граф знаний из уроков

**Input**: `specs/054-ai-knowledge-graph/` (spec, plan, research, data-model, contracts)

**Tests**: в `packages/core/tests/common/knowledge_cases.rs`, на обоих путях.

**Зависит от**: 051; 052 и 053 — для выбора утверждений (US3).

## Phase 1: Основа

- [X] T001 Хранилище графа: строки трёх файлов, чтение с пропуском битых, дописывание, проекция с надгробиями в `packages/core/src/ai/knowledge/store.rs` (data-model.md, research §1)
- [X] T002 Настройки `knowledge_auto`, `knowledge_agent_reads`, `knowledge_map` в `packages/core/src/ai/settings.rs`; перенос источника при переименовании урока или заметки в `packages/core/src/vault/relocate.rs`

## Phase 2: US1 — граф копится из уроков (P1)

**Goal**: фоновое извлечение из объяснений учителя с объединением.
**Independent Test**: урок из трёх объяснений → утверждения с источником; повтор — 0 дублей.

- [X] T003 [US1] Пачки непокрытых объяснений, запрос с инструментом `submit_findings` и до 50 найденных локально утверждений (research §2) в `packages/core/src/ai/knowledge/extract.rs`
- [X] T004 [US1] Объединение: id по хэшу, `same_as`, отклонение висячих цепочек (research §3) в `packages/core/src/ai/knowledge/merge.rs`
- [X] T005 [US1] Реестр заданий, прогресс, отмена, автоматический запуск после хода учителя (research §4) в `packages/core/src/ai/knowledge/jobs.rs`, `packages/core/src/ai/lesson/turn.rs`
- [X] T006 [US1] Тесты: только объяснения в запросе (SC-004); 0 дублей (SC-002); `same_as`; висячая цепочка; обрыв посреди пачек сохраняет готовое; автозапуск по порогу
- [X] T007 [US1] Команды `ai_knowledge_extract`, `ai_knowledge_jobs` (host, shell, сервер с пределом, wasm `knowledgeBegin/Send/Finish/Jobs`); клиент `ui/src/vault-client{,-web,-tauri}.ts`; локали `ui/src/i18n/locales/{ru,en}/knowledge.ts` — Rust-часть (ядро, host, shell, сервер, wasm) и клиент с локалями сделаны; клиент проверен в браузере (сценарий ai-knowledge)
- [X] T008 [US1] В уроке: «Пополнить граф», строка итога, полоса задания с отменой по макету A, C в `ui/src/lessons/LessonTab.tsx`, `ui/src/knowledge/KnowledgeJobs.tsx`; настройки по макету D в `ui/src/settings/AiSection.tsx`

## Phase 3: US2 — смотреть граф (P1)

**Goal**: карта, таблица цепочек, утверждение с источниками, поиск.
**Independent Test**: после извлечения вид содержит узлы, рёбра, группы; переход к источнику открывает урок на событии.

- [X] T009 [US2] Форма графа в виде `VaultGraph` 025, развилки цепочек, группы по источникам, поиск по формулировкам в `packages/core/src/ai/knowledge/view.rs`; тест 2000 утверждений < 1 с
- [X] T010 [US2] Команды `ai_knowledge_graph`, `ai_knowledge_chains`, `ai_knowledge_proposition` (host, shell, сервер, wasm)
- [X] T011 [US2] Вид «Граф знаний» по макету A, B, F: карта на `ui/src/graph/GraphCanvas.tsx`, таблица, поиск, колонка утверждения, переход к событию урока, команда палитры — в `ui/src/knowledge/{KnowledgeView,KnowledgeChains,PropositionPanel}.tsx`, `ui/src/workspace/{Workspace,CommandPalette}.tsx`

## Phase 4: US3 — уроки и практика читают граф (P2)

**Goal**: выбор утверждений для замера и практики, чтение агентом по разрешению.
**Independent Test**: выбранные утверждения в теле запроса и журнале; при выключенном чтении инструмента `read_knowledge` нет в запросе.

- [X] T012 [US3] Инструмент `read_knowledge` по настройке, событие `KnowledgeRead`, утверждения в `Material` 053 и в старте замера 052 (research §6) в `packages/core/src/ai/lesson/{turn,prompt,practice}.rs`; тесты, включая запрет папки графа
- [X] T013 [US3] Выбор утверждений в `ui/src/lessons/{NewPracticeDialog,NewLessonDialog}.tsx`, строка чтения графа в ленте урока

## Phase 5: US4 — править и чистить граф (P3)

- [X] T014 [US4] Удаление утверждения, вклада источника, урока с `keep` (research §7); команды `ai_knowledge_remove`, `ai_lesson_delete`; тесты
- [X] T015 [US4] Диалоги удаления по макету E в `ui/src/knowledge/PropositionPanel.tsx`, `ui/src/lessons/LessonsScreen.tsx`

## Phase 6: Завершение

- [X] T016 Сценарий `scripts/dev/scenarios/ai-knowledge.mjs`, сверка с макетом
- [X] T017 Гейты `gates.sh all`
- [X] T018 Проверка `security-review-agent`, правка находок
- [X] T019 `/speckit-converge`, README, `**Status**: Implemented`

## Phase 7: Находки ревью безопасности (T018, research §12)

- [X] T020 H1: предел пачек в ядре, сброс покрытия, конец задания без роста покрытия в `packages/core/src/ai/knowledge/{extract,store}.rs`; тесты пересозданного урока и будущего `Moved`
- [X] T021 M1: утверждения с закрытыми источниками не уходят модели (известные, `read_knowledge`, снимок `Material.knowledge`); тест
- [X] T022 M2: `Moved` только для путей источников, последние переносы; тест удаления вклада после 300 переносов
- [X] T023 M3: `resolve` без квадратичности; тест на враждебном `sources.jsonl`
- [X] T024 L1: время надгробия, одни часы записи; тест
- [X] T025 L2: `delete_lesson` — ключ vault, ссылки, отмена идущего задания; тест
- [X] T026 L3, L4, L5: охрана задания до `begin`, сверка `finish_extract` с реестром, `end()` снимает только свою отметку
- [X] T027 L6: сервер — слот на пачку, отдельный предел фоновых заданий на аккаунт в `apps/server/src/invoke.rs`; тест
- [X] T028 L7, L8: ссылки модели только на отправленные id; надгробия в заполненный файл; тесты
- [X] T029 L9: свёрнутая форма пути для покрытия и конфликта заданий; тест
- [X] T030 `ai_lesson_delete` даёт обычное событие удаления файла vault на обоих таргетах (как команда удаления заметки); обход в `ui/src/workspace/Workspace.tsx` убрать
