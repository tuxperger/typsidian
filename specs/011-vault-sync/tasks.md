# Tasks: Синхронизация vault — история версий и доставка между устройствами

**Input**: Design documents from `/specs/011-vault-sync/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для логики ядра на обоих таргетах (принцип I конституции). Политика
сессии тестируется на обоих путях через `FakeGitEngine` (feature `test-support`), настоящий
`NativeGitEngine` — интеграционно под native, `JsGitEngine` — по quickstart.md. Для UI
JS-тестов в репозитории нет — проверка по quickstart.md.

## Phase 1: Setup

- [X] T001 Объявить `pub mod sync;` в `packages/core/src/lib.rs` и создать скелет модулей по plan.md → Project Structure (`packages/core/src/sync/{mod.rs,engine/mod.rs,ignore.rs,conflict_copy.rs,merge.rs,pull.rs,keeper.rs,autosnapshot.rs,syncthing.rs,secrets.rs,device.rs}`; `engine/native.rs`, `engine/transport_http.rs`, `engine/transport_ssh.rs` под `cfg(not(wasm32))`, `engine/web.rs` и `web.rs` под `cfg(wasm32)`)
- [X] T002 Добавить зависимости в `packages/core/Cargo.toml` с комментариями-обоснованиями по research.md: `similar` (оба таргета), `getrandom` (оба; `js`/`wasm_js` под wasm32), под native — `git2 = { default-features = false, features = ["vendored-libgit2"] }` и `quick-xml`; убедиться, что `cargo build --target wasm32-unknown-unknown -p typsidian-core` не тянет `git2`
- [ ] T003 Проверить сборку `libgit2-sys` в dev-оболочке и под Android-таргетом; при нехватке нативных инструментов (cmake, pkg-config) дополнить `flake.nix` (агент `nix-env-agent`), без системного OpenSSL и libssh2 (research §2)
- [X] T004 [P] Добавить `isomorphic-git` (точная версия) в `ui/package.json` и `keyring = { version = "3", features = ["sync-secret-service", "crypto-rust", "apple-native", "windows-native"] }` в `packages/shell/Cargo.toml` только для не-Android таргетов

## Phase 2: Foundational

- [X] T005 Реализовать `DeviceIdentity` (32 hex из `getrandom`, имя хоста/«Browser» по умолчанию) в `packages/core/src/sync/device.rs` и поля `device_id`/`device_name` в `Settings` (`packages/core/src/settings/mod.rs`, `format.rs`): создаются при первом чтении настроек, сохраняются; старые файлы настроек читаются без ошибки
- [X] T006 Добавить `DeviceVaultSync` (data-model) в настройки устройства по `vault_id` в `packages/core/src/settings/mod.rs` и `format.rs` с валидацией (`cors_proxy` — https/loopback-http URL)
- [X] T007 Добавить `VaultSyncSettings` и `Keeper` под ключом `sync` в `packages/core/src/vault/vault_settings.rs` (валидация `remote_url` без учётных данных, диапазоны пауз/интервалов из data-model) с вариантами ошибок в `VaultSettingsError`
- [X] T008 [P] Добавить `service_dir::derived_paths()` (`.typsidian/cache.sqlite*`, `.typsidian/tmp`) с тестом в `packages/core/src/vault/service_dir.rs`
- [X] T009 Определить `SyncError`, `SyncOverview`, `HistoryState`, `Change`, `Snapshot`, `FileVersion`, `Content`, `ShellCapabilities`, `SyncEvent` (data-model, `tsify` под wasm32) в `packages/core/src/sync/mod.rs`
- [X] T010 Определить трейт `GitEngine`, `RepoProbe`, `HeadInfo`, `InProgressOp`, `Parents`, `RemoteSpec`, `Auth`, `Page`, `EngineError` → `SyncError` по contracts/core-sync-api.md в `packages/core/src/sync/engine/mod.rs`
- [X] T011 Реализовать `FakeGitEngine` (в памяти: коммиты, деревья, ветка, удалённый как второй экземпляр, режимы сбоя «нет сети»/«отказ доступа»/«non-ff») под feature `test-support` в `packages/core/src/sync/engine/fake.rs`
- [X] T012 Реализовать `ignore.rs`: разбор/сборка блока между маркерами (`// typsidian:begin|end` для `.stignore`, `# typsidian:begin|end` для `.gitignore`), вставка в начало для `.stignore` и в конец для `.gitignore`, замена блока без дублей, `IgnoreReport` (research §5, §6) — в `packages/core/src/sync/ignore.rs`
- [X] T013 Реализовать трейт `SecretStore` (`get/set/delete` по `(vault_id, SecretKind)`) и `MemorySecretStore` для тестов в `packages/core/src/sync/secrets.rs`
- [X] T014 Реализовать `NativeGitEngine` — локальные операции (`discover` с `ForeignParent` и `in_progress`, `init`, `status` с учётом `.gitignore`, `commit` выбранных путей, `log` постранично, `read_blob`, `tree_diff`, `merge_base`, `head`, `set_head`) поверх `git2` в `packages/core/src/sync/engine/native.rs`
- [X] T015 [P] Реализовать `JsGitEngine` (`#[wasm_bindgen] extern "C"` с методами-`Promise`, типы через `tsify`) в `packages/core/src/sync/engine/web.rs` и JS-адаптер `ui/src/sync/git-engine-web.ts`: `fs` isomorphic-git поверх `FileSystemDirectoryHandle` vault и локальные операции из T014 (research §3)
- [X] T016 Реализовать `SyncSession<E: GitEngine>` с `overview()` и подпиской на события, создание сессии при открытии vault; `SyncthingFolder.detected` по маркеру `.stfolder` в корне без связи с Syncthing (FR-021) в `packages/core/src/sync/mod.rs`; `WebSyncSession` (`#[wasm_bindgen]`) в `packages/core/src/sync/web.rs`
- [X] T017 [P] Общие тест-кейсы сессии на `FakeGitEngine` и модулей `ignore` в `packages/core/tests/common/sync_cases.rs` с раннерами `packages/core/tests/sync_native.rs` и `packages/core/tests/sync_wasm.rs`; интеграционные тесты `NativeGitEngine` на временных репозиториях (сверка с `git` CLI в тестах не требуется — читать результат через `git2`) в `packages/core/tests/sync_engine_native.rs`
- [X] T018 Интерфейс `SyncClient` по contracts/core-sync-api.md в `ui/src/sync/sync-client.ts` и реализации `ui/src/sync/sync-client-web.ts` (через `wasm-call-queue`) и `ui/src/sync/sync-client-tauri.ts`; каркас команд `sync_*` и состояние сессии в `packages/shell/src/lib.rs`
- [X] T019 [P] Ключи локализации `sync.*` (ru, en) для всех `SyncError` и состояний индикатора в `ui/src/i18n/locales/ru/` и `ui/src/i18n/locales/en/` (FR-039)

## Phase 3: User Story 1 — Включить историю и делать снимки (P1) 🎯 MVP

**Goal**: включение истории (с правилами исключения), список изменений, снимок всех/выбранных файлов офлайн.

**Independent Test**: копия `/home/apokryf/notes` → включить историю → изменить две заметки → снимок одной; `git log`/`git status` вне приложения видят снимок и одно изменение.

- [X] T020 [US1] `ignore_reports()`, `apply_ignore()`, `decline_stignore()` и `enable_history()` (порядок FR-028: сначала `.stignore`, если есть маркер Syncthing; затем `.gitignore`; `init`; первый снимок с `CancelToken` и прогрессом, отмена не оставляет коммита (принцип X); подхват существующей истории без изменений, отказ включать свою при `ForeignParent`; «история была и пропала» по `last_seen_history` в `DeviceVaultSync` → предложение создать новую или скачать из удалённого (Edge Cases)) в `packages/core/src/sync/mod.rs`
- [X] T021 [US1] `changes()` и `snapshot(paths, message, flush_token)` с автосообщением по изменённым заметкам, автором из `DeviceVaultSync`/глобального git-конфига/запросом (FR-005…FR-007), отказами `InProgressOperation`; при `ForeignParent` список изменений и снимки ограничены путями внутри vault (Edge Cases) в `packages/core/src/sync/mod.rs`
- [X] T022 [P] [US1] Тесты US1 в `packages/core/tests/common/sync_cases.rs`: блоки правил без дублей и с сохранением строк пользователя, порядок FR-028, снимок выбранных путей, пустое сообщение, отсутствие автора, незавершённая операция, работа без сети (FakeGitEngine в режиме «нет сети»)
- [X] T023 [US1] wasm-биндинги операций US1 в `packages/core/src/sync/web.rs` и команды Tauri в `packages/shell/src/lib.rs`
- [X] T024 [US1] `flushAll()` (сохранение открытых заметок с выдачей `flush_token`) в `ui/src/vault-client.ts` и обеих реализациях
- [X] T025 [US1] Индикатор синхронизации в строке состояния `ui/src/sync/SyncIndicator.tsx` и хук `ui/src/sync/useSync.ts` (подписка на `SyncEvent`)
- [X] T026 [US1] Панель «Синхронизация», раздел «История»: список изменений с выбором, поле сообщения, «Сделать снимок», сравнение изменения — в `ui/src/sync/SyncPanel.tsx`; стили `.ts-sync-*` в `ui/src/styles/app.css`
- [X] T027 [US1] Мастер включения истории (правила `.stignore` с точным текстом → `.gitignore` → автор → первый снимок с прогрессом и отменой) в `ui/src/sync/EnableHistoryWizard.tsx`
- [X] T028 [US1] Подключить индикатор, панель и команды палитры `sync.open-panel`, `sync.snapshot` (`CommandId` в `packages/core/src/settings/keybindings.rs`) в `ui/src/workspace/Workspace.tsx`

## Phase 4: User Story 2 — История заметки и откат (P1)

**Goal**: история файла и vault, сравнение версий, откат и восстановление удалённого.

**Independent Test**: три снимка → история заметки → сравнение первой с текущей → откат → `Ctrl+Z` возвращает текущую.

- [X] T029 [US2] `history(path, cursor)` с прослеживанием переименований (по `tree_diff` соседних снимков) и `diff(path, from, to)` с `Binary { size }` в `packages/core/src/sync/mod.rs`
- [X] T030 [US2] `restore(path, snapshot)` — запись версии через `VaultBackend` (атомарно, событие внешнего изменения 001), восстановление удалённого файла на прежнее место в `packages/core/src/sync/mod.rs`
- [X] T031 [P] [US2] Тесты US2 в `packages/core/tests/common/sync_cases.rs`: история через переименование, постраничность, двоичный файл, откат не создаёт снимков, восстановление удалённого
- [X] T032 [US2] wasm-биндинги и команды Tauri для `history`, `diff`, `restore` в `packages/core/src/sync/web.rs`, `packages/shell/src/lib.rs`, методы `SyncClient`
- [X] T033 [US2] Панель истории файла (список снимков, сравнение с текущей и с предыдущей, «вернуть эту версию» как правка редактора с отменой) в `ui/src/sync/FileHistory.tsx`; история vault с восстановлением удалённых — раздел в `ui/src/sync/SyncPanel.tsx`
- [X] T034 [US2] Команда палитры `sync.file-history` и пункт контекстного меню дерева в `ui/src/workspace/Workspace.tsx`

## Phase 5: User Story 3 — Удалённый репозиторий (P1)

**Goal**: подключение по токену/SSH, отправка, получение со слиянием и разрешением конфликтов, клонирование vault, офлайн-деградация.

**Independent Test**: bare-репозиторий как удалённый → отправить → клонировать во вторую папку → встречные правки одной строки → получить → три колонки → отмена возвращает файлы; без сети — сообщение ≤ 30 с.

- [X] T035 [US3] Smart-HTTP subtransport поверх `reqwest` (`info/refs`, `upload-pack`, `receive-pack`, Basic-авторизация по хосту, тайм-ауты research §9, `http://` только для loopback) и регистрация через `git2::transport::register` в `packages/core/src/sync/engine/transport_http.rs`
- [X] T036 [US3] SSH subtransport через системный `ssh -o BatchMode=yes` (только desktop; аргументы задаёт приложение, stdio — канал протокола; поправка конституции 1.2.0) в `packages/core/src/sync/engine/transport_ssh.rs`; на Android — `UnsupportedScheme`
- [X] T037 [US3] `fetch`, `push` (non-ff → `RejectedNonFastForward`), `clone_into` с `CancelToken` и прогрессом в `packages/core/src/sync/engine/native.rs`; то же через isomorphic-git (`http/web`, `corsProxy` из `DeviceVaultSync`, распознавание CORS-отказа → `CorsBlocked`) в `ui/src/sync/git-engine-web.ts` и `packages/core/src/sync/engine/web.rs`
- [X] T038 [US3] Реализации `SecretStore`: desktop — `keyring` в `packages/shell/src/lib.rs` (или `packages/shell/src/secrets.rs`); Android — файл в приватном каталоге приложения; web — IndexedDB в `packages/core/src/sync/secrets.rs` под wasm32
- [X] T039 [US3] `connect_remote` (очистка `user:token@` из URL с переносом токена в `SecretStore`, проверка доступа, запись `remote_url` в `vault.json` и в `.git/config` без секрета), `disconnect_remote`, `clone_vault` в пустую папку в `packages/core/src/sync/mod.rs`
- [X] T040 [US3] `merge3` (diff3 поверх `similar`, `Clean | Hunks`, двоичные — без авто-слияния) в `packages/core/src/sync/merge.rs`
- [X] T041 [US3] `pull`: fetch → план (fast-forward / слияние / конфликты) → запись файлов через `VaultBackend` с журналом исходных версий → коммит слияния и `set_head`; `abort_pull` и откат по журналу при ошибке или отмене; `resolve` для конфликтов получения; блокировка `PullConflictsPending` (FR-016, FR-017, research §9) в `packages/core/src/sync/pull.rs`
- [X] T042 [US3] `push`, `RemoteStatus` (ahead/behind с временем), автоотправка после снимка и автополучение при открытии/по интервалу через `tick` в `packages/core/src/sync/mod.rs`
- [X] T043 [P] [US3] Тесты US3 в `packages/core/tests/common/sync_cases.rs`: `merge3` (непересекающиеся абзацы — чисто, SC-007; пересечение — ханки), fast-forward, конфликт и отмена с возвратом файлов, отмена посреди записи, non-ff, отказ доступа, офлайн (FakeGitEngine), секрет не попадает в `vault.json`/сообщения ошибок (SC-008)
- [X] T044 [P] [US3] Интеграционные тесты native-транспорта в `packages/core/tests/sync_engine_native.rs`: push/fetch/clone с `file://` bare-репозиторием, smart HTTP против локального тестового сервера на loopback, закрытый порт → `Offline` за предел из research §9
- [X] T045 [US3] wasm-биндинги и команды Tauri для `connect_remote`, `disconnect_remote`, `clone_vault`, `push`, `pull`, `abort_pull`, `conflicts`, `resolve` в `packages/core/src/sync/web.rs`, `packages/shell/src/lib.rs`, методы `SyncClient`
- [X] T046 [US3] Диалог подключения удалённого (адрес, имя, токен в поле пароля, предупреждение о хранилище браузера в web, посредник в web) в `ui/src/sync/RemoteDialog.tsx`; раздел «Удалённый репозиторий» в `ui/src/sync/SyncPanel.tsx` (↑/↓, время, отправить/получить, ошибки в индикаторе, не модально; `AuthRejected` ведёт к замене данных доступа, `RejectedNonFastForward` — к «сначала получить»)
- [X] T047 [US3] Редактор слияния (2/3 колонки, авто-совмещение с предпросмотром результата, выбор версии, ручная правка, «отменить получение») в `ui/src/sync/MergeEditor.tsx`; отметки конфликтов в дереве `ui/src/vault-tree/`
- [X] T048 [US3] «Создать vault из удалённого репозитория» в выборе vault (`ui/src/settings/VaultSection.tsx` и экран открытия vault) с прогрессом и отменой; команды палитры `sync.push`, `sync.pull`

## Phase 6: User Story 4 — Syncthing: состояние и конфликтные копии (P2)

**Goal**: распознавание папки Syncthing, состояние с разрешения пользователя, конфликтные копии вне индекса и их разрешение.

**Independent Test**: файл `note.sync-conflict-20261003-120000-ABCDEFG.typ` → нет в дереве/поиске/графе, есть в «Конфликтах»; «взять копию» → копия удалена.

- [X] T049 [US4] `conflict_copy::parse` (исходное имя, время, короткий ID устройства; с расширением и без, вложенные копии) с модульными тестами в `packages/core/src/sync/conflict_copy.rs`
- [X] T050 [US4] Общая `backend::is_index_excluded(path)` вместо `native::is_excluded`/`web::is_excluded` (точка в начале + конфликтные копии) и `VaultBackend::list_conflict_copies()` в `packages/core/src/vault/backend.rs`, `native.rs`, `web.rs`; события вотчера о конфликтных копиях → `ConflictsChanged`
- [X] T051 [US4] Конфликты Syncthing в `conflicts()` и `resolve` (`KeepOurs`, `TakeTheirs`, `Merged`, `KeepBothAsFile`; удаление копии после решения; копия без исходного файла) в `packages/core/src/sync/mod.rs`
- [X] T052 [US4] Клиент Syncthing (только чтение): `system/status`, `config/folders` (папка по пути корня vault), `db/status`, `system/connections`, `config/devices`, long-poll `events` с запасным опросом; закрепление сертификата loopback (TOFU); поиск `config.xml` в стандартных местах на desktop (FR-022a) в `packages/core/src/sync/syncthing.rs`
- [X] T053 [P] [US4] Тесты US4 в `packages/core/tests/common/sync_cases.rs` (разбор имён, исключение из индекса на обоих backend, разрешение конфликтов, копия без исходника) и в `packages/core/tests/sync_syncthing_native.rs` (разбор `config.xml`, ответы REST на тестовом loopback-сервере, `Unauthorized`, `Unreachable`)
- [X] T054 [US4] wasm-биндинги и команды Tauri `syncthing_discover`, `syncthing_connect`, поток состояния в `packages/core/src/sync/web.rs`, `packages/shell/src/lib.rs`, методы `SyncClient`
- [X] T055 [US4] Раздел «Syncthing» (обнаружено/состояние/прогресс/устройства, «найти Syncthing», ручной адрес и ключ) и раздел «Конфликты» (список, открытие в `MergeEditor`) в `ui/src/sync/SyncPanel.tsx`; команда `sync.resolve-conflicts`

## Phase 7: User Story 5 — Git и Syncthing вместе (P2)

**Goal**: проверка правил на каждом устройстве, хранитель, автоснимки, история не-хранителя через зеркало, три версии для конфликтных копий.

**Independent Test**: vault с Syncthing → предупреждение о `.stignore` и исправление в одно действие; хранитель + автоснимки 30 с → снимок; второй профиль устройства → снимок объясняет, кто хранитель; конфликтная копия `vault.json` → автоснимки стоп.

- [X] T056 [US5] Проверка правил `.stignore` при открытии vault с историей и маркером Syncthing → `ignore_warnings` в `overview()` (FR-026), с учётом `stignore_declined` в `packages/core/src/sync/mod.rs`
- [X] T057 [US5] `keeper.rs`: `set_keeper`/`clear_keeper`, `last_seen_keeper`, спор хранителей (конфликтная копия `vault.json` или смена хранителя без ведома) → `KeeperDisputed`; гейт `KeeperElsewhere` с `confirm_not_keeper` для `snapshot`/`push`/`pull` в `packages/core/src/sync/keeper.rs` и `mod.rs`
- [X] T058 [US5] `autosnapshot::decide` (чистая функция: пауза, хранитель, спор, занятость Syncthing, конфликты получения, незавершённая операция) и `tick(now)` с учётом изменений от Syncthing как своих в `packages/core/src/sync/autosnapshot.rs`, `mod.rs`
- [X] T059 [US5] Зеркало не-хранителя: `clone_into(bare)`/`fetch` в каталог устройства (native — каталог данных приложения, web — OPFS), `HistoryState::Mirror`, `history`/`diff`/`restore` из зеркала (FR-033) в `packages/core/src/sync/mod.rs` и движках
- [X] T060 [US5] Третья версия для конфликтов Syncthing: база — версия файла в последнем снимке (локальная история или зеркало) и авто-совмещение `merge3` (FR-034) в `packages/core/src/sync/mod.rs`
- [X] T061 [P] [US5] Тесты US5 в `packages/core/tests/common/sync_cases.rs`: `decide` по всем причинам `Blocked` и переходам `Wait→Snapshot`, спор хранителей, гейт не-хранителя, предупреждение `.stignore` и отказ от правил, история из зеркала, три версии с базой
- [X] T062 [US5] Таймер `tick` в оболочках: `tokio::time` в `packages/shell/src/lib.rs`, `setTimeout` в `ui/src/sync/sync-client-web.ts` (только вызов ядра, без решений)
- [X] T063 [US5] Раздел «Хранитель» в `ui/src/sync/SyncPanel.tsx` (кто, когда последний снимок, «сделать хранителем», спор) и раздел настроек синхронизации (автоснимки, пауза, автоотправка, автополучение) `ui/src/sync/SyncSettingsSection.tsx`, подключённый в `ui/src/settings/SettingsPanel.tsx`; диалог подтверждения действия не-хранителя

## Phase 8: User Story 6 — Синхронизация в каждой оболочке (P3)

**Goal**: явная деградация по оболочкам.

**Independent Test**: один vault в трёх оболочках: недоступное видно и объяснено, доступное работает одинаково.

- [X] T064 [US6] `ShellCapabilities` по таргету (`ssh` — desktop, `direct_remote` — `CorsOnly` в web, `syncthing_status` — native) в `overview()` и отклонение недоступной схемы при `connect_remote` в `packages/core/src/sync/mod.rs`
- [X] T065 [US6] Отключённые действия с подсказками причин (FR-036) и предложение указать посредника при `CorsBlocked` в `ui/src/sync/SyncPanel.tsx`, `ui/src/sync/RemoteDialog.tsx`
- [X] T066 [P] [US6] Тесты возможностей оболочек и отказов `UnsupportedScheme`/`CorsBlocked` в `packages/core/tests/common/sync_cases.rs`

## Phase 9: Polish & Cross-Cutting

- [X] T067 [P] Бенчмарк статуса и обзора на 5 000 заметок (SC-002) в `packages/core/benches/sync_status.rs` и запись в `[[bench]]` `packages/core/Cargo.toml`
- [X] T068 Ревью безопасности (агент `security-review-agent`): хранение и утечки секретов, аргументы `ssh`, loopback-TOFU Syncthing, запись `.stignore`/`.gitignore`, пути из удалённого репозитория (`..`, абсолютные, `.git` внутри дерева) при `pull`/`restore`; находки исправить
- [X] T069 Отключение синхронизации оставляет приложение рабочим и не удаляет `.git`/файлы (FR-038): тест в `packages/core/tests/common/sync_cases.rs` и проверка по quickstart.md
- [ ] T070 Прогнать quickstart.md на копии `/home/apokryf/notes` (desktop и web), обновить `README.md` разделом о синхронизации, `**Status**: Implemented` в `specs/011-vault-sync/spec.md`

## Dependencies

- Setup (T001–T004) → Foundational (T005–T019) → истории.
- US1 (T020–T028) — MVP, нужен всем остальным (история существует).
- US2 зависит от US1. US3 зависит от US1 (снимки) и использует `merge3`.
- US4 независим от US1–US3 (конфликтные копии и состояние Syncthing без истории), кроме общего `MergeEditor` (T047) — при опережении US4 создаёт его в упрощённом виде (2 колонки).
- US5 зависит от US1, US3 (зеркало, удалённый) и US4 (Syncthing-клиент, конфликтные копии).
- US6 — после US3 и US4.

## Parallel Execution

- Phase 2: T008, T015, T017, T019 параллельно после T009–T010.
- US3: T035/T036 (транспорты) ‖ T040 (`merge3`) ‖ T038 (секреты); T043/T044 после реализации.
- US4: T049 ‖ T052; затем T050, T051.
- Ядро (`rust-core-agent`) и UI (`frontend-agent`) одной истории — параллельно после биндингов.

## Implementation Strategy

1. MVP — Setup + Foundational + US1: история и снимки в любом vault, включая `/home/apokryf/notes`, с правилами `.stignore`.
2. US2 → US3: откат и удалённая копия (паритет с typst.app).
3. US4 → US5: Syncthing и взаимодополнение.
4. US6 и Polish.

## Phase 10: Convergence

- [ ] T071 Вынести сетевую часть `pull`/`push`/`tick`/`refresh_mirror` из-под блокировки vault: fetch и push без `VaultBackend`, запись файлов — отдельным шагом под блокировкой, в `packages/core/src/sync/session.rs` и `packages/shell/src/sync.rs`; в web — не занимать очередь `wasm-call-queue` на время сети (`ui/src/sync/sync-client-web.ts`), чтобы сохранение заметок не ждало обмена per FR-020, SC-004, Constitution X (partial)
- [X] T072 Предел простоя сетевого обмена (20 с без данных, 30 с без прогресса — research §9) в `packages/core/src/sync/engine/transport_http.rs` вместо одного общего предела 600 с; в `packages/core/src/sync/engine/transport_ssh.rs` — тот же предел и завершение процесса `ssh` по `CancelToken`, а не только в `Drop`; тест «сервер принял соединение и молчит» в `packages/core/tests/sync_engine_native.rs` per FR-020, SC-004 (partial)
- [X] T073 Получение при локальном файле больше предела заметки (10 МБ): читать версии для `pull`/`resolve` без этого предела (отдельный метод `VaultBackend` или потоковое сравнение по хэшу) вместо отказа «локальный файл не читается» в `packages/core/src/sync/session.rs`; тест в `packages/core/tests/common/sync_cases.rs` per Edge Cases «огромные или двоичные файлы», FR-016 (partial)
- [ ] T074 Vault внутри чужого репозитория: либо реализовать список изменений и снимки, ограниченные путями vault (префикс из `RepoProbe::ForeignParent` в `packages/core/src/sync/engine/native.rs`, `session.rs`), либо через `/speckit-clarify` закрепить в spec.md нынешнее поведение «только показ, без записи» per Edge Cases «vault — часть более крупного репозитория», T021 (contradicts)
- [ ] T075 Живой прогресс массовых операций: доставлять `SyncEvent::Progress` во время первого снимка, получения и клонирования (отдельный канал событий, не ждущий блокировки сессии) и показывать «N из M» в `ui/src/sync/useSync.ts`, `SyncIndicator.tsx`, `EnableHistoryWizard.tsx`, `RemoteDialog.tsx` per Constitution X, contracts/core-sync-api.md «События» (partial)
- [ ] T076 Закрепление сертификата Syncthing внутри TLS-рукопожатия (свой проверяющий сертификат `rustls` со сравнением отпечатка) вместо сверки на отдельном запросе `/rest/noauth/health` в `packages/core/src/sync/syncthing.rs` per T068 п. 10, FR-014 (partial)
- [ ] T077 Хранилище секретов и память: не считать секрет удалённым при ошибке удаления из keyring, блокировка и защита от повреждённого файла секретов на Android в `packages/shell/src/sync.rs`; пределы размера блоба, тела отправки и `stderr` ssh с отказом, видимым пользователю, в `packages/core/src/sync/engine/native.rs`, `transport_ssh.rs` per T068 п. 13–14, FR-014 (partial)
- [ ] T078 Обновление состояния Syncthing по long-poll `/rest/events` с запасным опросом вместо опроса по такту в `packages/core/src/sync/syncthing.rs`, `packages/shell/src/sync.rs` per plan research §8, FR-022 (partial)
- [X] T079 При `HistoryState::Lost` и записанном адресе удалённого репозитория предлагать, кроме новой истории, загрузку истории из удалённого в `packages/core/src/sync/session.rs`, `ui/src/sync/SyncPanel.tsx` per Edge Cases «история удалена» (partial)

---

## Доделка 2026-10-09

**T073.** `VaultBackend::read_bytes_any_size`: чтение без предела размера заметки. Им читаются
локальные версии при получении и разрешении конфликтов (`read_local`, `read_bytes` в
`session.rs`); редактор и предпросмотр по-прежнему ограничены. Тест
`a_file_over_the_note_size_limit_is_pulled_like_any_other` на обоих таргетах.

**T079.** `SyncSession::restore_history`: пустая локальная история и обычное получение в неё.
Адрес обязан быть подтверждён на этом устройстве (T068) — проверяется до создания истории,
иначе отказ оставил бы пустую историю. Совпавшие файлы не трогаются, разошедшиеся становятся
конфликтами получения. Привязки `restoreHistory` / `sync_restore_history`; в панели — кнопка
«Загрузить историю из удалённого репозитория» при `Lost` и записанном адресе. Тест
`a_lost_history_is_restored_from_the_remote` на обоих таргетах. В окне и в браузере не
прогонялось: `FakeGitEngine` покрывает политику, а не isomorphic-git и не libgit2.

**T072 — выполнена для HTTP, для SSH вынесена в T080.** Общий предел передачи в 600 с заменён
пределом простоя `NETWORK_IDLE_LIMIT` = 20 с: у блокирующего клиента `reqwest` заданный
предел применяется к ожиданию ответа и к каждому чтению тела по отдельности. Для запроса с
телом (отправка) ответ ждётся дольше — соразмерно размеру тела при наименьшей скорости
16 КиБ/с: ход отправки изнутри не виден. Отдельного предела «30 с без прогресса» нет —
прогресс здесь и есть приход данных. Тест `a_silent_server_is_given_up_on_after_the_idle_limit`
(сервер принимает соединение и молчит) в `sync_engine_native.rs`.

- [ ] T080 Предел простоя и отмена для SSH-транспорта: чтение `stdout` процесса `ssh` в
      `packages/core/src/sync/engine/transport_ssh.rs` блокирующее, без предела, а
      `CancelToken` проверяется только перед чтением — зависший `ssh` не прерывается ни
      отменой, ни временем. Читать в отдельном потоке с ожиданием по такту, завершать
      процесс по отмене и по `idle_limit` из `TransportContext`; тест с подставным `ssh`
      per FR-020, SC-004 (вынесено из T072)

## Доделка 2026-10-19 (из замера 025 T029)

- [ ] T081 Такт и `overview` синхронизации на web не должны обходить весь vault: сейчас
      `list_conflict_copies` в `packages/core/src/vault/web.rs` на каждом такте и каждом
      `overview` идёт по всем хендлам OPFS, даже при выключенной истории. На vault из 5 000
      файлов такт при открытии — 2,5 с, дальше такт и `overview` — 0,4–1 с операции
      очереди вызовов wasm (025 research.md §15). Брать конфликтные копии из последнего
      полного обхода опроса (001 T136) — собирать их попутно, включая служебное место, — и
      обходить заново, только если опроса ещё не было; тест на обоих путях, что копия,
      появившаяся извне, видна в `overview` после ближайшего опроса per FR-024, 025 SC-001
