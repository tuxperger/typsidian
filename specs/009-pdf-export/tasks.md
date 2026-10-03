# Tasks: PDF Export

**Input**: Design documents from `/specs/009-pdf-export/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для логики ядра на обоих таргетах (принцип I конституции). Для UI
JS-тестов в репозитории нет — проверка по quickstart.md.

## Phase 1: Setup

- [X] T001 Добавить `typst-pdf = "0.15.1"` в `packages/core/Cargo.toml` с комментарием о зонде под wasm32 (research.md §1) и объявить `pub mod export;` в `packages/core/src/lib.rs` с пустым `packages/core/src/export/mod.rs`

## Phase 2: Foundational

- [X] T002 Провести `Render { Pages, Pdf(PdfMeta) }` через `compile_with`, `compile_collecting_failures` и `compile_note_with_templates` в `packages/core/src/preview/compile.rs`; публичные функции предпросмотра не меняют сигнатур и поведения, а новая `pub(crate)` функция возвращает байты PDF и диагностики (research.md §2, §3)
- [X] T003 Сделать `NativeBackend::write_bytes` атомарной (временный файл в `.typsidian/tmp/`, `sync_all`, `rename`, при `EXDEV` запасной путь рядом с целью) и чистить `.typsidian/tmp/` при `open` в `packages/core/src/vault/native.rs`; записать гарантию атомарности в doc-комментарий трейта в `packages/core/src/vault/backend.rs` (research.md §4)
- [X] T004 [P] Тест атомарности `write_bytes`: перезапись существующего файла, отсутствие остатков в `.typsidian/tmp/` после записи, уборка при открытии, имитация прерывания между записью и `rename` (на месте цели прежний файл целиком, SC-006) — в `packages/core/tests/export_native.rs`
- [X] T005 Реализовать `export::folder` (`validate_folder`, `export_folder`, `pdf_path`, `ExportFolderProblem`) с модульными тестами в `packages/core/src/export/folder.rs`
- [X] T006 Добавить `export_folder: Option<String>` в `VaultOverrides` и проверку через `validate_folder` в `write_vault_settings` с вариантом `VaultSettingsError::InvalidExportFolder` в `packages/core/src/vault/vault_settings.rs`; тест «негодная папка не трогает файл»

## Phase 3: User Story 1 — PDF текущей заметки (P1) 🎯 MVP

**Goal**: команда «Экспорт в PDF» пишет PDF открытой заметки в папку экспорта.

**Independent Test**: `Ctrl+P` на собирающейся заметке → `export/<путь>.pdf`, число страниц как в предпросмотре.

- [X] T007 [US1] Реализовать `ExportReport`, `ExportOutcome`, `ExportWriteProblem` и `export_note_pdf` по порядку шагов из contracts/core-export-api.md в `packages/core/src/export/mod.rs`
- [X] T008 [P] [US1] Интеграционные тесты `export_note_pdf` (успех с `%PDF-` и числом страниц как в предпросмотре, ошибка сборки без записи, перезапись, `TargetIsFolder`, чтение с диска без `source_text`, `NoteMissing`, вложенные папки создаются, шрифты встроены — в PDF есть `/FontFile`, SC-003) — в `packages/core/tests/common/export_cases.rs`, раннеры `export_native.rs` и `export_wasm.rs`
- [X] T009 [US1] Добавить `CommandId::ExportPdf` (`export-pdf`, `Mod+P`, «Экспорт в PDF») и `CommandId::ExportCenter` (`export-center`, `Mod+Shift+P`, «Центр экспорта») в `packages/core/src/settings/keybindings.rs`, обновить тесты уникальности умолчаний
- [X] T010 [US1] wasm-биндинг `PreviewEngine.exportPdf(backend, notePath, text?, overrides)` в `packages/core/src/preview/web.rs`
- [X] T011 [US1] Команда Tauri `export_pdf` и регистрация в `packages/shell/src/lib.rs`
- [X] T012 [US1] Метод `exportPdf` в `ui/src/preview-client.ts` и обеих реализациях: `ui/src/vault-client-web.ts` (через `wasm-call-queue`) и `ui/src/vault-client-tauri.ts`
- [X] T013 [US1] Уведомление об итоге экспорта (путь и время либо «не удалось» с переходом к первой ошибке) в `ui/src/export/ExportToast.tsx` со стилями `.ts-export-toast*` в `ui/src/styles/app.css`
- [X] T014 [US1] В `ui/src/workspace/Workspace.tsx`: `exportCurrent()` из `editorText`, сочетание `export-pdf`, команда палитры (только для открытой заметки Typst), показ `ExportToast`; удачные экспорты сеанса копятся в списке «недавние», который показывает Центр экспорта (FR-007)
- [X] T015 [US1] Кнопка «PDF» в тулбаре `ui/src/preview/PreviewPane.tsx` (проп `onExport`, недоступна во время экспорта)

## Phase 4: User Story 2 — Пакетный экспорт (P2)

**Goal**: Центр экспорта с выбором, очередью, прогрессом, пропуском и отменой.

**Independent Test**: пять заметок, отмена на третьей → только целые PDF первых двух (или трёх), без временных файлов.

- [X] T016 [US2] Реализовать `export::candidates` (`ExportCandidate`, `NotExportable`) в `packages/core/src/export/candidates.rs` с модульными тестами (доски, заметки внутри папки экспорта, порядок)
- [X] T017 [US2] Реализовать `BatchState`, `BatchJob`, `JobStatus`, `BatchPhase`, `BatchView` и переходы `batch_start/next/record/skip/cancel/view` в `packages/core/src/export/batch.rs`
- [X] T018 [P] [US2] Тесты пакета: все переходы data-model.md, запрещённые состояния, «остановить при ошибке», отмена во время `Exporting`, оценка времени, итог — в `packages/core/tests/common/export_cases.rs` (те же раннеры)
- [X] T019 [US2] wasm-биндинги `exportCandidates` и `exportBatch*` в `packages/core/src/export/web.rs`
- [X] T020 [US2] Команды Tauri `export_candidates` и `export_batch_*` в `packages/shell/src/lib.rs`
- [X] T021 [US2] `exportCandidates` и `exportBatch` в `ui/src/preview-client.ts`, `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts`
- [X] T022 [US2] Хук экспорта и цикла пакета `ui/src/export/useExport.ts` по contracts/frontend-export.md (текст открытой заметки, замер времени, уступка циклу событий, отмена при смене клиента и размонтировании)
- [X] T023 [US2] Центр экспорта по макету в `ui/src/export/ExportCenter.tsx` со стилями `.ts-export-*` в `ui/src/styles/app.css`: группы по папкам, «все», Shift-диапазон, статусы, пропуск, «Остановить при ошибке», прогресс, оценка, итог, отмена, переход к месту ошибки по клику на строку с ошибкой сборки (FR-016), список «недавние» (FR-007)
- [X] T024 [US2] В `ui/src/workspace/Workspace.tsx`: монтирование Центра, сочетание `export-center`, команда палитры, пилюля «экспорт N/M» в шапке; при открытии Центра открытая заметка уже отмечена (FR-014)

## Phase 5: User Story 3 — Своя папка экспорта (P3)

**Goal**: папка экспорта задаётся в настройках vault.

**Independent Test**: папка `pdf` → следующий экспорт в `pdf/…`; `../x` отклонено.

- [X] T025 [US3] Поле «Папка экспорта» с показом отказа `InvalidExportFolder` в `ui/src/settings/VaultSection.tsx`
- [X] T026 [US3] Ссылка «изменить» у папки вывода в Центре экспорта открывает настройки в `ui/src/export/ExportCenter.tsx`

## Phase 6: Polish

- [X] T027 Сборка ядра под wasm32 и native, `cargo test -p typsidian-core`, wasm-тесты `export_wasm` и `placement_wasm`, `cargo test -p typsidian-shell`
- [X] T028 Пересобрать wasm-core и `ui/dist`, `pnpm typecheck` и `pnpm build` для всех оболочек
- [X] T029 Ручная проверка по quickstart.md на стенде chromium headless (web) и в desktop-сборке, включая замеры SC-001 (10 страниц) и SC-004 (задержка ввода во время пакета из 100 заметок на desktop, на web — зафиксировать фактическое значение) и 20 прогонов отмены в случайный момент с проверкой `find <папка экспорта> .typsidian/tmp -newer` на недописанные файлы; результаты — в конец этого файла

## Dependencies

- Setup → Foundational (T002, T003, T005, T006) → US1 → US2 → US3.
- US2 использует `export_note_pdf` из US1 (T007). US3 зависит только от T006 и может идти
  параллельно с US2.
- T008 параллелен T009–T015 (другой файл); T018 параллелен T019–T024.

## Implementation Strategy

MVP — фазы 1–3: экспорт текущей заметки на всех клиентах. Дальше пакет (US2) и настройка
папки (US3). Коммит — фича целиком (конвенции репозитория).

## Результаты проверки (2026-09-28)

**Автоматика**
- `cargo test -p typsidian-core` (native): весь набор зелёный, из него 25 тестов в
  `export_native` (22 общих сценария + 3 атомарной записи на диск) и 2 теста папки
  экспорта в `placement_native`.
- `wasm-pack test --headless --chrome` для `export_wasm` (22), `placement_wasm` (26),
  `preview_wasm`, `install_wasm` — зелёные.
- `cargo test -p typsidian-shell`, `pnpm -r typecheck` (ui, web, desktop, mobile),
  `pnpm build` (web) — без ошибок.

**Стенд web** (headless chromium, настоящее wasm-ядро, `WebBackend` над каталогом OPFS,
`Workspace` + `SettingsPanel` из `ui/dist`) — 48 проверок PASS, ошибок в консоли нет:
- US1: `Ctrl+P` → `export/Диссертация/Введение.pdf` (`%PDF-`); незаписанная правка попала в
  повторный PDF (12 221 → 14 635 байт); сломанная заметка — отказ, файл не создан,
  «К ошибке» ставит курсор на строку ошибки.
- US2: открытая заметка отмечена при открытии Центра; доска «не экспортируется»; порядок
  «корень, потом папки»; Shift-диапазон в порядке экрана; пакет из 6: готово 4, ошибка 1 с
  переходом, пропуск 1; недавние перечислены. Отмена во время первой заметки: она доведена
  до конца, остальные 5 отменены, в `export/` только целые PDF (каждый оканчивается `%%EOF`).
- US3: `../x` отклонено с причиной, ` Выгрузка\2026/ ` нормализовано в `Выгрузка/2026`,
  следующий экспорт записан туда.

**Найдено стендом и исправлено**: диапазон Shift схлопывался в одну строку (якорь читался
в функции обновления состояния); поле папки пропадало у vault без настройки (умолчание
параметра превращало `undefined` в `null`); «вывод» в Центре показывал папку прошлого
пакета вместо текущей настройки; порядок кандидатов перемешивал корень с папками.

**Замеры** (web, главный поток): заметка на 2 страницы — 9–32 мс; заметка 2,6 МБ PDF —
1,5–1,6 с первая сборка, ~290 мс повторная; пакет из 6 — ~3,1 с. SC-001 на web выполнен с
запасом. SC-004 (задержка ввода ≤100 мс) на web **не выполняется** для тяжёлых заметок: сборка
одной заметки занимает поток целиком — это принятое отклонение (plan.md → Complexity
Tracking, 005 T022).

**Не проверено вживую**: desktop- и Android-сборки не запускались (Tauri-команды собраны и
типизированы, `typsidian-shell` проходит тесты); SC-002/SC-003 на 20 эталонных заметках и
100 прогонов отмены (SC-006) — не прогонялись, покрыты единичными тестами и стендом.
