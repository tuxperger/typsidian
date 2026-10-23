# Tasks: Экспорт презентации

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/core-slides-export-api.md](contracts/core-slides-export-api.md)

## Phase 1: Основа

- [X] T001 Зависимость `typst-render` 0.15.1; `PdfMeta.pages` → `page_ranges`; `Render::Images` (SVG/PNG, упаковка байтов) в `preview/compile.rs` и воркере

## Phase 2: User Story 1 — PDF с заметками (P1)

- [X] T002 [US1] Тесты ядра `packages/core/tests/common/slides_export_cases.rs`, вызовы в `slides_export_native.rs` и `slides_export_wasm.rs`: выбор страниц (последний шаг / все), PDF слайдов — число страниц равно числу слайдов или шагов (SC-001), PDF со страницами заметок, PDF + `.pdfpc` (JSON, подписи, шаги, заметки — SC-002), ошибка сборки — ничего не записано (FR-001, FR-003, FR-004)
- [X] T003 [US1] `packages/core/src/slides/export.rs`: выбор страниц, `pdfpc_json`, `export_slides` для трёх PDF; экспорт в `slides/mod.rs`

## Phase 3: User Story 2 — картинки (P2)

- [X] T004 [US2] Тесты и ядро: PNG масштаба 1×–4× и SVG, имена `NN`/`NN-K`, папка `Доклад.slides/` (FR-002)
- [X] T005 [US2] Привязки (wasm-метод движка, Tauri, `previewClient.exportSlides`) и UI: окно `ui/src/slides/SlidesExportDialog.tsx`, пункт меню «Показ», команда палитры, тексты, стили (макет P1)

## Phase 4: Polish

- [X] T006 Сценарий `scripts/dev/scenarios/slides-export.mjs` по quickstart, сверка с макетом, README, `**Status**: Implemented`
