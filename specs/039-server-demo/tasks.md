# Tasks: Демо-режим сервера

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/demo-api.md](contracts/demo-api.md)

## Phase 1: Ядро

- [X] T001 `ShellCapabilities.remote` (по умолчанию `true`), `SyncError::RemoteUnavailable`: подключение и обмен при `remote = false` отклоняются; тест в `sync_cases.rs` на обоих путях (research §3)

## Phase 2: Сервер (US1, US2, US3)

- [X] T002 [US1] `serve --demo`, `[demo]` в настройках, демо-vault в `apps/server/demo-vault/` (встроен), проверка при запуске; столбцы `expires_at`, `created_ip`, гостевые методы хранилища (research §1, §2, §6)
- [X] T003 [US1] `POST /api/demo/start`, `GET /api/demo/usage`, `/api/info` и `/api/me` с демо-сведениями; закрытые точки входа и операции vault на демо-сервере; `DemoExpired` в проверке сессии (FR-001, FR-002, FR-004)
- [X] T004 [US3] Квота до записывающих команд, предел по адресу, запрет удалённого (возможность `remote = false` в окружении синхронизации гостя) (FR-005, FR-006)
- [X] T005 [US2] Уборка просроченных гостей при запуске и раз в минуту; тесты `apps/server/tests/demo.rs`: старт, два гостя независимы, закрытые точки, квота, предел по адресу, истечение и уборка без следов (SC-002, SC-003)

## Phase 3: Интерфейс

- [X] T006 [US1] [US2] Экран входа демо (D1), сразу в vault «Демо», пилюля статуса в шапке со временем и объёмом (D2), полоса за 5 минут (D3), окно окончания (D4), отказы квоты и предела (D5); без раздела «Аккаунт»; «Подключить…» выключено при `remote = false`; тексты ru/en

## Phase 4: Polish

- [X] T007 Сценарий `scripts/dev/scenarios/server-demo.mjs` по quickstart; README; `security-review-agent`, находки исправлены; `**Status**: Implemented`
