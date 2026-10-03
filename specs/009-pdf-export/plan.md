# Implementation Plan: PDF Export

**Branch**: `009-pdf-export` | **Date**: 2026-09-28 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/009-pdf-export/spec.md`

## Summary

Экспорт заметок Typst в PDF: одной командой для открытой заметки и пакетом из «Центра
экспорта» с очередью, прогрессом, пропуском и отменой. Результат пишется в папку экспорта
внутри vault (`export` по умолчанию, настройка vault).

Технически это крейт `typst-pdf 0.15.1`, подключённый к существующему циклу резолва
предпросмотра через параметр «что получить на выходе» (research.md §3). Добавляются
атомарная `write_bytes` на native (research.md §4) и чистая машина состояний пакета в ядре,
которую крутит тонкий цикл в UI (research.md §5).

Макет — Claude Design, проект «Typsidian: Typst Editor Design»
(`b7a3b1f6-608f-403d-8c07-8101d15dce66`), файл `Typsidian Утилиты.dc.html`, экран Export.

## Technical Context

**Language/Version**: Rust 2021 (ядро, shell), TypeScript strict + React 19 (ui)

**Primary Dependencies**: новый `typst-pdf = "0.15.1"` (собирается под wasm32, проверено
зондом, research.md §1); существующие `typst`, `typst-layout`, `tsify-next`, `wasm-bindgen`,
Tauri 2

**Storage**: файлы vault через `VaultBackend`; настройка — поле `VaultOverrides` в
`.typsidian/vault.json`

**Testing**: `cargo test` и `wasm-bindgen-test` (ядро, оба таргета); `pnpm typecheck`; ручная
проверка по quickstart.md (JS-тестов в репозитории нет)

**Target Platform**: web (wasm32), desktop Linux (Tauri), Android (Tauri)

**Project Type**: монорепо — Rust-ядро + общий UI + три оболочки

**Performance Goals**: SC-001 — 10 страниц за ≤3 с на desktop и ≤6 с на web/Android; SC-004 —
отклик ввода ≤100 мс во время пакета (desktop)

**Constraints**: без полузаписанных файлов при любой отмене или сбое (SC-006); без сети,
кроме доустановки шаблонов по правилам 007

**Scale/Scope**: пакет до всего vault (предел vault — 10 000 заметок, фича 001)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Путь результата, проверка папки, кандидаты, сборка PDF, атомарная запись, переходы пакета, оценка времени и итог — в `packages/core/src/export/`. UI крутит цикл и рисует. |
| II. Паритет таргетов | PASS | `typst-pdf` собран под wasm32 зондом; одна публичная поверхность, биндинги wasm и Tauri 1:1 (contracts/core-export-api.md). `write_bytes` атомарна на обоих за трейтом. |
| III. Спека без реализации | PASS | Пути в спеке — пути в vault пользователя (наблюдаемое поведение). |
| IV. TS strict, без any | PASS | Все новые типы — `Tsify` из Rust, в UI через `import type`. |
| V. Мост к Neovim | N/A | Не затрагивается. |
| VI. Local-first | PASS | PDF — обычные файлы в папке пользователя. |
| VII. Офлайн | PASS | Сеть только при доустановке шаблона — то же поведение и те же тесты, что у 007. |
| VIII. Внешние движки | N/A | Excalidraw не вызывается: доски попадают в PDF снимками, как в предпросмотре. |
| IX. Локальность | PASS | Ничего не покидает устройство. |
| X. Массовые операции | **PARTIAL** | Отменяемо, прогресс виден, отмена оставляет vault предсказуемым (атомарная запись). На web сборка одной заметки идёт в главном потоке — см. Complexity Tracking. |

Пересмотр после проектирования (Phase 1): статусы не изменились.

## Project Structure

### Documentation (this feature)

```text
specs/009-pdf-export/
├── plan.md · research.md · data-model.md · quickstart.md
├── contracts/core-export-api.md · contracts/frontend-export.md
├── checklists/requirements.md
└── tasks.md            # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/
├── Cargo.toml                    # + typst-pdf
├── src/lib.rs                    # pub mod export
├── src/export/
│   ├── mod.rs                    # ExportReport, ExportOutcome, export_note_pdf
│   ├── folder.rs                 # validate_folder, export_folder, pdf_path
│   ├── candidates.rs             # candidates()
│   ├── batch.rs                  # BatchState и переходы
│   └── web.rs                    # wasm-bindgen: свободные функции пакета и кандидатов
├── src/preview/compile.rs        # Render { Pages, Pdf } через цикл резолва
├── src/preview/web.rs            # PreviewEngine.exportPdf
├── src/vault/native.rs           # атомарная write_bytes, уборка .typsidian/tmp
├── src/vault/vault_settings.rs   # export_folder + InvalidExportFolder
├── src/settings/keybindings.rs   # ExportPdf, ExportCenter
└── tests/common/export_cases.rs · tests/export_native.rs · tests/export_wasm.rs

packages/shell/src/lib.rs         # команды export_pdf, export_candidates, export_batch_*

ui/src/
├── preview-client.ts             # + exportPdf, exportCandidates, exportBatch
├── vault-client-web.ts · vault-client-tauri.ts
├── export/useExport.ts · ExportCenter.tsx · ExportToast.tsx · describe.ts
├── workspace/Workspace.tsx       # команды, сочетания, пилюля в шапке, монтирование Центра
├── preview/PreviewPane.tsx       # кнопка «PDF»
├── settings/VaultSection.tsx     # поле «Папка экспорта»
└── styles/app.css                # .ts-export-*
```

**Structure Decision**: новый модуль ядра `export/` рядом с `preview/`, где он переиспользует
цикл сборки. Новый каталог UI `ui/src/export/` устроен по образцу `ui/src/search/`.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| Принцип X на web: сборка одной заметки пакета блокирует главный поток на время своей сборки | Сборка Typst в wasm идёт в главном потоке; исполнителя в Web Worker у предпросмотра нет (005 T022 открыта) | Свой воркер только для экспорта — второй экземпляр компилятора, шрифтов и доступа к vault из воркера, то есть ровно работа T022, сделанная наполовину и в отрыве от предпросмотра. Смягчение: цикл уступает цикл событий между заметками, отмена срабатывает после текущей. Устраняется вместе с 005 T022 без изменения формы вызова. **Принято пользователем 2026-09-28** (по результатам `/speckit-analyze`, D1). |
