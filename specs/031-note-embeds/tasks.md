# Tasks: Встраивание заметок

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/core-embeds-api.md](contracts/core-embeds-api.md)

## Phase 1: Основа

- [X] T001 `packages/core/src/embeds/`: разбор цели и вызовов `embed`, вырезание раздела и элемента по метке, библиотека `embed.typ` с плашками, `ensure_embed_library`, `embed_import_edits` (research.md §1, §3)

## Phase 2: User Story 1 — встроить заметку или раздел (P1)

- [X] T002 [US1] Тесты ядра `packages/core/tests/common/embeds_cases.rs`, вызовы в `embeds_native.rs` и `embeds_wasm.rs`: разбор целей, раздел без соседних, элемент по метке, план сборки с виртуальными файлами и входами, сборка предпросмотра и PDF с текстом встройки (FR-001, FR-002), зависимости (FR-003), дополнение заметок/разделов/меток (US1 AS4)
- [X] T003 [US1] `embeds::plan`; `preview/compile.rs` и `world.rs`: входы-словарь `select` + `embed`, виртуальные файлы в снимке, пути встроенных заметок в `dependencies`; библиотека подставляется в снимок, если файла нет (research.md §2)
- [X] T004 [US1] Дополнение: повод `Embed` в `completion/context.rs`, группы разделов и меток в `completion/mod.rs`; UI-группы в `ui/src/editor/completion/*`
- [X] T005 [US1] Привязки и UI: `client.embeds.insert` (wasm, Tauri, клиенты), команда палитры «Встроить заметку» — правки и запись библиотеки, затем дополнение; тексты `ui/src/i18n/locales/{ru,en}/embeds.ts` (макет E1–E3)

## Phase 3: User Story 2 — ошибки встраивания (P2)

- [X] T006 [US2] Тесты ядра: нет заметки, нет раздела, цикл A→B→A на втором заходе, глубина > 5, доска — входы с ошибкой; отметки 018 `MissingNote`/`MissingEmbedPart`; встраивание в индексе ссылок и обратных ссылках; перенаправление 028 сохраняет `#раздел`, «убрать» удаляет вызов (FR-004, FR-005, SC-002)
- [X] T007 [US2] `graph::link_sites` с сайтами встраиваний (`call`), `links/health.rs` — перенаправление и «убрать»; `completion/marks.rs` — `MissingEmbedPart`; текст отметки в UI

## Phase 4: Polish

- [X] T008 Бенч `packages/core/benches/embeds_scale.rs`: план 10 встраиваний ≤ 20 мс (SC-001); запись в `Cargo.toml`
- [X] T009 Сценарий `scripts/dev/scenarios/embeds.mjs` по quickstart, сверка с макетом E1–E3, README, `**Status**: Implemented`
