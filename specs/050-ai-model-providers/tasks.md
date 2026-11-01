# Tasks: Провайдеры моделей для обучения с ИИ

**Input**: `specs/050-ai-model-providers/` (spec, plan, research, data-model, contracts)

**Tests**: тесты ядра — в общей таблице `tests/common/ai_cases.rs`, на обоих путях.

## Phase 1: Основа

- [X] T001 Подключить `futures-util` и `futures-channel` к ядру и объявить модуль `ai` в `packages/core/Cargo.toml`, `packages/core/src/lib.rs`
- [X] T002 Типы `AiSettings`, `AiProvider`, `AiFailure`, чтение и запись `.typsidian/ai.json` в `packages/core/src/ai/settings.rs` (нечитаемый файл = выключено)
- [X] T003 Добавить `ai-journal.jsonl` в `derived_paths()` в `packages/core/src/vault/service_dir.rs`
- [X] T004 Виды секретов `AiAnthropicKey`, `AiOpenAiKey`, `AiCustomKey` в `packages/core/src/sync/secrets.rs` и `forget_vault` в `packages/host/src/sync.rs`

## Phase 2: Отказы и адрес (US4, FR-009, FR-010, FR-017)

- [X] T005 [US4] `classify_endpoint` в `packages/core/src/ai/endpoint.rs`
- [X] T006 [US4] Таблица отказов по кодам ответа и разбор ответов Anthropic и совместимого протокола в `packages/core/src/ai/protocol.rs`
- [X] T007 [US4] `AiTransport`: native — `reqwest` без перенаправлений, wasm — `fetch` с `redirect: "error"` и `AbortController`, в `packages/core/src/ai/transport.rs`
- [X] T008 [US4] Предел ожидания и реестр отмены `AiCancels` в `packages/core/src/ai/client.rs`
- [X] T009 [US4] Тесты: отказы, отмена, адреса; native — отказ соединения и перенаправление на loopback в `packages/core/tests/ai_native.rs`
- [X] T010 [US4] Сверить браузерный заголовок Anthropic с живой документацией и записать результат в `research.md` §2

## Phase 3: Журнал и стоимость (US2)

- [X] T011 [US2] Журнал: `sent`, `done`, проекция, лимит 500, очистка в `packages/core/src/ai/journal.rs`
- [X] T012 [US2] Справочник цен и `estimate` в `packages/core/src/ai/price.rs`
- [X] T013 [US2] `probe` и `list_models`: выключено = `Disabled`, журнал до запроса, сбой журнала останавливает запрос, в `packages/core/src/ai/client.rs`
- [X] T014 [US2] Тесты журнала и `probe`, включая ноль запросов при выключенной функции (SC-002), на обоих путях в `packages/core/tests/common/ai_cases.rs`, `packages/core/tests/ai_wasm.rs`

## Phase 4: Запрещённые папки (US3)

- [X] T015 [US3] `check_access` в `packages/core/src/ai/access.rs`
- [X] T016 [US3] Следование запрета за переносом в `packages/core/src/vault/relocate.rs`
- [X] T017 [US3] Тесты доступа и переноса в `packages/core/tests/common/ai_cases.rs`

## Phase 5: Команды (US1)

- [X] T018 [US1] Команды `ai_*` в `packages/host/src/commands.rs`, перегенерация `dispatch.rs`
- [X] T019 [US1] Обёртки и регистрация в `packages/shell/src/commands.rs`, `packages/shell/src/lib.rs`
- [X] T020 [US1] Сервер: `allowed.rs`, флаг `ai_custom_endpoint` в `config.rs`, отказ и подстановка своего `vaultId` в `invoke.rs`, тест изоляции в `apps/server/tests`
- [X] T021 [US1] wasm-биндинги в `packages/core/src/ai/web.rs`
- [X] T022 [US1] `AiClient` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts`

## Phase 6: Интерфейс (US1, US2, US3, US4)

- [X] T023 [US1] Локали `ui/src/i18n/locales/{ru,en}/ai.ts`
- [X] T024 [US1] Раздел настроек по макету A–F, I в `ui/src/settings/AiSection.tsx`, `ui/src/settings/SettingsPanel.tsx`
- [X] T025 [US2] Журнал по макету G, H в `ui/src/settings/AiJournal.tsx`
- [X] T026 [US4] Сценарий `scripts/dev/scenarios/ai-settings.mjs` и сверка снимка с макетом

## Phase 7: Завершение

- [ ] T027 Гейты `gates.sh all` (оба таргета, тесты, типы, i18n)
- [ ] T028 Проверка `security-review-agent`, правка находок (конституция, принцип IX)
- [ ] T029 `/speckit-converge`, цепочка ядро → биндинг → `ui/src`, README, `**Status**: Implemented`

## Phase 8: Находки ревью безопасности (T028)

- [ ] T030 [US4] M1: адрес своей модели одобряется на устройстве — `SecretKind::AiCustomEndpoint`, отказ `EndpointNotApproved`, запись при `ai_settings_set` (host, shell, сервер, wasm, `ui/src`), локали и тесты
- [ ] T031 [US4] M2: на сервере транспорт ИИ разрешает имя через `server_policy` и отказывает адресам-числам во внутренней сети в `packages/core/src/ai/transport.rs`
- [ ] T032 [US2] M3: производные пути не попадают в изменения истории — фильтр в `packages/core/src/sync/session.rs`, тест
- [ ] T033 [US3] M4, L5: `check_access` без учёта регистра и Unicode-формы, без хвостовых точек и пробелов, `false` при выключенной функции; `check_access_in` отказывает симлинкам; тесты
- [ ] T034 [US4] L1: в wasm предел ответа по `content-length` до чтения тела
- [ ] T035 [US4] L2: предел одновременных сетевых команд `ai_*` на сервере в `apps/server/src/invoke.rs`
- [ ] T036 [US4] L3: повтор `requestId` занятого запроса — отказ, слот не затирается, в `packages/core/src/ai/cancel.rs`
