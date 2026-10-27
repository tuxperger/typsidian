# Implementation Plan: Массовое разрешение и подсветка изменений в конфликтах синхронизации

**Branch**: `040-conflict-bulk-highlight` | **Date**: 2026-10-26 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/040-conflict-bulk-highlight/spec.md`

## Summary

Ядро (`packages/core/src/sync/merge.rs`) перестаёт терять сторону при совмещении: общий
проход трёхстороннего слияния порождает **участки с происхождением** (общий, своя, их, обе)
и **пометки строк** каждой версии. `Conflict` получает поле `view` с этими данными; прежний
`auto_merge` выводится из того же прохода, поэтому подсветка не может разойтись со спорными
участками (FR-006). Сборка результата из выбора сторон и перекраска строк после ручной
правки — чистая функция ядра `merge_annotate` (команда `sync_merge_annotate`, на web — wasm).
Массовое решение — операция сессии `resolve_many` (команда `sync_resolve_many`): решает
конфликты по одному тем же путём, что `resolve`, показывает ход и отменяется между файлами
(принцип X), возвращает отчёт «решено / пропущено / не удалось». UI: редактор слияния с
легендой, подсветкой колонок и пар, массовым выбором и переходом по участкам, поле
результата на CodeMirror с построчной подсветкой; список конфликтов с отметками, бейджами,
панелью действий, подтверждением в строку и отчётом.

Макет — временный (не Claude Design), https://claude.ai/code/artifact/a15460f4-8835-4b39-855f-dbc6cee48d73, экраны 1a, 1b, 2a–2c, 3.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`, `packages/host`, `packages/shell`, `apps/server`), TypeScript strict (`ui/`)

**Primary Dependencies**: новых нет — `similar` уже в ядре; `@codemirror/{state,view}` уже в `ui/`

**Storage**: не меняется; решения пишут те же файлы, что одиночное `resolve`

**Testing**: `cargo test -p typsidian-core` и `wasm-bindgen-test` — общие случаи `tests/common/merge_view_cases.rs` (пометки, происхождение, сборка, ручная правка) и новые случаи в `tests/common/sync_cases.rs` (пачка: свои/их/совмещённые, частичный сбой, завершение получения, отмена); `cargo test -p typsidian-server` (команды разрешены); `scripts/dev/scenarios/sync-merge.mjs` в браузере

**Target Platform**: web (wasm), desktop и Android (Tauri, команды host), web серверного режима (dispatch)

**Project Type**: local-first редактор, ядро + общий UI на три оболочки

**Performance Goals**: открытие редактора слияния с подсветкой для 5 000 строк ≤ 1 с (SC-003); перекраска после правки — с задержкой 250 мс после последнего нажатия, без блокировки ввода

**Constraints**: подсветка и спорные участки из одного прохода (FR-006); цвет не единственный признак (FR-005); тексты — только из локализации

**Scale/Scope**: десятки конфликтов в пачке, файлы до нескольких тысяч строк

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Пометки строк, происхождение, сборка результата, перекраска после правки и пачка решений — в ядре; UI хранит только выбор сторон, номер участка и отметки в списке. Прежняя сборка результата в `MergeEditor.tsx` уходит в ядро. |
| II. Паритет | PASS | Новые функции чистые или идут через `VaultBackend`; общие случаи прогоняются `cargo test` и `wasm-bindgen-test`. |
| III. Спека | PASS | Спека без API и структур; clarify пройден. |
| IV. TS strict | PASS | `MergeView`, `Segment`, `LineOrigin`, `Pick`, `Annotated`, `BulkResolution`, `BulkReport` — Tsify, типы UI порождаются. |
| V. Внешние процессы | PASS | Не затрагиваются: конфликтные копии Syncthing читаются как файлы, как в 011. |
| VI. Local-first | PASS | Результат — обычные файлы vault, как при одиночном решении. |
| VII. Офлайн | PASS | Сети нет ни в одном новом пути; получение завершается локально (011 §FR-017). |
| VIII. Внешние движки | PASS | Не затрагиваются; CodeMirror — наш редактор, стилизуется своими классами. |
| IX. Локальность | PASS | Содержимое никуда не уходит; в серверном режиме — только между браузером и своим сервером. |
| X. Массовые операции | PASS | `resolve_many` пишет ход в `ProgressSink` (`Operation::Resolve`), проверяет отмену между файлами; каждый файл решается целиком; отменённые остаются конфликтами. UI не блокируется: операция идёт через прежний `sync.run`. |
| XI. Сервер | PASS | Две новые команды в `allowed.rs`. `sync_resolve_many` проходит те же проверки vault и путей, что `sync_resolve`: id ищутся только среди конфликтов, построенных сервером. Пачка — не больше 10 000 уникальных id, пути сервера вычищаются и из успешного отчёта. `sync_merge_annotate` не трогает данные; построчное сравнение ограничено `LINE_LIMIT` (research §10). Ревью `security-review-agent` пройдено, находки исправлены. |

## Project Structure

### Documentation (this feature)

```text
specs/040-conflict-bulk-highlight/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/merge-api.md
└── tasks.md
```

### Source Code (repository root)

```text
packages/core/src/sync/merge.rs        — Segment, LineOrigin, MergeView, merge_view, Pick, Annotated, merge_annotate
packages/core/src/sync/session.rs      — Conflict.view, BulkResolution, BulkReport, resolve_many
packages/core/src/sync/mod.rs          — Operation::Resolve
packages/core/src/sync/web.rs          — resolveMany, mergeAnnotate (wasm)
packages/core/tests/common/merge_view_cases.rs, merge_view_{native,wasm}.rs
packages/core/tests/common/sync_cases.rs, sync_{native,wasm}.rs
packages/host/src/commands.rs          — sync_resolve_many, sync_merge_annotate
packages/shell/src/{commands,lib}.rs, packages/host/src/dispatch.rs — генерируются / регистрация
apps/server/src/allowed.rs
ui/src/sync/sync-client{,-web,-tauri}.ts
ui/src/sync/MergeEditor.tsx            — легенда, колонки, массовый выбор, участки
ui/src/sync/MergeResult.tsx            — поле результата (CodeMirror) с подсветкой
ui/src/sync/SyncPanel.tsx              — ConflictsSection: отметки, пачка, отчёт
ui/src/i18n/locales/{ru,en}/sync.ts
ui/src/styles/app.css                  — роли --ts-merge-*
scripts/dev/scenarios/sync-merge.mjs
```

**Structure Decision**: логика — в модуле синхронизации ядра рядом с `merge3`; команды — в
`packages/host` (одна реализация для Tauri и сервера, 037/038); UI — в `ui/src/sync`.

## Complexity Tracking

Нарушений нет.
