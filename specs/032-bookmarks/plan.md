# Implementation Plan: Закладки

**Branch**: `032-bookmarks` | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

## Summary

Закладки — список в настройках vault (`bookmarks` в `.typsidian/vault.json`): заметка,
раздел, поиск или папка, группы в один уровень. Добавление без дублей, порядок, группы,
переименование и доступность целей решает модуль ядра `bookmarks`; переименование и перенос
заметок переписывают пути закладок в самих backend (как отметки 028). UI: раздел «Закладки»
над деревом, пункты «В закладки» в меню дерева, вкладки, оглавления, палитре и окне поиска.

Макет — временный (не Claude Design), https://claude.ai/code/artifact/739be26c-597b-40a5-a05e-c8c69fc1970c, экраны B1–B2.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`), TypeScript strict (`ui/`)
**Primary Dependencies**: без новых
**Storage**: `.typsidian/vault.json`, ключ `bookmarks`
**Testing**: `cargo test` (`bookmarks_native.rs`), `wasm-pack test` (`bookmarks_wasm.rs`),
сценарий `scripts/dev/scenarios/bookmarks.mjs`
**Target Platform**: web (wasm32), desktop и Android (Tauri)
**Performance Goals**: вид из 200 закладок ≤ 50 мс (SC-001 — не дольше открытия из дерева)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Дубли, порядок, группы, доступность, перенос путей — `bookmarks`. |
| II. Паритет | PASS | Настройки vault через `VaultBackend`; тесты на обоих путях. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`. |
| V–VIII | N/A | — |
| VI. Local-first | PASS | Обычный JSON в vault. |
| IX. Локальность | PASS | Всё на устройстве; едет с синхронизацией vault. |
| X. Массовые операции | N/A | По одной закладке. |

## Project Structure

```text
packages/core/src/bookmarks/mod.rs, web.rs
packages/core/src/vault/vault_settings.rs   — ключ `bookmarks`
packages/core/src/vault/{native,web,relocate}.rs — `move_bookmarks`
packages/core/tests/common/bookmarks_cases.rs, bookmarks_{native,wasm}.rs
packages/shell/src/lib.rs, ui/src/vault-client{,-web,-tauri}.ts
ui/src/bookmarks/BookmarksPanel.tsx, useBookmarks.ts
ui/src/vault-tree/*, ui/src/workspace/Workspace.tsx, оглавление и поиск — «В закладки»
ui/src/i18n/locales/{ru,en}/bookmarks.ts, ui/src/styles/app.css
scripts/dev/scenarios/bookmarks.mjs
```

## Complexity Tracking

Нет отклонений.
