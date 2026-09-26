---

description: "Task list for Template Download"
---

# Tasks: Template Download

**Input**: Design documents from `/specs/007-template-download/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md),
[contracts/template-install.md](./contracts/template-install.md)

**Tests**: включены обязательно. Принцип I конституции требует, чтобы каждая единица логики
ядра проходила и `cargo test`, и `wasm-bindgen-test`; логика без тестов на обоих путях не
считается готовой. Это гейт проекта, а не выбор режима TDD. Сеть в тестах не участвует:
загрузчик принимает уже полученные байты (contracts § Обязательства).

**Organization**: задачи сгруппированы по user story, чтобы каждую можно было реализовать и
проверить независимо.

> **Файл заведён задним числом (2026-09-25).** Ядро фичи было написано вместе с фичей 006
> (коммит `2543e431`), а `tasks.md` не создавался. Здесь зафиксировано фактическое состояние:
> закрытые задачи отмечены `[X]` **по результатам чтения кода**, а не по памяти, — для каждой
> указан файл, в котором она выполнена. Незакрытое — то, что действительно отсутствует.

> **Состояние на 2026-09-25 (вечер).** Закрыто всё, кроме T049 — ручного прогона сценариев
> quickstart в собранных web и desktop. Что прогнано автоматически, на стенде и на живом
> реестре — в `checklists/requirements.md`, раздел «Прогон quickstart».
>
> **Где задачи разошлись с формулировкой.** T031: явная установка — `install_template` в
> `preview/compile.rs`, а не в `packages.rs`: ей нужна `TemplatePolicy` (адрес из настроек),
> которая живёт там. T038: список адресов — общий компонент `ui/src/settings/TemplatesSection.tsx`,
> подключённый в обе панели; выключатель автозагрузки переехал туда же из `VaultSection.tsx`.
> T044 — тот же компонент.
>
> **Исправлено сверх списка** (найдено при закрытии задач):
> - FR-005 / US1 AS4: для `@local/…` сборка дописывала ложное «в источнике нет версии».
>   Теперь шаблон без источника только назван отсутствующим (`TemplatePolicy::has_source`).
> - FR-007: `InstallError::Offline` не нёс адреса — теперь `Offline { url }`.
> - FR-015: `Blocked` не подсказывал положить шаблон вручную — подсказка в тексте.
> - FR-010: интерфейсу нечем было узнать, какой шаблон предложить загрузить, кроме разбора
>   текста. Добавлено поле `Diagnostic.template`.
> - FR-004: годность пары «шаблон → адрес» не проверялась нигде. Проверка в ядре при записи
>   (`is_valid_template_source`, `VaultSettingsError::InvalidTemplateSource`).
> - FR-011: панель настроек и рабочая область держат разные экземпляры `useVaultSettings`, и
>   выключатель не доходил до предпросмотра до переоткрытия папки. Рабочая область
>   перечитывает настройки при закрытии панели.
> - `useVaultSettings.write`: причина отказа записи стиралась удачным перечитыванием и не
>   показывалась вовсе. Порядок исправлен.
> - `vault-client-tauri.ts` импортировал `InstalledTemplate`/`InstallReport` из модуля, где их
>   нет, — `apps/desktop` не проходил `tsc`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: можно выполнять параллельно (разные файлы, нет незакрытых зависимостей)
- **[Story]**: к какой user story относится задача (US1…US5)
- В описании — точный путь к файлу

## Path Conventions

Структура из `plan.md` → Project Structure: ядро в `packages/core/src/`, тесты ядра в
`packages/core/tests/`, общий UI в `ui/src/`, команды Tauri в `packages/shell/src/lib.rs`,
панели настроек — `ui/src/settings/`.

---

## Phase 1: Setup

- [X] T001 Добавить `reqwest 0.12` (`default-features = false`, на native `rustls-tls`),
      `flate2 1` (`rust_backend`) и `tar 0.4` в `packages/core/Cargo.toml` для обоих таргетов
- [X] T002 Завести модуль `packages/core/src/preview/packages.rs` и подключить его в
      `packages/core/src/preview/mod.rs`

---

## Phase 2: Foundational (блокирует все истории)

**Purpose**: разбор архива, отпечаток и запись — без них ни одна история не работает.

- [X] T003 Описать `TemplateSource` и `InstallError` со всеми различимыми причинами отказа в
      `packages/core/src/preview/packages.rs` (contracts § Различимые причины)
- [X] T004 Реализовать `source_url` — адрес реестра для пространства `preview`, `None` для
      прочих (FR-005) в `packages/core/src/preview/packages.rs`
- [X] T005 Реализовать `unpack`: разбор `.tar.gz` целиком в памяти, отклонение записи с `..`,
      абсолютным путём и без манифеста, срезание общей обёртки (FR-019) в
      `packages/core/src/preview/packages.rs`
- [X] T006 Реализовать `fingerprint` — устойчивый к порядку файлов и чувствительный к длине
      (FR-022, FR-024) в `packages/core/src/preview/packages.rs`
- [X] T007 Реализовать `install`: запись файлов в `.typsidian/packages/…`, манифест последним
      (FR-020), отказ `NotWritable` на vault только для чтения (FR-018) в
      `packages/core/src/preview/packages.rs`
- [X] T008 Реализовать `fetch` с пределом ожидания и различением причин отказа сети (FR-012,
      FR-014, FR-015) в `packages/core/src/preview/packages.rs`
- [X] T009 [P] Собрать общую таблицу проверок в `packages/core/tests/common/install_cases.rs`
- [X] T010 [P] Прогнать её нативно — `packages/core/tests/install_native.rs`
- [X] T011 [P] Прогнать её под wasm — `packages/core/tests/install_wasm.rs`

**Checkpoint**: разбор, отпечаток и установка проверены на обоих таргетах без сети.

---

## Phase 3: US1 — Заметка с подключённым шаблоном собирается сама (P1) 🎯 MVP

**Goal**: недостающий шаблон устанавливается сам, и документ собирается.

**Independent Test**: открыть заметку с подключением отсутствующего шаблона — предпросмотр
показывает собранный документ, шаблон появился в служебном месте папки заметок.

- [X] T012 [US1] Собрать недостающие шаблоны из структурных причин отказа (`missing_templates`)
      в `packages/core/src/preview/compile.rs`
- [X] T013 [US1] Реализовать `fetch_and_install` — получить, разобрать, установить, вернуть
      отчёт — в `packages/core/src/preview/packages.rs`
- [X] T014 [US1] Встроить установку в сборку: `compile_note_with_templates` ставит недостающее
      и пересобирает документ один раз, в `packages/core/src/preview/compile.rs`
- [X] T015 [US1] Сбрасывать кэш байтов установленного шаблона перед пересборкой
      (`packages.forget`) в `packages/core/src/preview/compile.rs`
- [X] T016 [US1] Не ходить в сеть за пространствами имён, которых реестр не обслуживает
      (FR-005) — ветка `source_url` → `None` в `packages/core/src/preview/packages.rs`
- [X] T017 [US1] Объявить путь с загрузкой на границе wasm — `compileWithTemplates` в
      `packages/core/src/preview/web.rs`
- [X] T018 [US1] Объявить тот же путь командой Tauri — `preview_compile_with_templates` в
      `packages/shell/src/lib.rs`
- [X] T019 [US1] Звать путь с загрузкой из панели предпросмотра, когда настройки прочитаны, в
      `ui/src/preview/PreviewPane.tsx`
- [X] T020 [US1] Проверка «установленная версия не перекачивается»: повторная сборка той же
      заметки не делает ни одного обращения (SC-002, AS2) — случай в
      `packages/core/tests/common/install_cases.rs` + пары в `install_native.rs`,
      `install_wasm.rs`
- [X] T021 [US1] Проверка «соседняя версия не трогается»: установка `0.2.0` при локальной
      `0.1.0` оставляет `0.1.0` на месте (AS3) — там же

**Checkpoint**: US1 работает и покрыта тестами.

---

## Phase 4: US2 — Видно, что и откуда получено (P2)

**Goal**: ни одной молчаливой загрузки; у каждой попытки названы шаблон, версия и адрес.

**Independent Test**: открыть заметку с подключением отсутствующего шаблона — в диагностике
названы имя, версия и полный адрес.

- [X] T022 [US2] Диагностика успешной загрузки: имя, версия, адрес, число файлов, отпечаток
      (FR-006) в `packages/core/src/preview/compile.rs`
- [X] T023 [US2] Диагностика отказа: причина словами и тот же адрес (FR-007) — там же
- [X] T024 [US2] Показывать эти диагностики в колонке предпросмотра как обычные замечания
      сборки в `ui/src/preview/PreviewPane.tsx`
- [X] T025 [US2] Проверка «заметка без подключений не порождает ни сообщений, ни обращений»
      (AS3, FR-008) — случай в `packages/core/tests/common/install_cases.rs` + пары
- [X] T026 [US2] Проверка состава запроса: наружу уходит только имя и версия шаблона (FR-021,
      принцип IX) — расширить `the_request_carries_nothing_but_the_template_name` на путь
      `fetch_and_install` в `packages/core/tests/common/install_cases.rs`

---

## Phase 5: US3 — Автозагрузку можно выключить (P2)

**Goal**: выключатель прекращает походы в сеть немедленно и на всех клиентах, но не отнимает
саму возможность загрузить шаблон.

**Independent Test**: выключить автозагрузку, открыть заметку с отсутствующим шаблоном —
обращения в сеть нет; нажать явную загрузку — шаблон установлен.

- [X] T027 [US3] Поле `auto_download_templates` в настройках уровня vault и его чтение в
      `TemplatePolicy` — `packages/core/src/vault/vault_settings.rs`,
      `packages/core/src/preview/compile.rs`
- [X] T028 [US3] Отказ `AutoDownloadDisabled` вместо похода в сеть при выключенной настройке
      (FR-009) в `packages/core/src/preview/compile.rs`
- [X] T029 [US3] Выключатель в панели настроек web — `ui/src/settings/VaultSection.tsx`,
      `ui/src/settings/SettingsPanel.tsx`
- [X] T030 [US3] Тот же выключатель в панели настроек папки для desktop и Android (FR-016,
      принцип II) — `ui/src/settings/VaultSettingsPanel.tsx`
- [X] T031 [US3] Объявить в ядре явную установку одного шаблона независимо от настройки
      (FR-010): функция поверх `fetch_and_install`, игнорирующая `auto_download`, в
      `packages/core/src/preview/packages.rs`
- [X] T032 [US3] Вывести её на границы: `installTemplate` в `packages/core/src/preview/web.rs`
      и команда `preview_install_template` в `packages/shell/src/lib.rs`
- [X] T033 [US3] Объявить вызов в клиентах: `installTemplate` в `ui/src/preview-client.ts`,
      `ui/src/vault-client-web.ts`, `ui/src/vault-client-tauri.ts`
- [X] T034 [US3] Кнопка «Загрузить шаблон» рядом с диагностикой об отсутствующем шаблоне при
      выключенной автозагрузке в `ui/src/preview/PreviewPane.tsx`
- [X] T035 [US3] Проверка «выключено — в сеть не ходим, включено — ходим; явный запрос работает
      при выключенном» в `packages/core/tests/common/install_cases.rs` + пары

**Checkpoint**: возможность объявлена одинаково на всех трёх клиентах.

---

## Phase 6: US4 — Шаблон с произвольного адреса (P3)

**Goal**: свой адрес архива для конкретного шаблона.

**Independent Test**: указать адрес, открыть заметку с подключением этого шаблона — архив
берётся по указанному адресу.

- [X] T036 [US4] Поле `template_sources` в настройках уровня vault и его разбор —
      `packages/core/src/vault/vault_settings.rs`
- [X] T037 [US4] Выбор источника по спецификации шаблона (`TemplatePolicy::source_for`) в
      `packages/core/src/preview/compile.rs`
- [X] T038 [US4] Правка адресов в интерфейсе: список «шаблон → адрес» с добавлением и
      удалением в `ui/src/settings/VaultSection.tsx` и `ui/src/settings/VaultSettingsPanel.tsx`
- [X] T039 [US4] Объявить запись списка в `ui/src/settings/useVaultSettings.ts`
      (`setTemplateSources`)
- [X] T040 [US4] Проверка «указанный адрес не подменяется реестром, а его отказ называется
      вместе с адресом» (AS2) в `packages/core/tests/common/install_cases.rs` + пары

---

## Phase 7: US5 — Отпечаток полученного (P3)

**Goal**: отпечаток записан, виден и сверяется.

**Independent Test**: установить шаблон, изменить его файл руками, открыть заметку —
приложение сообщает о расхождении и не переустанавливает молча.

- [X] T041 [US5] Запись отпечатка при установке и чтение записанного (`recorded_fingerprint`) в
      `packages/core/src/preview/packages.rs`
- [X] T042 [US5] Отпечаток в диагностике успешной загрузки — `packages/core/src/preview/compile.rs`
- [X] T043 [US5] Сверка содержимого с записанным отпечатком при использовании установленного
      шаблона и диагностика о расхождении **без переустановки** (FR-023) в
      `packages/core/src/preview/compile.rs`
- [X] T044 [US5] Сведения об установленных шаблонах (имя, версия, источник, отпечаток) в
      интерфейсе: раздел «Внешние шаблоны Typst» в `ui/src/settings/VaultSection.tsx`
- [X] T045 [US5] Объявить перечисление установленного на границах ядра —
      `packages/core/src/preview/packages.rs`, `web.rs`, `packages/shell/src/lib.rs`
- [X] T046 [US5] Проверка «изменённый файл даёт расхождение, и переустановки не происходит» в
      `packages/core/tests/common/install_cases.rs` + пары

---

## Phase 8: Polish

- [X] T047 [P] Дописать `specs/007-template-download/data-model.md` — сущности «Источник»,
      «Отчёт об установке», «Отпечаток», на которые ссылается план
- [X] T048 [P] Дописать `specs/007-template-download/quickstart.md` — сценарии проверки
      руками, включая офлайн и vault только для чтения
- [ ] T049 [P] Прогон офлайн-сценариев по quickstart на web и desktop, отметка результата в
      `specs/007-template-download/checklists/requirements.md`
- [X] T050 Обновить README: раздел о внешних шаблонах — что загружается, куда кладётся и как
      выключить

---

## Dependencies

- **Phase 2** блокирует все истории.
- **US1** (Phase 3) — MVP; остальные истории от неё не зависят и могут идти в любом порядке.
- **US3** зависит от US1 только тем, что выключает уже существующий поход в сеть.
- **US5** опирается на отпечаток из Phase 2 (T006, T041).
- Задачи с `[P]` в одной фазе трогают разные файлы и идут параллельно.

## Implementation Strategy

1. Закрыть оставшиеся проверки US1 и US2 (T020, T021, T025, T026) — они доказывают, что
   написанное ядро делает то, что обещает спека.
2. Затем US3 целиком (T030–T035): это паритет клиентов и FR-010 — самая заметная дыра.
3. Затем US4 (T038–T040) и US5 (T043–T046).
4. Phase 8 — документы и прогон руками.

## Что осталось незакрытым

| Требование | Задача | Почему |
|---|---|---|
| Прогон quickstart руками в собранных web и desktop (офлайн, vault только для чтения) | T049 | требует GUI и управления сетью; ядро и UI проверены автоматически, на стенде и на живом реестре — см. checklists/requirements.md |
