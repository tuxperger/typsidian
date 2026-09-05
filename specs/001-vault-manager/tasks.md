---

description: "Task list template for feature implementation"
---

# Tasks: Vault Manager

**Input**: Design documents from `/specs/001-vault-manager/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md (все присутствуют)

**Tests**: Включены. Конституция (принципы I и II, NON-NEGOTIABLE) требует, чтобы каждая
единица логики `packages/core` была покрыта и `cargo test`, и `wasm-bindgen-test` — это не
опциональный запрос, а Definition of Done проекта.

**Organization**: Задачи сгруппированы по user story из spec.md (US1…US5, приоритеты
P1…P5) для независимой реализации и проверки каждой.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: можно выполнять параллельно (разные файлы, нет зависимости от незавершённых задач)
- **[Story]**: к какой user story относится задача (US1…US5)
- Пути к файлам — по структуре из plan.md (`packages/core/src/vault/…`, `packages/core/src/graph/…`, `ui/src/…`)

## Path Conventions

Структура — Rust-workspace ядро + общий UI-код, см. `plan.md` → Project Structure:
`packages/core/src/vault/`, `packages/core/src/graph/`, `packages/core/tests/`, `ui/src/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: инициализация проекта — структура каталогов, зависимости, инструменты.

- [X] T001 Создать скелет Rust-workspace `packages/core`: `Cargo.toml` (member воркспейса),
      `src/lib.rs`, файлы-заглушки `src/vault/{mod.rs,backend.rs,native.rs,web.rs,index.rs,cache.rs,conflict.rs,events.rs}`,
      `src/graph/mod.rs` — по структуре из `plan.md`
- [X] T002 [P] Создать скелет общего UI-пакета `ui/`: `package.json`, `tsconfig.json` в режиме
      `strict`, каталоги `src/vault-tree/`, `src/editor/` — по структуре из `plan.md`
- [X] T003 Добавить cfg-разделённые зависимости в `packages/core/Cargo.toml`: нативные
      (`notify`, `notify-debouncer-full`, `sqlx` с драйвером `sqlite`) под
      `not(target_arch = "wasm32")`; веб (`wasm-bindgen`, `web-sys`, `wasm-bindgen-futures`,
      `wa-sqlite`) под `target_arch = "wasm32"`; общие (`serde`, `blake3`, генератор TS-типов)
      без cfg — согласно research.md §1, §5, §6, §9
- [X] T004 [P] Настроить `rustfmt.toml`/clippy-линты для `packages/core` и ESLint/`tsconfig`
      строгие правила для `ui/` — принцип IV конституции (TypeScript strict, без `any`)
- [X] T005 [P] Настроить гарнесс `wasm-bindgen-test`: `packages/core/tests/vault_wasm.rs`
      (точка входа) и конфигурацию `wasm-pack test`, а также точку входа `cargo test` в
      `packages/core/tests/vault_native.rs` — оба таргета обязательны (принцип II, plan.md
      Testing)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: общая инфраструктура, без которой не может начаться ни одна user story.

**⚠️ CRITICAL**: ни одна user story не начинается до завершения этой фазы.

- [X] T006 Определить сигнатуры трейта `VaultBackend` (open/list/list_folder/watch/read/write/
      create/rename/delete/backlinks) и связанные типы ошибок (`OpenError`, `ReadError`,
      `WriteError`, `CreateError`, `RenameError`, `DeleteError`) без реализации в
      `packages/core/src/vault/backend.rs` — по `contracts/vault-backend-trait.md`
- [X] T007 [P] Определить `Vault { root_path, access_state, last_opened_at, note_count }` и
      `AccessState` в `packages/core/src/vault/mod.rs` — по `data-model.md`
- [X] T008 [P] Определить `Note { path, name, mtime, size, content_hash, has_unsaved_edits }`
      в `packages/core/src/vault/index.rs` — по `data-model.md`
- [X] T009 [P] Определить размеченный enum `VaultEvent` (NoteCreated/NoteUpdated/NoteRenamed/
      NoteDeleted/ConflictDetected/NoteDeletedWithUnsavedEdits/BulkChangeStarted/
      BulkChangeSettled/VaultAccessChanged/LinksInvalidated) с `serde`/генератором TS-типов в
      `packages/core/src/vault/events.rs` — по `data-model.md` и принципу IV (типы границы
      WASM/JS порождаются из Rust)
- [X] T010 [P] Определить `EditConflict { note_path, local_buffer, disk_snapshot, status }`
      (только в памяти) в `packages/core/src/vault/conflict.rs` — по `data-model.md`
- [X] T011 Реализовать структуру in-memory индекса (map путь→`Note` + вывод иерархии папок из
      общих префиксов путей) в `packages/core/src/vault/index.rs` (зависит от T008)
- [X] T012 [P] Определить SQL-схему персистентного кэша (таблицы `notes`, `links`), общую для
      `sqlx` и `wa-sqlite`, в `packages/core/migrations/0001_vault_cache.sql` — research.md §6
- [X] T013 Реализовать трейт `IndexCache` и его native-реализацию на `sqlx` в
      `packages/core/src/vault/cache.rs` (зависит от T012)
- [X] T014 Реализовать web-реализацию `IndexCache` на `wa-sqlite` в
      `packages/core/src/vault/cache.rs` (зависит от T012, T013 — тот же файл, без `[P]`)
- [X] T015 [P] Создать заглушку `NativeBackend`, реализующую `VaultBackend` через
      `unimplemented!()`, `cfg(not(target_arch = "wasm32"))`, в
      `packages/core/src/vault/native.rs` (зависит от T006)
- [X] T016 [P] Создать заглушку `WebBackend`, реализующую `VaultBackend` через
      `unimplemented!()`, `cfg(target_arch = "wasm32")`, в `packages/core/src/vault/web.rs`
      (зависит от T006)
- [X] T017 Проверить сборку `packages/core` под оба таргета на заглушках: `cargo build -p core`
      и `wasm-pack build packages/core --target web` — гейт принципа II перед началом работы
      над user stories

**Checkpoint**: скелет ядра собирается под оба таргета — можно начинать user stories.

---

## Phase 3: User Story 1 - Открыть папку как vault (Priority: P1) 🎯 MVP

**Goal**: пользователь выбирает папку, видит дерево всех заметок (рекурсивно), может открыть
любую на чтение/редактирование; последний vault предлагается при следующем запуске; ошибки
доступа не ломают приложение.

**Independent Test**: подготовить папку с заметками во вложенных подпапках, открыть её как
vault, убедиться, что дерево содержит все заметки в правильной иерархии и любая из них
открывается (spec.md US1 Independent Test).

### Tests for User Story 1 ⚠️

> Пишутся первыми, должны падать до реализации.

- [X] T018 [P] [US1] cargo test: `NativeBackend::open()` рекурсивно индексирует вложенные
      заметки и исключает скрытые/служебные каталоги в `packages/core/tests/vault_native.rs`
      (FR-002, FR-003, FR-004)
- [X] T019 [P] [US1] wasm-bindgen-test: то же поведение `WebBackend::open()` в
      `packages/core/tests/vault_wasm.rs` (паритет, принцип II)
- [X] T020 [P] [US1] cargo test: `open()` на пустой папке успешен, дерево пустое, без ошибки
      (US1 AS3) в `packages/core/tests/vault_native.rs`
- [X] T021 [P] [US1] cargo test: `open()` на недоступной папке возвращает `OpenError` с ясной
      причиной, backend остаётся рабочим после этого (FR-008) в
      `packages/core/tests/vault_native.rs`

### Implementation for User Story 1

- [X] T022 [US1] Реализовать `NativeBackend::open()`/`list()`/`list_folder()`: параллельный
      обход дерева (research.md §4), исключение скрытых/служебных каталогов (FR-004),
      символические ссылки не раскрываются (Assumptions), заполнение индекса и кэша в
      `packages/core/src/vault/native.rs` (зависит от T011, T013, T015)
- [X] T023 [US1] Реализовать индикатор прогресса `NativeBackend`, если обход не завершился за
      1 секунду (FR-005) в `packages/core/src/vault/native.rs`
- [X] T024 [US1] Реализовать `WebBackend::open()`/`list()`/`list_folder()`: `showDirectoryPicker`
      + чанкованный обход в Web Worker (research.md §4), те же исключения, заполнение индекса и
      кэша через `wa-sqlite` в `packages/core/src/vault/web.rs` (зависит от T011, T014, T016)
- [X] T025 [US1] Реализовать индикатор прогресса `WebBackend`, зеркалящий FR-005, в
      `packages/core/src/vault/web.rs`
- [X] T026 [US1] Реализовать быстрый путь повторного открытия (SC-003): при валидном кэше для
      `root_path` — немедленный возврат дерева из кэша с фоновой сверкой, в
      `packages/core/src/vault/cache.rs` (зависит от T013, T014, T022, T024)
- [X] T027 [P] [US1] Реализовать запоминание последнего vault (FR-007): хранение
      `root_path`/ссылки на доступ вне кэша vault (native — конфиг приложения; web —
      IndexedDB-хендл) в `packages/core/src/vault/mod.rs`
- [X] T028 [US1] Реализовать `read(path)` для обоих backend, возвращающий содержимое заметки, в
      `packages/core/src/vault/native.rs` и `packages/core/src/vault/web.rs` (зависит от T022,
      T024)
- [X] T029 [P] [US1] Собрать компонент дерева vault (рендер иерархии из списка `Note`, открытие
      заметки по клику) в `ui/src/vault-tree/VaultTree.tsx` — по
      `contracts/frontend-events.md`
- [X] T030 [P] [US1] Собрать поток «открыть vault» (вызов выбора папки, предложение продолжить с
      последним vault, баннер ошибки доступа) в `ui/src/vault-tree/OpenVault.tsx` (FR-001,
      FR-007, FR-008)
- [X] T031 [US1] Связать вызовы open/list/read из UI с `VaultBackend` по
      `contracts/vault-backend-trait.md` в `ui/src/vault-tree/useVault.ts`
- [ ] T032 [US1] Прогнать `specs/001-vault-manager/quickstart.md` Сценарий 1 на native- и
      web-сборке; подтвердить паритет (принцип II)

**Checkpoint**: US1 полностью функциональна и тестируема независимо — пользователь открывает
любую локальную папку и просматривает/читает заметки на обоих таргетах.

---

## Phase 4: User Story 2 - Управлять заметками из интерфейса (Priority: P2)

**Goal**: создание, переименование, удаление заметки прямо из интерфейса, с немедленным
отражением на диске и в дереве.

**Independent Test**: в открытом vault создать заметку, переименовать её, удалить — проверить в
файловом менеджере, что файл появился, сменил имя и исчез (spec.md US2 Independent Test).

### Tests for User Story 2 ⚠️

- [X] T033 [P] [US2] cargo test: `create()` отклоняет конфликт имён без перезаписи существующего
      файла (FR-012) в `packages/core/tests/vault_native.rs`
- [X] T034 [P] [US2] cargo test: `create()`/`rename()` отклоняют недопустимые для ФС имена до
      касания диска (FR-013) в `packages/core/tests/vault_native.rs`
- [X] T035 [P] [US2] wasm-bindgen-test: та же валидация на `WebBackend` в
      `packages/core/tests/vault_wasm.rs`
- [X] T036 [P] [US2] cargo test: неудавшаяся запись (смоделированная ошибка прав/места) сообщает
      причину и оставляет индекс согласованным с диском (FR-014) в
      `packages/core/tests/vault_native.rs`

### Implementation for User Story 2

- [X] T037 [US2] Реализовать `NativeBackend::create()`/`rename()`/`delete()` с проверкой до
      записи (конфликт имён, недопустимое имя) в `packages/core/src/vault/native.rs` (зависит
      от T022)
- [X] T038 [US2] Реализовать `WebBackend::create()`/`rename()`/`delete()` с той же проверкой в
      `packages/core/src/vault/web.rs` (зависит от T024)
- [X] T039 [US2] Реализовать удаление в корзину на native (с откатом на безвозвратное) и
      безвозвратное удаление на web — research.md §7 — в `packages/core/src/vault/native.rs` и
      `packages/core/src/vault/web.rs`
- [X] T040 [US2] Обеспечить сохранение связи открытого редактора с переименованной заметкой
      (FR-015) — обновление пути в индексе без разрыва сессии редактирования — в
      `packages/core/src/vault/index.rs`
- [X] T041 [US2] Обновлять индекс и кэш синхронно при успешном create/rename/delete в
      `packages/core/src/vault/cache.rs` (зависит от T037, T038)
- [X] T042 [P] [US2] Собрать UI-действия create/rename/delete (контекстное меню + инлайн-
      переименование) в `ui/src/vault-tree/VaultTreeActions.tsx`
- [X] T043 [P] [US2] Отобразить сообщения об ошибках из вариантов `CreateError`/`RenameError`/
      `DeleteError` в `ui/src/vault-tree/ErrorBanner.tsx` (FR-013, FR-014 — без общего
      «что-то пошло не так»)
- [ ] T044 [US2] Прогнать `specs/001-vault-manager/quickstart.md` Сценарий 2 на native- и
      web-сборке; подтвердить паритет

**Checkpoint**: US1 и US2 работают независимо друг от друга.

---

## Phase 5: User Story 3 - Подхватывать внешние изменения без перезапуска (Priority: P3)

**Goal**: приложение само замечает создание/изменение/переименование/удаление файлов вне себя
(включая другое окно приложения) и приводит состояние в соответствие с диском без перезапуска;
корректно обрабатывает потерю доступа и отзыв прав на запись.

**Independent Test**: при открытом приложении выполнить извне create/edit/rename/delete в vault
и убедиться, что дерево и содержимое неотредактированных заметок обновились без перезапуска
(spec.md US3 Independent Test).

### Tests for User Story 3 ⚠️

- [X] T045 [P] [US3] cargo test: `NativeBackend::watch()` эмитит NoteCreated/Updated/Renamed/
      Deleted на внешние изменения ФС в `packages/core/tests/vault_native.rs` (FR-016)
- [X] T046 [P] [US3] cargo test: дебаунсинг схлопывает атомарную запись через временный файл +
      rename в одно событие, без мигания (Edge Case) в `packages/core/tests/vault_native.rs`
- [X] T047 [P] [US3] wasm-bindgen-test: `WebBackend::watch()` на основе поллинга эмитит
      эквивалентные события в пределах бюджета (FR-016, SC-004) в
      `packages/core/tests/vault_wasm.rs`
- [X] T048 [P] [US3] cargo test: массовое внешнее изменение (1000 файлов) эмитит
      BulkChangeStarted/Settled, индекс сходится к состоянию диска, backend остаётся отзывчивым
      (FR-019, SC-005) в `packages/core/tests/vault_native.rs`
- [X] T049 [P] [US3] cargo test: потеря доступа к vault эмитит `VaultAccessChanged(Unavailable)`,
      несохранённые правки остаются в памяти, разрушительных действий не выполняется (FR-020,
      FR-035) в `packages/core/tests/vault_native.rs`
- [X] T050 [P] [US3] cargo test: отзыв прав на запись при сохранённом чтении эмитит
      `VaultAccessChanged(ReadOnly)`; `write()` немедленно возвращает `AccessDenied` (FR-036) в
      `packages/core/tests/vault_native.rs`
- [X] T051 [P] [US3] cargo test: повторное открытие vault после закрытия обнаруживает
      изменения, накопившиеся, пока приложение было закрыто (FR-018) в
      `packages/core/tests/vault_native.rs`
- [X] T052 [P] [US3] cargo test: два экземпляра `VaultBackend` на одном `root_path` (модель двух
      окон) — изменение из экземпляра B доходит до A как обычное внешнее `VaultEvent` (FR-034)
      в `packages/core/tests/vault_native.rs`

### Implementation for User Story 3

- [X] T053 [US3] Реализовать `NativeBackend::watch()` на `notify` + дебаунсер (research.md §2),
      эмитящий поток `VaultEvent`, в `packages/core/src/vault/native.rs` (зависит от T022)
- [X] T054 [US3] Реализовать `WebBackend::watch()` на адаптивной периодической сверке
      (research.md §3), эмитящий поток `VaultEvent`, в `packages/core/src/vault/web.rs`
      (зависит от T024)
- [X] T055 [US3] Реализовать обрамление массовых изменений (BulkChangeStarted/Settled) вокруг
      пачек событий на обоих backend в `packages/core/src/vault/events.rs` (зависит от T053,
      T054)
- [X] T056 [US3] Реализовать определение переходов доступа (ReadWrite↔ReadOnly↔Unavailable) и
      эмиссию `VaultAccessChanged` в `packages/core/src/vault/native.rs` и
      `packages/core/src/vault/web.rs` (FR-020, FR-036)
- [X] T057 [US3] Реализовать удержание несохранённых правок в памяти через состояние
      Unavailable, сброс только при закрытии приложения (FR-035) в
      `packages/core/src/vault/conflict.rs`
- [X] T058 [US3] Связать поток `watch()` с обновлением индекса и персистентного кэша на каждое
      событие в `packages/core/src/vault/cache.rs` (зависит от T053, T054)
- [X] T059 [US3] Реализовать сверку при повторном открытии: сравнение снимка кэша с реальным
      состоянием диска в `open()` (FR-018) в `packages/core/src/vault/native.rs` и
      `packages/core/src/vault/web.rs` (зависит от T026)
- [X] T060 [P] [US3] Реализовать обработку в UI событий NoteCreated/Updated/Renamed/Deleted с
      живым обновлением дерева/редактора в `ui/src/vault-tree/useVaultEvents.ts` — по
      `contracts/frontend-events.md`
- [X] T061 [P] [US3] Собрать баннеры UI для `VaultAccessChanged` (полноэкранное уведомление для
      Unavailable, заблокированные контролы для ReadOnly) в
      `ui/src/vault-tree/AccessStateBanner.tsx` (FR-020, FR-036)
- [X] T062 [P] [US3] Собрать индикатор BulkChangeStarted/Settled (единый индикатор вместо
      шторма отдельных уведомлений) в `ui/src/vault-tree/BulkChangeIndicator.tsx`
- [ ] T063 [US3] Прогнать `specs/001-vault-manager/quickstart.md` Сценарии 3 и 6 на native- и
      web-сборке; подтвердить паритет

**Checkpoint**: US1, US2, US3 работают независимо — vault остаётся синхронизирован с диском без
перезапуска.

---

## Phase 6: User Story 4 - Разрешить конфликт с внешним изменением (Priority: P4)

**Goal**: при внешнем изменении заметки с несохранёнными правками пользователь получает выбор
(оставить свою / принять внешнюю / посмотреть diff) — ни одна версия не применяется
автоматически.

**Independent Test**: внести несохранённые правки в открытую заметку, изменить тот же файл
снаружи, проверить появление выбора из трёх вариантов и ожидаемый результат для каждого (spec.md
US4 Independent Test).

### Tests for User Story 4 ⚠️

- [X] T064 [P] [US4] cargo test: внешнее изменение заметки с несохранёнными правками даёт
      `ConflictDetected` вместо тихого `NoteUpdated`, ни одна версия не применяется
      автоматически (FR-021) в `packages/core/tests/vault_native.rs`
- [X] T065 [P] [US4] cargo test: `write()` с устаревшим `expected_snapshot` возвращает
      `WriteError::Conflict`, а не перезаписывает диск (contracts/vault-backend-trait.md) в
      `packages/core/tests/vault_native.rs`
- [X] T066 [P] [US4] cargo test: разрешение «оставить свою версию» записывает локальный буфер на
      диск и снимает конфликт (US4 AS2) в `packages/core/tests/vault_native.rs`
- [X] T067 [P] [US4] cargo test: разрешение «принять внешнюю» требует явного подтверждения перед
      отбрасыванием локального буфера (US4 AS3, FR-023) в `packages/core/tests/vault_native.rs`
- [X] T068 [P] [US4] cargo test: повторное изменение диска, пока конфликт не разрешён, обновляет
      `disk_snapshot` до актуального, а не оставляет устаревший (US4 AS5) в
      `packages/core/tests/vault_native.rs`
- [X] T069 [P] [US4] cargo test: внешнее удаление заметки с несохранёнными правками даёт
      `NoteDeletedWithUnsavedEdits`, позволяет пересохранить (FR-025) в
      `packages/core/tests/vault_native.rs`
- [X] T070 [P] [US4] wasm-bindgen-test: паритет обнаружения/разрешения конфликта на `WebBackend`
      в `packages/core/tests/vault_wasm.rs`

### Implementation for User Story 4

- [X] T071 [US4] Реализовать двухступенчатое обнаружение конфликта (предфильтр mtime/size +
      подтверждение хэшем содержимого, research.md §5) в `packages/core/src/vault/conflict.rs`
      (зависит от T010, T058)
- [X] T072 [US4] Реализовать конечный автомат `EditConflict`
      (Pending→ResolvedKeepLocal/ResolvedAcceptExternal) в
      `packages/core/src/vault/conflict.rs` (зависит от T071)
- [X] T073 [US4] Реализовать генерацию diff между `local_buffer` и `disk_snapshot` для FR-024 в
      `packages/core/src/vault/conflict.rs`
- [X] T074 [US4] Связать эмиссию `ConflictDetected`/`NoteDeletedWithUnsavedEdits` с пайплайном
      `watch()` на обоих backend в `packages/core/src/vault/native.rs` и
      `packages/core/src/vault/web.rs` (зависит от T053, T054, T071)
- [X] T075 [US4] Реализовать проверку оптимистичной конкурентности `write()` по
      `expected_snapshot` на обоих backend в `packages/core/src/vault/native.rs` и
      `packages/core/src/vault/web.rs`
- [X] T076 [P] [US4] Собрать диалог разрешения конфликта (оставить свою / принять внешнюю /
      посмотреть diff) в `ui/src/editor/ConflictDialog.tsx` — по
      `contracts/frontend-events.md`
- [X] T077 [P] [US4] Собрать компонент отображения diff в `ui/src/editor/DiffView.tsx`
- [X] T078 [US4] Связать `ConflictDialog` с действиями разрешения `write()`/`EditConflict` в
      `ui/src/editor/useConflictResolution.ts`
- [ ] T079 [US4] Прогнать `specs/001-vault-manager/quickstart.md` Сценарий 4 на native- и
      web-сборке; подтвердить паритет

**Checkpoint**: US1–US4 работают независимо — конфликтующие правки не теряются молча
(включая закрытие окна с неразрешённым конфликтом — см. T099, добавлена после второй
сессии `/speckit-clarify`).

---

## Phase 7: User Story 5 - Не терять ссылки при удалении (Priority: P5)

**Goal**: перед удалением/переименованием заметки со ссылками пользователь видит список
заметок, которые останутся с битой ссылкой, и подтверждает операцию.

**Independent Test**: создать заметку A со ссылками на неё из B и C, удалить A, убедиться, что
перед удалением показан список B и C (spec.md US5 Independent Test).

### Tests for User Story 5 ⚠️

- [X] T080 [P] [US5] cargo test: `backlinks(path)` возвращает заметки, ссылающиеся на неё, после
      индексации минимального извлечения ссылок в `packages/core/tests/vault_native.rs`
      (FR-026)
- [X] T081 [P] [US5] cargo test: индекс ссылок остаётся актуальным после create/rename/delete, в
      том числе вызванных извне (FR-031) в `packages/core/tests/vault_native.rs`
- [X] T082 [P] [US5] cargo test: rename/delete заметки без обратных ссылок проходит без запроса
      подтверждения (US5 AS2) в `packages/core/tests/vault_native.rs`
- [X] T083 [P] [US5] wasm-bindgen-test: паритет `backlinks()` на `WebBackend` в
      `packages/core/tests/vault_wasm.rs`

### Implementation for User Story 5

- [X] T084 [US5] Реализовать минимальное извлечение ссылок из содержимого заметки, достаточное
      для заполнения обратных ссылок, в `packages/core/src/graph/mod.rs` — узко под нужды
      FR-026…FR-031; полная семантика графа ссылок — предмет отдельной будущей фичи
      (research.md §8) (зависит от T012)
- [X] T085 [US5] Связать извлечение ссылок с пайплайном обновления индекса/кэша при
      create/update/rename/delete, включая внешние изменения, запись в общую таблицу `links` в
      `packages/core/src/vault/cache.rs` (зависит от T084, T058)
- [X] T086 [US5] Реализовать `VaultBackend::backlinks(path)`, запрашивающий таблицу `links`, в
      `packages/core/src/vault/native.rs` и `packages/core/src/vault/web.rs` (зависит от T085)
- [X] T087 [US5] Реализовать эмиссию `LinksInvalidated`, когда набор битых ссылок меняется
      (FR-030), в `packages/core/src/vault/events.rs` (зависит от T085)
- [X] T088 [P] [US5] Собрать UI-диалог подтверждения перед удалением/переименованием со списком
      затронутых заметок (FR-027, FR-028) в `ui/src/vault-tree/BacklinksConfirmDialog.tsx`
- [X] T089 [P] [US5] Собрать индикатор/поиск битых ссылок в дереве и просмотре заметки (FR-030)
      в `ui/src/vault-tree/BrokenLinkBadge.tsx`
- [X] T090 [US5] Связать действия delete/rename с вызовом `backlinks()` и показом
      `BacklinksConfirmDialog` перед выполнением (FR-027, FR-028) в
      `ui/src/vault-tree/VaultTreeActions.tsx` (расширяет T042)
- [ ] T091 [US5] Прогнать `specs/001-vault-manager/quickstart.md` Сценарий 5 на native- и
      web-сборке; подтвердить паритет

**Checkpoint**: все пять user stories работают независимо друг от друга.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: улучшения, затрагивающие несколько user stories, и финальные гейты готовности.

- [X] T092 [P] Написать бенчмарк на 10 000 заметок на эталонном оборудовании (SSD),
      подтверждающий SC-002/SC-003, в `packages/core/benches/vault_scale.rs`
- [X] T093 [P] Написать бенчмарк массового внешнего изменения 1000 файлов, подтверждающий
      SC-005, в `packages/core/benches/vault_bulk_change.rs`
- [X] T094 [P] Задокументировать best-effort поведение выше предела 10 000 заметок (SC-002) в
      doc-комментариях `packages/core/src/vault/mod.rs`
- [ ] T095 Прогнать весь `specs/001-vault-manager/quickstart.md` (все 7 сценариев) на native- И
      wasm32-unknown-unknown сборках подряд; зафиксировать любое расхождение паритета
      (принцип II)
- [X] T096 [P] Проверить все пользовательские сообщения об ошибках (`OpenError`/`CreateError`/
      `RenameError`/`DeleteError`/`WriteError`) на ясность и конкретность (FR-008, FR-013,
      FR-014) в `ui/src/`
- [X] T097 [P] Убедиться в отсутствии ручных дублирующих TS-типов для `VaultEvent`/
      `EditConflict`/`AccessState` — подтвердить, что все они порождены из Rust-определений
      (принцип IV) в `ui/src/`
- [X] T098 Финальный прогон: сборка под оба таргета + полный `cargo test` + `wasm-bindgen-test`
      по всем модулям `packages/core/src/vault/` и `packages/core/src/graph/` — Definition of
      Done конституции

---

## Дополнение: FR-037 (после второй сессии `/speckit-clarify`)

**Purpose**: задача добавлена после `tasks.md` был впервые сгенерирован — закрывает
FR-037 (предупреждение о несохранённых правках при закрытии окна с неразрешённым
конфликтом, US4 AS7). Логически относится к Phase 6 (US4); ID вынесен в конец файла,
чтобы не переприсваивать номера уже существующим задачам.

- [X] T099 [US4] Связать закрытие окна (`beforeunload` на web / `onCloseRequested` в
      Tauri) с проверкой незавершённого `EditConflict`/несохранённых правок и показом
      предупреждения перед закрытием (FR-037), переиспользуя существующее состояние
      конфликта (T072) в `ui/src/editor/useWindowCloseGuard.ts` (зависит от T072, T078)

---

## Состояние прогонов quickstart (T032, T044, T063, T079, T091, T095)

Шесть незакрытых задач — это ручные прогоны `quickstart.md`, и все шесть заблокированы
одной и той же причиной: **запускаемого приложения ещё не существует**. `ui/` — набор
компонентов без точки входа (нет ни `index.html`, ни бандлера, ни оболочки Tauri:
`ui/package.json` содержит единственный скрипт `typecheck`). Ни один сценарий
quickstart нельзя выполнить «как пользователь» до появления оболочки.

Дополнительно web-часть каждого сценария требует живого `FileSystemDirectoryHandle`,
который выдаётся только по жесту пользователя через `showDirectoryPicker()` и потому
недостижим в headless-прогоне (см. шапку `packages/core/tests/vault_wasm.rs`).

Что вместо них уже проверено автоматически и не требует повторного доказательства
при будущем ручном прогоне:

| Сценарий | Автоматическое покрытие |
|---|---|
| 1 (открытие, дерево, FR-004) | `open_recursively_indexes_nested_notes_and_excludes_hidden_dirs`, `open_on_empty_folder_succeeds_with_empty_tree`, `read_returns_note_content` |
| 2 (CRUD, FR-012, FR-013, FR-015) | `create_rejects_name_conflict_without_overwriting_existing_file`, `create_rename_delete_round_trip_persists_across_reopen`, `index_rename_preserves_note_identity_for_editor_session_continuity` |
| 3 и 6 (внешние изменения, два окна) | `watch_emits_created_updated_renamed_deleted_for_external_changes`, `debounce_collapses_atomic_write_via_tmp_and_rename_into_one_event`, `losing_vault_access_emits_unavailable_and_does_not_wipe_index`, `changes_from_second_backend_instance_on_same_root_are_observed_as_external_by_the_first` |
| 4 (конфликт, FR-021…FR-025) | `external_change_to_note_with_unsaved_edits_raises_conflict_instead_of_silent_update` и остальные пять тестов US4; на wasm — те же правила через `ConflictRegistry` |
| 5 (ссылочная целостность) | `backlinks_returns_notes_that_link_to_the_target`, `deleting_a_linked_note_reports_affected_notes_and_leaves_them_findable`, `broken_links_are_findable_after_the_operation_completes` |
| 7 (масштаб, SC-002/003/005) | `benches/vault_scale.rs`, `benches/vault_bulk_change.rs` — оба прогнаны, бюджеты выдержаны |

Незакрытым остаётся именно то, что автотесты не заменяют: сквозная проверка глазами и
паритет **наблюдаемого пользователем** поведения между native- и web-сборкой (принцип II).
Эти задачи следует выполнить сразу после появления оболочки приложения.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: без зависимостей — можно начинать сразу
- **Foundational (Phase 2)**: зависит от Setup; блокирует все user stories
- **User Stories (Phase 3–7)**: все зависят от завершения Foundational
  - После Foundational истории можно вести параллельно (при наличии ресурсов)
  - Либо последовательно по приоритету: US1 → US2 → US3 → US4 → US5
- **Polish (Phase 8)**: зависит от завершения нужных user stories

### User Story Dependencies

- **US1 (P1)**: только после Foundational — не зависит от других историй
- **US2 (P2)**: только после Foundational; переиспользует `open`/индекс из US1
  (T022, T024, T011), но независимо тестируема через собственный Independent Test
- **US3 (P3)**: только после Foundational; переиспользует `open`/индекс из US1
  (T022, T024, T026) для сверки при повторном открытии
- **US4 (P4)**: только после Foundational; требует пайплайн `watch()` из US3
  (T053, T054, T058) — конфликт обнаруживается через тот же поток событий
- **US5 (P5)**: только после Foundational; требует пайплайн обновления индекса из US3
  (T058) для поддержания актуальности ссылок при внешних изменениях

Несмотря на техническое переиспользование инфраструктуры US1/US3, каждая история
самостоятельно тестируема по своему Independent Test из spec.md — переиспользование не
создаёт функциональной зависимости в поведении, видимом пользователю.

### Within Each User Story

- Тесты пишутся первыми, должны падать до реализации
- Определения типов/сущностей — перед логикой backend
- Логика `NativeBackend`/`WebBackend` — перед связыванием с кэшем/событиями
- Ядро — перед UI поверх него
- Прогон соответствующего сценария `quickstart.md` — последний шаг фазы

### Parallel Opportunities

- Все задачи Setup с `[P]` — параллельно
- Все задачи Foundational с `[P]` — параллельно (T007–T010, T015–T016)
- После Foundational — US1…US5 можно вести параллельно разными людьми
- Тесты каждой истории с `[P]` — параллельно
- UI-задачи каждой истории с `[P]` — параллельно (разные файлы `ui/src/…`)

---

## Parallel Example: User Story 1

```bash
# Тесты US1 параллельно:
Task: "cargo test: NativeBackend::open() рекурсивно индексирует вложенные заметки в packages/core/tests/vault_native.rs"
Task: "wasm-bindgen-test: то же для WebBackend::open() в packages/core/tests/vault_wasm.rs"
Task: "cargo test: open() на пустой папке в packages/core/tests/vault_native.rs"
Task: "cargo test: open() на недоступной папке в packages/core/tests/vault_native.rs"

# UI-задачи US1 параллельно:
Task: "Собрать VaultTree.tsx в ui/src/vault-tree/VaultTree.tsx"
Task: "Собрать OpenVault.tsx в ui/src/vault-tree/OpenVault.tsx"
```

---

## Implementation Strategy

### MVP First (только User Story 1)

1. Завершить Phase 1: Setup
2. Завершить Phase 2: Foundational (критично — блокирует все истории)
3. Завершить Phase 3: User Story 1
4. **Остановиться и проверить**: прогнать quickstart Сценарий 1 независимо, на обоих таргетах
5. Демонстрировать/поставлять при готовности

### Incremental Delivery

1. Setup + Foundational → фундамент готов
2. + US1 → проверка независимо → MVP
3. + US2 → проверка независимо
4. + US3 → проверка независимо
5. + US4 → проверка независимо
6. + US5 → проверка независимо
7. Каждая история добавляет ценность, не ломая предыдущие

### Parallel Team Strategy

1. Команда вместе завершает Setup + Foundational
2. После Foundational: разработчик A — US1, B — US2, C — US3, и т.д. (учитывая, что
   US4 технически опирается на пайплайн `watch()` из US3, а US5 — на пайплайн индекса
   из US3, см. User Story Dependencies)
3. Истории интегрируются независимо

---

## Notes

- `[P]` — разные файлы, нет зависимости от незавершённых задач
- `[Story]` привязывает задачу к конкретной user story для трассируемости
- Каждая user story независимо завершаема и тестируема по своему Independent Test
- Перед реализацией убедиться, что тесты падают
- Коммит после каждой задачи или логической группы
- Останавливаться на любом чекпоинте для независимой проверки истории
- Дуальность таргетов (принцип II) проверяется в конце каждой истории (T032, T044, T063,
  T079, T091) и финально в T098 — фича не считается готовой при расхождении
- T099 (FR-037) — задача, добавленная после второй сессии `/speckit-clarify`; её ID
  идёт после Polish-фазы, но по смыслу и меткой `[US4]` она относится к Phase 6

---

## Phase 9: Convergence

**Purpose**: закрыть расхождения между обновлённой спекой (сессия `/speckit-clarify`
2026-08-09: FR-038, уточнённый FR-020, порог размера заметки, привязка SC-004) и
фактическим состоянием кода. Задачи дописаны `/speckit-converge`; существующие ID не
переиспользуются и не перенумеровываются.

- [X] T100 **CRITICAL** Убрать автовосстановление доступа в `WebBackend`: `poll_for_changes`
      не должен выводить `access_state` из `Unavailable` (`packages/core/src/vault/web.rs`,
      строки 512-522) — выход из недоступности только по явному переоткрытию, как уже
      сделано в `NativeBackend` (`packages/core/src/vault/native.rs:503,533`). Сейчас
      таргеты ведут себя по-разному, что нарушает NON-NEGOTIABLE принцип
      per Constitution II, FR-020 (contradicts)
- [X] T101 **CRITICAL** Вернуть `pnpm typecheck` в `ui/` в зелёное состояние: 5 ошибок в
      `ui/src/settings/SettingsPanel.tsx` — не импортированы `useState`, `ModePreference`,
      `applyWithNoteSave`, `describeApplyOutcome`, и в `KeybindingsSection` не переданы
      `configPath`/`fullBackendUnavailableReason`. Пока проверка типов падает, Definition
      of Done конституции не выполняется ни для одной фичи, включая эту
      per Constitution IV (contradicts)
- [X] T102 Реализовать отказ открывать vault свыше `SUPPORTED_NOTE_COUNT`: прервать обход,
      как только число найденных заметок превысило предел, не показывать частичное дерево
      как рабочий vault и вернуть ошибку с внятной причиной — в
      `packages/core/src/vault/native.rs` и `packages/core/src/vault/web.rs`
      per FR-038 (missing)
- [X] T103 Покрыть отказ по пределу тестами на обоих таргетах: `cargo test` в
      `packages/core/tests/vault_native.rs` и `wasm-bindgen-test` в
      `packages/core/tests/vault_wasm.rs` per FR-038, Constitution II (missing)
- [X] T104 Переписать doc-комментарий `SUPPORTED_NOTE_COUNT` в
      `packages/core/src/vault/mod.rs` (строки 29-40): убрать описание best-effort режима и
      фразу «Отдельного режима „слишком большой vault“ не вводится» — режим отменён,
      vault свыше предела не открывается per FR-038 (contradicts)
- [X] T105 Добавить проверку `access_state` в `create()`, `rename()` и `delete()` на обоих
      backend (`packages/core/src/vault/native.rs`, `packages/core/src/vault/web.rs`):
      сейчас её имеет только `write()`, тогда как FR-036 требует отклонять КАЖДУЮ попытку
      записи при `ReadOnly`, а FR-020 запрещает удаление и переименование при
      `Unavailable` per FR-036, FR-020 (missing)
- [X] T106 Покрыть гейты записи из T105 тестами на обоих таргетах — существующий тест
      `revoking_write_access_emits_read_only_and_write_is_denied_immediately` проверяет
      только `write()` per FR-036, Constitution II (missing)
- [X] T107 Создать оболочку приложения, названную в `plan.md` → Project Structure:
      `apps/web` (Next.js), `apps/desktop` и `apps/mobile` (Tauri) поверх общего `ui/`,
      которому сейчас не хватает точки входа. Без неё нельзя выполнить ни один сценарий
      `quickstart.md`, из-за чего заблокированы T032, T044, T063, T079, T091, T095
      per plan: Project Structure (missing)
      - **Сделано частично, по явному решению 2026-08-09: только `apps/web`.**
        Собран Next.js-клиент (`apps/web`) поверх `@typsidian/ui`: pnpm-workspace в
        корне, `ui` собирается своим `tsc` в `ui/dist`, `next build` проходит,
        сервер отдаёт страницу с точкой входа «Открыть vault». Этого достаточно,
        чтобы разблокировать web-половину сценариев `quickstart.md`.
      - **Остаётся**: `apps/desktop` и `apps/mobile` (Tauri). Требуют системных
        зависимостей, которых нет в `flake.nix` (webkit2gtk, libsoup, pkg-config),
        а mobile — ещё и Android SDK/Xcode. Native-половина сценариев
        `quickstart.md` до этого момента остаётся невыполнимой.
- [X] T108 Реализовать порог размера отдельной заметки 10 МБ: заметка выше порога остаётся
      видимой в дереве, но открытие отклоняется с сообщением о причине — в
      `packages/core/src/vault/native.rs` и `packages/core/src/vault/web.rs`, с тестами на
      обоих таргетах per spec Assumptions, Edge Cases (missing)
- [X] T109 Измерить фактическую задержку обнаружения внешнего изменения на `WebBackend`
      при vault предельного размера (10 000 заметок) и подтвердить бюджет 2 секунды: SC-004
      теперь распространяется на весь поддерживаемый диапазон и на все клиенты, а
      существующие бенчмарки (`benches/vault_scale.rs`, `benches/vault_bulk_change.rs`) —
      только нативные per SC-004 (partial)
      - Измерено тестом `external_change_is_detected_within_sc004_budget_at_full_scale`
        (`tests/vault_wasm.rs`, headless Chrome): **113 мс** на 10 000 заметок в
        debug-сборке при бюджете 2000 мс. Тест падает, если бюджет не выдержан.
      - **Оговорка**: двойник каталога держит содержимое в памяти JS, поэтому число
        отражает стоимость самого алгоритма сверки, а не задержку реального диска и
        не работу File System Access API. Это нижняя граница; подтверждение на живом
        хендле остаётся за ручным прогоном (T063).
- [X] T110 Согласовать дизайн-артефакты с FR-038: `data-model.md` (строка 18, `note_count`
      описан как признак best-effort режима) и `plan.md` → Scale/Scope (строки 74-76,
      «выше — best-effort») описывают отменённое поведение per FR-038 (contradicts)

---

## Что остаётся незакрытым (состояние на 2026-08-09)

Фаза 9 закрыта полностью; ниже — задачи, которые остались открытыми **не** из-за
пропуска, а потому что требуют того, чего в автоматическом прогоне нет.

**T032, T044, T063, T079, T091, T095 — ручные прогоны `quickstart.md`.**
Каждый сценарий начинается с выбора папки через системный диалог
(`showDirectoryPicker` на web, файловый диалог ОС на native) — это жест
пользователя, который нельзя выполнить из тестового рантайма. Дополнительно
сценарии требуют размонтировать диск, отозвать права и править файлы снаружи в
реальном времени.

Состояние блокировки изменилось: web-половина этих сценариев **разблокирована**
— `apps/web` собирается и запускается (`pnpm --filter @typsidian/web dev`), точка
входа «Открыть vault» отдаётся браузеру. Native-половина остаётся заблокированной
до `apps/desktop` (см. T107).

Поведение, которое эти сценарии проверяют вручную, покрыто автотестами на обоих
таргетах: 148 тестов `cargo test` и 40 `wasm-bindgen-test` — включая обнаружение
внешних изменений, конфликты, потерю доступа и пределы масштаба. Ручной прогон
подтверждает интеграцию с реальной ФС и реальным диалогом, а не сами правила.

---

## Phase 10: Native-оболочка (разблокировка ручных прогонов)

**Purpose**: закрыть остаток T107 — `apps/desktop` и `apps/mobile`. Пока их нет,
native-половина T032, T044, T063, T079, T091, T095 невыполнима, а значит фича не может
быть отчитана как завершённая. Задачи дописаны `/speckit-tasks`; существующие ID не
переиспользуются и не перенумеровываются.

- [X] T111 Добавить в `flake.nix` системные зависимости сборки Tauri для NixOS —
      `pkg-config`, `webkitgtk_4_1`, `libsoup_3`, `gtk3`, `librsvg`, `openssl` — и
      экспортировать `PKG_CONFIG_PATH`/`LD_LIBRARY_PATH` в `shellHook`. Сейчас `buildInputs`
      содержит только Rust-тулчейн, node/pnpm, chromium, sqlite и neovim, поэтому
      `cargo tauri build` не найдёт webkit и упадёт на этапе линковки
      per plan: Project Structure (missing)
- [X] T112 Создать `apps/desktop` — Tauri-оболочку поверх общего `ui/`: `apps/desktop/package.json`
      с зависимостью `@typsidian/ui` (workspace:*), `apps/desktop/src-tauri/tauri.conf.json`,
      `apps/desktop/src-tauri/Cargo.toml` с зависимостью на `typsidian-core` по пути.
      Точка входа повторяет `apps/web/app/VaultWorkspace.tsx`, но получает backend не из
      WASM per plan: Project Structure (missing)
- [X] T113 Пробросить `NativeBackend` (`packages/core/src/vault/native.rs`) в оболочку
      командами Tauri по контракту `contracts/vault-backend-trait.md` — `open`, `list`,
      `read`, `write`, `create`, `rename`, `delete`, `backlinks`, `begin_edit`. Логику не
      дублировать: команда обязана быть тонкой обёрткой над ядром (принцип I)
      per contracts/vault-backend-trait.md (missing)
- [X] T114 Пробросить поток `VaultEvent` из `NativeBackend` в UI по
      `contracts/frontend-events.md`: native использует `notify`/`notify-debouncer-full`,
      то есть push, а не поллинг, — события идут в окно через Tauri event channel, а
      `ui/src/vault-tree/useVaultEvents.ts` должен принимать оба источника без ветвления
      по таргету в бизнес-логике per contracts/frontend-events.md, Constitution II (missing)
- [X] T115 Реализовать выбор папки vault нативным диалогом ОС (`tauri-plugin-dialog`) вместо
      `showDirectoryPicker`, сохранив тот же жест пользователя и ту же классификацию отказов,
      что в `ui/src/vault-tree/useVault.ts` → `classifyOpenFailure` (отмена диалога ошибкой
      не считается) per FR-001, FR-008 (missing)
- [X] T116 [P] Создать `apps/mobile` (Tauri mobile) поверх того же `ui/`. Требует Android SDK
      и/или Xcode, которых нет в `flake.nix`; если решено отложить — зафиксировать это
      решение в `plan.md` → Project Structure, чтобы структура плана перестала обещать
      несуществующий таргет per plan: Project Structure (missing)
- [ ] T117 Прогнать native-половину `quickstart.md` на собранной `apps/desktop`: Сценарии 1-7,
      включая те шаги, которые в вебе невыполнимы, — размонтирование диска, отзыв прав на
      папку, атомарная запись сторонним редактором. Зафиксировать результат в разделе
      «Состояние прогонов quickstart» этого файла и закрыть T032, T044, T063, T079, T091,
      T095 per Constitution II (blocked-by: T112)

---

## Phase 11: Что потребовалось для оболочки (2026-08-10)

**Purpose**: зафиксировать работу, без которой T112-T115 невыполнимы, но которой в списке не
было. Существующие ID не переиспользуются.

Главное, чего не предвидел план: `ui/` был типизирован на **класс** `WebBackend` — вплоть до
синхронных статических вызовов в render-путях. Оболочка рабочего стола не может его
предоставить: ядро там за границей процесса, и синхронного ответа не существует. Поэтому
T112 начинается не с `apps/desktop`, а с границы, которую оболочки реализуют.

- [X] T118 Ввести `ui/src/vault-client.ts` — поверхность ядра для общего UI, и
      `ui/src/vault-client-web.ts` — её реализацию на WASM. Общий UI переведён с `WebBackend`
      на этот интерфейс (`useVault`, `useVaultEvents`, `VaultTreeActions`, `BrokenLinkBadge`,
      `useConflictResolution`, `useWindowCloseGuard`, все хуки досок). **Типы остались
      сгенерированными из Rust** и импортируются как типы, поэтому desktop-сборка WASM не
      тянет — проверено отсутствием `.wasm` в `apps/desktop/dist` per Constitution I, IV
- [X] T119 Заменить опрос событий на подписку: `VaultClient.subscribe` вместо
      `pollForChanges`+`pollEvents` в `useVaultEvents`. Именно это требование T114
      («без ветвления по таргету в бизнес-логике») в прежней форме было невыполнимо: хук
      реализовывал web-овый поллинг сам per T114, Constitution II
- [X] T120 Ввести `Note.kind` (`packages/core/src/vault/index.rs` → `Note::new`).
      `isBoard(path)` в дереве был синхронным вызовом `WebBackend.fileKind` на каждую
      отрисовку — на desktop такого вызова нет. Вид теперь приходит вместе с записью, а
      спрашивать его отдельно нужно только для пути, которого в дереве ещё нет (создание
      файла) per FR-004 фичи 002, принцип I
- [X] T121 Ввести `NativeBackend::events_ready()` — сигнал «в очереди есть события»
      (`tokio::sync::Notify`, `notify_one`, чтобы не терять сигнал без ждущего). Без него
      оболочка пересылала бы события в окно по таймеру, то есть добавила бы поллинг там, где
      обнаружение push-овое. Покрыто тестом
      `external_change_signals_the_shell_without_polling` per T114
- [X] T122 Перенести рабочую область и редактор в общий `ui/`:
      `ui/src/workspace/Workspace.tsx` и `ui/src/editor/NoteEditor.tsx` (были в
      `apps/web/app/`). Обе оболочки рендерят одну и ту же композицию и различаются одним
      пропом — жестом открытия папки. Копия композиции в `apps/desktop` означала бы, что
      «одинаковое поведение на всех таргетах» проверяется чтением двух файлов per T112,
      Constitution II
- [X] T123 Сделать панель настроек внешним пропом `Workspace`. `ui/src/settings/useSettings.ts`
      работает через `WebStore` (OPFS + IndexedDB) — на рабочем столе он писал бы настройки в
      песочницу окна вместо каталога конфигурации ОС. На web панель передаётся как раньше, на
      desktop кнопки настроек нет: отсутствие честнее, чем настройки, уходящие не туда.
      Хранилище настроек для desktop — задача фичи 004 (её T096 этого и требует)
- [X] T124 Разрешить сборочный скрипт `esbuild` в `pnpm-workspace.yaml` (`allowBuilds`):
      pnpm ≥10 их блокирует, а без postinstall у esbuild нет платформенного бинаря, то есть
      Vite не запускается вовсе
- [X] T125 Добавить `pkgs.dbus` в `flake.nix`. Сборка проходила и без него, а собранный
      бинарь не находил `libdbus-1.so.3` во время работы — то есть отказ проявился бы только
      при запуске у пользователя. Проверка: `ldd target/debug/typsidian-desktop` внутри
      dev-shell не имеет ни одной ненайденной библиотеки

**Что проверено и чем.**

- `nix develop` собирается, `cargo-tauri` 2.11.4, `pkg-config` находит `webkit2gtk-4.1`
  (2.52.5), `javascriptcoregtk-4.1`, `libsoup-3.0` (3.6.6), `gtk+-3.0` (3.24.52), `dbus-1`.
- `cargo build -p typsidian-desktop` — бинарь собран и слинкован (275 МБ debug); `ldd`
  внутри dev-shell: 0 ненайденных библиотек.
- `pnpm --dir apps/desktop build:vite` — окно собирается; `.wasm` в `dist/` отсутствует.
- `cargo test -p typsidian-core` — 223 теста, `wasm-pack test --headless --chrome` — 60.
- `pnpm -r typecheck` — три проекта чисто; `pnpm build` (Next) — web-сборка цела.

**Чего проверить нельзя из терминала.** T117 — прогон сценариев в собранном окне: нужен
дисплей, нативный диалог выбора папки, размонтирование диска и отзыв прав на папку. Иконка
`apps/desktop/src-tauri/icons/icon.png` — заглушка 512×512, сгенерированная скриптом
(косая черта из вордмарка): для сборки её достаточно, для релизного бандла нужен настоящий
набор размеров.
