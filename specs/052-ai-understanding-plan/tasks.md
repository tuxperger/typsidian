# Tasks: Замер понимания и план урока

**Input**: `specs/052-ai-understanding-plan/` (spec, plan, research, data-model, contracts)

**Tests**: в `packages/core/tests/common/lesson_cases.rs`, на обоих путях, поддельный поток 051.

**Зависит от**: 051 (модуль `ai/lesson`, вкладка урока).

## Phase 1: Основа

- [X] T001 Поле `start` в шапке, события `Phase`, `ProbeResult`, `Plan`, `PlanProgress`, `part` у вопроса, `dont_know` у ответа, производные поля `LessonView` в `packages/core/src/ai/lesson/file.rs` (data-model.md)

## Phase 2: US1 — замер перед уроком (P1)

**Goal**: стадия замера с вопросами по частям, итог, пропуск.
**Independent Test**: поддельный поток: два вопроса замера, «Не знаю», `probe_done` → стадия `Teach`.

- [X] T002 [US1] Инструкции и инструменты замера `probe_question`, `probe_done` ru/en, предел 15 вопросов (research §2, §6) в `packages/core/src/ai/lesson/prompt.rs`; выбор инструкций по стадии и обработка инструментов в `packages/core/src/ai/lesson/turn.rs`
- [X] T003 [US1] `create_lesson` со `start`, `skip_probe` в `packages/core/src/ai/lesson/mod.rs`
- [X] T004 [US1] Тесты: стадия замера в запросе, «Не знаю», итог и переход, пропуск, старый урок без `start` — `Teach`
- [X] T005 [US1] Команды: `start` у `ai_lesson_create`, `ai_lesson_skip_probe` (host, shell, сервер `allowed.rs`, wasm `lessonSkipProbe`)
- [X] T006 [US1] Клиент и локали: `ui/src/vault-client{,-web,-tauri}.ts`, `ui/src/i18n/locales/{ru,en}/lessons.ts`
- [X] T007 [US1] UI по макету A–C: выбор «С замера / Сразу» в `ui/src/lessons/NewLessonDialog.tsx`, метка стадии, «Не знаю», «Пропустить замер» в `ui/src/lessons/LessonTab.tsx`, итог в `ui/src/lessons/ProbeResult.tsx`

## Phase 3: US2 — видеть план к цели (P1)

**Goal**: план-карта и список с текущим и пройденными шагами.
**Independent Test**: после `plan_set` и `plan_progress` вид урока содержит шаги, рёбра, координаты и состояния.

- [X] T008 [US2] Инструменты `plan_set` (проверки research §3) и `plan_progress`, отказ модели с причиной, якоря шагов (research §5) в `packages/core/src/ai/lesson/{plan,turn}.rs`
- [X] T009 [US2] Раскладка слоями (research §4) в `packages/core/src/ai/lesson/plan.rs`
- [X] T010 [US2] Тесты: цикл, висячая ссылка, > 40 шагов — отклонены и не в виде; верный план; прогресс; раскладка без наложений
- [X] T011 [US2] Колонка плана по макету D, E: карта SVG и список, текущий и пройденные, переход к якорю, на узком экране — лист со списком, в `ui/src/lessons/LessonPlan.tsx`, `ui/src/lessons/LessonTab.tsx`

## Phase 4: US3 — план перестраивается (P2)

**Goal**: новые версии с пометкой и историей.
**Independent Test**: второй `plan_set` даёт версию 2, пометку `added` и причину; версия 1 доступна.

- [X] T012 [US3] `plan_history` и `plan_version` в `packages/core/src/ai/lesson/plan.rs`; команда `ai_lesson_plan_version` (host, shell, сервер, wasm); тест
- [X] T013 [US3] Пометка изменения и история версий по макету D, F в `ui/src/lessons/LessonPlan.tsx`

## Phase 5: Завершение

- [X] T014 Сценарий `scripts/dev/scenarios/ai-lesson-plan.mjs`, сверка с макетом B–F
- [X] T015 Гейты `gates.sh all`
- [X] T016 Проверка `security-review-agent` (принцип IX), правка находок
- [X] T017 `/speckit-converge`, README, `**Status**: Implemented`

## Phase 6: Находки ревью безопасности (T016, research §9)

- [X] T018 M1: якорь шага из прямого прохода в `packages/core/src/ai/lesson/plan.rs`; тест на файле с 200 тыс. `PlanProgress`
- [X] T019 M2: разбор `after` обрывается сверх предела в `packages/core/src/ai/lesson/plan.rs`; тест
- [X] T020 L1: обрезка неизвестной ссылки в причине отказа; тест
- [X] T021 L2: пределы длины и пометка «данные урока» для блоков плана и замера в `packages/core/src/ai/lesson/prompt.rs`; тест на подброшенном длинном плане
- [X] T022 `waiting: User` после итога замера в `packages/core/src/ai/lesson/file.rs`; тест

## Dependencies

051 → T001 → US1 → US2 → US3.
