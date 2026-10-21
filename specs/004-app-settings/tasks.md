---

description: "Task list for Application Settings implementation"
---

# Tasks: Application Settings

**Input**: Design documents from `/specs/004-app-settings/` (ревизия 2 плана, 2026-08-09)

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md
(все присутствуют)

**Tests**: Включены. Конституция (принципы I и II, NON-NEGOTIABLE) требует, чтобы каждая
единица логики `packages/core` была покрыта и `cargo test`, и `wasm-bindgen-test` — это не
опциональный запрос, а Definition of Done проекта.

**Organization**: Задачи сгруппированы по user story из spec.md (US1…US4, приоритеты P1…P4)
для независимой реализации и проверки каждой.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: можно выполнять параллельно (разные файлы, нет зависимости от незавершённых задач)
- **[Story]**: к какой user story относится задача (US1…US4)
- Пути к файлам — по структуре из `plan.md` (`packages/core/src/settings/…`, `ui/src/settings/…`)

## Path Conventions

Структура — Rust-workspace ядро + общий UI-код, см. `plan.md` → Project Structure:
`packages/core/src/settings/`, `packages/core/src/stamp.rs`, `packages/core/tests/`,
`ui/src/settings/`.

## Три обстоятельства, влияющие на исполнение

1. **Моста к внешнему редактору в кодовой базе нет** (research.md §7). Все задачи US3 выполняются против
   трейта `EditorModeBridge` с `NullBridge`/`FakeBridge`. Реальный мост — отдельная будущая
   фича со своим `/speckit-clarify` (принцип V); в этот список он не входит.
2. **Идентичность записи о vault непрозрачна** (research.md §10). Ни одна задача не сравнивает
   папки по пути, имени или показываемому расположению: тождество устанавливает платформа за
   ассоциированным типом `FolderRef`. Задача T046 — единственная проверка, ловящая регрессию
   этого механизма в вебе; без неё дефект вернётся молча.
3. **Компонента редактора в `ui/` ещё нет** — каталог `ui/src/editor/` пуст. Тема и размер
   шрифта применяются на уровне корня UI через CSS-переменные, чтобы редактор потреблял их,
   когда появится. Полная проверка FR-017 отложена; см. T044 и T107.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: зависимости, скелет модулей, точки входа тестов.

- [X] T001 Добавить зависимости в `packages/core/Cargo.toml`: `toml` и `toml_edit` без cfg
      (оба таргета), `directories` под `cfg(not(target_arch = "wasm32"))`; расширить features
      существующей зависимости `web-sys` (`StorageManager`, `FileSystemWritableFileStream`,
      `Navigator`, `IdbFactory`, `IdbDatabase`, `IdbObjectStore`, `IdbRequest`) под
      `cfg(target_arch = "wasm32")` — plan.md → Primary Dependencies, research.md §1, §2, §10
- [X] T002 [P] Создать скелет модуля настроек:
      `packages/core/src/settings/{mod,store,native,web,format,apply,vaults,folder_ref,editor_mode,diagnostics,logging,events}.rs`
      и зарегистрировать `pub mod settings;` в `packages/core/src/lib.rs` — по структуре из
      `plan.md`
- [X] T003 [P] Создать точки входа тестов `packages/core/tests/settings_native.rs`
      (`cargo test`) и `packages/core/tests/settings_wasm.rs` (`wasm-bindgen-test`) — оба
      таргета обязательны (принцип II)
- [X] T004 [P] Объявить фичу `test-support` в `packages/core/Cargo.toml` и включить её в
      dev-dependencies крейта — без неё `FakeBridge` (T022) будет невиден интеграционным
      тестам в `packages/core/tests/` (research.md §15)
- [X] T005 [P] Создать скелет UI-панели:
      `ui/src/settings/{useSettings.ts,SettingsPanel.tsx,VaultSection.tsx,AppearanceSection.tsx,KeybindingsSection.tsx,AdvancedSection.tsx}`
      — по структуре из `plan.md`
- [X] T006 [P] Проверить и зафиксировать в `flake.nix`, что новые крейты (`serde_json`,
      `directories`) не требуют системных библиотек и dev-окружение остаётся воспроизводимым;
      при необходимости обновить

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: типы, трейты и базовое хранилище, без которых не может начаться ни одна user story.

**⚠️ CRITICAL**: ни одна user story не начинается до завершения этой фазы.

- [X] T007 Поднять `FileStamp` из `packages/core/src/vault/index.rs` в новый
      `packages/core/src/stamp.rs`, зарегистрировать `pub mod stamp;` в
      `packages/core/src/lib.rs` и оставить ре-экспорт из `vault::index`, чтобы существующий
      код и тесты vault не сломались — research.md §8
- [X] T008 [P] Определить `Settings`, `Theme`, `ModePreference` и значения по умолчанию
      (тема `Light`, шрифт `14`, `current_vault: Option<VaultId>`) в
      `packages/core/src/settings/mod.rs` — по `data-model.md`
- [X] T009 [P] Определить `VaultId`, `VaultRecord { id, location, label, availability }` и
      `Availability` в `packages/core/src/settings/vaults.rs` — по `data-model.md`; запись
      **не хранит** ссылку на папку, только непрозрачный идентификатор и показываемое
      расположение
- [X] T010 [P] Определить трёхвариантный `EditorMode` (`FullBackend`/`SimplifiedEmulation`/
      `PlainInput`), `EditorConfig`, `ConfigDiskState`, `OutageReason` в
      `packages/core/src/settings/editor_mode.rs` — по `data-model.md`
- [X] T011 [P] Определить `SettingsEvent`, `StoreCapabilities`, `StorageState` с
      `serde` + `tsify-next` в `packages/core/src/settings/events.rs` и
      `packages/core/src/settings/store.rs` — принцип IV, типы границы порождаются из Rust
- [X] T012 [P] Определить `SettingsChange`, `ApplyPlan`, `RejectReason` (сигнатуры, без
      реализации предусловий) в `packages/core/src/settings/apply.rs` — по `data-model.md`
- [X] T013 Определить сигнатуры трейта `SettingsStore` (`open`/`load`/`apply`/`capabilities`/
      `reveal`/`poll_events`) в `packages/core/src/settings/store.rs` — по
      `contracts/settings-store-trait.md` (зависит от T011)
- [X] T014 Определить ассоциированный тип `FolderRef` и сигнатуры реестра папок
      (`find_existing`/`add`/`resolve`/`set_label`/`forget`), включая трёхвариантный результат
      `resolve` (`Resolved`/`NeedsPermission`/`Missing`), в
      `packages/core/src/settings/folder_ref.rs` — по `contracts/settings-store-trait.md`
      → «Реестр папок vault», research.md §10 (зависит от T009)
- [X] T015 Определить трейт `EditorModeBridge` и реализацию `NullBridge` (полноценный режим
      всегда недоступен с причиной) в `packages/core/src/settings/editor_mode.rs` — по
      `contracts/editor-mode-bridge.md` (зависит от T010)
- [X] T016 Реализовать общую валидацию в `packages/core/src/settings/mod.rs`: диапазон
      размера шрифта `8..=32`, инварианты списка vault (одна и та же папка не заводится
      дважды, подпись в тождестве не участвует и уникальности не требует) — общий код для
      обеих реализаций, по образцу `is_valid_note_name` в `vault::backend`; платформенным
      остаётся **только** сравнение самих папок (research.md §10, §11, §14)
- [X] T017 Реализовать чтение и запись JSON в `packages/core/src/settings/format.rs`:
      `serde_json` на чтение, разбор через `serde_json::Value` на запись, чтобы уцелели
      ключи, которых приложение не знает — research.md §2
      - **Переформулировано 2026-09-09 вслед за сменой формата.** Прежняя формулировка
        требовала форматосохраняющей записи TOML через `toml_edit`. Комментариев в JSON нет,
        и эта часть требования оплачена, а не выполнена другими средствами. Переноса с
        прежнего формата нет — тоже по решению владельца проекта (research.md §2).
- [X] T018 [P] Тесты `format.rs` в `packages/core/tests/settings_native.rs` и
      `packages/core/tests/settings_wasm.rs`: дописанные пользователем ключи переживают
      запись приложения, порядок ключей от записи к записи стабилен (ключевая проверка
      research.md §2)
- [X] T019 Реализовать `NativeStore` (`open`/`load`/`apply`) в
      `packages/core/src/settings/native.rs`: файл `settings.json` в каталоге конфигурации ОС
      через `directories`, атомарная запись «временный файл + fsync + rename» — research.md
      §1, §5 (зависит от T013, T017)
- [X] T020 Реализовать `WebStore` (`open`/`load`/`apply`) в
      `packages/core/src/settings/web.rs`: `settings.json` в OPFS, атомарная запись через
      `FileSystemWritableFileStream` с фиксацией на `close()` — research.md §1, §5
      (зависит от T013, T017)
- [X] T021 [P] Тесты паритета `open`/`load`/`apply` в `packages/core/tests/settings_native.rs`
      и `packages/core/tests/settings_wasm.rs`: одинаковые значения, одинаковые отказы,
      атомарность записи на обоих таргетах (принцип II)
- [X] T022 [P] Реализовать `FakeBridge` (управляемое воспроизведение трёх состояний и
      отказных переходов) в `packages/core/src/settings/editor_mode.rs` под
      `#[cfg(any(test, feature = "test-support"))]` — `contracts/editor-mode-bridge.md`,
      research.md §15; голый `#[cfg(test)]` не подходит, интеграционные тесты его не увидят
      (зависит от T004)
- [X] T023 Реализовать каркас хука `useSettings.ts` в `ui/src/settings/useSettings.ts` поверх
      ядра, по образцу `ui/src/vault-tree/useVault.ts` — только вызовы ядра и рендер,
      бизнес-логики нет (принцип I)

**Checkpoint**: хранилище читается и пишется на обоих таргетах — user stories могут начинаться

---

## Phase 3: User Story 1 - Изменить настройку и сразу увидеть результат (Priority: P1) 🎯 MVP

**Goal**: панель с четырьмя разделами, в которой изменение вступает в силу немедленно без
кнопки «сохранить»; файл настроек ведёт себя как пользовательский артефакт — правка руками
равноправна правке через панель.

**Independent Test**: открыть настройки, переключить тему и изменить размер шрифта — результат
виден сразу и переживает перезапуск; отредактировать `settings.json` сторонним редактором —
изменение подхватывается без перезапуска, а дописанный руками ключ переживает следующую
запись из панели.

### Tests for User Story 1

- [X] T024 [P] [US1] Тест: изменение применяется без явного сохранения и переживает
      перезапуск (FR-002, FR-003, FR-007) в `packages/core/tests/settings_native.rs` и
      `packages/core/tests/settings_wasm.rs`
- [X] T025 [P] [US1] Тест: размер шрифта `7` и `33` отклоняются с причиной, `8` и `32`
      принимаются (FR-016, research.md §11) в `packages/core/tests/settings_native.rs` и
      `packages/core/tests/settings_wasm.rs`
- [X] T026 [P] [US1] Тест: внешнее изменение файла настроек отражается ≤2 с (FR-013, SC-014)
      в `packages/core/tests/settings_native.rs` и `packages/core/tests/settings_wasm.rs`
- [X] T027 [P] [US1] Тест: повреждённый файл → `DefaultsAfterUnreadable` с уведомлением,
      файл **не перезаписан**; при первой записи переименован рядом (FR-013, research.md §6)
      в `packages/core/tests/settings_native.rs` и `packages/core/tests/settings_wasm.rs`
- [X] T028 [P] [US1] Тест: каталог недоступен на запись → `ReadOnlyForSession`, изменения
      действуют до конца сессии, пользователь предупреждён (FR-014) в
      `packages/core/tests/settings_native.rs`
- [X] T029 [P] [US1] Тест: ранее нечитаемый файл, починенный руками, возвращает
      `StorageRecovered` без перезапуска (FR-013) в `packages/core/tests/settings_native.rs`
- [X] T030 [P] [US1] Тест: удаление хранилища настроек **не влияет на содержимое vault** —
      заметки читаются и правятся как прежде, приложение поднимается на значениях по
      умолчанию (FR-011) в `packages/core/tests/settings_native.rs`

### Implementation for User Story 1

- [X] T031 [US1] Реализовать базовый путь `apply()` без предусловий (возврат
      `ApplyPlan::Applied`) и отказ `RejectReason::ValueOutOfRange` в
      `packages/core/src/settings/apply.rs` (зависит от T012, T016)
- [X] T032 [US1] Реализовать `StorageState`: переход в `DefaultsAfterUnreadable`, перенос
      испорченного файла в `settings.json.invalid-<UTC-метка>` перед первой записью и событие
      `InvalidFileSetAside` в `packages/core/src/settings/store.rs` — research.md §6
- [X] T033 [US1] Реализовать `ReadOnlyForSession` и событие `StorageBecameReadOnly` при
      недоступности каталога на запись в `packages/core/src/settings/store.rs` (FR-014)
- [X] T034 [US1] Реализовать `poll_events()` для `NativeStore` через `notify` +
      `notify-debouncer-full` (наблюдение за каталогом настроек) в
      `packages/core/src/settings/native.rs` — research.md §3; крейты уже в зависимостях
- [X] T035 [US1] Реализовать `poll_events()` для `WebStore` через периодическую сверку файла
      в OPFS с интервалом ≤1 с (сначала размер и время, содержимое — только при расхождении)
      в `packages/core/src/settings/web.rs` — research.md §3
- [X] T036 [US1] Реализовать `capabilities()` и `reveal()`: на native — путь к файлу и
      раскрытие в файловом менеджере, `can_reveal = true`; на web — обозначение хранилища
      OPFS и `can_reveal = false` с отказом вместо отсутствия метода
      (`packages/core/src/settings/{native,web}.rs`) — research.md §13
- [X] T037 [US1] Реализовать хранение и применение `Theme` и `editor_font_size` как полей
      `Settings` с немедленной публикацией изменения в `packages/core/src/settings/mod.rs`
      (FR-015, FR-016)
- [X] T038 [US1] Реализовать исчерпывающий разбор `SettingsEvent` в
      `ui/src/settings/useSettings.ts` (`never` в `default`, TS `strict`, без `any`) — по
      `contracts/frontend-settings.md`, принцип IV
- [X] T039 [US1] Реализовать каркас `ui/src/settings/SettingsPanel.tsx` с четырьмя разделами
      (Vault, Внешний вид, Кейбиндинги, Дополнительно) и **без** кнопки «сохранить»/«применить»
      (FR-001, FR-003)
- [X] T040 [P] [US1] Реализовать `ui/src/settings/AppearanceSection.tsx`: переключение темы и
      выбор размера шрифта с показом причины отказа при выходе за диапазон (FR-015, FR-016)
- [X] T041 [US1] Реализовать показ расположения хранилища и действие раскрытия только при
      `capabilities().can_reveal` в `ui/src/settings/AdvancedSection.tsx` (FR-036) — сама
      диагностика добавляется в US4
- [X] T042 [US1] Реализовать отображение состояний хранилища (`DefaultsAfterUnreadable`,
      `ReadOnlyForSession`, `InvalidFileSetAside`) в `ui/src/settings/SettingsPanel.tsx`
      (FR-005, FR-013, FR-014)
- [X] T043 [US1] Проверить согласованность между окнами: изменение в одном окне применяется в
      другом через тот же механизм наблюдения, отдельного канала нет (FR-006, research.md §4)
      — сценарий 3 из `quickstart.md`
- [X] T044 [US1] Реализовать применение темы и размера шрифта на корне UI через CSS-переменные
      в `ui/src/settings/SettingsPanel.tsx`, чтобы редактор потреблял их при появлении
      (FR-017, SC-002; см. обстоятельство 3 в шапке файла)

**Checkpoint**: US1 полностью работоспособна и проверяема независимо; это MVP

---

## Phase 4: User Story 2 - Видеть текущий vault и переключаться между несколькими (Priority: P2)

**Goal**: раздел Vault показывает расположение текущего vault и список известных, позволяет
добавить папку, подписать запись, переключиться и убрать запись из списка. Тождество папок
непрозрачно и устанавливается платформой.

**Independent Test**: добавить две **разные** папки с одинаковым именем — появляются две
самостоятельные записи; подписать одну из них; переключиться на вторую и увидеть её заметки,
при этом тема, шрифт и режим редактирования не меняются.

### Tests for User Story 2

- [X] T045 [P] [US2] Тест: повторное добавление **той же самой** папки переиспользует запись
      и не создаёт вторую (FR-020, research.md §10) в `packages/core/tests/settings_native.rs`
      и `packages/core/tests/settings_wasm.rs`
- [X] T046 [P] [US2] Тест регрессии идентичности: две **разные** папки с одинаковым именем
      дают две записи с разными `VaultId`, обе разрешаются в свои папки (FR-020, US2 AS9) в
      `packages/core/tests/settings_native.rs` и `packages/core/tests/settings_wasm.rs` —
      единственная проверка, ловящая схлопывание записей в вебе
- [X] T047 [P] [US2] Тест: подпись показывается вместо расположения, не обязана быть
      уникальной и **не влияет** на тождество папок (FR-018, US2 AS10, research.md §14) в
      `packages/core/tests/settings_native.rs` и `packages/core/tests/settings_wasm.rs`
- [X] T048 [P] [US2] Тест: `forget(id)` убирает запись и ссылку из реестра, не трогая папку и
      заметки на диске (FR-024) в `packages/core/tests/settings_native.rs`
- [X] T049 [P] [US2] Тест: переключение vault не меняет тему, размер шрифта и предпочтение
      режима редактирования (FR-010, SC-007) в `packages/core/tests/settings_native.rs` и
      `packages/core/tests/settings_wasm.rs`
- [X] T050 [P] [US2] Тест: внутри папок vault не появляется ни одного служебного файла
      приложения ни при добавлении, ни при переключении, ни при удалении записи (FR-020,
      принцип VI) в `packages/core/tests/settings_native.rs`
- [X] T051 [P] [US2] Тест: перенесённая снаружи папка не разыскивается приложением; при утрате
      связи запись становится `Unavailable` и добавляется заново (FR-023, research.md §10) в
      `packages/core/tests/settings_native.rs` — сохранение связи как гарантия **не**
      проверяется
- [X] T052 [P] [US2] Тест миграции: существующий `last_vault.txt` импортируется как
      единственная известная запись и текущий vault, исходный файл удаляется (research.md §10)
      в `packages/core/tests/settings_native.rs`

### Implementation for User Story 2

- [X] T053 [US2] Реализовать ведение списка известных vault (добавление, порядок, инварианты)
      в `packages/core/src/settings/vaults.rs` (FR-019; зависит от T009, T016)
- [X] T054 [US2] Реализовать `find_existing` и `add` для native в
      `packages/core/src/settings/native.rs`: `FolderRef = PathBuf`, сравнение
      канонизированных путей через `std::fs::canonicalize`; сравнение имён и показываемых
      расположений запрещено (FR-020, research.md §10)
- [X] T055 [US2] Реализовать `find_existing` и `add` для web в
      `packages/core/src/settings/web.rs`: `FolderRef = FileSystemDirectoryHandle`, сравнение
      через асинхронный `isSameEntry()` перебором по списку (FR-020, research.md §10)
- [X] T056 [US2] Реализовать реестр ссылок на папки в IndexedDB под ключом `VaultId` в
      `packages/core/src/settings/web.rs` — хендлы невозможно сериализовать в JSON; значения
      настроек в IndexedDB не попадают (research.md §1, §10)
- [X] T057 [US2] Реализовать `resolve(id)` с тремя исходами (`Resolved`/`NeedsPermission`/
      `Missing`) в `packages/core/src/settings/folder_ref.rs` и обеих реализациях —
      `NeedsPermission` не является ошибкой (contracts/settings-store-trait.md)
- [X] T058 [US2] Реализовать `set_label(id, label)` и вычисление показываемого значения
      (подпись, иначе расположение) в `packages/core/src/settings/vaults.rs` (FR-018)
- [X] T059 [US2] Реализовать `forget(id)` с удалением записи и ссылки из реестра таргета, без
      обращения к содержимому папки, в `packages/core/src/settings/vaults.rs` (FR-024)
- [X] T060 [US2] Реализовать вычисление `Availability` при показе списка и запрет розыска
      перенесённой папки в `packages/core/src/settings/vaults.rs` (FR-023)
- [X] T061 [US2] Реализовать смену `current_vault` по `VaultId` в
      `packages/core/src/settings/vaults.rs` **без** вызова `VaultBackend` из настроек: ядро
      меняет значение, открытие vault остаётся за вызывающей стороной (research.md §9, FR-021)
- [X] T062 [US2] Реализовать миграцию с `vault::last_vault`: импорт содержимого
      `last_vault.txt` в список, удаление файла, изъятие модуля из
      `packages/core/src/vault/mod.rs` **и снятие его вызова
      `crate::vault::last_vault::remember(root_path)` в
      `packages/core/src/vault/native.rs:577`** — без этого сборка не пройдёт (research.md §10)
- [X] T063 [US2] Реализовать `ui/src/settings/VaultSection.tsx`: расположение текущего vault,
      список известных с пометкой недоступных, действия добавления, подписи, переключения и
      удаления записи (FR-018, FR-019, FR-023, FR-024). Переключение MUST проходить через
      существующие предупреждения фичи 001 о несохранённых правках и неразрешённых конфликтах
      — машинерия не переизобретается, данные не теряются без явного подтверждения (FR-022,
      SC-008)
- [X] T064 [US2] Реализовать выбор папки как жест пользователя и разрешение `NeedsPermission`
      переспросом по жесту в `ui/src/settings/VaultSection.tsx` (по образцу
      `showDirectoryPicker` в `ui/src/vault-tree/useVault.ts`); UI передаёт ссылку в
      `find_existing()` **не разбирая** её и не сравнивая папки сам
      (contracts/frontend-settings.md, принцип I)

**Checkpoint**: US1 и US2 работают независимо друг от друга

---

## Phase 5: User Story 3 - Видеть и менять режим редактирования (Priority: P3)

**Goal**: раздел Кейбиндинги однозначно показывает одно из трёх состояний, показывает путь к
применённому конфигу и признак его устаревания, позволяет указать конфиг и переключить режим с
соблюдением защитных правил.

**Independent Test**: на `FakeBridge` довести полноценный режим до отказа при открытой заметке
с правками — текст сохранён, состояние стало `PlainInput` (а не эмуляцией), выход только по
явному действию.

> Все задачи фазы выполняются против `EditorModeBridge`; реальный мост не требуется и в эту
> фичу не входит (research.md §7, принцип V).

### Tests for User Story 3

- [X] T065 [P] [US3] Тест: отказ внешнего процесса переводит состояние в `PlainInput` и
      **никогда** в `SimplifiedEmulation`; текст и позиция курсора сохранены (FR-035) в
      `packages/core/tests/settings_native.rs` и `packages/core/tests/settings_wasm.rs`
- [X] T066 [P] [US3] Тест: выход из `PlainInput` происходит только по явному действию,
      самопроизвольного восстановления нет (FR-035) в
      `packages/core/tests/settings_native.rs`
- [X] T067 [P] [US3] Тест: переключение между полноценным режимом и упрощённой эмуляцией по
      выбору пользователя меняет наблюдаемое состояние (FR-027) в
      `packages/core/tests/settings_native.rs` и `packages/core/tests/settings_wasm.rs`
- [X] T068 [P] [US3] Тест: при недоступном полноценном режиме `current_mode()` не возвращает
      `FullBackend` ни при каких обстоятельствах (FR-034, FR-004) в
      `packages/core/tests/settings_native.rs` и `packages/core/tests/settings_wasm.rs`
- [X] T069 [P] [US3] Тест: указание пути к несуществующему или нечитаемому файлу конфига даёт
      причину отказа, и конфиг **не** помечается применённым (FR-033) в
      `packages/core/tests/settings_native.rs`
- [X] T070 [P] [US3] Тест: изменение конфига с последующим возвратом прежнего содержимого до
      применения даёт `InSync`, а не ложное `Changed` (FR-026, research.md §8) в
      `packages/core/tests/settings_native.rs`
- [X] T071 [P] [US3] Тест: удалённый или нечитаемый конфиг показывается, но полноценный режим
      продолжает работать на ранее применённом (FR-026) в
      `packages/core/tests/settings_native.rs`
- [X] T072 [P] [US3] Тест: смена режима при открытой заметке с правками возвращает
      `RequiresNoteSave` (FR-029) в `packages/core/tests/settings_native.rs` и
      `packages/core/tests/settings_wasm.rs`
- [X] T073 [P] [US3] Тест: неудавшееся сохранение даёт `Rejected { SaveFailed }`, режим не
      меняется, правки не потеряны (FR-030) в `packages/core/tests/settings_native.rs`
- [X] T074 [P] [US3] Тест: неразрешённый конфликт даёт `Rejected { UnresolvedConflict }` без
      единой записи в файл; при отсутствии открытой заметки смена режима возвращает `Applied`
      без шага сохранения (FR-031, FR-032) в `packages/core/tests/settings_native.rs` и
      `packages/core/tests/settings_wasm.rs`

### Implementation for User Story 3

- [X] T075 [US3] Реализовать машину состояний `EditorMode` с запретом автоотката
      `FullBackend → SimplifiedEmulation` по отказу процесса в
      `packages/core/src/settings/editor_mode.rs` (FR-035, `data-model.md` → запрещённые
      переходы)
- [X] T076 [US3] Реализовать переключение между полноценным режимом и упрощённой эмуляцией по
      выбору пользователя и хранение `editor_mode_preference` в
      `packages/core/src/settings/editor_mode.rs` (FR-027)
- [X] T077 [US3] Реализовать вычисление `ConfigDiskState` через `FileStamp` из
      `packages/core/src/stamp.rs` и хранение `applied_stamp` в
      `packages/core/src/settings/editor_mode.rs` (FR-026, research.md §8; зависит от T007)
- [X] T078 [US3] Реализовать применение конфига только по явному действию (`apply_config`),
      без автоматического перечитывания, в `packages/core/src/settings/editor_mode.rs`
      (FR-026)
- [X] T079 [US3] Реализовать обработку отсутствующего или нечитаемого конфига по указанному
      пути в `packages/core/src/settings/editor_mode.rs`: сообщить причину и **не** показывать
      конфиг применённым (FR-033)
- [X] T080 [US3] Реализовать предусловия `ApplyPlan` для смены режима — `RequiresNoteSave`,
      `Rejected { SaveFailed }`, `Rejected { UnresolvedConflict }`, немедленный `Applied` без
      открытой заметки — в `packages/core/src/settings/apply.rs` (FR-029…FR-032)
- [X] T081 [US3] Реализовать события `EditorModeChanged` и `EditorConfigDiskStateChanged` в
      `packages/core/src/settings/events.rs` (contracts/frontend-settings.md)
- [X] T082 [US3] Реализовать `ui/src/settings/KeybindingsSection.tsx`: три состояния как
      размеченный union с исчерпывающим `switch`, переключение режима, путь к применённому
      конфигу, признак «изменён, не применён» с действием применения (FR-025, FR-026, FR-027,
      принцип IV)
- [X] T083 [US3] Реализовать выбор и смену файла конфига жестом пользователя в
      `ui/src/settings/KeybindingsSection.tsx` с передачей пути в ядро (FR-028) — вызывается в
      UI, не в ядре, по образцу `showDirectoryPicker` в `ui/src/vault-tree/useVault.ts`
- [X] T084 [US3] Реализовать двухшаговый разбор `ApplyPlan` в `ui/src/settings/useSettings.ts`
      и заметное сообщение о переходе в `PlainInput` с причиной и действием возврата в
      `ui/src/settings/KeybindingsSection.tsx`: `RequiresNoteSave` → `VaultBackend::write` →
      сообщение о сохранении → повтор `apply`; отказ → показать причину, режим не менять
      (FR-029, FR-035, contracts/frontend-settings.md)

**Checkpoint**: US1, US2 и US3 работают независимо друг от друга

---

## Phase 6: User Story 4 - Обслужить и продиагностировать приложение (Priority: P4)

**Goal**: раздел Дополнительно даёт сброс к умолчаниям как операцию «всё или ничего»,
сведения о состоянии моста и окружении и доступ к журналам, не выпускающим содержимое заметок.

**Independent Test**: выполнить сброс к умолчаниям и убедиться, что список известных vault
сохранён, а ни один файл внутри vault не изменён; прогнать сценарии и просканировать журналы
на отсутствие текста заметок.

### Tests for User Story 4

- [X] T085 [P] [US4] Тест: сброс к умолчаниям применяется целиком; при неразрешённом
      конфликте по открытой заметке **ни одна** настройка не меняется (FR-037, кларификация
      2026-08-09) в `packages/core/tests/settings_native.rs` и
      `packages/core/tests/settings_wasm.rs`
- [X] T086 [P] [US4] Тест: сброс не удаляет список известных vault и не изменяет ни одного
      файла внутри vault (FR-038, SC-011) в `packages/core/tests/settings_native.rs`
- [X] T087 [P] [US4] Тест-сканер: на vault с заметками-маркерами прогнать сценарии и
      просканировать журналы на присутствие маркерного текста — совпадений быть не должно
      (FR-041, research.md §12) в `packages/core/tests/settings_native.rs`
- [X] T088 [P] [US4] Тест: в журналах отсутствуют абсолютные пути, используются только пути
      относительно корня vault (research.md §12) в `packages/core/tests/settings_native.rs`
- [X] T089 [P] [US4] Тест: ротация журналов удерживает суммарный объём ≤2 МиБ (FR-043) в
      `packages/core/tests/settings_native.rs`

### Implementation for User Story 4

- [X] T090 [US4] Реализовать сброс к умолчаниям как полный `SettingsChange`, проходящий те же
      предусловия, что смена режима, в `packages/core/src/settings/apply.rs` (FR-037;
      зависит от T080)
- [X] T091 [US4] Гарантировать, что сброс не затрагивает `known_vaults` и не обращается к
      содержимому vault, в `packages/core/src/settings/apply.rs` (FR-038)
- [X] T092 [P] [US4] Реализовать сбор `Diagnostics` (версия приложения, текущий `EditorMode` с
      причиной, сведения об окружении, `StoreCapabilities` + `StorageState`) в
      `packages/core/src/settings/diagnostics.rs` (FR-039)
- [X] T093 [P] [US4] Реализовать журналирование с ротацией двух файлов по 1 МиБ и записью
      только перечислимых событий и путей относительно корня vault в
      `packages/core/src/settings/logging.rs` (FR-040, FR-041, FR-043, research.md §12)
- [X] T094 [US4] Обеспечить отсутствие автоматической отправки диагностики и журналов куда бы
      то ни было в `packages/core/src/settings/diagnostics.rs` (FR-042, принцип IX)
- [X] T095 [US4] Реализовать `ui/src/settings/AdvancedSection.tsx`: сброс к умолчаниям с явным
      подтверждением, показ диагностики и доступ к журналам (FR-037, FR-039, FR-040)

**Checkpoint**: все четыре user story независимо работоспособны

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: сквозные проверки, паритет таргетов и бюджеты производительности.

- [ ] T096 Прогнать все девять сценариев `quickstart.md` на native-сборке и зафиксировать
      результат по каждому FR/SC
- [ ] T097 Прогнать все применимые сценарии `quickstart.md` на web-сборке
      (`wasm32-unknown-unknown`) и сверить с native — расхождения допустимы **только** шесть,
      перечисленные в таблице `quickstart.md` (принцип II)
- [ ] T098 [P] Проверить бюджет SC-002 по сценарию 1 из `quickstart.md`: тема и размер шрифта
      видны ≤200 мс после действия пользователя; замер на пути
      `ui/src/settings/AppearanceSection.tsx` → корень UI
- [ ] T099 [P] Проверить бюджет SC-009 по сценариям 1 и 4 из `quickstart.md`: задержка отклика
      на ввод ≤100 мс во время открытия `ui/src/settings/SettingsPanel.tsx` и применения
      настроек
- [ ] T100 [P] Проверить бюджет SC-014 по сценарию 2 из `quickstart.md` на обоих таргетах:
      внешнее изменение файла настроек отражается ≤2 с (`packages/core/src/settings/native.rs`
      через `notify`, `packages/core/src/settings/web.rs` через сверку)
- [ ] T101 [P] Проверить SC-003 по сценарию 1 из `quickstart.md`: нужная категория настроек
      достигается не более чем за 2 действия от запущенного приложения — замер по
      `ui/src/settings/SettingsPanel.tsx`
- [ ] T102 [P] Проверить SC-004 по сценариям 5 и 6 из `quickstart.md`: текущее состояние
      редактирования и применённый конфиг определяются по
      `ui/src/settings/KeybindingsSection.tsx` без обращения к документации и без открытия
      файлов, включая состояние `PlainInput`
- [ ] T103 [P] Проверить SC-013 по сценарию 8 из `quickstart.md`: при работе приложения не
      происходит ни одного сетевого обращения — инспектор сети браузера для web-сборки,
      трассировка сокетов для native (FR-008, FR-042)
- [ ] T104 [P] Проверить SC-005 по сценариям 4–7 из `quickstart.md`: показанное в
      `ui/src/settings/` значение не расходится с фактически действующим ни в одном из
      состояний — недоступный vault, отсутствующий конфиг, недоступный полноценный режим, его
      отказ посреди сессии, изменённый на диске конфиг
- [X] T105 Прогнать `pnpm --dir ui typecheck`: TypeScript `strict` без `any`,
      `@ts-ignore`/`@ts-expect-error` (принцип IV); убедиться, что пропуск варианта
      `EditorMode` даёт ошибку компиляции
- [X] T106 Прогнать `cargo test -p typsidian-core` и `wasm-pack test packages/core` целиком,
      включая существующие `packages/core/tests/vault_native.rs` и
      `packages/core/tests/vault_wasm.rs`, — убедиться, что подъём `FileStamp` в
      `packages/core/src/stamp.rs` (T007) и изъятие `vault::last_vault` из
      `packages/core/src/vault/mod.rs` и `native.rs:577` (T062) ничего не сломали
- [X] T107 Зафиксировать в `research.md` результат проверки FR-017: полная валидация «в уже
      открытых редакторе и preview» отложена до появления компонента редактора (см.
      обстоятельство 3 в шапке файла); отметить как известное ограничение, а не как
      выполненное требование

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: без зависимостей, стартует немедленно
- **Foundational (Phase 2)**: зависит от Setup — БЛОКИРУЕТ все user story
- **User Stories (Phase 3…6)**: все зависят от завершения Phase 2; между собой независимы и
  могут вестись параллельно при наличии людей
- **Polish (Phase 7)**: зависит от завершения нужных user story

### User Story Dependencies

- **US1 (P1)**: только Phase 2. Ни от одной другой истории не зависит
- **US2 (P2)**: только Phase 2 (включая `FolderRef` и сигнатуры реестра из T014). Независима
  от US1 — раздел Vault не требует раздела Внешний вид
- **US3 (P3)**: только Phase 2 (включая `FakeBridge` из T022). Независима от US1 и US2
- **US4 (P4)**: Phase 2 плюс T080 из US3 — сброс переиспользует предусловия смены режима.
  Это единственная межисторийная зависимость; при желании обойти её US4 реализуется без
  сброса режима, но тогда FR-037 покрывается не полностью

### Внутри каждой user story

- Тесты пишутся первыми и должны падать до реализации
- Типы перед логикой, логика ядра перед UI
- История завершена до перехода к следующей по приоритету

### Parallel Opportunities

- T002…T006 (Setup) — параллельно
- T008…T012 (типы Foundational) — параллельно, разные файлы
- T018, T021, T022 — параллельно после соответствующих реализаций
- Все тесты внутри одной истории помечены [P] — параллельно
- После Phase 2 четыре истории могут вестись четырьмя исполнителями одновременно
- T098…T104 (бюджеты Polish) — параллельно

---

## Parallel Example: User Story 2

```bash
# Тесты US2 запускаются вместе:
Task: "Та же папка не заводится дважды (T045)"
Task: "Две разные одноимённые папки дают две записи (T046)"
Task: "Подпись не влияет на тождество (T047)"
Task: "forget не трогает диск (T048)"
Task: "Переключение vault не меняет тему/шрифт/режим (T049)"
Task: "Служебных файлов внутри vault не появляется (T050)"
Task: "Перенесённая папка не разыскивается (T051)"
Task: "Миграция last_vault (T052)"
```

```bash
# Типы Foundational запускаются вместе:
Task: "Settings/Theme/ModePreference (T008)"
Task: "VaultId/VaultRecord/Availability (T009)"
Task: "EditorMode/EditorConfig/ConfigDiskState (T010)"
Task: "SettingsEvent/StoreCapabilities/StorageState (T011)"
Task: "SettingsChange/ApplyPlan/RejectReason (T012)"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Phase 1: Setup
2. Phase 2: Foundational (КРИТИЧНО — блокирует все истории)
3. Phase 3: US1
4. **ОСТАНОВИТЬСЯ И ПРОВЕРИТЬ**: сценарии 1–3 из `quickstart.md` на обоих таргетах
5. На этом этапе есть работающая панель настроек с внешним видом и корректным поведением
   файла настроек — самостоятельная ценность

### Incremental Delivery

1. Setup + Foundational → фундамент готов
2. + US1 → проверить независимо → MVP
3. + US2 → раздел Vault, непрозрачная идентичность, миграция с `last_vault`
4. + US3 → три состояния режима на `FakeBridge`
5. + US4 → обслуживание и диагностика
6. Phase 7 → паритет таргетов и бюджеты

### Порядок с оглядкой на риск

US3 намеренно стоит третьей, а не первой, несмотря на то что раздел Кейбиндинги —
самая заметная часть фичи: он единственный зависит от подсистемы, которой ещё нет.
`FakeBridge` снимает эту зависимость для реализации и тестов, но появление реального моста
может потребовать доработок именно здесь — поэтому три остальных раздела к тому моменту уже
должны быть закрыты.

В US2 наиболее рискованная часть — не список как таковой, а **тождество папок**: именно там
первая ревизия плана содержала дефект, схлопывавший разные vault в вебе. T046 написан как
проверка регрессии и должен появиться раньше реализации T054/T055.

---

## Notes

- [P] — разные файлы, нет зависимостей между задачами
- Каждая задача ядра требует прохождения **и** `cargo test`, **и** `wasm-bindgen-test`, если
  логика не является платформенно-специфичной (принцип II, NON-NEGOTIABLE)
- Задачи, помеченные только `settings_native.rs`, проверяют поведение, недостижимое в вебе
  (права на запись каталога, файлы конфига на диске, ротация журналов, перенос папки с
  потерей связи) — это не пропуск паритета, а отсутствие соответствующего источника в вебе
  (research.md §3, §7, §10, §13)
- Ни одна задача не сравнивает папки по пути, имени или показываемому расположению: тождество
  устанавливает платформа за `FolderRef` (research.md §10)
- Коммит после каждой задачи или логической группы
- На любом checkpoint можно остановиться и проверить историю независимо

---

## Phase 8: Convergence (состояние на 2026-08-09)

**Purpose**: чекбоксы фаз 5-7 отстали от кода. Задачи T065-T095 помечены незакрытыми, но
реализация и тесты для большинства из них существуют и проходят. Ниже — зафиксированные
свидетельства и задачи на то, что действительно осталось. Существующие ID не
переиспользуются и не перенумеровываются; отметки в фазах 5-7 намеренно не переставлены —
их должен подтвердить прогон, а не чтение файлов.

**Свидетельства выполнения (проверено прогоном `cargo test -p typsidian-core` — 148 тестов,
и `wasm-pack test --headless --chrome packages/core` — 41 тест, все зелёные):**

| Задача | Свидетельство |
|---|---|
| T065, T066 | `backend_failure_lands_in_plain_input_and_never_in_emulation`, `plain_input_is_left_only_by_an_explicit_action` |
| T067 | `user_can_switch_between_full_backend_and_emulation` |
| T069, T071 | `missing_config_path_is_reported_and_not_marked_applied`, `deleted_config_is_reported_but_the_mode_keeps_working`, `unreadable_config_is_distinguished_from_a_missing_one` |
| T070 | `config_reverted_to_previous_content_stops_looking_changed` |
| T072, T073, T074 | `mode_change_with_a_dirty_note_requires_save_first`, `apply.rs` → `RejectReason::SaveFailed`, `unresolved_conflict_blocks_the_switch_without_touching_the_file` |
| T075-T081 | `packages/core/src/settings/editor_mode.rs` (669 строк), `apply.rs` (434), `events.rs` |
| T082, T083 | `ui/src/settings/KeybindingsSection.tsx` (172 строки), проп `onPickConfig` |
| T084 | `ui/src/settings/useSettings.ts` → `apply(): Promise<ApplyPlan \| null>` |
| T085, T086 | `reset_keeps_known_vaults_and_returns_values_to_defaults`, `mode_preference_change_follows_the_same_plan_as_reset` |
| T087, T088 | `rendered_entry_has_no_absolute_paths_and_no_note_text`, `absolute_note_paths_are_reduced_to_vault_relative`, `path_outside_vault_is_dropped_entirely` |
| T089 | `rotation_keeps_total_within_budget`, `rotation_moves_current_to_previous_without_cutting_lines` |
| T092, T093 | `settings/diagnostics.rs` (142 строки), `settings/logging.rs` (236) |
| T094 | В `packages/core/src` и `ui/src` нет ни одного вызова `fetch`/`XMLHttpRequest`/`reqwest` |

**Что действительно не сделано — обрыв между ядром и UI:**

- [X] T108 Экспортировать сброс к умолчаниям в WASM-биндинг: `reset_to_defaults()` существует
      в `packages/core/src/settings/apply.rs:97`, но в списке `*_js`-методов `WebStore`
      (`packages/core/src/settings/web.rs`, строки 542-611) его нет, и UI получить полный
      `SettingsChange` со значениями по умолчанию не может. Собирать этот объект на стороне
      TS запрещено: умолчания — знание ядра (принцип I) per FR-037 (missing)
- [X] T109 [P] Экспортировать диагностику в WASM-биндинг: `diagnostics::collect()`
      (`packages/core/src/settings/diagnostics.rs:81`) не проброшен в `WebStore`; тип
      `Diagnostics` в `ui/src/wasm-core/typsidian_core.d.ts:62` объявлен, но получить
      значение неоткуда per FR-039 (missing)
- [X] T110 Подключить сброс к умолчаниям в интерфейсе: `ui/src/settings/SettingsPanel.tsx:141`
      передаёт `onResetToDefaults={() => {}}` — кнопка в `AdvancedSection` есть, нажатие не
      делает ничего. Провести сброс через тот же `ApplyPlan`, что и обычное изменение, чтобы
      неразрешённый конфликт отменял его целиком, и добавить явное подтверждение перед
      применением per FR-037, кларификация 2026-08-09 (missing)
- [X] T111 Показать диагностику в `ui/src/settings/AdvancedSection.tsx`: версия приложения,
      состояние моста, сведения об окружении. Сейчас файл (57 строк) показывает только
      расположение хранилища, а его собственный doc-комментарий признаёт пропуск —
      «Диагностика, журналы и подтверждение сброса добавляются задачами US4» per FR-039 (missing)
- [X] T112 [P] Вывести журналы в раздел «Дополнительно» per FR-040, FR-041 (missing)
      - **Заблокировано, обнаружено при реализации 2026-08-09.** Формулировка выше
        неверна: `settings/logging.rs` ничего не пишет и не ротирует. Модуль
        платформенно-нейтрален (единственный `cfg` в нём — `#[cfg(test)]`), задаёт типы
        `LogEntry`/`LogRotation` и политику бюджета, но **его никто не вызывает**:
        приёмника журналов не существует ни на одном таргете, и файла, который можно
        было бы показать, тоже нет.
      - Прежде чем выводить журналы в интерфейс, нужна отдельная задача: подключить
        запись — на native в файл рядом с `settings.json`, в вебе в OPFS, — и решить,
        какие события в неё попадают. Это работа уровня ядра, а не UI, и в объём
        «две дыры в UI-контуре» она не входила.
- [X] T113 [P] Покрыть T108-T110 тестами на обоих таргетах: сброс через биндинг возвращает
      значения к умолчаниям, не трогает `known_vaults` и отклоняется целиком при
      неразрешённом конфликте — `packages/core/tests/settings_native.rs` и
      `settings_wasm.rs` per FR-037, FR-038, Constitution II (missing)
- [X] T114 Автоматизировать перенос сборки wasm в UI: `wasm-pack build packages/core --target web`
      кладёт результат в `packages/core/pkg`, а `ui/src/wasm-core` — ручная копия, и сейчас
      эти два каталога разошлись (`pkg` от 11:45, копия от 19:55, разный размер `.wasm`).
      Добавить скрипт `build:wasm` в корневой `package.json`, выполняющий сборку и
      копирование одной командой per plan: Project Structure (missing)

---

## Доделка 2026-10-09

**Отметки фаз 5–6.** T065–T092, T094, T095 отмечены по свидетельствам из Phase 8: названные
там тесты на месте и зелёные в прогоне от 2026-10-09 (`cargo test -p typsidian-core` и
`wasm-pack test` целиком). T068 покрыта `null_bridge_never_reports_full_backend`.

**T093, T112 — журнал приложения.** Блокировка из Phase 8 снята: приёмника журналов не
было, теперь он есть.

- `settings/logging.rs`: `Journal` — какие события попадают в журнал и что из него ещё не
  записано. События хранилища (`record_event`), применённые изменения по одному на поле
  (`record_change`, без значений — только имена полей), добавление и забывание vault.
  Внешняя правка значений и смена состояния файла конфига редактора в журнал не идут: они
  приходят слишком часто.
- `NativeStore` пишет `typsidian.log` и `typsidian.previous.log` рядом с `settings.json`,
  `WebStore` — те же два файла в OPFS. Отказ записи журнала ничего не отменяет.
- `SettingsStore::journal_text`; привязки `journalText` (wasm) и `device_journal` (Tauri).
  Отдельным вызовом, а не полем снимка: журнал — до двух мегабайт, снимок запрашивается
  постоянно.
- Раздел «Дополнительно»: «Показать журнал», обновить, скопировать, скрыть. Журнал покидает
  приложение только копированием (FR-042).

Тест `the_journal_records_events_without_values_and_survives_a_restart` на обоих таргетах;
сценарий `scripts/dev/scenarios/settings-journal.mjs` в headless-браузере. На рабочем столе
не запускалось.

**Оговорка.** Время в записи — миллисекунды эпохи, как задаёт прежний `LogEntry::render`;
читаемой даты в журнале нет.

Открыто 9 задач — все ручные: прогоны quickstart (T096, T097) и проверки бюджетов по ним
(T098–T104).

---

## Доделка 2026-10-18

- [X] T115 Раскрытый список `<select>` и подсказки `<datalist>` рисуются белым полем со
      светлым текстом: интерфейс тёмный, а браузеру об этом не сказано, и всплывающие
      списки он берёт из светлой системной темы. Объявить `color-scheme: dark` на корне в
      `ui/src/styles/app.css` и задать фон и цвет `option` токенами; проверить раскрытый
      список в headless-браузере (FR-017)

T115: в headless chromium раскрытый список и `option` получают `color-scheme: dark` и фон
`--steel-900`. Сам всплывающий список в снимок не попадает — проверены вычисленные стили.
Рабочий стол (WebKitGTK рисует список средствами GTK) и Android не проверялись.
