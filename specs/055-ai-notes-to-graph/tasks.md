# Tasks: Заметки в граф знаний

**Input**: `specs/055-ai-notes-to-graph/` (spec, plan, research, data-model, contracts)

**Tests**: в `packages/core/tests/common/knowledge_cases.rs`, на обоих путях.

**Зависит от**: 054.

## Phase 1: Основа

- [ ] T001 Текст PDF по страницам через `hayro-interpret` (или запасной вариант research §3), признак «нет текста» в `packages/core/src/ai/knowledge/pdf_text.rs`; тест на PDF из `packages/core/tests/fixtures/`

## Phase 2: US1 — импортировать выбранные заметки (P1)

**Goal**: оценка, подтверждение, фоновый импорт, итог по файлам.
**Independent Test**: папка из трёх заметок → утверждения с источниками-заметками; запрещённая подпапка — в пропущенных.

- [ ] T002 [US1] `import_estimate`: раскрытие папок, проверка доступа, виды, части, объём, стоимость (research §2, §4) в `packages/core/src/ai/knowledge/import.rs`
- [ ] T003 [US1] Задание импорта по частям через конвейер 054 с инструкциями для заметок, итог по файлам, отмена (research §1) в `packages/core/src/ai/knowledge/{import,extract,jobs}.rs`
- [ ] T004 [US1] Тесты: пропуски с причинами; запрещённые не в запросах (SC-001); отмена — оставшиеся не отправлены; частичный сбой — `Partial`
- [ ] T005 [US1] Команды `ai_knowledge_import_estimate`, `ai_knowledge_import` (host, shell, сервер с пределом, wasm); клиент; локали
- [ ] T006 [US1] Пункт меню дерева, подтверждение, прогресс и итог по макету A–D в `ui/src/vault-tree/VaultTreeActions.tsx`, `ui/src/knowledge/{ImportConfirm,ImportJob}.tsx`; кнопка «Добавить заметки…» в `ui/src/knowledge/KnowledgeView.tsx`

## Phase 3: US2 — повторный импорт (P2)

**Goal**: обновление вклада без дублей, узнавание после переименования.
**Independent Test**: повтор неизменённой — 0 запросов; изменённой — только изменённые части.

- [ ] T007 [US2] Покрытие частей по хэшу, `Moved` при переименовании, повтор незавершённых частей в `packages/core/src/ai/knowledge/import.rs`; тесты SC-002
- [ ] T008 [US2] «Повторить» у файлов итога (макет D)

## Phase 4: Завершение

- [ ] T009 Сценарий `scripts/dev/scenarios/ai-knowledge-import.mjs`, сверка с макетом
- [ ] T010 Гейты `gates.sh all`
- [ ] T011 Проверка `security-review-agent`, правка находок
- [ ] T012 `/speckit-converge`, README, `**Status**: Implemented`
