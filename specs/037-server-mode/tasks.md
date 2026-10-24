# Tasks: Серверный режим web

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/server-api.md](contracts/server-api.md)

## Phase 1: Одна реализация команд

- [x] T001 Крейт `packages/host`: состояния (`Vault`, `Preview`, `Sources`, `Speller`, `HighlightSessions`, `TemplatePreviews`, `PropertiesReindex`, `ImportSource`), `Host`, `Answer`/`Failure`, `with_vault!`, помощники; перенос команд ядра скриптом `scripts/dev/extract-host.mjs`; оболочка `packages/shell` — тонкие обёртки с прежними именами; desktop и mobile собираются (research.md §1)
- [x] T002 `scripts/dev/gen-host-dispatch.mjs` → `packages/host/src/dispatch.rs` (`dispatch(&Host, имя, JSON)`), проверка свежести в `gates.sh fast`; тест: каждая команда `host::commands` есть в таблице

## Phase 2: User Story 6 + 1 — аккаунты, вход, работа с vault (P1)

- [x] T003 [US6] `apps/server`: конфигурация, SQLite-хранилище аккаунтов и сессий, команды администратора `user add|passwd|block|unblock|delete|list|link-oidc`, тесты хранилища
- [x] T004 [US1] Вход паролем, сессии (cookie, продление, срок), CSRF, ограничение попыток, `GET /api/me`, `POST /api/logout`; тесты: верный/неверный пароль без уточнения, заблокированный, лимит попыток, CSRF обязателен (FR-002, FR-011)
- [x] T005 [US1] `POST /api/vaults/{v}/invoke/{cmd}` — белый список, `Host` на (аккаунт, vault), простои; `GET /api/vaults/{v}/file/…`; раздача статики web; тесты: чтение/запись заметки через invoke, команда вне списка — отказ (FR-005, FR-009)
- [x] T006 [US1] UI: стартовый экран «Папка на устройстве | Сервер» в `apps/web`, экран входа, `remote-invoke.ts` и клиент Tauri с транспортом параметром, открытие vault на сервере, сборка web статикой для сервера; тексты (макет S1, S2) (FR-001)

## Phase 3: User Story 2 — свои vault (P2)

- [x] T007 [US2] `GET/POST/PATCH/DELETE /api/vaults` с проверкой имени и корзиной; изоляция: чужой vault — 404 на каждой операции, пути вне vault — отказ; тесты на два аккаунта (FR-003, FR-004, SC-002)
- [x] T008 [US2] UI «Мои vault»: открыть, создать, переименовать, удалить с подтверждением (макет S3)

## Phase 4: User Story 3 — провайдер входа (P2)

- [x] T009 [US3] OIDC: `start`/`callback` (PKCE, state, nonce), сопоставление по привязке `(issuer, sub)`, неизвестный — отказ с объяснением; кнопка на экране входа; тесты с поддельным провайдером

## Phase 5: User Story 4 — изменения приходят сами (P2)

- [x] T010 [US4] SSE событий vault; клиент: подписка, переподключение, перечитывание дерева; баннер обрыва связи и повтор правок (FR-006, FR-010, SC-003, SC-004)

## Phase 6: User Story 5 — сессии (P3)

- [x] T011 [US5] `GET /api/sessions`, завершение одной и всех, смена пароля завершает прочие; раздел настроек «Аккаунт» (макет S4) (FR-007)

## Phase 7: Polish

- [x] T012 Пакет `typsidian-server` во flake, пример конфигурации и прокси в README; сценарий `scripts/dev/scenarios/server-mode.mjs` по quickstart; проверка `security-review-agent`, находки исправлены; `**Status**: Implemented`
