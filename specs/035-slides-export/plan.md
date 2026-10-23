# Implementation Plan: Экспорт презентации

**Branch**: `035-slides-export` | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

## Summary

Экспорт презентации 019 — модуль ядра `slides/export.rs` поверх сборки 009: выбор страниц по
слайдам и шагам, PDF подмножества страниц, PDF со страницами заметок (вторая сборка из SVG
слайдов и заметок), PDF + `.pdfpc`, картинки PNG (`typst-render`) и SVG в папке
`Доклад.slides/`. UI: пункт «Экспорт презентации…» в меню «Показ», команда палитры и окно
выбора с итогом.

Макет — временный (не Claude Design), https://claude.ai/code/artifact/c65b6604-827b-4a78-af91-14a8292ec4e9, экран P1.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`), TypeScript strict (`ui/`)
**Primary Dependencies**: новая — `typst-render` 0.15.1 (растр PNG, чистый Rust, оба таргета)
**Storage**: папка экспорта vault (009)
**Testing**: `cargo test` (`slides_export_native.rs`), `wasm-pack test` (`slides_export_wasm.rs`),
сценарий `scripts/dev/scenarios/slides-export.mjs`

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Слайды и шаги, PDF, заметки, pdfpc, картинки, имена файлов — ядро. |
| II. Паритет | PASS | Одна сборка, воркер web несёт новый вид вывода; тесты на обоих путях. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`. |
| V, VII, VIII | N/A | — |
| VI. Local-first | PASS | Обычные файлы в папке экспорта. |
| IX. Локальность | PASS | Всё на устройстве. |
| X. Массовые операции | N/A | Один документ за раз; запись каждого файла атомарна. |

## Project Structure

```text
packages/core/src/slides/export.rs, web.rs
packages/core/src/preview/{compile,worker}.rs — PdfMeta.pages, Render::Images
packages/core/tests/common/slides_export_cases.rs, slides_export_{native,wasm}.rs
packages/shell/src/lib.rs, ui/src/preview-client.ts, ui/src/vault-client{-web,-tauri}.ts
ui/src/slides/SlidesExportDialog.tsx, ShowMenu.tsx; ui/src/workspace/Workspace.tsx
ui/src/i18n/locales/{ru,en}/slides.ts, ui/src/styles/app.css
scripts/dev/scenarios/slides-export.mjs
```

## Complexity Tracking

Нет отклонений.
