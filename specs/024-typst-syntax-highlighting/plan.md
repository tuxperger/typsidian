# Implementation Plan: Подсветка синтаксиса Typst

**Branch**: `024-typst-syntax-highlighting` | **Date**: 2026-10-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/024-typst-syntax-highlighting/spec.md`

## Summary

Раскраска Typst-текста по ролям с учётом режимов разметки, кода и формул взамен
построчной `StreamLanguage` в `CodeEditor.tsx`. Разбор — в ядре, функцией
`typst::syntax::highlight` того же парсера, которым собирается заметка (research.md §1),
с четырьмя уточнениями ролей (§4) и незакрытой парой в пределах абзаца (§9). Ядро держит
сессию на редактор: правки обновляют разбор инкрементально, ответ — только для видимых окон,
смещения в UTF-16 (§2, §3; сессия — по замеру на реализации). В редакторе — `ViewPlugin` с
декорациями: на web раскраска считается в той же транзакции, что и правка, на Tauri —
асинхронно, со сдвигом правками, пока ответ в пути (§2, §6). Палитра — алиасы `--ts-syn-*`
на токенах «Спецформы» (§7).

Макет — временный (не Claude Design), https://claude.ai/code/artifact/7c1a68f4-9a9c-4a5c-9aef-165fc970637f,
экраны «Редактор», «Роли и цвета», «Состояния во время набора».

## Technical Context

**Language/Version**: Rust 2021 (ядро, shell), TypeScript strict + React 19 (ui)

**Primary Dependencies**: существующие `typst = "0.15.1"` (через него `typst::syntax`),
`tsify-next`, `wasm-bindgen`, Tauri 2, `@codemirror/view`, `@codemirror/state`. Новых нет.

**Storage**: нет — функция чистая, без состояния

**Testing**: `cargo test` и `wasm-bindgen-test` по одной таблице
`tests/common/highlight_cases.rs`; `pnpm typecheck`; сценарий
`scripts/dev/scenarios/syntax-highlight.mjs` для браузера (JS-тестов в репозитории нет)

**Target Platform**: web (wasm32), desktop Linux (Tauri), Android (Tauri)

**Project Type**: монорепо — Rust-ядро + общий UI + три оболочки

**Performance Goals**: SC-002 — ≤16 мс от нажатия до раскрашенного символа в 95 % нажатий
на 5 000 строках; SC-003 — +≤100 мс к открытию такой заметки

**Constraints**: ввод не ждёт ядра (FR-008); раскраска не мигает между ответами; цвета
только из токенов (FR-009)

**Scale/Scope**: заметки до 5 000 строк и строки до 100 000 символов (Edge Cases)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Разбор, роли, флаги, окна, перевод смещений — `packages/core/src/highlight/`. UI только сопоставляет число классу и рисует декорации. Нынешняя раскраска в UI, которая нарушает принцип, удаляется (FR-013). |
| II. Паритет таргетов | PASS | `typst-syntax` уже собирается под оба таргета в составе `typst`. Одна сессия ядра, биндинги wasm (класс) и Tauri (команды) с одной формой ответа (contracts/core-highlight-api.md), одна таблица тестов на `cargo test` и `wasm-bindgen-test`. |
| III. Спека без реализации | PASS | В спеке синтаксис Typst — предметная область. Имён библиотек и API нет. |
| IV. TS strict, без any | PASS | `HighlightWindow` — `Tsify`, перечень ролей — `as const` с проверкой тестом ядра. |
| V. Мост к Neovim | N/A | Не затрагивается. |
| VI. Local-first | PASS | Ничего не пишется, раскраска — производное от текста. |
| VII. Офлайн | PASS | Сети нет (FR-014). |
| VIII. Внешние движки | N/A | Не затрагивается. |
| IX. Локальность | PASS | Ничего не покидает устройство. |
| X. Массовые операции | N/A | Массовых операций нет. Разбор одной заметки — единицы миллисекунд вне пути ввода. |

Пересмотр после проектирования (Phase 1): статусы не изменились.

## Project Structure

### Documentation (this feature)

```text
specs/024-typst-syntax-highlighting/
├── plan.md · research.md · data-model.md · quickstart.md
├── contracts/core-highlight-api.md · contracts/frontend-highlight.md
├── checklists/requirements.md
└── tasks.md            # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/
├── src/lib.rs                    # pub mod highlight; pub(crate) mod text_offsets
├── src/text_offsets.rs           # utf16_at, byte_at, перевод одним проходом (из elements)
├── src/elements/mod.rs           # берёт смещения из text_offsets
├── src/highlight/
│   ├── mod.rs                    # SyntaxRole, StyleFlags, HighlightWindow, HighlightSpans, highlight()
│   ├── roles.rs                  # Tag + уточнения → SyntaxRole (research §4)
│   ├── walk.rs                   # обход листьев, флаги предков, окна, склейка, незакрытые пары
│   ├── session.rs                # HighlightSession, HighlightEdit
│   └── web.rs                    # класс SyntaxSession
└── tests/common/highlight_cases.rs · tests/highlight_native.rs · tests/highlight_wasm.rs

packages/shell/src/lib.rs         # HighlightSessions, команды syntax_session_open/update/close

ui/src/
├── vault-client.ts · vault-client-web.ts · vault-client-tauri.ts   # + openHighlight
├── editor/highlight/
│   ├── roles.ts                  # перечень ролей и флагов → классы
│   └── typstHighlight.ts         # ViewPlugin, сессия, окна, правки, map
├── editor/CodeEditor.tsx         # − StreamLanguage/HighlightStyle; + languageData, проп highlight
├── editor/NoteEditor.tsx         # передаёт расширение для Typst-заметок
└── styles/app.css                # --ts-syn-* и .ts-editor .ts-syn-*

scripts/dev/scenarios/syntax-highlight.mjs   # проверка ролей в браузере и замер (quickstart §2, §4)
```

**Structure Decision**: новый модуль ядра `highlight/` рядом с `structure/`, тоже со
свободной функцией над текстом заметки. Общие смещения UTF-16 выносятся из `elements/` в
`text_offsets.rs`, чтобы не держать вторую копию. В UI — каталог
`ui/src/editor/highlight/` рядом с редактором, который его использует.

## Complexity Tracking

Нарушений конституции нет.
