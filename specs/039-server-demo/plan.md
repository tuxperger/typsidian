# Implementation Plan: Демо-режим сервера

**Branch**: `039-server-demo` | **Date**: 2026-10-25 | **Spec**: [spec.md](spec.md)

## Summary

`typsidian-server serve --demo` запускает сервер только для гостей. `POST /api/demo/start`
заводит гостевой аккаунт со сроком (`expires_at`), кладёт ему vault «Демо» — копию
встроенного в сервер демо-vault (или папки администратора) — и открывает сессию. Вход
паролем, OIDC, смена пароля, список сессий и операции над vault на демо-сервере закрыты;
подключение удалённого репозитория закрыто возможностью ядра `ShellCapabilities.remote`.
Квота проверяется до каждой записывающей команды; предел создания гостей с адреса считается
по базе (хеш адреса). Уборка раз в минуту и при запуске удаляет просроченных гостей целиком.
UI: экран входа с одной кнопкой, пилюля «демо · осталось N мин» с объёмом в шапке, полоса за 5
минут, окно «Демо закончилось».

Макет — временный (не Claude Design), https://claude.ai/code/artifact/ca477342-0e02-4b8b-aa9b-a32ffdddc73f, экраны D1–D5.

## Technical Context

**Language/Version**: Rust 2021 (`apps/server`, `packages/core`), TypeScript strict (`ui/`)
**Primary Dependencies**: новых нет; демо-vault встроен в бинарь `include_bytes!`
**Storage**: столбцы `accounts.expires_at`, `accounts.created_ip` (blake3 адреса)
**Testing**: `cargo test -p typsidian-server --test demo`; ядро — `remote` на обоих путях; сценарий `scripts/dev/scenarios/server-demo.mjs`
**Target Platform**: публичный сервер Linux за прокси с TLS

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Запрет удалённого — возможность ядра; демо, квоты, сроки — свойства сервера (транспорт и аккаунты). |
| II. Паритет | PASS | `remote` проверяется в общих случаях синхронизации на native и wasm. |
| III. Спека | PASS | Clarify пройден. |
| IV. TS strict | PASS | — |
| VI. Local-first | PASS | Демо — отдельный режим сервера; локальные оболочки не затронуты. |
| IX. Локальность | PASS | Адрес гостя хранится только хешем и только для предела. |
| XI. Безопасность сервера | PASS | Изоляция 037, закрытые точки входа, квота, предел по адресу, уборка; `security-review-agent` перед «готово». |

## Project Structure

```text
packages/core/src/sync/{mod,session}.rs     — ShellCapabilities.remote, SyncError::RemoteUnavailable
apps/server/src/demo.rs                     — старт гостя, демо-vault, квота, уборка, запреты
apps/server/demo-vault/                     — заметки примеров (встраиваются в бинарь)
apps/server/src/{config,store,auth,lib,main,invoke,vaults,sync,oidc,error}.rs
apps/server/tests/demo.rs
ui/src/server/{ServerLogin,ServerApp,DemoStatus,server-api}.tsx/ts, ui/src/workspace/Workspace.tsx (слот статуса)
ui/src/sync/{SyncPanel,describe,sync-client}.ts(x), ui/src/i18n/locales/{ru,en}/server.ts
scripts/dev/scenarios/server-demo.mjs
```
