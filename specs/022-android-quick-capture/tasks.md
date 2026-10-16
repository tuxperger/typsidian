# Tasks: Быстрый захват на Android

**Input**: Design documents from `/specs/022-android-quick-capture/`

**Tests**: ядро — оба таргета (`tests/common/capture_cases.rs`, раннеры `capture_native.rs`,
`capture_wasm.rs`); web — сценарий `scripts/dev/scenarios/daily.mjs`; Android — quickstart.md §3.

## Phase 1: Setup

- [X] T001 Завести `packages/core/src/capture/{mod,entry,daily,write,web}.rs`, `pub mod capture;`, раннеры и `tests/common/capture_cases.rs`
- [X] T002 [P] Переводы `ui/src/i18n/locales/{en,ru}/capture.ts` (лист D1–D3, «Сегодня», ‹ ›, настройки D6, причины отказа, названия команд)

## Phase 2: Foundational

- [X] T003 Типы `CaptureSettings`, `Capture`, `CaptureTarget`, `CaptureOutcome`, `DailyDate`, `DailyNeighbours` в `packages/core/src/capture/mod.rs`; ключ `capture` в `VaultOverrides` с разбором и записью в `packages/core/src/vault/vault_settings.rs` per FR-010
- [X] T004 [P] Тесты настроек на обоих путях: умолчания, неизвестные значения, запись не трогает прочие ключи — `tests/common/capture_cases.rs`

## Phase 3: User Story 2 — «Сегодня» (P2, но без Android-работы — первой)

> Порядок: US2 реализуется первым — он не требует нативного кода, а `open_daily` и запись
> нужны листу захвата US1 (назначение «Сегодня»).

- [X] T005 [P] [US2] Тесты: путь по трём видам названия, создание из шаблона и без, повтор не создаёт вторую (SC-004), соседние через пропуски — `tests/common/capture_cases.rs` per FR-007, FR-008, FR-011
- [X] T006 [US2] `daily_path`, `open_daily`, `daily_neighbours` в `packages/core/src/capture/daily.rs`
- [X] T007 [US2] Биндинги `dailyPath`, `openDaily`, `dailyNeighbours`; команды `daily_*` в `packages/shell/src/lib.rs`; методы в `ui/src/vault-client*.ts` — ядро и shell готовы, остаётся `ui/src/vault-client*.ts`
- [X] T008 [US2] `ui/src/capture/useDailyNote.ts`; кнопка «Сегодня» в шапке и ‹ › у ежедневной заметки (D4), команды палитры «Сегодня», «Предыдущий день», «Следующий день» в `ui/src/workspace/Workspace.tsx`; каретка в конце и фокус редактора; `CommandId::Today` без сочетания по умолчанию в `packages/core/src/settings/keybindings.rs` per FR-007, FR-009, SC-002 — `CommandId::Today` (`Mod+Alt+J`, research.md §12) готов, остаётся `ui/`

## Phase 4: User Story 1 — «Поделиться» (P1) 🎯

- [X] T009 [P] [US1] Тесты: все виды записи, приписка, экранирование, дописывание, прежний текст неизменен, `capture_write` — правка буфера для открытой заметки, запись со снимком, повтор при конфликте, предел размера, vault только для чтения — `tests/common/capture_cases.rs` per FR-003, FR-004, FR-005
- [X] T010 [US1] `entry`, `append` в `packages/core/src/capture/entry.rs`; `capture_write` в `packages/core/src/capture/write.rs`; картинка — сохранение вложения по `placement` 006
- [X] T011 [US1] Биндинги `captureSettings`, `captureEntry`, `captureWrite`; команды в `packages/shell/src/lib.rs`; методы в `ui/src/vault-client*.ts` — ядро и shell готовы, остаётся `ui/src/vault-client*.ts`
- [X] T012 [US1] Плагин `apps/mobile/src-tauri/plugins/capture/` (Cargo.toml, `src/lib.rs` c `register_android_plugin`, `android/src/main/java/.../CapturePlugin.kt`): приём `ACTION_SEND` и `ACTION_VIEW` ярлыков, очередь `filesDir/captures/pending/`, команды `pending`, `resolve`, `returnToSource`, события `shared`, `shortcut`; подключение в `apps/mobile/src-tauri/src/lib.rs`, права в `apps/mobile/src-tauri/capabilities/` per FR-001 — готово: плагин, TS-обёртка `captureSource.ts`, проп в `MobileWorkspace.tsx`; Kotlin и сборка APK не проверены (нет Android SDK)
- [X] T013 [US1] `intent-filter` `SEND` (`text/plain`, `image/*`) и `meta-data` ярлыков в `apps/mobile/src-tauri/gen/android/app/src/main/AndroidManifest.xml`; `res/xml/shortcuts.xml` («Сегодня», «Во входящие») per FR-009 — готово: фильтры SEND/VIEW, `shortcuts.xml`, строки en/ru
- [X] T014 [US1] `ui/src/capture/CaptureSheet.tsx` (D1–D3) и `useCapture.ts`: очередь из `captureSource.pending()`, назначение (входящие, сегодня, недавние), приписка, «Сохранить» → `captureWrite` → `resolve` → `returnToSource`; отказ — D3 с причиной и счётчиком ожидающих; разбор очереди при открытии vault; команда «Во входящие» во всех оболочках; проп `captureSource` в `WorkspaceProps` из `apps/mobile/src/MobileWorkspace.tsx` per FR-002, FR-005 — готово: лист, хук, проп `captureSource`; передача плагина в `MobileWorkspace.tsx` — вместе с T012

## Phase 5: User Story 3 — Настройки (P3)

- [X] T015 [US3] `ui/src/settings/CaptureSection.tsx` (D6) с образцом записи от `captureEntry`; раздел в `SettingsPanel` per FR-010
- [X] T016 [US3] Сценарий `scripts/dev/scenarios/daily.mjs` (quickstart.md §2); снимки сверить с D4, D6

## Phase 6: Polish

- [ ] T017 Ручная проверка Android по quickstart.md §3; итог в quickstart.md
- [X] T018 Гейты `.specify/scripts/typsidian/gates.sh all`
- [ ] T019 README и `**Status**: Implemented` в `specs/022-android-quick-capture/spec.md`

## Phase 7: Convergence

- [X] T020 Устранить расхождение таргетов 001: `NativeBackend::create` создаёт родительские папки, `WebBackend::create` — нет (`packages/core/src/vault/web.rs`); выровнять по одному правилу и закрепить тестом на обоих путях (wasm — на OPFS); «Сегодня» и захват уже заводят папку явно (`capture::daily::ensure_parent`) per Constitution II (contradicts)
- [X] T021 Сценарий `scripts/dev/scenarios/daily.mjs` начинать с чистого vault без заранее созданной папки `дневник/`, чтобы quickstart.md §2 проверялся как написан per FR-007 (partial)
