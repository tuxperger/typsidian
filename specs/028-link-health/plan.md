# Implementation Plan: Битые ссылки и заметки-сироты

**Branch**: `028-link-health` | **Date**: 2026-10-22 | **Spec**: [spec.md](spec.md)

## Summary

Ядро строит «состояние ссылок» одним вызовом из источника графа 025 (`graph_source`:
разрешённые ссылки и вставки из индекса, без чтения файлов) и дочитывает только заметки,
в которых есть битые ссылки, — ради строк-примеров. Починка — по заметке за вызов тем же
путём, что замена 027: правка редактора открытой заметки или запись со сверкой отпечатка,
с записью для отмены; отмена — `replace_undo` 027. Отметка «намеренно без ссылок» хранится
в настройках vault и переезжает при переименовании, как пометки разделов 006.

Макет — временный (не Claude Design), https://claude.ai/code/artifact/c629e9a9-e502-483b-bd30-78c611b051af, экраны L1–L3.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`), TypeScript strict (`ui/`)
**Primary Dependencies**: без новых
**Storage**: `.typsidian/vault.json` — таблица `links`, ключ `intentional_orphans`
**Testing**: `cargo test` (`links_native.rs`), `wasm-pack test` (`links_wasm.rs`), сценарий `scripts/dev/scenarios/link-health.mjs`, бенч `link_health_scale`
**Target Platform**: web (wasm32), desktop и Android (Tauri)
**Performance Goals**: экран на 5 000 заметок ≤ 1 с (SC-001)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Что битое, что сирота, что вход, строки-примеры, правка ссылок и подписи — `links/health.rs`. UI показывает и зовёт. |
| II. Паритет | PASS | Функции над `VaultBackend`; wasm и Tauri 1:1; тесты на обоих путях. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`. |
| V. Внешние процессы | N/A | — |
| VI. Local-first | PASS | Правка — обычная запись файлов; отметки — в `vault.json`. |
| VII. Офлайн | N/A | — |
| VIII. Движки | N/A | — |
| IX. Локальность | PASS | Из индекса на устройстве. |
| X. Массовые операции | PASS | Починка группы — по заметке за вызов, итог и отмена; группа — десятки заметок, хода не требует. |

## Project Structure

```text
packages/core/src/links/health.rs           — состояние ссылок, починка
packages/core/src/links/{mod,web}.rs         — экспорт, wasm-функции
packages/core/src/vault/vault_settings.rs    — `links.intentional_orphans`
packages/core/src/vault/relocate.rs          — перенос отметки при переименовании
packages/core/tests/common/links_cases.rs, links_{native,wasm}.rs
packages/core/benches/link_health_scale.rs
packages/core/src/settings/keybindings.rs    — нет: команда только в палитре
packages/shell/src/lib.rs                    — команды Tauri
ui/src/vault-client{,-web,-tauri}.ts
ui/src/link-health/LinkHealth.tsx, useLinkHealth.ts
ui/src/workspace/Workspace.tsx, ui/src/vault-tree/* — счётчик в подвале дерева
ui/src/styles/app.css, ui/src/i18n/locales/{ru,en}/links.ts
scripts/dev/scenarios/link-health.mjs
```

## Complexity Tracking

Нет отклонений.
