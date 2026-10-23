# Implementation Plan: Локальные снимки заметок

**Branch**: `029-note-snapshots` | **Date**: 2026-10-22 | **Spec**: [spec.md](spec.md)

## Summary

Ядро ведёт снимки заметок в служебной папке vault (`.typsidian/snapshots/`): опись
`index.json` и по файлу на снимок. Пора ли делать снимок, одинаков ли текст, что истекло по
сроку, различия, возврат и восстановление удалённой заметки решает модуль `snapshots`
ядра поверх `VaultBackend`; удаление и переименование заметки сами откладывают снимок и
переносят историю (тот же приём, что у отметок 006 и 028). Снимки — производный путь
устройства: правила истории 011 и Syncthing их не видят. UI зовёт ядро при открытии,
сохранении, закрытии и внешнем изменении заметки и показывает окно истории; версии git
примешиваются к списку из клиента синхронизации 011.

Макет — временный (не Claude Design), https://claude.ai/code/artifact/f02eee51-0f80-4c84-b986-6e2045fe3b6d, экраны S1–S4.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`), TypeScript strict (`ui/`)
**Primary Dependencies**: без новых; хэш — `blake3` (уже в ядре), различия — `vault::conflict::line_diff`
**Storage**: `.typsidian/snapshots/index.json`, `.typsidian/snapshots/<id>.txt`; настройки — таблица `snapshots` в `.typsidian/vault.json`
**Testing**: `cargo test` (`snapshots_native.rs`), `wasm-pack test` (`snapshots_wasm.rs`), сценарий `scripts/dev/scenarios/snapshots.mjs`, бенч `snapshots_scale`
**Target Platform**: web (wasm32), desktop и Android (Tauri)
**Performance Goals**: история из 200 снимков ≤ 500 мс (SC-003); снимок не на пути набора (SC-001)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Интервал, дубли, срок, различия, возврат, восстановление, перенос истории — `snapshots`. UI только зовёт в нужные моменты и показывает. |
| II. Паритет | PASS | Один путь хранения через `VaultBackend` на всех оболочках; тесты на обоих путях. Новый метод `remove_service_file` реализован в обоих backend. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`. |
| V. Внешние процессы | N/A | git читается механизмом 011, нового процесса нет. |
| VI. Local-first | PASS | Снимки — обычные файлы в папке vault на устройстве. |
| VII. Офлайн | PASS | Сети нет. |
| VIII. Движки | N/A | — |
| IX. Локальность | PASS | Производный путь: не синхронизируется (FR-005). |
| X. Массовые операции | PASS | «Очистить» — подтверждение до действия, итог после; снимок — одна заметка за вызов. |

## Project Structure

```text
packages/core/src/snapshots/mod.rs          — опись, снимок, история, различия, возврат, удалённые, очистка
packages/core/src/snapshots/web.rs          — wasm-функции
packages/core/src/vault/backend.rs          — `remove_service_file`
packages/core/src/vault/{native,web}.rs     — снимок перед удалением, перенос истории при переименовании, `remove_service_file`
packages/core/src/vault/relocate.rs         — перенос истории при переносе (023)
packages/core/src/vault/service_dir.rs      — путь снимков, производные пути
packages/core/src/vault/vault_settings.rs   — таблица `snapshots`
packages/core/tests/common/snapshots_cases.rs, snapshots_{native,wasm}.rs
packages/core/benches/snapshots_scale.rs
packages/shell/src/lib.rs                   — команды Tauri
ui/src/vault-client{,-web,-tauri}.ts
ui/src/snapshots/NoteHistory.tsx, RecentlyDeleted.tsx, useSnapshots.ts
ui/src/workspace/Workspace.tsx              — моменты снимков, команды палитры
ui/src/settings/*                           — раздел «Снимки заметок»
ui/src/styles/app.css, ui/src/i18n/locales/{ru,en}/snapshots.ts
scripts/dev/scenarios/snapshots.mjs
```

## Complexity Tracking

Нет отклонений.
