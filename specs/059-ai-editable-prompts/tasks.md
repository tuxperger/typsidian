# Tasks: Видимые и редактируемые инструкции агентов

**Input**: `specs/059-ai-editable-prompts/` (spec, plan, research, data-model, contracts)

**Tests**: в `packages/core/tests/common/prompts_cases.rs`, на обоих путях.

**Зависит от**: 051; агенты 052–056 добавляют свои инструкции по мере готовности.

## Phase 1: Основа

- [ ] T001 Исходные инструкции всех готовых агентов с версиями и единая точка `prompt_for` в `packages/core/src/ai/prompts.rs`; ходы 051 (и готовых 052–056) берут инструкцию только оттуда

## Phase 2: US1 — прочитать инструкции (P1)

- [ ] T002 [US1] `PromptView`, `ai_prompts_get` (ядро, host, shell, сервер, wasm); тест SC-001: тело запроса = действующий текст
- [ ] T003 [US1] Подраздел «Инструкции агентов» по макету A в `ui/src/settings/AiPrompts.tsx`, `ui/src/settings/AiSection.tsx`; клиент; локали

## Phase 3: US2 — править и вернуть (P1)

- [ ] T004 [US2] Хранение правок, `ai_prompt_set` (пустая — `PromptEmpty`), `ai_prompt_reset` в `packages/core/src/ai/prompts.rs`; тесты, включая SC-003 (правка не открывает запрещённую папку)
- [ ] T005 [US2] Правка, сохранение, возврат, предупреждение о длине по макету B, D в `ui/src/settings/AiPrompts.tsx`

## Phase 4: US3 — исходная обновилась (P3)

- [ ] T006 [US3] `base_version`, пометка, `ai_prompt_keep`; тест
- [ ] T007 [US3] Сравнение по макету C в `ui/src/settings/PromptCompare.tsx`

## Phase 5: Завершение

- [ ] T008 Сверка с макетом в браузере
- [ ] T009 Гейты `gates.sh all`
- [ ] T010 `/speckit-converge`, README, `**Status**: Implemented`
