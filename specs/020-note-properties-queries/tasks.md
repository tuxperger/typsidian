# Tasks: Свойства заметок и выборки

**Input**: Design documents from `/specs/020-note-properties-queries/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для ядра на обоих таргетах — `packages/core/tests/common/properties_cases.rs`,
раннеры `properties_native.rs` и `properties_wasm.rs`. UI — сценарий
`scripts/dev/scenarios/properties.mjs` и quickstart.md.

## Phase 1: Setup

- [X] T001 Объявить `pub mod properties;` в `packages/core/src/lib.rs`, завести `packages/core/src/properties/{mod,read,write,query,select,web}.rs` (`web.rs` под wasm32), `packages/core/tests/common/properties_cases.rs` (с `pub mod` в `tests/common/mod.rs`) и раннеры `packages/core/tests/properties_native.rs`, `packages/core/tests/properties_wasm.rs` по образцу `structure_*`
- [X] T002 [P] Область переводов `ui/src/i18n/locales/{en,ru}/properties.ts`, подключённая в `ui/src/i18n/locales/en.ts` и `ru.ts`: режим «Свойства», виды значений, «Добавить свойство», «+ тег», «Сведения о файле», плашка B2 и её причины, фишки условий и операторы, форма выборки, пустые состояния

## Phase 2: Foundational

- [X] T003 Типы `PropertyValue`, `Property`, `NoteProperties`, `PropertiesProblem`, `PropertyEdit` (serde + Tsify) по data-model.md в `packages/core/src/properties/mod.rs`
- [X] T004 Разбор записи `read(text)` по research.md §1–§2 (`typst::syntax`, литералы, места, однозначность) в `packages/core/src/properties/read.rs`
- [X] T005 [P] Тесты разбора на обоих путях по contracts/core-properties-api.md § Тесты (все виды, кириллица, строковые ключи, не-литерал, повтор, две записи, кириллица и символы вне BMP в местах) — в `packages/core/tests/common/properties_cases.rs` и раннерах per FR-001, FR-004, FR-005
- [X] T006 Свойства в индексе: поле в записи `IndexCache` и его реализации — таблица `properties` в `SqliteCache` с миграцией версии кэша и поле в `MemoryCache`/IndexedDB — в `packages/core/src/vault/cache.rs`; обновление свойств там же, где пересчитываются ссылки заметки, в `packages/core/src/vault/mod.rs` per FR-009

**Checkpoint**: свойства читаются и лежат в индексе.

## Phase 3: User Story 1 — Задать свойства и видеть их (P1) 🎯 MVP

**Independent Test**: тег и «статус» через панель → запись в файле, сборка проходит; правка во внешнем редакторе видна в панели.

- [X] T007 [P] [US1] Тесты правки на обоих путях: добавить, изменить, удалить, удалить последнее, создать запись; остальной текст побайтно тот же; запись собирается `PreviewWorld` без входов и не даёт ни одного глифа на странице (SC-003) — в `packages/core/tests/common/properties_cases.rs` per FR-002, FR-003
- [X] T008 [US1] Правка `edit(text, edit) -> EditOutcome` по research.md §3 в `packages/core/src/properties/write.rs`
- [X] T009 [US1] wasm `propertiesRead`, `propertiesEdit` в `packages/core/src/properties/web.rs`; команды `properties_read`, `properties_edit` в `packages/shell/src/lib.rs`; методы `properties.read`, `properties.edit` в `ui/src/vault-client.ts`, `vault-client-web.ts`, `vault-client-tauri.ts`
- [X] T010 [US1] Режим «Свойства» — `ui/src/note-panel/properties/PropertiesMode.tsx` и `PropertyRow.tsx` по макету B1/B2 и contracts/frontend-properties.md; регистрация в `noteModes` в `ui/src/workspace/Workspace.tsx`; применение `EditOutcome` к редактору путём параметров 008; стили `.ts-props*` в `ui/src/styles/app.css` per FR-001, FR-002, FR-004
- [X] T011 [US1] Сценарий `scripts/dev/scenarios/properties.mjs` в объёме US1 (quickstart.md §2 первая часть, §5); снимок сверить с B1/B2

## Phase 4: User Story 2 — Отобрать заметки (P2)

**Independent Test**: три заметки с тегом `проект`, клик по тегу → ровно они.

- [X] T012 [P] [US2] Тесты отбора на обоих путях: условия всех видов, несравнимые, «и», сортировка с пустыми в конце, `limit`/`truncated`; обновление индекса меняет результат; бенч 10 000 заметок (`packages/core/benches/properties_scale.rs`) ≤300 мс per FR-006, SC-001
- [X] T013 [US2] `Selection`, `Condition`, отбор и сортировка в `packages/core/src/properties/query.rs`; `VaultSession::select`, `property_keys` в `packages/core/src/vault/mod.rs`; `SearchQuery.properties` и разбор `тег:`/`папка:`/`ключ:` (`parse_conditions`) в `packages/core/src/search/mod.rs`
- [X] T014 [US2] Биндинги `vaultSelect`, `vaultPropertyKeys` и команды `vault_select`, `vault_property_keys`; методы клиента в `ui/src/vault-client*.ts`
- [X] T015 [US2] Фишки условий в `ui/src/search/SearchOverlay.tsx` по макету B3; клик по тегу в `PropertiesMode.tsx` открывает поиск с условием; обновление результата по событию изменения индекса per FR-006, US2 сценарий 3
- [X] T016 [US2] Пакетный пересчёт свойств с прогрессом и отменой (`reindex_properties`) по образцу переиндексации 001 в `packages/core/src/vault/mod.rs`, биндинг и команда, запуск при смене версии формата; тест отмены на обоих путях per FR-010

## Phase 5: User Story 3 — Блок выборки (P3)

**Independent Test**: блок по тегу; тег ещё одной заметке → строка в предпросмотре без правки блока.

- [X] T017 [P] [US3] Тесты блока на обоих путях: `select_blocks` (ключ не зависит от порядка аргументов), `select_inputs`, сборка с входом — таблица и ссылки, пусто — пояснение, без входа — замещающий текст; экспорт PDF с тем же входом per FR-007, FR-008
- [X] T018 [US3] `select_blocks`, `select_inputs`, `select_library` и текст `select.typ` по contracts/select-library.md в `packages/core/src/properties/select.rs`
- [X] T019 [US3] Входы сборки: `PreviewWorld` с `Library::builder().with_inputs` в `packages/core/src/preview/world.rs`; расчёт входов перед сборкой заметки с блоками в `packages/core/src/preview/compile.rs` (и в пути экспорта 009); изменение индекса свойств помечает такие сборки устаревшими per FR-008, SC-002
- [X] T020 [US3] Схема формы `select` для параметров 008 с видами полей «условия», «порядок», «столбцы» в `packages/core/src/elements/schema.rs`; запись `select.typ` при вставке (§10) через `VaultBackend`
- [X] T021 [US3] Пункт «Выборка» в меню вставки 008 и поля формы «условия», «порядок», «столбцы» в `ui/src/editor/` по макету B4; добавление строки `#import` при вставке; пересборка открытой заметки по событию индекса в `ui/src/preview/useCompiledDocument.ts` per FR-007
- [X] T022 [US3] Дополнить `scripts/dev/scenarios/properties.mjs` (quickstart.md §2 вторая часть) и сверить снимки с B4–B6

## Phase 6: Polish

- [X] T023 [P] Проверка значений свойств в сообщениях ошибок и журнале (IX) — тест на обоих путях в `packages/core/tests/common/properties_cases.rs`
- [X] T024 Гейты `.specify/scripts/typsidian/gates.sh all`
- [X] T025 README: свойства, поиск по ним, блок выборки и его вид вне приложения в `README.md`; `**Status**: Implemented` в `specs/020-note-properties-queries/spec.md`

## Dependencies

Setup → Foundational (T003 → T004 → T006) → US1 → US2 → US3 (US3 использует отбор US2). Внутри
историй — тесты [P] параллельно реализации ядра; UI — после клиента.

## Phase 7: Convergence

- [X] T026 Найти и устранить пометку «конфликт» на вкладке заметки сразу после вставки блока выборки в web (сценарий `scripts/dev/scenarios/properties.mjs`, снимок B5; на B4 её нет): ядро шлёт `ConflictDetected` из `poll_for_changes` в `packages/core/src/vault/web.rs` — выяснить, кто меняет файл заметки на диске при несохранённых правках, и добавить в сценарий проверку отсутствия пометки per FR-002 (contradicts)
