# Implementation Plan: Синхронизация на своём сервере

**Branch**: `038-server-sync` | **Date**: 2026-10-24 | **Spec**: [spec.md](spec.md)

## Summary

Команды синхронизации 011, которые сейчас живут в оболочке Tauri (`packages/shell/src/sync.rs`),
переносятся в `packages/host`, как в 037 — команды vault: оболочка получает сгенерированные
обёртки, сервер — те же команды через `dispatch`. Окружение синхронизации (настройки
устройства, хранилище секретов, папка зеркал, возможности) становится состоянием `SyncHome`:
оболочка собирает его из папки приложения и keyring, сервер — из папки данных аккаунта и
файла секретов 0600. На сервере возможности без ssh и Syncthing и с новым флагом `server`,
по которому ядро отклоняет адреса «этого устройства» (`file://`, `http://localhost`); сервер
сверх того отклоняет HTTPS на внутренние адреса без разрешения администратора. Vault на
сервере получает постоянный id синхронизации (SQLite), переименование его сохраняет, удаление
vault — удаляет его секреты и зеркало. UI: клиент синхронизации Tauri с транспортом
параметром, `openServerVault` открывает сессию, подсказка о хранении токена на сервере.

Макет — не требуется: экраны 011 не меняются; в серверном режиме недоступность ssh и
Syncthing уже выражена возможностями оболочки, меняется только текст подсказки о токене.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`, `packages/host`, `packages/shell`, `apps/server`), TypeScript strict (`ui/`)
**Primary Dependencies**: новых нет (`git2` vendored уже в ядре; `keyring` остаётся в оболочке)
**Storage**: `<данные>/sync/<аккаунт>/{device/, secrets.json, mirrors/<id>.git}`; таблица `vault_sync_ids` в `server.sqlite`
**Testing**: `cargo test -p typsidian-core` (флаг `server` на обоих путях), `cargo test -p typsidian-host` (файл секретов), `cargo test -p typsidian-server` (история, отказы адресов, изоляция секретов на двух аккаунтах — research §6), сценарий `scripts/dev/scenarios/server-sync.mjs`
**Target Platform**: сервер Linux (037); оболочки Tauri — без изменения поведения

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Правила адресов — в ядре (`parse_remote` по флагу `server`); host и сервер — транспорт и место хранения. |
| II. Паритет | PASS | Флаг `server` тестируется в общих случаях синхронизации на native и wasm. |
| III. Спека | PASS | Clarify пройден (XI). |
| IV. TS strict | PASS | `ShellCapabilities` — Tsify. |
| V. Внешние процессы | PASS | Новых нет; ssh на сервере выключен. |
| VI. Local-first | PASS | Vault — обычная папка с обычным `.git`. |
| VII. Офлайн | PASS | Обмен — прежние шаги без блокировок; обрыв связи браузера не прерывает операцию сервера. |
| IX. Локальность | PASS | Обмен только с репозиторием, который назвал пользователь. |
| X. Массовые операции | PASS | Ход и отмена — прежние `sync_progress`/`sync_cancel`. |
| XI. Безопасность сервера | PASS | Секреты по аккаунтам, 0600, не в ответах и журнале; id vault задаёт сервер; локальные и внутренние адреса отклоняются; `security-review-agent` перед «готово». |

## Project Structure

```text
packages/core/src/sync/{mod,session}.rs   — ShellCapabilities.server, отказ адресов устройства
packages/host/src/sync.rs                 — SyncHome, SyncState, SyncCancel, SyncProgressSlot, FileSecrets, DynSecrets, open_session, помощники
packages/host/src/commands.rs             — команды sync_* (кроме sync_open, Syncthing и клонирования)
packages/shell/src/sync.rs                — ShellSecrets (keyring), sync_open, Syncthing, sync_clone_vault
scripts/dev/gen-host-dispatch.mjs         — новые состояния
apps/server/src/{sync,vaults,invoke,allowed,store,config,main}.rs
ui/src/sync/sync-client-tauri.ts          — транспорт параметром
ui/src/server/ServerApp.tsx               — открытие сессии синхронизации
scripts/dev/scenarios/server-sync.mjs
```
