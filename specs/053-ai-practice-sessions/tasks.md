# Tasks: Практика с ИИ

**Input**: `specs/053-ai-practice-sessions/` (spec, plan, research, data-model, contracts)

**Tests**: в `packages/core/tests/common/lesson_cases.rs`, на обоих путях.

**Зависит от**: 051.

## Phase 1: Основа

- [ ] T001 `kind` в шапке, события `Material`, `Problem`, `Hint`, `HintRequest`, `GiveUp`, `Verdict`, `NextProblem`, производное `practice` со скрытием решения до итога в `packages/core/src/ai/lesson/{file,practice}.rs` (data-model.md, research §3)

## Phase 2: US1 — решить задачу по пройденному (P1)

**Goal**: практика по материалу, задача без названия знания, итог, разбор, решение.
**Independent Test**: поддельный поток: задача → вид без решения → ответ → итог и решение.

- [ ] T002 [US1] Инструкции агента практики ru/en (задача не называет знание, разнообразие) и инструменты `pose_problem`, `conclude` в `packages/core/src/ai/lesson/prompt.rs`; обработка и ожидание пользователя после итога в `packages/core/src/ai/lesson/turn.rs`
- [ ] T003 [US1] `create_practice`: сбор материала из уроков, заметок через `check_access_in`, описания; событие `Material` (research §4) в `packages/core/src/ai/lesson/practice.rs`
- [ ] T004 [US1] Тесты: решение скрыто до итога; `NextProblem` обязателен; материал из запрещённой папки отклонён; задача без решения (Edge Case)
- [ ] T005 [US1] Команды `ai_practice_create`, действия `ai_lesson_answer` (host, shell, сервер `allowed.rs`, wasm `practiceCreate`); клиент `ui/src/vault-client{,-web,-tauri}.ts`; локали
- [ ] T006 [US1] Диалог «Новая практика» по макету A в `ui/src/lessons/NewPracticeDialog.tsx`, команда палитры; лента задач, итог, решение, «Следующая задача» по макету B–D в `ui/src/lessons/PracticeFeed.tsx`, `ui/src/lessons/LessonTab.tsx`

## Phase 3: US2 — подсказка вместо ответа (P2)

**Goal**: подсказка без раскрытия решения, «Сдаюсь».
**Independent Test**: два запроса подсказки — решение скрыто; «Сдаюсь» — итог `Unsolved` и решение.

- [ ] T007 [US2] Инструмент `give_hint`, действия `HintRequest` и `GiveUp` в `packages/core/src/ai/lesson/turn.rs`; тест
- [ ] T008 [US2] Кнопки «Подсказка», «Сдаюсь», карточки подсказок по макету B в `ui/src/lessons/PracticeFeed.tsx`

## Phase 4: US3 — сводка (P3)

- [ ] T009 [US3] Сводка задач по макету E с переходом к разбору в `ui/src/lessons/PracticeSummary.tsx`; метка «Практика» в списке уроков `ui/src/lessons/LessonsScreen.tsx`

## Phase 5: Завершение

- [ ] T010 Сценарий `scripts/dev/scenarios/ai-practice.mjs`, сверка с макетом
- [ ] T011 Гейты `gates.sh all`
- [ ] T012 Проверка `security-review-agent`, правка находок
- [ ] T013 `/speckit-converge`, README, `**Status**: Implemented`
