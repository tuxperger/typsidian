# Tasks: Редактор правил исключения

**Input**: Design documents from `/specs/012-ignore-rules-editor/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для логики ядра на обоих таргетах (принцип II конституции): чистые
функции — модульными тестами в `ignore.rs`, операции сессии — общими случаями в
`tests/common/sync_cases.rs`, которые идут из `sync_native.rs` и `sync_wasm.rs`. Для UI
JS-тестов в репозитории нет — проверка по quickstart.md.

## Phase 1: Setup

- [X] T001 Объявить типы из data-model.md (`LineEnding`, `LineRange`, `Editability`, `IgnoreCheck`, `IgnoreDocument`, `IgnoreSaveRequest`, `IgnoreRefusal`, `IgnoreSaveOutcome`) с `serde` и `tsify` в `packages/core/src/sync/ignore.rs` и реэкспортировать их из `packages/core/src/sync/mod.rs`

## Phase 2: Foundational

**Purpose**: чистые функции и привязки, без которых не работает ни одна история.

- [X] T002 Дословная отмена обязательного правила (`!<правило>`, сильнее правила по порядку формата: ниже — в `.gitignore`, выше — в `.stignore`) в `ignore::report`, функции `ignore::check` (недостающие правила, диапазон строк блока, предложенный блок) и `ignore::restore_block` в `packages/core/src/sync/ignore.rs`; модульные тесты там же: совпадение `check().missing` с `report().missing`, отмена, диапазон блока при закрытом, незакрытом и отсутствующем блоке, идемпотентность `restore_block`, неизменность строк пользователя per FR-010, FR-011, FR-013, SC-004
- [X] T003 [P] Разбор и восстановление окончаний строк (`LineEnding::detect`, перевод текста `\n` ↔ вид файла) и отметка версии (blake3, hex) в `packages/core/src/sync/ignore.rs`; модульные тесты: LF, CRLF, смешанные, без завершающего перевода строки per FR-003, research §3, §5
- [X] T004 [P] `VaultBackend::is_symlink` с умолчанием `false` в `packages/core/src/vault/backend.rs` и реализацией по `symlink_metadata` в `packages/core/src/vault/native.rs` per FR-016, research §6

## Phase 3: User Story 1 — Открыть файл правил и изменить свои правила (P1) 🎯 MVP

**Goal**: файл правил открывается из панели, правится и сохраняется; несуществующий —
создаётся.

**Independent Test**: quickstart.md, сценарии 1, 2, 5, 7.

- [X] T006 [US1] `SyncSession::ignore_open` (чтение, `editable`, `in_effect`, `foreign_parent`, предложенный блок для отсутствующего файла) и `SyncSession::ignore_save` (проверка возможности правки, запись текста с окончаниями файла через атомарную запись, событие `OverviewChanged`, снятие `stignore_declined` при полных правилах) в `packages/core/src/sync/session.rs` per FR-001, FR-003…FR-005, FR-007, FR-009, FR-016, research §10
- [X] T007 [US1] Общие случаи `ignore_editor_*` в `packages/core/tests/common/sync_cases.rs` и их вызовы в `packages/core/tests/sync_native.rs`, `packages/core/tests/sync_wasm.rs`: открыть существующий, открыть отсутствующий (файл не создаётся), сохранить (текст на диске равен тексту редактора, CRLF сохраняется), `in_effect` без истории и без Syncthing, не UTF-8 → `NotText` и файл цел, после сохранения — событие `OverviewChanged` и сам файл правил в списке изменений (действие пользовательского правила на список — по quickstart: двойник движка правил не читает), сохранение на устройстве-не-хранителе проходит per FR-003, FR-005, FR-007, FR-009, FR-016, FR-020
- [X] T008 [US1] Привязки: wasm — `ignoreOpen`, `ignoreCheck`, `ignoreRestore`, `ignoreSave` в `packages/core/src/sync/web.rs`; Tauri — команды `sync_ignore_open`, `sync_ignore_check`, `sync_ignore_restore`, `sync_ignore_save` в `packages/shell/src/sync.rs` и их регистрация в `packages/shell/src/lib.rs` per contracts/core-ignore-api.md, FR-019
- [X] T005 [US1] Методы `ignoreOpen`, `ignoreCheck`, `ignoreRestore`, `ignoreSave` в интерфейсе `SyncClient` (`ui/src/sync/sync-client.ts`) (типы — из `ui/src/wasm-core` после T008)
- [X] T009 [US1] Реализации методов `SyncClient` в `ui/src/sync/sync-client-web.ts` и `ui/src/sync/sync-client-tauri.ts` per contracts/frontend-ignore.md
- [X] T010 [P] [US1] Строки `sync.ignore.*` (раздел панели, заголовки, пояснения FR-007/FR-008, «будет создан», причины `Editability`, кнопки, вопрос при закрытии) в `ui/src/i18n/locales/ru/sync.ts` и `ui/src/i18n/locales/en/sync.ts` per FR-021
- [X] T011 [US1] Хук состояния диалога `ui/src/sync/useIgnoreEditor.ts`: загрузка документа, текст и признак правки, сохранение с разбором `IgnoreSaveOutcome`, вызов `sync.refresh()` после `Saved` per data-model.md «Переходы состояния»
- [X] T012 [US1] Диалог `ui/src/sync/IgnoreEditor.tsx`: поле CodeMirror на весь файл, пояснения, состояние «нередактируемый» с причиной, сохранение (`Ctrl/Cmd+S`), вопрос «сохранить / отбросить / вернуться» при закрытии с правкой; стили в `ui/src/styles/app.css` (узкий экран — на всю ширину) per FR-002, FR-005…FR-008, FR-016, FR-018, FR-022
- [X] T013 [US1] Раздел «Правила исключения» с входом в оба файла в `ui/src/sync/SyncPanel.tsx`; действие «Открыть» в предупреждении о недостающих правилах per FR-001, FR-020

## Phase 4: User Story 2 — Не сломать правила, которые нужны приложению (P2)

**Goal**: нехватка обязательных правил видна по мере набора, возвращается одним действием,
сохранение без них — только с подтверждением.

**Independent Test**: quickstart.md, сценарий 3.

- [X] T014 [US2] `NeedsConfirmation` в `SyncSession::ignore_save` (`packages/core/src/sync/session.rs`): без `confirm_incomplete` текст с недостающими правилами не пишется; с ним — пишется, `stignore_declined` не меняется; случаи в `packages/core/tests/common/sync_cases.rs` (+ вызовы в `sync_native.rs`, `sync_wasm.rs`): отказ без подтверждения, запись с ним, предупреждение сводки остаётся, дословная отмена per FR-012, FR-013, SC-003
- [X] T015 [US2] Подсветка и подпись блока приложения по `check.block`, проверка по мере набора (`ignoreCheck`, ~300 мс), список недостающих правил с последствием, «Вернуть правила приложения» (`ignoreRestore`), подтверждение при `NeedsConfirmation` в `ui/src/sync/useIgnoreEditor.ts`, `ui/src/sync/IgnoreEditor.tsx`; строки в `ui/src/i18n/locales/{ru,en}/sync.ts` per FR-002, FR-010…FR-012

## Phase 5: User Story 3 — Не затереть чужую правку файла (P2)

**Goal**: файл, изменённый или удалённый извне, не перезаписывается молча.

**Independent Test**: quickstart.md, сценарий 4.

- [X] T016 [US3] Сверка `base` в `SyncSession::ignore_save` (`packages/core/src/sync/session.rs`): `Diverged { disk, diff }` при несовпадении и при удалённом файле, запись при `overwrite`; `Refused { HistoryBusy }` для `.gitignore` при незавершённой операции истории или неразрешённых конфликтах получения; случаи в `packages/core/tests/common/sync_cases.rs` (+ вызовы в `sync_native.rs`, `sync_wasm.rs`): расхождение не пишет файл, `overwrite` пишет, удалённый файл, то же содержимое не считается расхождением, занятость истории per FR-014…FR-017, SC-002
- [X] T017 [US3] В `ui/src/sync/useIgnoreEditor.ts` и `ui/src/sync/IgnoreEditor.tsx`: перечитывание документа при росте `sync.revision` (без правки — замена текста и уведомление, с правкой — предупреждение), экран расхождения с `DiffView` и действиями «взять с диска / записать свою / вернуться», причины `Refused`; строки в `ui/src/i18n/locales/{ru,en}/sync.ts` per FR-014, FR-015, FR-017, FR-018

## Phase 6: Polish

- [ ] T018 Гейты качества: `.specify/scripts/typsidian/gates.sh all` и `scripts/check-i18n.sh` — fmt, оба таргета ядра, `cargo test`, `wasm-bindgen-test`, типы UI per Definition of done конституции
- [ ] T019 Прогнать quickstart.md в web и на рабочем столе, дополнить раздел о синхронизации в `README.md`, `**Status**: Implemented` в `specs/012-ignore-rules-editor/spec.md`

## Dependencies

- T001 → T002, T003 → T006 → T007, T008 → T009 → T011 → T012 → T013.
- T004 нужен T006 (причина `NotAFile`); T005 — после T008 (типы в `wasm-core`), до T009;
  номер сохранён, порядок выполнения — как в списке.
- US2 (T014, T015) и US3 (T016, T017) зависят от US1 и независимы друг от друга; обе
  правят `session.rs`, `useIgnoreEditor.ts`, `IgnoreEditor.tsx` — выполняются по очереди.
- T018, T019 — после всех историй.

## Parallel Opportunities

- T003 и T004 — параллельно с T002 (разные функции и файлы).
- T010 — параллельно с T006–T009.

## Implementation Strategy

MVP — фазы 1–3 (US1): редактор открывает, правит и сохраняет оба файла. US2 и US3
добавляют защиту поверх готового диалога и сдаются по отдельности; до них сохранение уже
атомарно и не трогает нередактируемые файлы.
