# Tasks: Импорт из Obsidian

**Input**: Design documents from `/specs/021-obsidian-import/`

**Tests**: ядро — оба таргета (`tests/common/import_cases.rs`, раннеры `import_native.rs`,
`import_wasm.rs`, образцы `tests/fixtures/obsidian/`); UI — сценарий `scripts/dev/scenarios/import.mjs`.

## Phase 1: Setup

- [X] T001 Добавить `pulldown-cmark` и `mitex` в `packages/core/Cargo.toml`; проверить сборку `wasm32-unknown-unknown` и нативную до любой другой работы; при отказе `mitex` — Decision в research.md §5 и свой перевод подмножества
- [X] T002 Завести `packages/core/src/import/{mod,markdown,obsidian,math,names,batch,report,web}.rs`, `pub mod import;`, тест-раннеры и `tests/common/import_cases.rs`
- [X] T003 [P] Образцы `packages/core/tests/fixtures/obsidian/`: 20 заметок с типовым оформлением, вики-ссылками всех форм, картинками, формулами, YAML-шапкой, HTML, mermaid, встраиванием, `.obsidian/`, `.trash/`, файл с `?` в имени, файл не в UTF-8
- [X] T004 [P] Переводы `ui/src/i18n/locales/{en,ru}/import.ts` (C1–C5, причины пропуска, виды потерь, текст отчёта)

## Phase 2: Foundational

- [X] T005 `[[путь|текст]]` в `link_sites` (поле `label`) в `packages/core/src/graph/mod.rs`; перенос 023 и «Связать» 013 сохраняют `|текст` (`packages/core/src/vault/relocate.rs`, `packages/core/src/links/link.rs`); тесты на обоих путях в `packages/core/tests/common/links_cases.rs` и `relocate_cases.rs` per FR-005
- [X] T006 Типы `ImportJob`, `ImportSurvey`, `ImportState`, `ReportEntry`, `LossKind`, `SkipReason`, `ImportJournal` в `packages/core/src/import/mod.rs`
- [X] T007 Обёртка `ReadOnly<B: VaultBackend>` (запись — ошибка) в `packages/core/src/vault/backend.rs` с тестом per FR-002

## Phase 3: User Story 1 — Перенести папку (P1) 🎯 MVP

- [X] T008 [P] [US1] Тесты перевода по таблице research.md §2 и экранированию; каждый образец собирается `compile_note` (SC-001) — `tests/common/import_cases.rs`
- [X] T009 [US1] Перевод Markdown → Typst в `packages/core/src/import/markdown.rs`
- [X] T010 [US1] Имена и дерево (research.md §6, без ссылок) в `packages/core/src/import/names.rs`
- [X] T011 [US1] Пакет `import_begin`/`import_step`/`import_finish` (без журнала) с атомарной записью и политикой совпадений в `packages/core/src/import/batch.rs` per FR-003, FR-009, FR-010
- [X] T012 [US1] Биндинги `importSurvey`, `importBegin`, `importStep`, `importFinish` в `packages/core/src/import/web.rs`; команды `import_*` и `import_open_source` в `packages/shell/src/lib.rs`; методы в `ui/src/vault-client*.ts` — методы клиента: `VaultClient.import` (`open`/`close`/`survey`/`begin`/`step`/`finish`) в `ui/src/vault-client.ts`, `vault-client-web.ts`, `vault-client-tauri.ts`
- [X] T013 [US1] Выбор исходной папки только для чтения в оболочках: `apps/web/app/VaultWorkspace.tsx` (`showDirectoryPicker({ mode: "read" })`, нет API → C5), `apps/desktop/src/DesktopWorkspace.tsx` (диалог), `apps/mobile/src/MobileWorkspace.tsx` (тот же выбор папки, что для vault); проп `pickImportSource` в `WorkspaceProps` — общий тип источника `ui/src/import/import-source.ts`; проверено в web (сценарий `import.mjs`), desktop и mobile — только typecheck
- [X] T014 [US1] `ui/src/import/ImportDialog.tsx` (C1, C5), `ImportProgress.tsx` (C2), `useImport.ts` (шаги с уступкой UI-потоку, отмена); команда палитры в `ui/src/workspace/Workspace.tsx`; стили `.ts-import*` per FR-001, FR-010

## Phase 4: User Story 2 — Ссылки, вложения, формулы (P1)

- [X] T015 [P] [US2] Тесты: вики-ссылки всех форм и разрешение по правилу Obsidian; картинки во вложениях, общая картинка копируется один раз; формулы и запасной путь; YAML → свойства 020 или блок-комментарий
- [X] T016 [US2] Особенности Obsidian (research.md §3) в `packages/core/src/import/obsidian.rs`; разрешение ссылок в `names.rs`
- [X] T017 [US2] Формулы через `mitex` с пробной сборкой в `packages/core/src/import/math.rs` per FR-007
- [X] T018 [US2] Вложения по `placement` 006 в `batch.rs` per FR-006

## Phase 5: User Story 3 — Отчёт и ход (P2)

- [X] T019 [P] [US3] Тесты: отмена посреди (нет частичных файлов, отчёт есть); журнал и продолжение без дублей; отчёт собирается
- [X] T020 [US3] Журнал `.typsidian/imports/<id>.json` и продолжение в `batch.rs`; `survey` отдаёт `resume` per FR-011
- [X] T021 [US3] Заметка отчёта (C3) в `packages/core/src/import/report.rs`; предложение снимка 011 после импорта в `ImportProgress.tsx` per FR-012 — ядро (`report.rs`) готово; предложение снимка написано (`SnapshotOffer`, только при истории `Local`), в браузере не проверено: история в сценарии выключена
- [X] T022 [US3] Экран продолжения C4 в `ImportDialog.tsx`; сценарий `scripts/dev/scenarios/import.mjs` (quickstart.md §2), снимки сверить с C1–C4

## Phase 6: Polish

- [X] T023 [P] Генератор `scripts/dev/gen-obsidian.sh` и замер SC-004 на рабочем столе (quickstart.md §3); итог — Decision в research.md (замер — путь ядра нативно, бенч `import_scale`, а не окно рабочего стола)
- [X] T024 Гейты `.specify/scripts/typsidian/gates.sh all`
- [ ] T025 README и `**Status**: Implemented` в `specs/021-obsidian-import/spec.md`

## Dependencies

T001 блокирует всё ядро. T005 — до T016. US1 → US2 → US3; UI US1 — после T012.

## Phase 7: Convergence

- [X] T026 Вынести шаги импорта на web в отдельный воркер со вторым экземпляром ядра (условие пересмотра plan.md Complexity Tracking выполнено: шаг на заметке 1 МБ — 228 мс уже нативно при пороге 50 мс, research.md §12); форма вызова `VaultClient.import` не меняется; хендлы источника и vault передаются в воркер через `postMessage` per Constitution X, SC-004 (partial)
