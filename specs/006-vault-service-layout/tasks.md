---

description: "Task list for Vault Service Layout"
---

# Tasks: Vault Service Layout

**Input**: Design documents from `/specs/006-vault-service-layout/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md),
[data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: включены обязательно. Принцип I конституции требует, чтобы каждая единица логики
ядра проходила и `cargo test`, и `wasm-bindgen-test`; логика без тестов на обоих путях не
считается готовой. Это гейт проекта, а не выбор режима TDD.

**Organization**: задачи сгруппированы по user story, чтобы каждую можно было реализовать и
проверить независимо.

> **Про Phase 2.** Фаза тяжелее обычного, и это не разрастание скоупа. Политика расположения
> ассетов принадлежит vault (FR-002), то есть живёт в `.typsidian/vault.json`. Значит хранилище
> настроек уровня vault блокирует US1 так же, как US2 и US4, и обязано быть в Foundational, а
> не внутри одной из историй.

> **Ревизия после `/speckit-analyze` (2026-09-07).** Файл перенумерован целиком: на момент
> правки ни одна задача не была закрыта, поэтому история отметок не пострадала. Закрыты находки
> F1, F2, U1, C1, C2, C3, O1, L1 — см. раздел «Что изменил анализ» в конце файла.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: можно выполнять параллельно (разные файлы, нет незакрытых зависимостей)
- **[Story]**: к какой user story относится задача (US1…US5)
- В описании — точный путь к файлу

## Path Conventions

Структура из `plan.md` → Project Structure: ядро в `packages/core/src/`, тесты ядра в
`packages/core/tests/`, общий UI в `ui/src/`, native-биндинг в
`apps/desktop/src-tauri/src/lib.rs`, web-клиент в `apps/web/`.

**Цепочка границы обязательна для каждой операции ядра**: ядро → `#[tauri::command]` +
`#[wasm_bindgen]` → `vault-client.ts` / `tauriVaultClient.ts` → компонент. Операция, дошедшая
только до ядра, считается несделанной.

---

## Phase 1: Setup

- [X] T001 [P] Создать скелеты модулей ядра `packages/core/src/vault/service_dir.rs`,
      `packages/core/src/vault/vault_settings.rs`, `packages/core/src/vault/sections.rs` и
      подключить их в `packages/core/src/vault/mod.rs`
- [X] T002 [P] Создать скелет `packages/core/src/boards/placement.rs` и подключить его в
      `packages/core/src/boards/mod.rs`
- [X] T003 [P] Создать пары наборов тестов `packages/core/tests/placement_native.rs` и
      `placement_wasm.rs` — по образцу существующих `boards_native.rs` / `boards_wasm.rs`
      - **Отступление от исходной формулировки, принятое при реализации.** Отдельные
        `vault_settings_native.rs` / `vault_settings_wasm.rs` не заводились: их проверки
        живут в `packages/core/tests/common/vault_settings_cases.rs` и подключены в обе
        стороны пары `placement_*`. Два файла на одну общую таблицу были бы двумя копиями
        ожиданий, расходящимися при первой правке контракта.
      - `packages_native.rs` / `packages_wasm.rs` остаются за US5 (Phase 7).
- [X] T004 [P] Вынести общие таблицы проверок фичи в
      `packages/core/tests/common/placement_cases.rs` и
      `packages/core/tests/common/vault_settings_cases.rs`, подключив их в
      `packages/core/tests/common/mod.rs`
      - Отдельный фикстур «vault со служебной папкой» не понадобился: уже имевшийся
        `common::MemoryVault` покрывает эту роль на обоих таргетах.

---

## Phase 2: Foundational (блокирует все истории)

**Цель**: классификатор знает про служебное место, а настройки уровня vault читаются и
пишутся. До закрытия фазы ни одна история не проверяема.

### Классификация

- [X] T005 Добавить `FileKind::Service` в `packages/core/src/vault/mod.rs`, вернуть для него
      `is_listed() == false` и объявить `SERVICE_DIR_NAME = ".typsidian"`
- [X] T006 Расширить `classify()` в `packages/core/src/vault/mod.rs`: путь внутри
      `.typsidian/` → `Service`, **проверка идёт первой** (data-model.md → FileKind); правило
      снимка становится дизъюнкцией «внутри `*.assets`» ∨ «внутри `…/assets/<любая>/` и имя
      вида `board-<hex>.svg`» — research.md §5
- [X] T007 [P] Тесты классификатора в общей таблице `packages/core/tests/common/placement_cases.rs`,
      подключённой в обе стороны пары `placement_native.rs` / `placement_wasm.rs`: `.typsidian/vault.json` → `Service`; `Идея.assets/board-a1.svg` →
      `Snapshot` (прежнее правило не сломано, FR-009); `assets/Идея/board-a1.svg` →
      `Snapshot`; `assets/Идея/моя-картинка.svg` → **не** `Snapshot`;
      `.typsidian/assets/x/board-a1.svg` → `Service`, а не `Snapshot`

### Раскладка служебного места

- [X] T008 Реализовать `packages/core/src/vault/service_dir.rs`: `service_dir()`,
      `vault_settings_path()`, `cache_path()`, `packages_dir()`, `is_inside_service_dir()` —
      все пути относительны корню vault (contracts/vault-settings.md)
- [X] T009 [P] Тесты раскладки в `packages/core/src/vault/service_dir.rs` и в общей таблице
      `packages/core/tests/common/vault_settings_cases.rs`: пути совпадают на обоих таргетах, ни один не абсолютен

### Настройки уровня vault

- [X] T010 Объявить `AssetsPlacement`, `SectionMark`, `VaultOverrides` в
      `packages/core/src/vault/vault_settings.rs` с `Tsify` для границы WASM (data-model.md);
      `AssetsPlacement::default()` — `NextToNote` (FR-003); `editor_font_size` валидируется
      существующей `settings::is_valid_font_size`
- [X] T011 Реализовать разбор и сборку `vault.json` в
      `packages/core/src/vault/vault_settings.rs`, переиспользуя приём записи из
      `packages/core/src/settings/format.rs` — разбор через `serde_json::Value`, чтобы чужие
      ключи уцелели (contracts/vault-settings.md → «Формат»). Переноса с прежнего `vault.toml`
      нет — по решению владельца проекта (004/research.md §2)
- [X] T012 Реализовать `read_vault_settings()` и `write_vault_settings()` **функциями поверх
      `VaultBackend`** в `packages/core/src/vault/vault_settings.rs`; третьей реализации
      трейта не заводить (research.md §3)
- [X] T013 Реализовать создание `.typsidian/` через `ensure_folder` **при открытии vault,
      доступного на запись**, в `packages/core/src/vault/native.rs` и
      `packages/core/src/vault/web.rs` — одинаково на обоих таргетах, включая web, где кэша в
      файле нет (research.md §12). Vault только на чтение папки не получает нигде (FR-025)
- [X] T014 Реализовать обращение с испорченным `vault.json` в
      `packages/core/src/vault/vault_settings.rs`: не перезаписывать, отложить рядом через
      `settings::store::invalid_file_path`, вернуть `VaultSettingsError::Unreadable` (FR-034)
- [X] T015 [P] Тесты хранилища в `packages/core/tests/common/vault_settings_cases.rs`,
      подключённой в обе стороны пары `placement_native.rs` / `placement_wasm.rs`: отсутствующий файл → `VaultOverrides::default()` без ошибки и
      с политикой `NextToNote` (FR-003, FR-023); круговой рейс значений; дописанное
      пользователем руками переживает запись; открытие vault, доступного на запись, создаёт
      `.typsidian/` **на обоих таргетах** (research.md §12); vault без прав на запись →
      `NotWritable` и папка **не** создана (FR-025); испорченный файл не перезаписан и отложен
      рядом

### Граница

- [X] T016 [P] Добавить `#[tauri::command] vault_settings` и `set_vault_settings` в
      `apps/desktop/src-tauri/src/lib.rs` и зарегистрировать их в `invoke_handler`
- [X] T017 [P] Добавить `#[wasm_bindgen(js_name = vaultSettings)]` и `setVaultSettings` в
      `packages/core/src/vault/web.rs`
- [X] T018 Добавить `vaultSettings()` и `setVaultSettings()` в интерфейс
      `ui/src/vault-client.ts` и в обе реализации — `ui/src/vault-client-web.ts` и
      `apps/desktop/src/tauriVaultClient.ts`

**Checkpoint**: `pnpm build:wasm && pnpm -C ui build && pnpm typecheck` зелёные, `cargo test`
и `cargo test --target wasm32-unknown-unknown` проходят. Дальше истории независимы.

---

## Phase 3: User Story 1 — вставки не засоряют папку с заметками (P1) 🎯 MVP

**Цель**: пользователь выбирает корень для файлов вставок; раскладка внутри корня единая.

**Независимая проверка**: выбрать «в корне папки заметок», вставить доску в заметку из
вложенной папки — рядом с заметкой не создано ничего, файл лежит в `assets/<имя>/`, заметка
показывает доску (quickstart, сценарий 1).

### Ядро

- [X] T019 [US1] Реализовать `assets_root()`, `snapshot_path()`, `markup_path()` в
      `packages/core/src/boards/placement.rs` по contracts/asset-placement.md; функции
      **чистые**, условная компиляция в модуле запрещена
- [X] T020 [P] [US1] Таблица из contracts/asset-placement.md как тесты в
      `packages/core/tests/placement_native.rs` и `placement_wasm.rs` — все восемь строк
      `snapshot_path` и три строки `markup_path`, включая `Работа-архив` против раздела
      `Работа` (префикс по компонентам, не по строке) и отдельную строку на умолчание
      `NextToNote` для vault без выбора (FR-003)
- [X] T021 [US1] Перевести `plan_embed()` и `snapshot_path()` в
      `packages/core/src/boards/snapshot.rs` на вызовы `placement.rs`; сигнатуры принимают
      политику и список разделов. Прежняя `assets_folder_name()` остаётся — её использует
      распознавание старых вставок (FR-009)
- [X] T022 [US1] Расширить `embed_id_in()` в `packages/core/src/boards/embed.rs`: адрес
      сверяется с расчётным путём снимка для этой заметки **и** прежней веткой `*.assets`;
      `embeds_of()` получает политику и разделы (research.md §7)
- [X] T023 [P] [US1] Тесты распознавания в общей таблице `packages/core/tests/common/placement_cases.rs`,
      подключённой в обе стороны пары `placement_native.rs` / `placement_wasm.rs`: старая вставка распознаётся при любой политике (FR-009); новая
      распознаётся; заметка с обеими вставками отдаёт обе (FR-010); `#image("assets/Идея/
      моя-картинка.svg")` вставкой не считается
- [X] T024 [US1] Обновить проверку пути в `write_snapshot()`
      (`packages/core/src/boards/snapshot.rs`): новая раскладка проходит,
      `SnapshotWriteError::NotASnapshotPath` продолжает отсекать чужие пути

### Граница

- [X] T025 [P] [US1] Обновить `#[tauri::command] plan_embed`, `embeds_of`, `write_snapshot` в
      `apps/desktop/src-tauri/src/lib.rs` под новые сигнатуры
- [X] T026 [P] [US1] Обновить `#[wasm_bindgen]`-обёртки `planEmbed`, `embedsOf`,
      `writeSnapshot` в `packages/core/src/vault/web.rs`
- [X] T027 [US1] Провести изменения через `ui/src/vault-client.ts`,
      `ui/src/vault-client-web.ts` и `apps/desktop/src/tauriVaultClient.ts`

### UI

- [X] T028 [US1] Добавить выбор варианта расположения в `ui/src/settings/VaultSection.tsx`:
      три варианта, значение читается и пишется через `vaultSettings()`/`setVaultSettings()`
- [X] T029 [US1] Провести политику и разделы до места вставки в
      `ui/src/boards/InsertBoardDialog.tsx` и `ui/src/boards/useBoardDocument.ts`; расчёт пути
      **не воспроизводить в TS** — только вызов ядра (принцип I)
- [ ] T030 [US1] Пройти сценарий 1 из `specs/006-vault-service-layout/quickstart.md` на
      `~/media/notes`, включая шаг 5: обновление вставки, созданной при другой политике,
      переписывает файл **на его текущем месте**, а не создаёт копию по новому правилу
      (FR-008)

**Checkpoint**: US1 работает и проверяема отдельно от остальных историй. Это MVP.

---

## Phase 4: User Story 2 — пометка папки как раздела (P2)

**Цель**: пользователь помечает папку разделом из дерева; пометка принадлежит vault.

**Независимая проверка**: пометить папку, перезапустить приложение — пометка на месте; снять —
исчезла (quickstart, сценарий 2).

### Ядро

- [X] T031 [US2] Реализовать `nearest_section()`, `rename_section()`, `prune_missing()` в
      `packages/core/src/vault/sections.rs` по contracts/vault-settings.md → «Разделы»
- [X] T032 [P] [US2] Тесты разделов в `packages/core/tests/vault_settings_native.rs` и
      `vault_settings_wasm.rs`: ближайший предок при вложенных пометках (FR-016); сравнение по
      компонентам, а не по строке; помеченный корень vault; снятие пометки не меняет ни одного
      уже рассчитанного пути к существующим файлам вставок (FR-017); `prune_missing` только
      сообщает и ничего не удаляет (FR-019)
- [X] T033 [US2] Связать `VaultBackend::rename` с переносом пометки в
      `packages/core/src/vault/native.rs` и `packages/core/src/vault/web.rs` через
      `sections::rename_section` из `packages/core/src/vault/sections.rs`: переименование папки
      средствами приложения обновляет запись и все вложенные пометки под изменённым префиксом
      (FR-018). Розыска по новому расположению не добавлять
- [X] T034 [P] [US2] Тест переноса пометки при переименовании и её потери при внешнем
      переименовании в `packages/core/tests/vault_native.rs` и `vault_wasm.rs`

### Граница

- [X] T035 [P] [US2] `#[tauri::command] mark_section` и `unmark_section` в
      `apps/desktop/src-tauri/src/lib.rs`
- [X] T036 [P] [US2] `#[wasm_bindgen(js_name = markSection)]` и `unmarkSection` в
      `packages/core/src/vault/web.rs`
- [X] T037 [US2] `markSection()` / `unmarkSection()` в `ui/src/vault-client.ts` и обеих
      реализациях — `ui/src/vault-client-web.ts` и `apps/desktop/src/tauriVaultClient.ts`

### UI

- [X] T038 [US2] Пункты «Пометить как раздел» / «Снять пометку» в
      `ui/src/vault-tree/VaultTreeActions.tsx`
- [X] T039 [P] [US2] Индикация помеченной папки в `ui/src/vault-tree/VaultTree.tsx` (FR-014);
      правила стилей в `ui/src/styles/app.css` начинаются с `.ts-*`, чтобы не залетать в чужой
      DOM
- [X] T040 [US2] Сообщение об утративших адресата пометках при открытии vault в
      `ui/src/vault-tree/ErrorBanner.tsx` (FR-019)
- [ ] T041 [US2] Пройти сценарий 2 из `specs/006-vault-service-layout/quickstart.md`, включая
      шаг со снятием пометки: уже созданные файлы вставок остаются на местах (FR-017)

---

## Phase 5: User Story 3 — служебное состояние в одном месте (P2)

**Цель**: в корне vault не остаётся служебных файлов приложения, кроме `.typsidian/`.

**Независимая проверка**: удалить `.typsidian/` целиком — vault открывается, дерево то же, всё
на месте (quickstart, сценарий 3).

### Ядро

- [X] T042 [US3] Перевести путь кэша на `service_dir::cache_path()` в
      `packages/core/src/vault/native.rs` (сейчас `cache_db_path()` возвращает
      `.typsidian-cache.sqlite` в корне, строка 324); папка к этому моменту уже создана
      задачей T013
- [X] T043 [US3] Реализовать уборку наследия в `packages/core/src/vault/native.rs`: файл
      `.typsidian-cache.sqlite` в корне удаляется при открытии vault, кэш строится заново
      (FR-021, research.md §11). Отказ удаления не блокирует открытие
- [X] T044 [P] [US3] Тест уборки в `packages/core/tests/vault_native.rs`: файл прежней версии
      исчезает, ни одна заметка не потеряна. Обновить существующую проверку на строке ~210,
      которая исключала `.typsidian-cache.sqlite` из сверки
- [X] T045 [US3] Добавить вариант события «настройки уровня папки утрачены» в
      `packages/core/src/vault/events.rs` и взводить его из
      `packages/core/src/vault/vault_settings.rs` **только когда vault уже есть в
      `Settings::known_vaults` этого устройства**, а служебного места в нём нет (FR-024).
      Для папки, открываемой впервые, событие не взводится — нового состояния ради этого
      признака не заводить
- [X] T046 [US3] Обеспечить поведение read-only vault в `packages/core/src/vault/native.rs` и
      `packages/core/src/vault/web.rs`: служебное место не создаётся, причина названа,
      приложение работает на чтение (FR-025); кэш при этом остаётся
      `vault::cache::MemoryCache`

### UI

- [X] T047 [US3] Показать сообщение об утрате настроек уровня vault и локальных шаблонов в
      `ui/src/vault-tree/AccessStateBanner.tsx` (FR-024)
- [X] T048 [US3] Показать причину недоступности настроек для vault только на чтение в
      `ui/src/vault-tree/AccessStateBanner.tsx` (FR-025)
- [ ] T049 [US3] Пройти сценарий 3 из `specs/006-vault-service-layout/quickstart.md`, включая
      сверку с `/tmp/before.txt` (SC-007), проверку, что папка `.typsidian/preview/` **не**
      создана, и проверку, что при первом открытии новой папки сообщения об утрате настроек
      не появляется (FR-024)

---

## Phase 6: User Story 4 — настройки едут вместе с папкой (P3)

**Цель**: тема, шрифт, режим и путь к конфигу принадлежат vault; устройство — слой по
умолчанию.

**Независимая проверка**: сменить тему при открытом vault, открыть другой vault, вернуться —
тема первого сохранена (quickstart, сценарий 4).

### Ядро

- [X] T050 [US4] Реализовать `resolve(&Settings, &VaultOverrides) -> Settings` в
      `packages/core/src/settings/mod.rs` — чистая функция, поле за полем
- [X] T051 [P] [US4] Тесты разрешения слоёв в `packages/core/tests/settings_native.rs` и
      `settings_wasm.rs`: `None` пропускает значение устройства; `Some` перекрывает; список
      известных vault перекрытию не подлежит и в `VaultOverrides` отсутствует (FR-028)
- [X] T052 [US4] Реализовать маршрутизацию записи в `packages/core/src/settings/apply.rs`:
      изменение при открытом vault идёт в `vault.json`, «сделать по умолчанию» — в
      `settings.json` устройства; **в оба хранилища одно действие не пишет никогда**
      (FR-029, FR-031)
- [X] T053 [P] [US4] Тест маршрутизации в `packages/core/tests/settings_native.rs` и
      `settings_wasm.rs`: содержимое `settings.json` устройства не меняется при смене темы с
      открытым vault (FR-029); vault не открыт → запись идёт в устройство
- [X] T054 [US4] Реализовать поведение при несуществующем на этом устройстве
      `editor_config_path` в `packages/core/src/settings/editor_mode.rs`: причина названа,
      режим не переключается молча, редактирование остаётся доступным (FR-027).
      Переиспользовать третье состояние режима, введённое фичей 004, а не заводить новое
- [X] T055 [P] [US4] Тест сохранности значений устройства при обновлении в
      `packages/core/tests/settings_native.rs` и `settings_wasm.rs`: существующий
      `settings.json`, написанный прежней версией, читается новой без потери **ни одного**
      ключа и становится слоем по умолчанию (FR-032, SC-005). Отдельного кода миграции нет —
      тест закрепляет, что он и не нужен (research.md §4)
- [X] T056 [US4] Тест немедленности применения в `packages/core/tests/settings_native.rs` и
      `settings_wasm.rs`: изменение через панель действует сразу для обоих хранилищ (FR-033);
      внешняя правка `vault.json` при работающем приложении **не** подхватывается до
      следующего открытия vault, и это закреплённое поведение, а не дефект (FR-040)

### Граница

- [~] T057 [P] [US4] **Неприменимо на этом клиенте.** Команда Tauri для
      `apps/desktop/src-tauri/src/lib.rs` не заводится: у рабочего стола нет
      собственного хранилища настроек вовсе — панель настроек там не
      показывается по причине, записанной ещё фичей 004 (см. комментарий к
      пропу `settings` в `ui/src/workspace/Workspace.tsx`). Писать команду
      против хранилища, которого нет, значило бы завести мёртвый код.
      Задача открывается вместе с desktop-хранилищем настроек
- [X] T058 [P] [US4] `#[wasm_bindgen(js_name = makeSettingsDefault)]` в
      `packages/core/src/settings/web.rs`
- [X] T059 [US4] Провести операцию через `ui/src/settings/useSettings.ts`

### UI

- [X] T060 [US4] Перевести `ui/src/settings/AppearanceSection.tsx` на запись в vault и
      добавить действие «сделать значениями по умолчанию» (FR-031)
- [X] T061 [P] [US4] Отразить состояние «путь к конфигу не найден на этом устройстве» в
      `ui/src/settings/KeybindingsSection.tsx` (FR-027)
- [ ] T062 [US4] Пройти сценарий 4 из `specs/006-vault-service-layout/quickstart.md`

---

## Phase 7: User Story 5 — внешние шаблоны Typst (P3)

**Цель**: заметка с `#import "@preview/…"` собирается, если шаблон лежит локально; иначе
причина названа поимённо.

**Независимая проверка**: положить шаблон, открыть заметку — собралось; убрать — диагностика
называет имя и версию (quickstart, сценарий 5).

### Ядро

- [X] T063 [US5] Реализовать `package_path(spec, vpath)` в
      `packages/core/src/preview/resolve.rs` по contracts/package-resolve.md; раскладка
      `<namespace>/<name>/<major.minor.patch>/`
- [X] T064 [US5] Добавить `ResolveError::PackageMissing` и `PackageVersionMismatch` в
      `packages/core/src/preview/resolve.rs`; текст спецификации брать у
      `Display for PackageSpec`, вручную не собирать
- [X] T065 [US5] Снять отказ для `VirtualRoot::Package` в `vault_path()`/`read_dependency()`
      (`packages/core/src/preview/resolve.rs:58`), различив «нет пакета» и «нет версии» по
      наличию папки `<namespace>/<name>/`
- [X] T066 [US5] Расширить `describe()` в `packages/core/src/preview/resolve.rs` двумя
      формулировками, называющими шаблон (contracts/package-resolve.md → «Диагностика»)
- [X] T067 [P] [US5] Таблица «Проверяемые сценарии» из contracts/package-resolve.md как тесты
      в `packages/core/tests/packages_native.rs` и `packages_wasm.rs`, включая namespace
      `@local` и заметку без подключений при отсутствующей папке пакетов (FR-039)
- [X] T068 [US5] Провести новые причины отказа до диагностики предпросмотра в
      `packages/core/src/preview/compile.rs`, не схлопывая их в общее «не удалось собрать»

### UI

- [X] T069 [US5] Показать причину отсутствия шаблона в `ui/src/preview/PreviewPane.tsx`
      (FR-037, FR-038)
- [ ] T070 [US5] Пройти сценарий 5 из `specs/006-vault-service-layout/quickstart.md`, включая
      проверку офлайна

---

## Phase 8: Polish и сквозные проверки

- [ ] T071 [P] Пройти сценарий 6 из `specs/006-vault-service-layout/quickstart.md` —
      совместимость со старыми вставками на реальных `*.assets` в `~/media/notes`
      (FR-009, FR-010, SC-004)
- [ ] T072 [P] Проверить SC-007 и SC-008 по разделу «Сценарий 3» в
      `specs/006-vault-service-layout/quickstart.md`: после всех сценариев в корне
      `~/media/notes` нет ничего созданного приложением, кроме `.typsidian/` и одной папки
      ассетов; в папке с заметками не более одной созданной папки
- [X] T073 Гейт принципа II: `cargo test -p typsidian-core`,
      `cargo test -p typsidian-core --target wasm32-unknown-unknown`,
      `cargo build -p typsidian-core --target wasm32-unknown-unknown` — все зелёные
- [X] T074 Гейт принципа IV в правильном порядке: `pnpm build:wasm`, затем `pnpm -C ui build`,
      затем `pnpm typecheck`. Красный `apps/desktop` при зелёном `ui` означает устаревший
      `ui/dist`, а не ошибку типов
- [X] T075 [P] Обновить `specs/004-app-settings/spec.md` пометкой, что FR-010, FR-011 и FR-013
      пересмотрены фичей 006 — чтобы расхождение читалось из обеих спек, а не только из этой
- [X] T076 Обновить раздел «Известные ограничения» в
      `specs/006-vault-service-layout/plan.md`, если при реализации выяснилось что-то, чего
      там нет

---

## Dependencies

```text
Phase 1 (Setup)
   └─> Phase 2 (Foundational) ── блокирует всё
         ├─> Phase 3  US1 (P1)  ─ MVP
         ├─> Phase 4  US2 (P2)  ─ зависит от US1 только по смыслу проверки,
         │                        технически независима
         ├─> Phase 5  US3 (P2)  ─ T042 опирается на T013 (папка уже создана)
         ├─> Phase 6  US4 (P3)  ─ независима
         └─> Phase 7  US5 (P3)  ─ независима
               └─> Phase 8 (Polish)
```

**Внутри Phase 2** порядок жёсткий: T005→T006→T007 (классификация), T008→T009 (раскладка),
T010→T011→T012→T013→T014→T015 (хранилище), затем граница T016…T018.

**US2 без US1** осмысленна: пометки хранятся и показываются, просто ещё не на что влиять.
**US1 без US2** осмысленна: доступны варианты «рядом» и «в корне», третий вырождается в «в
корне» по FR-006.

## Parallel Execution

| Фаза | Можно параллельно |
|------|-------------------|
| Phase 1 | T001, T002, T003, T004 — разные файлы |
| Phase 2 | T007 с T008–T009; T016 с T017 (разные таргеты границы) |
| Phase 3 | T020 с T021; T025 с T026 |
| Phase 4 | T032 с T033; T035 с T036; T039 отдельно от T038 |
| Phase 6 | T051 с T052; T055 с T056; T057 с T058 |
| Phase 8 | T071, T072, T075 — независимы |

Пары `native`/`wasm` тестов пишутся вместе в одной задаче намеренно: разнесение их по разным
задачам — прямой путь к таргету, на котором тест забыли, а принцип II этого не прощает.

## Implementation Strategy

**MVP — Phase 1 + Phase 2 + Phase 3.** Закрывает исходную жалобу пользователя целиком: папки
перестают появляться рядом с каждой заметкой. Всё остальное — расширение.

**Дальше по ценности**: Phase 5 (US3) убирает второй источник беспорядка — служебный файл в
корне; Phase 4 (US2) добавляет гранулярность; Phase 6 и 7 — удобство и новая возможность.

**Гейты после каждой фазы**: `cargo test` на обоих таргетах и `pnpm typecheck` после
пересборки wasm и `ui/dist`. Фаза, оставившая один из таргетов красным, не считается закрытой
(принцип II, NON-NEGOTIABLE).

---

## Что изменил анализ

`/speckit-analyze` от 2026-09-07 нашёл 9 расхождений; CRITICAL среди них не было. Закрыты все.

| Находка | Что было | Что стало |
|---------|----------|-----------|
| **F1** (HIGH) | Спека утверждала, что правило фичи 004 о немедленном подхвате внешних правок остаётся «без изменений», и ссылалась не на тот пункт 004 | Добавлено **FR-040**: внешняя правка настроек уровня папки применяется при следующем открытии. Раздел «Отношение к принятым ранее решениям» переписан, ссылка исправлена |
| **F2** (HIGH) | Задача хранилища требовала **не** создавать `.typsidian/` при открытии vault, а задача кэша клала туда `cache.sqlite`, открываемый именно при открытии. Две задачи противоречили друг другу, и поведение расходилось по таргетам: native создавал папку, web — нет | Решение записано в research.md §12: папка создаётся при открытии vault, доступного на запись, **на обоих таргетах**. Появилась T013, T015 это проверяет, T042 на неё опирается |
| **U1** (MEDIUM) | FR-024 требовал сообщать об утрате настроек, не отличая «удалили» от «никогда не было»: свежий vault давал ложное сообщение | FR-024 сужен до случая, когда vault уже есть в списке известных на этом устройстве. Признак берётся из существующего `known_vaults`, нового состояния не заводится |
| **C1** (MEDIUM) | FR-032 и SC-005 (ни одно значение настроек не теряется) не были покрыты ни одной задачей | Добавлена T055 |
| **C2** (MEDIUM) | FR-017 (снятие пометки не двигает файлы) не покрыт | Добавлен в T032 и в T041 |
| **C3** (MEDIUM) | FR-033 (немедленное применение) после переезда шёл по новому пути записи и не проверялся | Добавлена T056, покрывающая и FR-040 |
| **O1** (MEDIUM) | FR-008 относится к US1, но проверялся только в Phase 8 | Добавлен шагом в T030 |
| **L1** (LOW) | Умолчание `NextToNote` (FR-003) нигде не проверялось явно | Добавлено в T010, T015 и T020 |
| **L2, L3** (LOW) | SC-002 и FR-035 покрыты косвенно | Оставлено как есть: отдельные задачи были бы церемонией без пользы |

---

## Доделка 2026-10-09

**T045.** Вариант события и правило в ядре (`service_dir_status`) существовали, но взводить
событие было некому: ни одна оболочка не спрашивала о нём при открытии, и сообщение об утрате
настроек (T047) не появлялось никогда — как и сообщение о vault только на чтение, хотя
комментарий в коде называл его подключённым. Теперь факт «папку открывали раньше» берётся в
момент, когда оболочка запоминает vault в хранилище устройства: рабочий стол и Android —
команда `remember_vault` (возвращает `{ id, notice }`), web — `WebBackend.serviceDirStatus`;
результат идёт в `VaultOpenResult.settingsNotice` и оттуда в `AccessStateBanner`. Проверено
компиляцией оболочки и типами; в окне и в браузере сценарий 3 не прогонялся (T049).

**T056.** Тест `vault_settings_apply_at_once_and_an_outside_edit_raises_no_event` — в
`packages/core/tests/vault_native.rs`, а не в `settings_native.rs`: ему нужен настоящий
наблюдатель за папкой. Парного теста под wasm нет: «правку мимо приложения» там нечем
произвести, а общий `MemoryVault` за файлами не наблюдает. Значения устройства (FR-033)
покрыты `apply_persists_values_and_they_survive_reopen` на обоих таргетах.
