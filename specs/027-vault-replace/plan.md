# Implementation Plan: Поиск с заменой по vault

**Branch**: `027-vault-replace` | **Date**: 2026-10-21 | **Spec**: [spec.md](spec.md)

## Summary

Замена встраивается в расширенный поиск 001: ядро строит предпросмотр каждого совпадения
(куски строки «было/станет»), применяет отмеченные совпадения по одной заметке за вызов —
записью файла со сверкой отпечатка или правкой для редактора открытой заметки, как «Связать»
013, — и отменяет пакет по записям «текст до / отпечаток после». Ритм применения (порции,
ход, остановка) — в интерфейсе, правила — в ядре.

Макет — временный (не Claude Design), https://claude.ai/code/artifact/fcb2c832-8f14-487f-90cf-13b34ccee836, экраны R1–R4.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`), TypeScript strict (`ui/`)
**Primary Dependencies**: `regex` (уже есть: поиск 001), `blake3` (уже есть: отпечатки)
**Storage**: файлы vault; запись — `VaultBackend::write` с ожидаемым отпечатком
**Testing**: `cargo test` (`search_native.rs`), `wasm-pack test` (`search_wasm.rs`), сценарий `scripts/dev/scenarios/replace.mjs`
**Target Platform**: web (wasm32), desktop и Android (Tauri)
**Performance Goals**: предпросмотр ≤ 2 с на 2 000 заметок (SC-001); ввод ≤ 100 мс во время применения (SC-004)
**Constraints**: ≤ 10 000 совпадений в предпросмотре (FR-008)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Совпадения, подстановка групп, куски «было/станет», проверка свежести, план записи и отмены — `search/replace.rs`. UI держит отметки и порядок вызовов. |
| II. Паритет | PASS | Три функции ядра над `VaultBackend`; wasm-методы `WebBackend` и команды Tauri 1:1; тесты на обоих путях. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`. |
| V. Внешние процессы | N/A | Внешних процессов нет. |
| VI. Local-first | PASS | Правка — обычная запись файлов vault. |
| VII. Офлайн | N/A | Сети нет. |
| VIII. Движки | N/A | — |
| IX. Локальность | PASS | Всё на устройстве. |
| X. Массовые операции | PASS | Применение по заметке за вызов: ход, остановка между заметками, названное состояние после остановки; отмена пакета. |

## Project Structure

```text
packages/core/src/search/replace.rs        — предпросмотр, применение к заметке, отмена
packages/core/src/search/mod.rs            — `pub mod replace`, общий `Matcher`
packages/core/src/vault/web.rs             — wasm-методы `replacePreview`, `replaceNote`, `replaceUndo`
packages/core/src/settings/keybindings.rs  — команда `ReplaceInVault` (Mod+Shift+H)
packages/core/tests/search_{native,wasm}.rs
packages/shell/src/lib.rs                  — команды Tauri
ui/src/vault-client{,-web,-tauri}.ts       — методы клиента
ui/src/search/SearchOverlay.tsx            — режим замены
ui/src/search/ReplacePreview.tsx           — выдача с отметками
ui/src/search/useReplace.ts                — отметки, порции применения, отмена
ui/src/workspace/{Workspace,useShortcuts}.tsx — открытие в режиме замены
ui/src/styles/app.css, ui/src/i18n/locales/{ru,en}/search.ts
scripts/dev/scenarios/replace.mjs
```

## Complexity Tracking

Нет отклонений.
