# Tasks: Вложения из буфера и перетаскиванием

**Input**: Design documents from `/specs/016-attachments-paste/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: обязательны для логики ядра на обоих таргетах (принципы I и II конституции):
общая таблица `packages/core/tests/common/attachment_cases.rs` и два раннера. Для UI
JS-тестов в репозитории нет — сценарий headless-браузера и quickstart.md.

**Зоны агентов**: `packages/core` — `rust-core-agent`; `packages/shell`, `apps/desktop`,
`apps/mobile` — `tauri-shell-agent`; `ui/` — `frontend-agent`. Нативных зависимостей нет,
`nix-env-agent` не нужен (research.md §2).

**Гейт принципа V**: правка текста идёт только через порт `InsertionTarget` фичи 003.
Реализации порта для режимов моста к Neovim в этой фиче не пишутся (plan.md → Complexity
Tracking).

## Phase 1: Setup

- [X] T001 Объявить `pub mod attachments;` в `packages/core/src/lib.rs` и создать `packages/core/src/attachments/mod.rs` с объявлениями подмодулей `kind`, `folder`, `name`, `markup`, `store`, `batch`, `web` (последний под `cfg(target_arch = "wasm32")`) и константой `LARGE_IMAGE_BYTES = 20 * 1024 * 1024`
- [X] T002 Создать `packages/core/tests/common/attachment_cases.rs`, объявить `pub mod attachment_cases;` в `packages/core/tests/common/mod.rs` и завести раннеры `packages/core/tests/attachments_native.rs` и `packages/core/tests/attachments_wasm.rs` по образцу `export_native.rs` / `export_wasm.rs`

## Phase 2: Foundational

**Purpose**: чистые правила и опоры в vault, без которых не работает ни одна история.

- [X] T003 Добавить `ImageFile { path, size }`, `IMAGE_EXTENSIONS` и `is_image_file(path)` (расширение без учёта регистра, вне служебного места) с модульными тестами в `packages/core/src/vault/mod.rs` (data-model.md «Вложение»)
- [X] T004 Добавить метод `list_images(&self, folder: Option<&Path>) -> Vec<ImageFile>` с умолчанием «пусто» и doc-комментарием из contracts/core-attachments-api.md в `packages/core/src/vault/backend.rs`
- [X] T005 Реализовать `list_images` в `packages/core/src/vault/native.rs`: `None` — обход по правилам `walk_fonts`, `Some(folder)` — файлы одной папки без вложенных, только метаданные
- [X] T006 Реализовать `list_images` в `packages/core/src/vault/web.rs` с тем же результатом для того же дерева (принцип II)
- [X] T007 Реализовать `list_images` у `MemoryVault` в `packages/core/tests/common/mod.rs`; при необходимости добавить двойнику режим «только чтение» и отказ записи по заданному пути — они нужны тестам пачки
- [X] T008 [P] Тест «`list_images` на настоящем бэкенде»: весь vault, одна папка, скрытые папки и служебное место пропущены, регистр расширения — в `packages/core/tests/vault_native.rs`
- [X] T009 [P] Тот же тест `list_images` для `WebBackend` в `packages/core/tests/vault_wasm.rs`
- [X] T010 [P] Реализовать `ImageKind`, `sniff`, `extension`, проверку принадлежности расширения виду с модульными тестами в `packages/core/src/attachments/kind.rs` (research.md §4, FR-006, FR-007)
- [X] T011 Добавить `attachments_placement: Option<AttachmentsPlacement>` и `attachments_folder: Option<String>` в `VaultOverrides`, таблицу `attachments` в `FileShape`/`render`/`parse`, вариант `VaultSettingsError::InvalidAttachmentsFolder { problem }` и проверку до записи в `packages/core/src/vault/vault_settings.rs`; обновить `sample()` и круговой тест (research.md §6, FR-017, FR-018)
- [X] T012 Сделать `relative_to` доступной как `pub(crate)` в `packages/core/src/boards/placement.rs` — её использует `attachments::folder::markup_path`; поведение и тесты 006 не меняются
- [X] T013 Реализовать `AttachmentsPlacement`, `AttachmentsFolderProblem`, `DEFAULT_ATTACHMENTS_FOLDER`, `validate_folder` (через `export::folder::validate_folder`), `attachments_folder`, `placement`, `target_folder`, `markup_path` с модульными тестами в `packages/core/src/attachments/folder.rs` (FR-016, FR-018, FR-019)
- [X] T014 [P] Реализовать `InsertMoment`, `generated_name`, `kept_name`, `free_name` с модульными тестами в `packages/core/src/attachments/name.rs` (research.md §5, §8, FR-009, FR-012)
- [X] T015 [P] Реализовать `image_markup` и `plan_markup` (результат — `templates::InsertionEdit`) с модульными тестами в `packages/core/src/attachments/markup.rs` (research.md §9, FR-001, FR-005)
- [X] T016 Общие случаи правил для обоих таргетов в `packages/core/tests/common/attachment_cases.rs` с раннерами в `packages/core/tests/attachments_native.rs` и `packages/core/tests/attachments_wasm.rs`: сигнатуры пяти видов и отказы (PDF, HEIC, пусто, текст), имя из заметки и момента, очистка имени и замена расширения, папка и путь для двух размещений и трёх глубин заметки, экранирование, `plan_markup` в точке и построчно, девять обязательств из contracts/core-attachments-api.md (FR-025)
- [X] T017 [P] Случаи настройки вложений в `packages/core/tests/common/vault_settings_cases.rs` (раннеры `placement_native.rs`, `placement_wasm.rs`): запись и чтение двух полей, негодное название отклонено и файл не тронут, негодное значение в файле отброшено, неизвестные ключи таблицы сохранены
- [X] T018 Читать изображения проекта без предела размера заметки: в `read_dependency` файла `packages/core/src/preview/resolve.rs` путь с `vault::is_image_file` идёт через `read_bytes_any_size`, остальное — как раньше (research.md §13, FR-008)
- [X] T019 [P] Случай «изображение больше 10 МБ собирается в предпросмотре, а `.typ` больше предела по-прежнему отклоняется» в `packages/core/tests/common/preview_cases.rs` с раннерами `preview_native.rs` и `preview_wasm.rs`
- [X] T020 Добавить `CommandId::InsertImage` (`insert-image`, `Mod+Alt+I`, «Вставить изображение…») в конец `COMMANDS` и обновить тесты уникальности умолчаний в `packages/core/src/settings/keybindings.rs` (research.md §14)

**Checkpoint**: правила проверены на обоих таргетах; можно писать пачку.

## Phase 3: User Story 1 — Вставить картинку из буфера обмена (P1) 🎯 MVP

**Goal**: вставка из буфера с картинкой без текста сохраняет файл в папку вложений и ставит `#image("…")` в место каретки одной отменяемой правкой.

**Independent Test**: скопировать снимок экрана, вставить в заметку — файл в `attachments/`, запись с путём в тексте, картинка в предпросмотре; вставка текста работает как раньше.

### Tests for User Story 1

- [X] T021 [P] [US1] Случаи пачки из одного элемента в `packages/core/tests/common/attachment_cases.rs` (те же раннеры): запись в пустую и в существующую папку, имя по правилу буфера, занятое имя даёт `-2` и прежний файл цел байт в байт (SC-003), то же содержимое в папке назначения даёт `Reused` без нового файла (FR-010, SC-004), неподдерживаемый вид даёт `Skipped` без записи, vault только на чтение даёт `Refused { ReadOnly }`, не заметка даёт `Refused { NotANote }`, отказ записи даёт `Failed` и в папке нет нового файла (FR-014), записанный файл — обычный файл vault вне служебного места, читается `read_bytes` байт в байт (FR-015), порядок шагов `store` из контракта
- [X] T022 [P] [US1] Случаи переходов в `packages/core/tests/common/attachment_cases.rs`: `step` выдаёт `Copy` по одному, `store` и `source_failed` для элемента не в `Copying` не меняют состояние, пустой список сразу `Finished`, `view` считает `processed`, итог и `markup_paths` в порядке элементов

### Implementation for User Story 1

- [X] T023 [US1] Реализовать типы пачки (`Origin`, `IncomingItem`, `BatchItem`, `ItemStatus`, `StoreProblem`, `BatchState`, `BatchPhase`, `RefusalReason`, `NextAction`, `BatchStep`, `BatchView`, `BatchSummary`) и чистые переходы `step`, `source_failed`, `cancel`, `view` в `packages/core/src/attachments/batch.rs` (data-model.md)
- [X] T024 [US1] Реализовать `store_item` — семь шагов из contracts/core-attachments-api.md: вид, имя, сопоставление по байтам в папке назначения, свободное имя, `ensure_folder`, `write_bytes` — в `packages/core/src/attachments/store.rs` (FR-009, FR-010, FR-014)
- [X] T025 [US1] Реализовать асинхронные `batch::start` (отказы, папка назначения, `known` из `list_images(Some(folder))`, статусы) и `batch::store` (вызов `store_item`, запись исхода, пополнение `known`) в `packages/core/src/attachments/batch.rs`
- [X] T026 [US1] wasm-биндинги `attachmentsBatchStart/Next/Store/SourceFailed/Cancel/View` и `attachmentsPlanMarkup` в `packages/core/src/attachments/web.rs`
- [X] T027 [US1] Команды Tauri `attachments_batch_start/next/source_failed/cancel/view`, `attachments_plan_markup` и `attachments_batch_store` с сырым телом (длина JSON, JSON, байты — research.md §11) с регистрацией в `packages/shell/src/lib.rs`; модульный тест разбора тела, включая короткое тело и неразбираемый JSON
- [X] T028 [US1] Объявить `attachments: AttachmentOps` в `ui/src/vault-client.ts` и реализовать в `ui/src/vault-client-web.ts` (через `wasm-call-queue`) по contracts/frontend-attachments.md
- [X] T029 [US1] Реализовать `attachments` в `ui/src/vault-client-tauri.ts`, включая сборку сырого тела для `attachments_batch_store`
- [X] T030 [P] [US1] Каталоги текстов `ui/src/i18n/locales/en/attachments.ts` (эталон) и `ui/src/i18n/locales/ru/attachments.ts`: отказы, итог, статус «вставлено: …», причины `StoreProblem` и `RefusalReason` — без хардкода в компонентах (FR-026); подключить в `ui/src/i18n/locales/en.ts` и `ui/src/i18n/locales/ru.ts`
- [X] T031 [US1] Добавить проп `extensions?: Extension[]` в `ui/src/editor/CodeEditor.tsx` (через `Compartment`, без пересоздания редактора)
- [X] T032 [US1] Расширение редактора `ui/src/attachments/editorExtension.ts`: обработчик `paste` с правилом «`text/plain` пуст и есть файлы» (research.md §3), порт `onFiles`; прочая вставка не трогается (FR-002)
- [X] T033 [US1] Хук `ui/src/attachments/useAttachments.ts` по contracts/frontend-attachments.md: `start`, цикл `next`/`store` с чтением `arrayBuffer` по одному файлу и уступкой циклу событий, `planMarkup` и одна правка через `InsertionTarget.apply`, отказ `Refused`, запрет второй пачки
- [X] T034 [P] [US1] Тексты исходов `ui/src/attachments/describe.ts` и панель отказа «Изображение не вставлено… Текст заметки не изменён» в `ui/src/attachments/AttachmentNotices.tsx` со стилями `.ts-attach-*` в `ui/src/styles/app.css` (макет 1b)
- [X] T035 [US1] Подключить в `ui/src/workspace/Workspace.tsx`: `useAttachments` с портом вставки и путём открытой заметки, `AttachmentNotices` в ряду с `ExportToast`
- [X] T036 [US1] Подключить в `ui/src/editor/NoteEditor.tsx`: расширение вложений только для заметки Typst, строка статус-бара «вставлено: <имя>» после одиночной вставки (макет 1a)

**Checkpoint**: US1 работает на web и desktop; quickstart, шаги 1–2 и шаг 11 в части вставки из буфера (пункт палитры — US2).

## Phase 4: User Story 2 — Перетащить файлы в редактор (P2)

**Goal**: несколько файлов перетаскиваются или выбираются командой, копируются с прогрессом и отменой и встают построчно в месте броска одной отменяемой правкой.

**Independent Test**: перетащить три картинки в середину заметки — три файла в папке вложений, три записи в месте броска в порядке перетаскивания, один `Ctrl+Z` убирает все.

### Tests for User Story 2

- [X] T037 [P] [US2] Случаи пачки из нескольких файлов в `packages/core/tests/common/attachment_cases.rs`: файл с тем же именем и содержимым в другой папке vault даёт `Reused` с путём через `../` (FR-011), два одинаковых файла в одной пачке дают один файл, смешанная пачка — поддерживаемые записаны, прочие `Skipped` и в vault не появились (FR-006), имя перетащенного файла сохранено (FR-012), отказ одного файла не останавливает остальные
- [X] T038 [P] [US2] Случаи подтверждения и отмены в `packages/core/tests/common/attachment_cases.rs`: файл ровно 20 МБ идёт без подтверждения, больше — `AwaitingConfirmation`; `step` выдаёт все `Confirm` раньше первого `Copy`; `confirm(false)` даёт `Declined` без записи; `confirm` не для ожидающего не меняет состояние; `cancel` при `Copying` оставляет текущий и отменяет остальные, записанные файлы целы и входят в `markup_paths` (FR-013, FR-023)

### Implementation for User Story 2

- [X] T039 [US2] Дополнить `batch::start` обходом `list_images(None)` при наличии `Origin::File` с отбором по имени и размеру и шаг 3 `store_item` (сопоставление по имени где угодно в vault) в `packages/core/src/attachments/store.rs` (research.md §7, FR-011)
- [X] T040 [US2] Реализовать `confirm` и статус `AwaitingConfirmation` в `start`/`step` в `packages/core/src/attachments/batch.rs` (FR-013)
- [X] T041 [US2] Биндинг `attachmentsBatchConfirm` в `packages/core/src/attachments/web.rs` и команда `attachments_batch_confirm` в `packages/shell/src/lib.rs`
- [X] T042 [US2] Метод `confirm` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts`
- [X] T043 [US2] Задать окну `"dragDropEnabled": false` в `apps/desktop/src-tauri/tauri.conf.json` (research.md §1)
- [X] T044 [US2] Перетаскивание в `ui/src/attachments/editorExtension.ts`: `dragenter`/`dragover`/`dragleave`/`drop`, `StateField` с декорацией индикатора (линия на границе строки, тег «N изображений — сюда, строка L», рамка редактора), точка вставки — начало строки под указателем, `trackPosition` с переносом позиции через правки, `onLinkDropped` для броска без файлов с `<img` (FR-003, research.md §9, §15)
- [X] T045 [US2] В `ui/src/attachments/useAttachments.ts`: построчная вставка (`block`), слежение за позицией во время копирования, `cancel`, ответ диалога через `confirm`, `sourceFailed` при отказе чтения файла, отмена при смене заметки или клиента с исходом «сохранены, но не вставлены» (research.md §10)
- [X] T046 [P] [US2] Диалог `ui/src/attachments/LargeImageDialog.tsx` по макету 1b: имя, размер через `formatBytes`, путь назначения, «Отмена» / «Вставить», `Escape`
- [X] T047 [US2] Панели прогресса и итога в `ui/src/attachments/AttachmentNotices.tsx` по макету 1b: «Копирую вложения: N из M» с hazard-полосой и «Отменить», «Вставлено N из M» с пометками «уже лежит в vault», «сохранён как …», «Пропущено — вид не поддерживается: …», отказами и отменёнными; сообщение о ссылке; показ прогресса при пачке больше одного файла либо дольше 400 мс
- [X] T048 [P] [US2] Выбор файлов `ui/src/attachments/pickFiles.ts`: скрытый `<input type="file">` для источников `files`, `gallery`, `camera` с атрибутами из research.md §2; отмена — пустой массив
- [X] T049 [US2] В `ui/src/workspace/Workspace.tsx`: команда `insert-image` — сочетание через `useShortcuts`, пункт палитры (только для открытой заметки Typst; в vault только на чтение — недоступен с причиной, FR-024), вызов `pickFiles("files")` и вставка к каретке
- [X] T050 [US2] Пункт «Изображение из файла…» в `ui/src/editor/InsertMenu.tsx` с вызовом той же команды (FR-004)
- [X] T051 [P] [US2] Дополнить `ui/src/i18n/locales/en/attachments.ts` и `ui/src/i18n/locales/ru/attachments.ts` текстами индикатора (с числом), прогресса, итога, диалога, ссылки; пункт палитры — в `ui/src/i18n/locales/en/palette.ts` и `ui/src/i18n/locales/ru/palette.ts`; пункт меню — в `ui/src/i18n/locales/en/insert.ts` и `ui/src/i18n/locales/ru/insert.ts`

**Checkpoint**: quickstart, шаги 3–9 и 13.

## Phase 5: User Story 3 — Решить, где лежат вложения (P2)

**Goal**: размещение и название папки вложений задаются в настройках vault; исключённая правилами папка даёт предупреждение.

**Independent Test**: выбрать общую папку `файлы`, вставить картинку в заметку из вложенной папки — файл в `файлы/` в корне, путь во вставке ведёт к нему; `../x` отклонено.

### Tests for User Story 3

- [X] T052 [P] [US3] Случаи сопоставления правил в `packages/core/tests/common/attachment_cases.rs`: таблица research.md §12 для `.gitignore` и `.stignore` — правило папки, якорь, `**`, отмена `!`, порядок «последнее/первое выигрывает», исключение через папку над ней, `(?i)`, `{a,b}`, комментарии; `start` заполняет `excluded_by` и это не мешает записи (FR-020)
- [X] T053 [P] [US3] Случаи размещения в `packages/core/tests/common/attachment_cases.rs`: общая папка для заметок трёх глубин, смена настройки между двумя вставками не трогает первый файл и его запись остаётся верной (FR-019), многочастное название папки

### Implementation for User Story 3

- [X] T054 [US3] Реализовать `excludes(file, text, path)` с переводом шаблонов в `regex` в `packages/core/src/sync/ignore_match.rs` и объявить модуль в `packages/core/src/sync/mod.rs`
- [X] T055 [US3] Реализовать `attachments::exclusion(backend, folder)` и заполнение `excluded_by` в `batch::start` в `packages/core/src/attachments/batch.rs`
- [X] T056 [US3] Биндинг `attachmentsValidateFolder` в `packages/core/src/attachments/web.rs`, команда `attachments_validate_folder` в `packages/shell/src/lib.rs`, метод `validateFolder` в `ui/src/vault-client.ts`, `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts`
- [X] T057 [US3] Раздел «Вложения» в `ui/src/settings/VaultSection.tsx` по макету 1c: две радиокнопки с примерами пути, поле названия, отказ `InvalidAttachmentsFolder` под полем с сохранением прежнего значения, строка о принадлежности vault; стили `.ts-attach-settings*` в `ui/src/styles/app.css`
- [X] T058 [US3] Панель предупреждения «Папка вложений не попадает в историю / не доставляется» с кнопкой «Открыть правила» в `ui/src/attachments/AttachmentNotices.tsx`; открытие `ui/src/sync/IgnoreEditor.tsx` на нужном файле — через `ui/src/workspace/Workspace.tsx`
- [X] T059 [P] [US3] Тексты раздела и причин `AttachmentsFolderProblem` в `ui/src/i18n/locales/en/settings.ts` и `ui/src/i18n/locales/ru/settings.ts`; тексты предупреждения — в `ui/src/i18n/locales/en/attachments.ts` и `ui/src/i18n/locales/ru/attachments.ts`

**Checkpoint**: quickstart, шаги 10, 12, 14.

## Phase 6: User Story 4 — Снимок с камеры на телефоне (P3)

**Goal**: на Android действия «Снять фото», «Выбрать из галереи», «Выбрать файл…» дают вложение и запись в заметке.

**Independent Test**: на устройстве Android «Снять фото» → файл в папке вложений и запись в тексте; на web и desktop этих действий нет.

### Tests for User Story 4

- [X] T060 [P] [US4] Случаи источника «камера» в `packages/core/tests/common/attachment_cases.rs`: имя по правилу буфера независимо от имени временного файла, JPEG определяется по содержимому, пустой выбор (отмена съёмки) — пачка из нуля элементов сразу `Finished` и `markup_paths` пуст (FR-021, FR-022)

### Implementation for User Story 4

- [X] T061 [US4] Проп `imageSources?: { camera: boolean; gallery: boolean }` в `ui/src/workspace/Workspace.tsx` (умолчание — оба `false`) и передача его действиям вставки
- [X] T062 [US4] Шторка `ui/src/attachments/InsertImageSheet.tsx` по макету 1c: «Снять фото», «Выбрать из галереи», «Выбрать файл…» — только доступные по `imageSources`; вызов `pickFiles` с источником и `Origin::Camera` для снимка; стили `.ts-attach-sheet*` в `ui/src/styles/app.css`
- [X] T063 [US4] Передать `imageSources` из `apps/mobile/src/MobileWorkspace.tsx`: `gallery: true`, `camera` — по наличию `videoinput` в `navigator.mediaDevices.enumerateDevices()` (research.md §2)
- [X] T064 [P] [US4] Тексты шторки в `ui/src/i18n/locales/en/attachments.ts` и `ui/src/i18n/locales/ru/attachments.ts`
- [ ] T065 [US4] Проверить на устройстве или эмуляторе Android шаги 15–17 из quickstart.md, включая съёмку без разрешения `CAMERA` (запрос разрешения не появляется, отмена съёмки не меняет текст — FR-022) и сырое тело команды; при отсутствии устройства записать «не проверено» в раздел результатов `specs/016-attachments-paste/tasks.md`, а не отмечать пройденным

**Checkpoint**: quickstart, шаги 15–17.

## Phase 7: Polish

- [X] T066 Сценарий `scripts/dev/scenarios/attachments.mjs` по образцу `scripts/dev/scenarios/rules-editor.mjs`: синтетические `paste` и `drop` с файлами, проверка файлов в OPFS и текста заметки, смешанная пачка, настройки vault, бросок ссылки без файла — текст не изменён и сетевых запросов нет (research.md §15)
- [ ] T067 Сверка вёрстки с макетом `Typsidian Вложения.dc.html` (экраны 1a–1c) снимками `scripts/dev/browser.mjs … --opfs-vault --shot`; расхождения сверх research.md §16 исправить в `ui/src/styles/app.css`
- [X] T068 Гейты: сборка ядра под wasm32 и native, `cargo test -p typsidian-core`, wasm-тесты `attachments_wasm`, `vault_wasm`, `placement_wasm`, `preview_wasm`, `cargo test -p typsidian-shell`, `pnpm -r typecheck`, `scripts/check-i18n.sh`, `pnpm build` — по `specs/016-attachments-paste/quickstart.md`
- [ ] T069 Ручная проверка по `specs/016-attachments-paste/quickstart.md` на web и в desktop-сборке (шаги 1–14), включая `drop` и вставку картинки в WebKitGTK (research.md §17), замер SC-001 (снимок 5 МБ) и SC-005 (50 файлов, отмена ≤1 с), проверку SC-006 в другой оболочке (шаг 14); результаты и «не проверено» — в конец `specs/016-attachments-paste/tasks.md`

## Dependencies

- Setup → Foundational → US1 → US2; US3 и US4 зависят от US1.
- T011 и T012 раньше T013 (поля настроек, `relative_to`); T003 раньше T004–T007
  и T018; T007 раньше любых случаев в `attachment_cases.rs`.
- US2 расширяет `batch.rs`, `store.rs`, `editorExtension.ts`, `useAttachments.ts`,
  `AttachmentNotices.tsx` из US1 — те же файлы, строго после US1.
- US3: T054 → T055; T057 зависит только от T011, T013 и T056 и может идти параллельно с US2.
- US4 зависит от T048 (`pickFiles`) и T049 (команда) из US2.
- Задачи `[P]` с общим файлом `attachment_cases.rs` (T021, T022, T037, T038, T052, T053,
  T060) параллельны реализации, но не друг другу.

**Соседние фичи**: 014 (вкладки) реализуется раньше и меняет `Workspace.tsx` — `useAttachments`
берёт путь заметки и порт вставки активной вкладки; закрытие вкладки во время копирования —
случай «заметка закрыта» (research.md §10). 012 — редактор правил для T058. 003 — порт
вставки. 009 — атомарная `write_bytes` и ряд уведомлений.

## Parallel Example: User Story 1

```text
T021, T022 (случаи ядра)  ‖  T030 (тексты)  ‖  T034 (панель отказа)
после T023–T025: T026 (wasm) ‖ T027 (Tauri)
```

## Implementation Strategy

MVP — фазы 1–3: вставка из буфера на web и desktop. Дальше перетаскивание и команда (US2),
настройка и предупреждение (US3), телефон (US4). Коммит — фича целиком (конвенции
репозитория).

## Результаты реализации (2026-10-19)

- Ядро: `packages/core/src/attachments/` (вид, папка, имя, запись, пачка, web), `sync/ignore_match.rs`,
  `VaultBackend::list_images` на обоих бэкендах, таблица `attachments` в `vault.json`, изображения в
  предпросмотре без предела размера, `CommandId::InsertImage`. Общая таблица
  `tests/common/attachment_cases.rs` — 26 случаев, проходит на native и wasm32.
- Типы пачки в TS — с приставкой: `AttachmentBatchState`, `AttachmentBatchStep`, `AttachmentBatchView`,
  `AttachmentBatchPhase`, `AttachmentBatchSummary`, `AttachmentRefusalReason`. У экспорта (009) есть
  одноимённые `BatchState` и прочие, а `tsify` не учитывает `serde(rename)` контейнера — два
  `interface BatchState` в `.d.ts` молча слились бы.
- Сценарий `scripts/dev/scenarios/attachments.mjs` (web, dev-сборка) проходит: вставка из буфера,
  отмена одним `Ctrl+Z`, буфер с текстом, бросок трёх файлов с PDF, повторный бросок того же файла,
  бросок ссылки. Картинки видны в предпросмотре.
- Расхождение с контрактом UI: у палитры нет «недоступных» пунктов, поэтому в vault только на
  чтение пункт «Вставить изображение…» показан обычным, а пачка отвечает отказом «Vault открыт
  только на чтение» — причина названа, текст не изменён (FR-024 выполнен).
- Смена вкладки во время копирования уничтожает её редактор: такая пачка, как и закрытие заметки,
  заканчивается «сохранены, но не вставлены» (research.md §10).
- Не проверено: сверка снимков с макетом `Typsidian Вложения.dc.html` (T067) — файла макета в
  репозитории нет, а читать проект Claude Design в этой сессии нечем; вёрстка сделана по описанию
  экранов в `tasks.md` и research.md §16. Desktop (WebKitGTK, T069) и Android (T065) не прогонялись.
