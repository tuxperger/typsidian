# Tasks: Разные модели для разных задач

**Input**: `specs/060-ai-model-roles/` (spec, plan, research, data-model, contracts)

**Tests**: в `packages/core/tests/common/roles_cases.rs`, на обоих путях.

**Зависит от**: 050, 051; роли 052–058 подключаются по мере их готовности.

## Phase 1: Основа

- [ ] T001 `connections`, `roles` в `AiSettings` (совместимо со старым `ai.json`); секреты `AiConnectionKey(id)`, `AiConnectionOrigin(id)` и `forget_vault` в `packages/core/src/ai/settings.rs`, `packages/core/src/sync/secrets.rs`
- [ ] T002 Сверить с живой документацией пометки кэширования Anthropic и хранение кэша OpenAI (поля, сроки, цены кэша) и записать в `research.md` §4

## Phase 2: US1 — модели по ролям (P1)

**Goal**: назначение подключения и модели роли; журнал с ролью.
**Independent Test**: извлечение на своём подключении — адрес, ключ и модель в запросе и журнале.

- [ ] T003 [US1] `resolve_role` и перевод всех ходов готовых агентов на него; одобрение адреса подключения (M1) в `packages/core/src/ai/{roles,client}.rs`
- [ ] T004 [US1] `Sent.role`; отказ роли с названием роли (`AiFailure` + роль в ответе команд) в `packages/core/src/ai/journal.rs`
- [ ] T005 [US1] Тесты: умолчания = 051 (SC-003); роль на своём подключении; ключ основного не уходит на адрес подключения; удалённое подключение — `fallback`
- [ ] T006 [US1] Команды `ai_connection_*`, `ai_roles_resolved` (host, shell, сервер с флагом и пределом, wasm); клиент; локали
- [ ] T007 [US1] Подключения и роли по макету A–C в `ui/src/settings/{AiConnections,AiRoles}.tsx`; роль в журнале по макету D в `ui/src/settings/AiJournal.tsx`

## Phase 3: US2 — хранение кэша (P3)

- [ ] T008 [US2] Предпочтение кэша в теле запроса по провайдеру (research §4), недоступно для своего адреса; объёмы кэша в `Done` и цене в `packages/core/src/ai/{protocol,price,journal}.rs`; тесты
- [ ] T009 [US2] «Дополнительно: хранение кэша» с предупреждением по макету B; столбец кэша в журнале

## Phase 4: Завершение

- [ ] T010 Сверка с макетом в браузере
- [ ] T011 Гейты `gates.sh all`
- [ ] T012 Проверка `security-review-agent` (секреты подключений, адреса), правка находок
- [ ] T013 `/speckit-converge`, README, `**Status**: Implemented`
