# Tasks: Экспорт в HTML

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/core-html-export-api.md](contracts/core-html-export-api.md)

## Phase 1: Основа

- [X] T001 Зависимость `typst-html` 0.15.1; `Render::Html` в `preview/compile.rs` (сборка `HtmlDocument`, библиотека с `Feature::Html` в `world.rs`), передача в воркер `preview/worker.rs`

## Phase 2: User Story 1 — заметка в HTML (P1)

- [X] T002 [US1] Тесты ядра `packages/core/tests/common/html_export_cases.rs`, вызовы в `html_export_native.rs` и `html_export_wasm.rs`: страница с заголовком, текстом, формулой MathML и встроенной картинкой без внешних адресов (FR-001, FR-004); `rect` — рисунком SVG и в `as_images` (FR-003); путь страницы в папке экспорта
- [X] T003 [US1] `packages/core/src/export/html.rs`: `export_note_html`, рисунки через `html.frame`, `as_images` в `ExportReport`

## Phase 3: User Story 2 — сайт из папки (P2)

- [X] T004 [US2] Тесты ядра: ссылки внутри набора — относительные адреса на существующие страницы (SC-001), вне набора — помечены; оглавление и «Оглавление» на страницах; ошибка сборки одной заметки не обрывает остальные; zip-архив содержит все страницы (FR-002, FR-005)
- [X] T005 [US2] Переписывание ссылок, `export_site_index`, `export_site_archive`; `BatchState.format` в `export/batch.rs`
- [X] T006 [US2] Привязки (wasm, Tauri, клиенты) и UI: формат в Центре экспорта, цикл пакета для HTML с оглавлением в конце, «Скачать архив» на web, `as_images` в строке и итоге; тексты (макет H1, H2)

## Phase 4: Polish

- [X] T007 Бенч `packages/core/benches/html_export_scale.rs`: 100 заметок ≤ 30 с (SC-002); запись в `Cargo.toml`
- [X] T008 Сценарий `scripts/dev/scenarios/html-export.mjs`, сверка с макетом, README, `**Status**: Implemented`
