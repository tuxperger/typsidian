---

description: "Tasks: мультиязычный интерфейс (фича 010)"
---

# Tasks: Мультиязычный интерфейс

**Input**: Design documents from `specs/010-ui-localization/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Тесты ядра обязательны по конституции (принцип I, II: `cargo test` + `wasm-bindgen-test`).
Для UI JS-тестов в репозитории нет — гейты UI: `pnpm typecheck` и `scripts/check-i18n.sh`.

**Organization**: по пользовательским историям spec.md (US1…US4).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: можно выполнять параллельно (разные файлы, нет зависимостей от незавершённых задач)
- **[Story]**: история из spec.md
- Все команды — внутри `nix develop`.

**Правило переноса строк (для всех задач US1/US3)**: каждый литерал интерфейса заменяется
вызовом `t("<область>.<ключ>", params?)` из `useI18n()`; ключ добавляется в
`ui/src/i18n/locales/en.ts` (английский текст) **и** в `ui/src/i18n/locales/ru.ts`
(текущий русский текст дословно). Числа — через `plural`/`formatNumber` из
`ui/src/i18n/format.ts`. Комментарии в коде не переводятся. Содержимое пользователя (имена файлов, пути, спецификации шаблонов, текст заметок) подставляется параметром и не переводится (FR-003, FR-005). После задачи файл проходит
`scripts/check-i18n.sh <файл>`.

---

## Phase 1: Setup

- [X] T001 Создать `scripts/check-i18n.sh`: bash + `grep`/`perl` (есть вне `nix develop`), без node; ищет строки с кириллицей в `ui/src`, `apps/desktop/src`, `apps/mobile/src`, `apps/web/app` (`*.ts`, `*.tsx`), исключая `ui/src/i18n/locales/`, `ui/src/wasm-core/`, строки-комментарии (`//`, `*`, `/*`) и строки с маркером `i18n-ignore`; принимает необязательный список файлов (иначе обходит всё); печатает `файл:строка: текст` и завершается кодом 1 при находках, иначе печатает `i18n: ok`
- [X] T002 [P] Создать хук агента `.claude/hooks/check-i18n-edit.sh`: читает JSON события `PostToolUse` со stdin, берёт `tool_input.file_path`, для файлов в зоне проверки T001 запускает `scripts/check-i18n.sh <файл>`; при находках пишет их в stderr и завершается кодом 2 (обратная связь агенту), иначе 0; вне зоны — 0 без вывода
- [X] T003 Зарегистрировать хук T002 в `.claude/settings.json` (новый, проектный): `hooks.PostToolUse` с `matcher: "Edit|Write"`, `type: "command"`, `command: "$CLAUDE_PROJECT_DIR/.claude/hooks/check-i18n-edit.sh"`; `chmod +x` для обоих скриптов
- [X] T004 Создать каркас `ui/src/i18n/`: `locales/en.ts` (`export const en = { meta: { code: "en", nativeName: "English" } } as const` и тип `Catalog`), `locales/ru.ts` (`export const ru: Catalog`), пустые `index.ts`, `resolve.ts`, `format.ts`, `messages.ts`; при необходимости добавить подпуть `./i18n/*` в `exports` `ui/package.json` по образцу существующих

---

## Phase 2: Foundational (блокирует все истории)

### Ядро: выбор языка в настройках устройства

- [X] T005 Добавить `pub language: Option<String>` в `Settings` и `InvalidSettings::LanguageTag { value }` с проверкой синтаксиса тега (`^[A-Za-z]{2,8}(-[A-Za-z0-9]{1,8})*$`, ≤35 знаков) в `packages/core/src/settings/mod.rs`; проверка применяется **только к `SettingsChange`**, а не к разбору файла: кривой тег в `settings.json` хранится как нераспознанный и не обнуляет остальные значения (FR-018, data-model.md § LanguageChoice)
- [X] T006 Добавить `LanguageChoice { System, Tag(String) }` (serde + tsify) и `SettingsChange.language: Option<LanguageChoice>` в `packages/core/src/settings/apply.rs`: `apply_to` (System → `None`), конструктор `SettingsChange::language(..)`, `reset_to_defaults()` → `Some(System)`; `touches_vault_scoped_values()` язык НЕ учитывает — набор с одним языком всегда `ChangeTarget::Device`; `device_defaults_change` язык не включает; добавить `route(change, vault_is_open) -> Vec<RoutedChange { target, change }>` — раскладку набора по хранилищам: язык всегда в `Device`, прочее — по `target_for`; набор «язык + тема» при открытом vault даёт две записи (принцип I: решение в ядре, не в UI)
- [X] T007 Читать и писать ключ `"language"` в `packages/core/src/settings/format.rs` (`parse`, `render` через `set_or_remove`, `initial_document`); отсутствие ключа ⇔ `None`; модульные тесты round-trip и удаления ключа
- [X] T008 [P] Тесты в `packages/core/tests/settings_native.rs`: разбор/запись `language`; нераспознанный, но синтаксически верный тег (`"xx"`) сохраняется при записи другой настройки; невалидный тег отклоняет набор; при открытом vault набор с языком уходит в устройство; сброс возвращает `System`; `route` для «язык + тема» при открытом vault даёт `[Device{language}, OpenVault{theme}]`; кривой тег, вписанный в `settings.json` руками, не обнуляет тему и размер шрифта
- [X] T009 [P] Те же проверки дословно в `packages/core/tests/settings_wasm.rs` (принцип II)
- [X] T010 Проверить прохождение `Settings.language`/`SettingsChange.language` через команды настроек устройства в `packages/shell/src/lib.rs` и web-биндинг `packages/core/src/settings/web.rs`; выставить `route` рядом с `target_for` (web-биндинг — `packages/core/src/settings/web.rs:713`, команда оболочки — `packages/shell/src/lib.rs:647`) (поля передаются целиком — правок может не потребоваться; если сериализация ручная — дописать); `cargo check` native и `--target wasm32-unknown-unknown`
- [X] T011 Пересобрать типы: `pnpm build:wasm`; убедиться, что `language`/`LanguageChoice` появились в `ui/src/wasm-core/typsidian_core.d.ts`; в клиентах настроек `ui/src/settings/device-settings-tauri.ts` и `ui/src/settings/device-settings-client.ts` брать типы `Settings`/`SettingsChange`/`RoutedChange` из `wasm-core` d.ts, не расширяя ручные дубликаты (принцип IV)

### UI: модуль i18n

- [X] T012 [P] Реализовать `resolveLanguage(choice, system)` в `ui/src/i18n/resolve.ts` по контракту `contracts/i18n-module.md`: явный поддерживаемый выбор → он; иначе первый из `system`, чей основной подтег поддерживается (`en-GB` → `en`); иначе `"en"`
- [X] T013 [P] Реализовать `ui/src/i18n/format.ts`: `plural(lang, n, forms)` через `Intl.PluralRules`, `formatNumber`, `formatDate`, `formatBytes` через `Intl`; перенести сюда смысл `ui/src/plural.ts`
- [X] T014 Реализовать `ui/src/i18n/index.ts`: реестр `LANGUAGES` (`en`, `ru`, `nativeName`, `complete`, `engineLang`), `I18nProvider` (контекст; `document.documentElement.lang` на каждую смену), `useI18n()` c `t` (откат на `en` по ключу), `tEn`, форматтерами; язык не участвует ни в одном `key` React
- [X] T015 Подключить `I18nProvider` в корень web: `apps/web/app/VaultWorkspace.tsx` (choice — из настроек устройства, до их загрузки — `undefined`); в `apps/web/app/layout.tsx` заменить `lang="ru"` на `lang="en"` (клиент выставит действующий)
- [X] T016 [P] Подключить `I18nProvider` в корень desktop: `apps/desktop/src/main.tsx` / `apps/desktop/src/DesktopWorkspace.tsx`
- [X] T017 [P] Подключить `I18nProvider` в корень Android: `apps/mobile/src/main.tsx` / `apps/mobile/src/MobileWorkspace.tsx`

**Checkpoint**: ядро хранит язык на обоих таргетах; `useI18n()` доступен во всех трёх оболочках.

---

## Phase 3: User Story 1 — интерфейс на языке системы (P1) 🎯 MVP

**Goal**: весь интерфейс (кроме сообщений об ошибках из US3) на английском при английской системе и на русском при русской.

**Independent Test**: чистые настройки + `--lang=en-GB` → проход всех экранов без русского текста (кроме текстов отказов — они переводятся в US3); `--lang=ru` → без английского; `--lang=de` → английский (quickstart §2).

- [X] T018 [P] [US1] Перенести строки дерева заметок: `ui/src/vault-tree/Sidebar.tsx`, `VaultTree.tsx`, `CreateFileMenu.tsx`, `BacklinksConfirmDialog.tsx`, `BrokenLinkBadge.tsx`, `BulkChangeIndicator.tsx`, `AccessStateBanner.tsx` (ключи `tree.*`)
- [X] T019 [P] [US1] Перенести строки `ui/src/vault-tree/VaultTreeActions.tsx` кроме текстов отказов (ключи `tree.actions.*`; отказы — T047)
- [X] T020 [P] [US1] Перенести строки рабочей области `ui/src/workspace/Workspace.tsx` (ключи `workspace.*`)
- [X] T021 [P] [US1] Перенести строки палитры `ui/src/workspace/CommandPalette.tsx` и названия команд там, где они объявлены (ключи `palette.*`); фильтр сравнивает запрос и с `t(...)`, и с `tEn(...)`, показывает `t(...)` (FR-024)
- [X] T022 [P] [US1] Перенести строки редактора: `ui/src/editor/NoteEditor.tsx`, `editorContextMenu.ts`, `useElementAtCaret.ts`, `useNoteAutoSave.ts`, `DiffView.tsx`, `ConflictDialog.tsx` (ключи `editor.*`)
- [X] T023 [US1] Перенести описания элементов Typst `ui/src/editor/typstElements.ts` (ключи `elements.<элемент>.*`); код Typst внутри описаний (имена функций, параметров) не переводится
- [X] T024 [P] [US1] Перенести строки вставки: `ui/src/editor/InsertMenu.tsx`, `InsertPanel.tsx`, `ParamForm.tsx`, `mathSymbols.ts`, `elementSamples.tsx` (ключи `insert.*`); образцы, вставляемые в заметку, берут текст-заглушку на языке интерфейса
- [X] T025 [P] [US1] Перенести строки досок: `ui/src/boards/BoardCanvas.tsx`, `BoardConflictDialog.tsx`, `EmbedLink.tsx`, `EmbedStaleBadge.tsx`, `InsertBoardDialog.tsx`, `useBoardDocument.ts`, `useNoteEmbeds.ts`, `exportSnapshot.ts`, `exportSvg.ts` (ключи `boards.*`)
- [X] T026 [US1] Передавать движку досок `langCode` из `LANGUAGES[lang].engineLang ?? "en"` в `ui/src/boards/BoardCanvas.tsx` (строка-примитив; не создавать объект пропа на рендер — память проекта о цикле рендера Excalidraw) (FR-025)
- [X] T027 [P] [US1] Перенести строки предпросмотра, не являющиеся сообщениями ядра: `ui/src/preview/PreviewPane.tsx`, `PreviewPage.tsx` (ключи `preview.*`; диагностики — T045)
- [X] T028 [P] [US1] Перенести строки настроек: `ui/src/settings/SettingsPanel.tsx`, `AppearanceSection.tsx`, `EditorFontSizeField.tsx`, `KeybindingsSection.tsx`, `ShortcutsSection.tsx`, `AdvancedSection.tsx`, `VaultSection.tsx`, `TemplatesSection.tsx`, `applyWithNoteSave.ts`, `useVaultSettings.ts` (ключи `settings.*`); подписи сочетаний клавиш в `ShortcutsSection.tsx`/`KeybindingsSection.tsx` строятся из самого сочетания, из каталога берутся только слова (FR-023)
- [X] T029 [P] [US1] Перенести строки шаблонов: `ui/src/templates/TemplateGallery.tsx`, `TemplateCard.tsx`, `SaveAsTemplateDialog.tsx`, `NewNoteFromTemplate.tsx`, `insertion/index.ts` (ключи `templates.*`)
- [X] T030 [P] [US1] Перенести строки экспорта, не являющиеся отказами ядра: `ui/src/export/ExportCenter.tsx`, `ExportToast.tsx`, `useExport.ts` (ключи `export.*`)
- [X] T031 [P] [US1] Перенести строки поиска `ui/src/search/SearchOverlay.tsx` (ключи `search.*`)
- [X] T032 [P] [US1] Перенести строки `ui/src/icons/Icons.tsx` (подписи `aria-label`/`title`) и `ui/src/menu/ContextMenu.tsx` (ключи `common.*`)
- [X] T033 [P] [US1] Перенести строки оболочек: `apps/desktop/src/openDesktopVault.ts` (заголовок нативного диалога — из каталога в момент вызова), `apps/desktop/src/main.tsx`, `apps/mobile/src/FolderPicker.tsx`, `apps/mobile/src/main.tsx`, `apps/web/app/layout.tsx` (мета-описание; ключи `shell.*`)
- [X] T056 [P] [US1] Перенести строки графа `ui/src/graph/GraphView.tsx` (ключи `graph.*`) — найдено анализом, в исходной разбивке задачи не было
- [X] T034 [US1] Заменить все вызовы `ui/src/plural.ts` на `plural` из `ui/src/i18n/format.ts` и удалить `ui/src/plural.ts`; даты и размеры файлов в интерфейсе — через `formatDate`/`formatBytes` (FR-021, FR-022)
- [X] T035 [US1] Прогон `scripts/check-i18n.sh` по всему дереву: 0 находок вне зоны отказов US3 (оставшиеся — только в файлах T045…T049); `pnpm typecheck` зелёный

**Checkpoint**: MVP — интерфейс определяется по системе и полностью переведён, кроме текстов отказов.

---

## Phase 4: User Story 2 — смена языка в настройках (P2)

**Goal**: выбор языка в настройках, немедленное применение, сохранение между запусками.

**Independent Test**: quickstart §3 — смена на лету без потери правок; перезапуск сохраняет; «Как в системе» удаляет ключ; `.typsidian/vault.json` не содержит `language`.

- [X] T036 [US2] Добавить выбор языка в `ui/src/settings/AppearanceSection.tsx`: вариант «Как в системе» + все `LANGUAGES` под `nativeName` (FR-011); изменение отправляет `SettingsChange.language` в хранилище устройства тем же путём, что прочие значения устройства (`ui/src/settings/useSettings.ts`)
- [X] T037 [US2] Протянуть действующий выбор из настроек устройства в `I18nProvider` во всех трёх оболочках (`apps/web/app/VaultWorkspace.tsx`, `apps/desktop/src/DesktopWorkspace.tsx`, `apps/mobile/src/MobileWorkspace.tsx`) так, чтобы смена в панели меняла контекст без перезапуска и без пересоздания редактора/доски/предпросмотра (FR-012, FR-013)
- [X] T038 [US2] Обработать недоступность хранилища настроек: язык по системе, сообщение о недоступности настроек — на этом языке (FR-029) в `ui/src/settings/useSettings.ts`
- [X] T039 [US2] Применение настроек в `ui/src/settings/useSettings.ts` / `useVaultSettings.ts` перевести на `route` из ядра: UI выполняет полученные записи по порядку и не решает сам, куда уходит язык (FR-016, принцип I)

**Checkpoint**: US1 + US2 — полноценный выбор языка.

---

## Phase 5: User Story 3 — сообщения ядра на выбранном языке (P3)

**Goal**: причины отказов и диагностики приложения — на языке интерфейса и перерисовываются при смене языка; сообщения Typst — как есть.

**Independent Test**: quickstart §4.

### Ядро

- [X] T040 [US3] Создать `packages/core/src/messages.rs`: `AppMessage` (`#[serde(tag = "code", rename_all_fields = "camelCase")]`, tsify) с вариантами из data-model.md (`DependencyUnreadable`, `TemplateMissing`, `TemplateDownloaded`, `TemplateInstallFailed`, `TemplateFingerprintDiverged`, `VaultFontUnusable`, `ExportFailed`) и вложенными `ResolveReason`, `InstallFailure`, `FontProblem`, `ExportFailure`; `log_text()` возвращает прежний текст; подключить `pub mod messages` в `packages/core/src/lib.rs`
  - _Отступление_: варианта `ExportFailed` нет — отказы экспорта уже структурны (`ExportOutcome`/`ExportWriteProblem` с `kind`) и переводятся в UI сопоставлением по `kind` (`ui/src/export/describe.ts`); заведён `NoteOutsideVault` (отказ сборки, общий у предпросмотра и экспорта).
- [X] T041 [US3] Добавить `app: Option<AppMessage>` в `Diagnostic` (`packages/core/src/preview/diagnostics.rs`; `translate` ставит `None`) и заполнять его во всех замечаниях приложения в `packages/core/src/preview/compile.rs` (`with_resolve_failures`, установка шаблонов, `fingerprint_notes`, шрифты vault) и `packages/core/src/export/mod.rs`; `message` = `log_text()`
- [X] T042 [P] [US3] Тесты в `packages/core/tests/preview_native.rs` и `packages/core/tests/export_native.rs`: у отказа связанного файла, битого шрифта, отключённой загрузки шаблона, отказа экспорта `app` заполнен и несёт путь/спецификацию дословно; у синтаксической ошибки Typst `app == None`; JSON каждого варианта `AppMessage` сверяется с эталоном
- [X] T043 [P] [US3] Те же проверки дословно в `packages/core/tests/preview_wasm.rs` и `packages/core/tests/export_wasm.rs` (принцип II)
- [X] T044 [US3] `pnpm build:wasm`; проверить, что `AppMessage` и `Diagnostic.app` есть в `ui/src/wasm-core/typsidian_core.d.ts`; на desktop/Android диагностики приходят через `packages/shell/src/lib.rs` тем же `Diagnostic` — сверить сериализацию

### UI

- [X] T045 [US3] Реализовать `ui/src/i18n/messages.ts`: `message(m: AppMessage)` с исчерпывающим `switch` по `code` и веткой `never`; `diagnostic(d)` — `d.app ? message(d.app) : d.message`; ключи `app.*` в обоих каталогах; подключить в `useI18n()`
  - _Отступление_: `message`/`diagnosticText`/`installFailure` — функции модуля `ui/src/i18n/messages.ts` с `t` первым аргументом, а не поля `I18n`: их зовут и хелперы вне компонентов (`export/describe.ts`), которым нужен тот же `t`.
- [X] T046 [US3] Показ диагностик в `ui/src/preview/PreviewPane.tsx` и `ui/src/export/ExportCenter.tsx` — только через `diagnostic(d)`; текст вычисляется при рендере, не хранится в состоянии (FR-014); перенести `ui/src/preview/describeInstallError.ts` и `ui/src/export/describe.ts` на ключи каталога
- [X] T047 [P] [US3] Перенести тексты отказов с `kind` на ключи `errors.*`: `ui/src/vault-tree/useVault.ts`, `VaultTreeActions.tsx`, `ErrorBanner.tsx`, `ui/src/editor/useConflictResolution.ts`
- [X] T048 [P] [US3] Убрать показ `error.message` в `ui/src/vault-tree/OpenVault.tsx` и `ui/src/settings/useVaultList.ts`: сопоставление по `kind` (при неизвестном `kind` — общий текст `errors.unknown` на языке интерфейса)
- [X] T049 [US3] Прогон `scripts/check-i18n.sh` по всему дереву — `i18n: ok`; `pnpm typecheck` зелёный

**Checkpoint**: все сообщения приложения переводятся и перерисовываются при смене языка.

---

## Phase 6: User Story 4 — добавление нового языка (P4)

**Goal**: новый язык = набор переводов; непереведённое — по-английски.

**Independent Test**: quickstart §5.

- [X] T050 [US4] Разрешить в реестре `LANGUAGES` (`ui/src/i18n/index.ts`) каталоги `Partial<Catalog>` с `complete: false`; `t` для отсутствующего ключа берёт `en` (FR-027); у поставляемых `en`/`ru` тип — полный `Catalog`
- [X] T051 [US4] Дописать в `scripts/check-i18n.sh` предупреждение (не провал) для языков с `complete: false` и провал, если `complete: true` у каталога, объявленного `Partial` (FR-028)
- [X] T052 [US4] Описать в шапке `ui/src/i18n/index.ts` порядок добавления языка (файл каталога + запись в `LANGUAGES`); пройти quickstart §5 с временным каталогом и удалить его

---

## Phase 7: Polish & Cross-Cutting

- [X] T053 [P] Проверка раскладки по quickstart §6 (headless Chromium, 400 px и минимальная ширина окна, `en` и `ru`); найденные переполнения чинить в `ui/src/styles/app.css` только селекторами `.ts-*` (память проекта); там же замерить смену языка (≤1 с, SC-003) и проверить, что правка, курсор, открытая доска и предпросмотр переживают смену (FR-013)
  - _Отступление_: сделано для минимального окна 880×560 (и 1280×800) на `en`/`ru` — переполнения тулбаров колонок и строки состояния исправлены в `app.css`; смена языка 22–24 мс; FR-013 проверен и **исправлен** (`CodeEditor` пересоздавал `EditorView` по переведённой `aria-label` — теперь по `documentId`, подпись через `Compartment`). Ширина 400 px **не закрыта**: у приложения нет телефонной раскладки (нет ни одного `@media` по ширине), это отдельная работа, а не дефект перевода. Итоги — `quickstart.md` § «Результаты прогона».
- [X] T054 Полные гейты: `cargo test -p typsidian-core`, wasm-тесты `settings_wasm`, `preview_wasm`, `export_wasm`, `pnpm build:wasm`, `pnpm typecheck`, `scripts/check-i18n.sh`, `pnpm build`
- [X] T055 Сборка desktop (`pnpm build` в `apps/desktop`) и Android (релизный APK, подпись debug-ключом — память проекта) и проверка SC-007 на `en` на обоих; отметить в `specs/010-ui-localization/quickstart.md` результаты прогона
  - _Отступление_: desktop `.deb` и релизный APK собраны, APK подписан debug-ключом (`app-universal-release-signed.apk`, `apksigner verify` проходит). SC-007 проверен стендом на собранных `apps/desktop/dist` и `apps/mobile/dist` с заглушкой Tauri (тексты `en` совпадают с web; выбор папки Android переведён) — на самом устройстве не гонялся: по USB не подключено ни одного.

---

## Dependencies & Execution Order

- **Setup (T001–T004)** → **Foundational (T005–T017)** → истории.
- T005 → T006 → T007 → T008/T009 (параллельно) → T010 → T011.
- T012, T013 параллельно; T014 после них; T015–T017 после T014.
- **US1** зависит только от Foundational. Внутри — задачи с [P] параллельны (разные файлы);
  T023 и T026 без [P] только из-за объёма/памяти; T034 после T018–T033 и T056; T035 последней.
- **US2** зависит от Foundational (T011, T014); от US1 не зависит логически, но проверять
  удобнее после US1.
- **US3**: T040 → T041 → T042/T043 → T044 → T045 → T046; T047/T048 параллельны после T045.
- **US4** после T014; удобнее после US1.
- **Polish** — после всех историй.

## Parallel Example: User Story 1

```text
T018 vault-tree   T020 workspace   T022 editor   T025 boards   T027 preview
T028 settings     T029 templates   T030 export   T031 search   T032 icons/menu   T033 shells
```

Каждая задача добавляет ключи в `en.ts`/`ru.ts` — при параллельной работе ключи группируются
по своей области (`tree.*`, `editor.*`…), конфликты слияния только по соседним строкам.

## Implementation Strategy

1. **MVP**: Setup + Foundational + US1 → интерфейс по языку системы.
2. **+US2**: ручной выбор.
3. **+US3**: сообщения ядра структурно — самая рискованная часть (форма `Diagnostic`
   меняется на всех трёх клиентах), поэтому после того, как каталоги уже устоялись.
4. **+US4**: расширяемость.
5. Polish: раскладка, паритет, полные гейты.

---

## Phase 8: Convergence

- [X] T057 Сделать телефонную раскладку для ширины 400 px (одна колонка: дерево / редактор / предпросмотр переключаются, шапка и тулбары без выхода за экран, панель настроек и модальные окна во всю ширину) в `ui/src/styles/app.css` селекторами `.ts-*` и при необходимости в `ui/src/workspace/Workspace.tsx`; проверить стендом из quickstart § «Результаты прогона» на `en` и `ru` per SC-006 (partial)
- [ ] T058 Проверить SC-007 на подключённом Android-устройстве (подписанный `app-universal-release-signed.apk`) и в запущенном desktop-бинаре: те же экраны на `en`, что и web; дописать результат в `specs/010-ui-localization/quickstart.md` per SC-007 / FR-019 (partial)
- [X] T059 Не сериализовать `None` у `Settings::language` и `Settings::editor_config_path` (`skip_serializing_if`) в `packages/core/src/settings/mod.rs`: команды Tauri отдают `Option::None` как `null`, а тип и проверки UI ждут `undefined` — `resolveLanguage` падает на `null.split`, и окно desktop/Android остаётся пустым; тест в том же модуле per SC-007 / FR-029
