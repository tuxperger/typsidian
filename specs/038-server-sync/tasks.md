# Tasks: Синхронизация на своём сервере

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/server-sync.md](contracts/server-sync.md)

## Phase 1: Ядро

- [X] T001 `ShellCapabilities.server` (`#[serde(default)]`, `current()` — `false`); `parse_remote` при `server` отклоняет `File` и `HttpLoopback`; тесты в `packages/core/tests/common/sync_cases.rs` на обоих путях: подключение и обмен с `file://` и `http://localhost` при `server` — `UnsupportedScheme` (research §3)

## Phase 2: Одна реализация команд (US1, US2)

- [X] T002 `packages/host/src/sync.rs`: `SyncHome`, `DynSecretStore`/`DynSecrets`, `FileSecrets` (перенос из Android-ветки оболочки, 0600, `forget_vault`), `SyncState`, `SyncCancel`, `SyncProgressSlot`, `open_session`, помощники контекста и сохранения значений устройства; тесты `FileSecrets`: права 0600, повреждённый файл откладывается, `forget_vault` удаляет только свой vault (research §2)
- [X] T003 Команды `sync_*` в `packages/host/src/commands.rs` (кроме `sync_open`, Syncthing, клонирования) + `sync_device_values`, `sync_set_author`; новые состояния в `gen-host-dispatch.mjs`, `Host`; оболочка: `Device(Arc<…>)`, `SyncHome` из папки приложения и keyring, Android — `FileSecrets`, руками — `sync_open`, Syncthing, `sync_clone_vault`; desktop и mobile собираются (research §1)

## Phase 3: Сервер (US1, US2)

- [X] T004 [US1] Сервер: `SyncHome` на аккаунт (`<данные>/sync/<аккаунт>/`), таблица `vault_sync_ids`, перехват `sync_open` с id сервера, белый список (contracts), переименование переносит id, удаление vault — секреты и зеркало, `user delete` — папка синхронизации; тест: включение истории, снимок, история, возврат через invoke (US1, FR-003, FR-004)
- [X] T005 [US2] Отказ внутренних адресов в `sync_connect_remote` (`sync_private_remotes`); тесты: ssh, `file://`, `http://localhost`, `https://10.0.0.1` — отказ; два аккаунта — токен A не встречается в ответах B и A, в файлах vault и в журнале; `secrets.json` 0600 (FR-002, FR-005, SC-002)

## Phase 4: Интерфейс

- [X] T006 [US1] [US2] `sync-client-tauri.ts` с транспортом параметром и значениями устройства через `sync_device_values`/`sync_set_author` для сервера; `openServerVault` открывает сессию; подсказка «хранится на сервере» по `capabilities.server` (ru/en)

## Phase 5: Polish

- [X] T007 Сценарий `scripts/dev/scenarios/server-sync.mjs` по quickstart (история в серверном режиме); README сервера: синхронизация, `sync_private_remotes`, остаточный риск DNS; `security-review-agent`, находки исправлены; `**Status**: Implemented`
