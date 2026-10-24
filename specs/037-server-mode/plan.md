# Implementation Plan: Серверный режим web

**Branch**: `037-server-mode` | **Date**: 2026-10-24 | **Spec**: [spec.md](spec.md)

## Summary

Web получает второй режим — вход на собственный сервер. Команды ядра, которые сейчас живут в
оболочке Tauri, один раз выносятся в крейт `packages/host` без Tauri; оболочка вызывает их
тонкими обёртками, сервер — через сгенерированную таблицу «имя → команда». `apps/server`
(Rust, axum) хранит аккаунты и сессии в SQLite, vault — обычными папками под папкой данных,
вход — паролем (argon2id) и через OIDC, CSRF-токен, ограничение попыток; команды vault
доступны по HTTP из белого списка, изменения — по SSE. Клиент — тот же клиент Tauri с
HTTP-транспортом; стартовый экран web предлагает «Папка на устройстве» или «Сервер», экраны
входа, списка vault, сессий и баннер обрыва связи.

Макет — временный (не Claude Design), https://claude.ai/code/artifact/93e6eda0-b8cc-486f-854f-07acb5ccff6c, экраны S1–S5.

## Technical Context

**Language/Version**: Rust 2021 (`packages/host`, `apps/server`), TypeScript strict (`ui/`)
**Primary Dependencies**: новые на сервере — `axum` 0.8, `tower-http`, `argon2`,
`openidconnect`, `clap`, `toml`, `rand`; `sqlx` (SQLite) уже в ядре. Нативных библиотек
нет — `nix-env-agent` нужен только для пакета сервера во flake.
**Storage**: `<данные>/server.sqlite`, `<данные>/users/<id>/<vault>/`
**Testing**: `cargo test -p typsidian-host`, `cargo test -p typsidian-server` (axum без
сети: `tower::ServiceExt::oneshot`), сценарий `scripts/dev/scenarios/server-mode.mjs`
**Target Platform**: Linux-сервер за обратным прокси с TLS; браузеры web
**Performance Goals**: SC-003 — событие во вкладке ≤ 3 с; ввод не ждёт сети (SC-005)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Сервер — транспорт и вход; работа с vault — ядро через `host`. |
| II. Паритет | PASS | Ядро не меняется; wasm-путь web с папкой прежний; тесты ядра на обоих путях. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`/общие с Tauri-клиентом. |
| V. Внешние процессы | PASS | Новых процессов нет; ssh/Syncthing на сервере — как на desktop. |
| VI. Local-first | PASS | Режим опциональный; vault — обычные папки; локальные оболочки не зависят от сервера (поправка 3.0.0). |
| VII. Офлайн | PASS | FR-010: баннер, правка в редакторе, повтор; тест обрыва. |
| VIII. Движки | N/A | — |
| IX. Локальность | PASS | Индексы на сервере пользователя; третьим сервисам ничего (поправка 3.0.0). |
| X. Массовые операции | PASS | Пакеты экспорта и импорта — прежние циклы клиента. |
| XI. Безопасность сервера | PASS | Изоляция путей, argon2id, cookie HttpOnly/Secure/SameSite, CSRF, лимит попыток, OIDC c PKCE/state/nonce, журналы без текста; `security-review-agent` перед «готово». |

## Project Structure

```text
packages/host/                      — состояния, помощники и команды ядра без Tauri; dispatch.rs (генерируется)
packages/shell/src/lib.rs           — тонкие обёртки Tauri над host; устройство и окна — как было
apps/server/src/{main,config,store,auth,oidc,vaults,invoke,events,static_files,allowed}.rs
apps/server/tests/*.rs
scripts/dev/extract-host.mjs, scripts/dev/gen-host-dispatch.mjs
ui/src/server/{ServerLogin.tsx,VaultList.tsx,AccountSection.tsx,ConnectionBanner.tsx,server-api.ts,remote-invoke.ts}
ui/src/vault-client-tauri.ts        — транспорт параметром
apps/web/app/*, next.config.mjs     — выбор режима, статическая сборка для сервера
flake.nix, nix/                     — пакет `typsidian-server`
scripts/dev/scenarios/server-mode.mjs
```

## Complexity Tracking

Нет отклонений. Перенос команд в `host` — разовая механическая правка, оправданная
конституцией (одна реализация для Tauri и HTTP).
