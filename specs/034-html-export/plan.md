# Implementation Plan: Экспорт в HTML

**Branch**: `034-html-export` | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

## Summary

Экспорт в HTML — второй формат Центра экспорта 009. Сборка та же, что у предпросмотра и PDF,
последний шаг — `typst-html` (семантический HTML, формулы MathML, картинки встроены).
Пропущенные HTML-выводом элементы выводятся `html.frame` (SVG) и называются в итоге.
Ссылки `[[…]]` на заметки набора становятся относительными ссылками на страницы, вне набора
— помеченным текстом. Набор из нескольких заметок получает `index.html` и ссылку
«Оглавление» на каждой странице; на web — zip-архив для скачивания.

Макет — временный (не Claude Design), https://claude.ai/code/artifact/349ce833-b50a-48b3-8554-1a2d55294c18, экраны H1–H2.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`), TypeScript strict (`ui/`)
**Primary Dependencies**: новая — `typst-html` 0.15.1 (часть Typst, чистый Rust, оба таргета)
**Storage**: папка экспорта vault (009)
**Testing**: `cargo test` (`html_export_native.rs`), `wasm-pack test` (`html_export_wasm.rs`),
бенч `html_export_scale`, сценарий `scripts/dev/scenarios/html-export.mjs`
**Performance Goals**: 100 заметок ≤ 30 с (SC-002)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Сборка, ссылки, рисунки, оглавление, архив — ядро; UI — формат и кнопки. |
| II. Паритет | PASS | Одна сборка; тесты на обоих путях; архив на web — те же файлы. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`. |
| V, VIII | N/A | — |
| VI. Local-first | PASS | Обычные файлы в папке экспорта. |
| VII. Офлайн | PASS | Страницы без внешних ресурсов (FR-004). |
| IX. Локальность | PASS | Всё на устройстве. |
| X. Массовые операции | PASS | Пакет 009: ход, пропуск, отмена между заметками. |

## Project Structure

```text
packages/core/src/export/html.rs            — страница, ссылки, рисунки, оглавление, zip
packages/core/src/export/{mod,batch,web}.rs — формат пакета, `as_images`, привязки
packages/core/src/preview/{compile,world,worker}.rs — `Render::Html`
packages/core/tests/common/html_export_cases.rs, html_export_{native,wasm}.rs
packages/core/benches/html_export_scale.rs
packages/shell/src/lib.rs, ui/src/preview-client*.ts
ui/src/export/{ExportCenter.tsx,useExport.ts,describe.ts}
ui/src/i18n/locales/{ru,en}/export.ts
scripts/dev/scenarios/html-export.mjs
```

## Complexity Tracking

Нет отклонений.
