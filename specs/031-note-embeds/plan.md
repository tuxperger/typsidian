# Implementation Plan: Встраивание заметок

**Branch**: `031-note-embeds` | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

## Summary

Встраивание — вызов `#embed("Методика#Выборка")` из библиотеки `.typsidian/lib/embed.typ`
(по схеме `#select` 020). Перед сборкой ядро находит вызовы, вырезает заметку, раздел или
помеченный элемент и кладёт их виртуальными файлами в снимок сборки рядом с исходной заметкой,
а соответствие цели и файла — в `sys.inputs`; библиотека делает `include` или печатает плашку
(нет заметки, раздела, цикл, глубина, доска). Предпросмотр и PDF идут одним циклом сборки.
Встраивания отдаёт тот же разбор ссылок `graph::link_sites` — их видят связи, граф, отметки
018 и «Состояние ссылок» 028. UI: команда «Встроить заметку», дополнение в строке цели.

Макет — временный (не Claude Design), https://claude.ai/code/artifact/9c36d818-78fc-40a4-8d54-d7cac3665bb8, экраны E1–E3.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`), TypeScript strict (`ui/`)
**Primary Dependencies**: без новых; `typst::syntax`, `structure` (015), `graph` (013)
**Storage**: библиотека `.typsidian/lib/embed.typ`; виртуальные файлы только в снимке сборки
**Testing**: `cargo test` (`embeds_native.rs`), `wasm-pack test` (`embeds_wasm.rs`), бенч
`embeds_scale`, сценарий `scripts/dev/scenarios/embeds.mjs`
**Target Platform**: web (wasm32), desktop и Android (Tauri)
**Performance Goals**: план встраиваний для заметки с 10 встраиваниями ≤ 20 мс (SC-001 —
сборка в пределах 005 SC-001)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Разбор цели, вырезание, цикл, глубина, входы сборки, сайты ссылок, дополнение — ядро. UI зовёт команду и показывает. |
| II. Паритет | PASS | Сборка и `VaultBackend` общие; тесты на обоих путях. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`. |
| V. Внешние процессы | N/A | — |
| VI. Local-first | PASS | Заметка — валидный Typst; без приложения — плашка с целью. |
| VII. Офлайн | PASS | Сети нет. |
| VIII. Движки | N/A | Доска — через вставку 002. |
| IX. Локальность | PASS | Всё на устройстве. |
| X. Массовые операции | N/A | Перенаправление — механизмом 028. |

## Project Structure

```text
packages/core/src/embeds/mod.rs, library.rs, embed.typ, web.rs
packages/core/src/graph/mod.rs              — сайты `embed("…")` в `link_sites`
packages/core/src/links/health.rs           — перенаправление и «убрать» для встраиваний
packages/core/src/completion/{context,mod,marks}.rs — повод Embed, MissingEmbedPart
packages/core/src/preview/{compile,world}.rs — входы-словарь, виртуальные файлы, зависимости
packages/core/tests/common/embeds_cases.rs, embeds_{native,wasm}.rs, benches/embeds_scale.rs
packages/shell/src/lib.rs, ui/src/vault-client{,-web,-tauri}.ts
ui/src/editor/NoteEditor.tsx, ui/src/workspace/Workspace.tsx — команда «Встроить заметку»
ui/src/editor/completion/*                  — группы Sections/Labels
ui/src/i18n/locales/{ru,en}/embeds.ts
scripts/dev/scenarios/embeds.mjs
```

## Complexity Tracking

Нет отклонений.
