# Tasks: Наглядные объяснения и демонстрации

**Input**: `specs/057-ai-visuals-demos/` (spec, plan, research, data-model, contracts)

**Tests**: в `packages/core/tests/common/figure_cases.rs`, на обоих путях.

**Зависит от**: 051.

## Phase 1: Основа

- [ ] T001 `FigureWorld` без файлов vault, только главный файл, встроенные шрифты и локальные пакеты (research §3) в `packages/core/src/preview/figure_world.rs`
- [ ] T002 Сборка с пределом 5 с (native — поток, web — воркер), предел 200 КБ, `FigureRender` в `packages/core/src/ai/figure.rs`, `ui/src/preview/compile-worker.ts`
- [ ] T003 Тесты: чтение файла и картинки — ошибка; бесконечный цикл — `TimedOut`; 300 КБ — `TooLarge` (SC-001, SC-002)

## Phase 2: US1 — схема в уроке (P1)

**Goal**: инструмент `draw`, заметка, показ собранной.
**Independent Test**: поддельный поток с ошибочной и исправленной схемой.

- [ ] T004 [US1] Инструмент `draw`, запись заметки, сборка до показа, ошибки модели, событие `Figure` (research §1) в `packages/core/src/ai/lesson/{turn,prompt,file}.rs`, `packages/core/src/ai/figure.rs`; тесты
- [ ] T005 [US1] Команда `ai_figure_render` (host, shell, сервер, wasm); клиент; локали
- [ ] T006 [US1] Схема в ленте по макету A, C в `ui/src/lessons/{LessonFigure,LessonTab}.tsx`; пересборка при правке заметки

## Phase 3: US2 — интерактивная демонстрация (P2)

**Goal**: параметры, ползунки, пересборка.
**Independent Test**: значение параметра меняет SVG; выключенная настройка убирает инструмент.

- [ ] T007 [US2] `DemoParams` из первой строки, `sys.inputs`, инструмент `demonstrate`, настройка `demos_enabled` в `packages/core/src/ai/{figure,settings}.rs`, `packages/core/src/ai/lesson/prompt.rs`; команда `ai_figure_params`; тесты
- [ ] T008 [US2] Ползунки и пересборка с задержкой по макету B, настройка по макету D в `ui/src/lessons/DemoControls.tsx`, `ui/src/settings/AiSection.tsx`

## Phase 4: Завершение

- [ ] T009 Сценарий `scripts/dev/scenarios/ai-figures.mjs`, сверка с макетом
- [ ] T010 Гейты `gates.sh all`
- [ ] T011 Проверка `security-review-agent` (изоляция мира сборки), правка находок
- [ ] T012 `/speckit-converge`, README, `**Status**: Implemented`
