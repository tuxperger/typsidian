# Implementation Plan: Задачи из заметок

**Branch**: `033-vault-tasks` | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

## Summary

Задачи — пункты списка `- [ ]`/`- [x]` в заметках, срок — `📅`/`срок:` с датой. Модуль ядра
`tasks` разбирает заметку (отсекая код и комментарии разбором Typst), держит индекс задач
устройства `.typsidian/tasks.json` с отпечатками заметок (перечитываются только изменённые),
фильтрует по состоянию, сроку, папке и условиям свойств 020 и группирует по заметке или
сроку. Отметка меняет один знак одной строки, найденной по тексту; открытая заметка — через
редактор. UI: окно «Задачи» из палитры и команда «Переключить задачу».

Макет — временный (не Claude Design), https://claude.ai/code/artifact/de16fb28-6cd9-4197-afef-fe6d3495e127, экраны T1–T2.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`), TypeScript strict (`ui/`)
**Primary Dependencies**: без новых
**Storage**: `.typsidian/tasks.json` (производный путь устройства)
**Testing**: `cargo test` (`tasks_native.rs`), `wasm-pack test` (`tasks_wasm.rs`), бенч
`tasks_scale`, сценарий `scripts/dev/scenarios/tasks.mjs`
**Target Platform**: web (wasm32), desktop и Android (Tauri)
**Performance Goals**: 5 000 заметок: повторный показ ≤ 1 с (SC-001)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Разбор, индекс, фильтры, группы, отметка, переключение — `tasks`. |
| II. Паритет | PASS | Индекс — файл через `VaultBackend`; тесты на обоих путях. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`. |
| V, VII, VIII | N/A | — |
| VI. Local-first | PASS | Задачи — текст заметок, без своего формата. |
| IX. Локальность | PASS | Индекс на устройстве, не синхронизируется. |
| X. Массовые операции | N/A | Отметка — одна строка. |

## Project Structure

```text
packages/core/src/tasks/mod.rs, web.rs
packages/core/src/vault/service_dir.rs      — путь индекса задач, производные пути
packages/core/tests/common/tasks_cases.rs, tasks_{native,wasm}.rs, benches/tasks_scale.rs
packages/shell/src/lib.rs, ui/src/vault-client{,-web,-tauri}.ts
ui/src/tasks/TasksScreen.tsx, useTasks.ts
ui/src/workspace/Workspace.tsx              — палитра «Задачи», «Переключить задачу»
ui/src/i18n/locales/{ru,en}/tasks.ts, ui/src/styles/app.css
scripts/dev/scenarios/tasks.mjs
```

## Complexity Tracking

Нет отклонений.
