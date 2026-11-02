# Tasks: Фоновое исследование в уроке

**Input**: `specs/056-ai-background-research/` (spec, plan, research, data-model, contracts)

**Tests**: в `packages/core/tests/common/research_cases.rs`, на обоих путях.

**Зависит от**: 051, 054 (реестр заданий), 055 (`pdf_text`).

## Phase 1: Основа

- [ ] T001 Общий реестр заданий `ai/jobs.rs` (вынести из `ai/knowledge/jobs.rs`), вид `Research`, очередь по `research_parallel`; настройки `web_research`, `research_parallel`; события урока (data-model.md)
- [ ] T002 Сверить с живой документацией серверные инструменты веб-поиска Anthropic и OpenAI (имя, версия, формат цитат, цена) и записать в `research.md` §1; форма запроса в `packages/core/src/ai/protocol.rs`

## Phase 2: US1 — исследование, пока идёт урок (P1)

**Goal**: веб-исследование в фоне, карточка, результат-заметка.
**Independent Test**: поддельный поток с `submit_research` → заметка Typst с источниками; урок в это время принимает ответ.

- [ ] T003 [US1] Задание веб-исследования: запрос с инструментом поиска провайдера и `submit_research`, сборка заметки Typst с экранированием (research §4) в `packages/core/src/ai/research.rs`
- [ ] T004 [US1] Инструмент учителя `research` (только при включённом и доступном поиске), событие в следующем ходе (research §3) в `packages/core/src/ai/lesson/{turn,prompt}.rs`
- [ ] T005 [US1] Тесты: выключено — инструмента поиска нет ни в одном запросе (SC-003); свой адрес — недоступно; заметка экранирована; источник обязателен или пометка (SC-002); отмена; `Interrupted`
- [ ] T006 [US1] Команды `ai_research_start`, `ai_jobs` (host, shell, сервер с пределом, wasm); клиент; локали
- [ ] T007 [US1] Карточки по макету A, B, D и поле «Исследовать…» в `ui/src/lessons/{ResearchCard,ResearchInput,LessonTab}.tsx`; настройки по макету E в `ui/src/settings/AiSection.tsx`

## Phase 3: US2 — прочесть PDF из vault (P2)

**Goal**: изучение PDF с оценкой и страницами.
**Independent Test**: PDF в разрешённой папке → итог со страницами; в запрещённой — отказ.

- [ ] T008 [US2] `ai_research_estimate`, задание `Pdf` с текстом страниц и источниками-страницами, инструмент учителя `study_pdf` в `packages/core/src/ai/research.rs`; тесты
- [ ] T009 [US2] «В PDF…», оценка до отправки по макету C

## Phase 4: Завершение

- [ ] T010 Сценарий `scripts/dev/scenarios/ai-research.mjs`, сверка с макетом
- [ ] T011 Гейты `gates.sh all`
- [ ] T012 Проверка `security-review-agent`, правка находок
- [ ] T013 `/speckit-converge`, README, `**Status**: Implemented`
