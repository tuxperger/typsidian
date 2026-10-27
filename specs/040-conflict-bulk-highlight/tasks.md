# Tasks: Массовое разрешение и подсветка изменений в конфликтах синхронизации

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/merge-api.md](contracts/merge-api.md)

## Phase 1: Foundational — ядро слияния

- [X] T001 Участки с происхождением: `LineOrigin`, `Segment`, внутренний проход `segments()` в `packages/core/src/sync/merge.rs`; `merge3` строит `MergeResult` из участков без изменения его формы; соседние `Agreed` склеиваются только при равном `origin` (research §1)
- [X] T002 `MergeView` и `merge_view(base, ours, theirs)` в `packages/core/src/sync/merge.rs`: пометки строк своей, их и общей колонок, сравнение своей и их без общей (research §2); `Conflict.view` в `build_conflict` (`packages/core/src/sync/session.rs`)
- [X] T003 `Pick`, `Annotated`, `merge_annotate(segments, picks, edited)` в `packages/core/src/sync/merge.rs` (research §3)
- [X] T004 Общие случаи `packages/core/tests/common/merge_view_cases.rs` + `packages/core/tests/merge_view_native.rs` и `packages/core/tests/merge_view_wasm.rs`: число `Disputed` = число `Hunk::Conflict`, пометки на примере из макета (строки 3/4/5/8/9), без общей версии, «обе» = своя + их, ручная правка → `Manual` только у тронутых строк, пустой текст, файл без завершающего перевода строки, 5 000 строк

## Phase 2: User Story 1 — видно, где чьи изменения (P1)

**Цель**: колонки, пары и результат подсвечены по сторонам; результат правится в поле с подсветкой.
**Независимая проверка**: конфликт из примера спеки — строки 3, 10, 6 помечены как в US1.

- [X] T005 [US1] Свободная функция wasm `mergeAnnotate` в `packages/core/src/sync/web.rs`; команда `sync_merge_annotate` в `packages/host/src/commands.rs`, перегенерация `node scripts/dev/gen-host-dispatch.mjs`, регистрация в `packages/shell/src/lib.rs`, разрешение в `apps/server/src/allowed.rs`
- [X] T006 [US1] `mergeAnnotate` в `ui/src/sync/sync-client.ts`, `sync-client-web.ts`, `sync-client-tauri.ts`
- [X] T007 [P] [US1] Роли цвета `--ts-merge-*`, стили легенды, строк с маркерами, пар и выбранной половины, поля результата, вкладок на узкой ширине в `ui/src/styles/app.css` (research §6, §7; макет 1a, 3)
- [X] T008 [US1] Поле результата `ui/src/sync/MergeResult.tsx`: CodeMirror с `Decoration.line` и колонкой маркеров по `Annotated.lines`, перекраска через 250 мс после правки (research §5)
- [X] T009 [US1] `ui/src/sync/MergeEditor.tsx`: легенда, колонки по `conflict.view` с номерами и маркерами, пары спорных участков по `view.segments` с цветами сторон и меткой «в результате», результат через `mergeAnnotate`; вкладки версий ниже 860px
- [X] T010 [P] [US1] Тексты легенды, меток, вкладок в `ui/src/i18n/locales/ru/sync.ts` и `ui/src/i18n/locales/en/sync.ts`

## Phase 3: User Story 2 — все спорные участки разом (P1)

**Цель**: «Везде своё / их / обе», переход по участкам, подтверждение замены ручной правки.
**Независимая проверка**: 5 участков, «Везде их» → все «их»; один переключён — остальные не меняются.

- [X] T011 [US2] Панель массового выбора в `ui/src/sync/MergeEditor.tsx`: три кнопки, счётчик участков, выбранных иначе, «участок N из M» с переходом и прокруткой к участку, подтверждение в строку при ручной правке (FR-007–FR-009; макет 1a, 1b)
- [X] T012 [P] [US2] Тексты панели и подтверждения в `ui/src/i18n/locales/{ru,en}/sync.ts`

## Phase 4: User Story 3 — разрешить конфликты пачкой (P2)

**Цель**: отметки в списке, «Оставить свои», «Взять их», «Принять совмещённые», отчёт.
**Независимая проверка**: 4 конфликта (2 чистых) — «Принять совмещённые» решает 2; «Оставить свои» для остальных завершает получение.

- [X] T013 [US3] `Operation::Resolve` в `packages/core/src/sync/mod.rs`; `BulkResolution`, `BulkReport`, `BulkFailure`, `Session::resolve_many` в `packages/core/src/sync/session.rs`: ход, отмена между файлами, частичный сбой (research §4)
- [X] T014 [US3] Случаи в `packages/core/tests/common/sync_cases.rs` и обёртки в `packages/core/tests/sync_native.rs`, `packages/core/tests/sync_wasm.rs`: «принять совмещённые» решает только чистые; «взять их» для копий Syncthing удаляет копии; последний конфликт пачки завершает получение; неизвестный id → `failed`, остальные решены; `conflicts()` несёт пометки строк. Отмена между файлами проверяется тем же `CancelToken`, что у получения; отдельного случая нет — в однопоточном тесте её не вставить между файлами
- [X] T015 [US3] `resolveMany` в `packages/core/src/sync/web.rs`; команда `sync_resolve_many` в `packages/host/src/commands.rs`, перегенерация dispatch и обёрток оболочки, `packages/shell/src/lib.rs`, `apps/server/src/allowed.rs`
- [X] T016 [US3] `resolveMany` в `ui/src/sync/sync-client.ts`, `sync-client-web.ts`, `sync-client-tauri.ts`; ход `Resolve` в описании хода (`ui/src/sync/describe.ts`)
- [X] T017 [US3] `ConflictsSection` в `ui/src/sync/SyncPanel.tsx`: флажки, «выбрать все», бейджи «N спорных / совместится само / двоичный», панель «отмечено N из M», подтверждение в строку, отчёт о несделанном, снятие отметок с исчезнувших записей (макет 2a–2c)
- [X] T018 [P] [US3] Стили списка с флажками, бейджей, панели действий и отчёта в `ui/src/styles/app.css`; тексты в `ui/src/i18n/locales/{ru,en}/sync.ts`

## Phase 5: Polish

- [X] T019 Сценарий `scripts/dev/scenarios/sync-merge.mjs` по quickstart; снимок сверен с макетом
- [X] T020 Гейты `quality-gates-typsidian` (`gates.sh all`), `security-review-agent` по двум новым командам сервера; README (раздел синхронизации); `**Status**: Implemented` в spec.md

## Dependencies

- Phase 1 → US1 → US2 (кнопки меняют выбор, который рисует US1).
- US3 зависит только от Phase 1 (`Conflict.view` нужен для бейджей) и может идти параллельно с US1/US2.
- T007, T010, T012, T018 — отдельные файлы, параллельно с соседними задачами.

## Implementation Strategy

MVP — Phase 1 + US1 + US2 (окно одного файла). US3 — следующим инкрементом.

## Phase 6: Convergence

- [X] T021 CRITICAL: ход пачки («решение конфликтов: N из M») и кнопка «Прервать» в разделе конфликтов `ui/src/sync/SyncPanel.tsx`, пока идёт `resolveMany` per Constitution X (partial)
