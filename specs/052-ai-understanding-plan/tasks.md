# Tasks: Замер понимания и план урока

**Input**: `specs/052-ai-understanding-plan/` (spec, plan, research, data-model, contracts)

**Tests**: в `packages/core/tests/common/lesson_cases.rs`, на обоих путях, поддельный поток 051.

**Зависит от**: 051 (модуль `ai/lesson`, вкладка урока).

## Phase 1: Основа

- [ ] T001 Поле `start` в шапке, события `Phase`, `ProbeResult`, `Plan`, `PlanProgress`, `part` у вопроса, `dont_know` у ответа, производные поля `LessonView` в `packages/core/src/ai/lesson/file.rs` (data-model.md)

## Phase 2: US1 — замер перед уроком (P1)

**Goal**: стадия замера с вопросами по частям, итог, пропуск.
**Independent Test**: поддельный поток: два вопроса замера, «Не знаю», `probe_done` → стадия `Teach`.

- [ ] T002 [US1] Инструкции и инструменты замера `probe_question`, `probe_done` ru/en, предел 15 вопросов (research §2, §6) в `packages/core/src/ai/lesson/prompt.rs`; выбор инструкций по стадии и обработка инструментов в `packages/core/src/ai/lesson/turn.rs`
- [ ] T003 [US1] `create_lesson` со `start`, `skip_probe` в `packages/core/src/ai/lesson/mod.rs`
- [ ] T004 [US1] Тесты: стадия замера в запросе, «Не знаю», итог и переход, пропуск, старый урок без `start` — `Teach`
- [ ] T005 [US1] Команды: `start` у `ai_lesson_create`, `ai_lesson_skip_probe` (host, shell, сервер `allowed.rs`, wasm `lessonSkipProbe`)
- [ ] T006 [US1] Клиент и локали: `ui/src/vault-client{,-web,-tauri}.ts`, `ui/src/i18n/locales/{ru,en}/lessons.ts`
- [ ] T007 [US1] UI по макету A–C: выбор «С замера / Сразу» в `ui/src/lessons/NewLessonDialog.tsx`, метка стадии, «Не знаю», «Пропустить замер» в `ui/src/lessons/LessonTab.tsx`, итог в `ui/src/lessons/ProbeResult.tsx`

## Phase 3: US2 — видеть план к цели (P1)

**Goal**: план-карта и список с текущим и пройденными шагами.
**Independent Test**: после `plan_set` и `plan_progress` вид урока содержит шаги, рёбра, координаты и состояния.

- [ ] T008 [US2] Инструменты `plan_set` (проверки research §3) и `plan_progress`, отказ модели с причиной, якоря шагов (research §5) в `packages/core/src/ai/lesson/{plan,turn}.rs`
- [ ] T009 [US2] Раскладка слоями (research §4) в `packages/core/src/ai/lesson/plan.rs`
- [ ] T010 [US2] Тесты: цикл, висячая ссылка, > 40 шагов — отклонены и не в виде; верный план; прогресс; раскладка без наложений
- [ ] T011 [US2] Колонка плана по макету D, E: карта SVG и список, текущий и пройденные, переход к якорю, на узком экране — лист со списком, в `ui/src/lessons/LessonPlan.tsx`, `ui/src/lessons/LessonTab.tsx`

## Phase 4: US3 — план перестраивается (P2)

**Goal**: новые версии с пометкой и историей.
**Independent Test**: второй `plan_set` даёт версию 2, пометку `added` и причину; версия 1 доступна.

- [ ] T012 [US3] `plan_history` и `plan_version` в `packages/core/src/ai/lesson/plan.rs`; команда `ai_lesson_plan_version` (host, shell, сервер, wasm); тест
- [ ] T013 [US3] Пометка изменения и история версий по макету D, F в `ui/src/lessons/LessonPlan.tsx`

## Phase 5: Завершение

- [ ] T014 Сценарий `scripts/dev/scenarios/ai-lesson-plan.mjs`, сверка с макетом B–F
- [ ] T015 Гейты `gates.sh all`
- [ ] T016 Проверка `security-review-agent` (принцип IX), правка находок
- [ ] T017 `/speckit-converge`, README, `**Status**: Implemented`

## Dependencies

051 → T001 → US1 → US2 → US3.
