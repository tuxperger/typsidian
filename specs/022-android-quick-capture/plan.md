# Implementation Plan: Быстрый захват на Android

**Branch**: `022-android-quick-capture` | **Date**: 2026-10-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/022-android-quick-capture/spec.md`

## Summary

Текст, ссылка или картинка из «Поделиться» Android попадают в конец выбранной заметки vault
через короткий лист захвата; «Сегодня» открывает или создаёт ежедневную заметку — кнопкой в
приложении на всех оболочках и ярлыком приложения на Android.

Технически: правила — в новом модуле ядра `packages/core/src/capture/`: настройки захвата в
`vault.json` (006), запись захвата по виду (список/абзац, с временем), дописывание в конец
без изменения прежнего текста, путь ежедневной заметки по дате и виду названия, соседние
ежедневные заметки, создание из шаблона 003 (research.md §1–§5). Запись в заметку —
по правилу «Связать» 013: открыта в редакторе — правка буфера, иначе запись файла со снимком
`expected` и повтор на свежем тексте (FR-004, research.md §6). Android: локальный
Tauri-плагин `capture` (Kotlin) принимает `ACTION_SEND` и намерения ярлыков, кладёт захват в
очередь в памяти приложения на диске и возвращает пользователя в исходное приложение после
сохранения (research.md §7–§9). Очередь живёт до успешной записи (FR-005).

Макет — временный (не Claude Design), https://claude.ai/code/artifact/eef06564-5ec2-4d90-b658-1991de9183ad, экраны D1–D6.

## Technical Context

**Language/Version**: Rust 2021, TypeScript strict + React 19, Kotlin (Android, плагин)

**Primary Dependencies**: новых крейтов нет; Tauri 2 mobile plugin API (`tauri::plugin::mobile`, `app.tauri.plugin.Plugin`) — часть Tauri

**Storage**: настройки — ключ `capture` в `.typsidian/vault.json`; очередь захватов — `<app data>/captures/` (не vault: vault может быть недоступен)

**Testing**: `cargo test` + `wasm-bindgen-test` (`tests/common/capture_cases.rs`); `pnpm typecheck`; сценарий `scripts/dev/scenarios/daily.mjs` (web: «Сегодня», ‹ ›, настройки); Android — вручную по quickstart.md

**Target Platform**: «Сегодня» и настройки — web, desktop, Android; «Поделиться» и ярлык — Android

**Performance Goals**: SC-001 — 3 касания и 5 с; SC-002 — «Сегодня» ≤2 с

**Constraints**: ни один подтверждённый захват не теряется (SC-003); прежний текст заметки не меняется; без сети

**Scale/Scope**: захват до предела размера заметки 001 §FR-013

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Вид записи, дописывание, путь по дате, соседние дни, создание из шаблона, выбор назначения, решение «буфер или файл» — в `capture/`. Kotlin только принимает намерение и кладёт байты в очередь. |
| II. Паритет | PASS | Правила одинаковы на всех таргетах, тесты на обоих путях; платформенное — только приём «Поделиться», за пропом оболочки `captureSource`. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`. |
| V. Мост к Neovim | N/A | Правка буфера — тем же путём, что «Связать» 013. |
| VI. Local-first | PASS | Захват — текст в обычной заметке; настройки — в `vault.json`. |
| VII. Офлайн | N/A | Сети нет (FR-012). |
| VIII. Внешние движки | N/A | — |
| IX. Локальность | PASS | Очередь — в памяти приложения на устройстве. |
| X. Массовые операции | N/A | — |

Внешних процессов фича не запускает: намерения Android — системный механизм приёма данных,
не интеграция с процессом (раздел «Внешние процессы» конституции не затрагивается).

## Project Structure

```text
packages/core/src/capture/
├── mod.rs          # CaptureSettings, Capture, CaptureTarget, реэкспорт
├── entry.rs        # запись захвата по виду, дописывание в конец
├── daily.rs        # путь по дате, соседние, создание из шаблона 003
├── write.rs        # CaptureOutcome: правка буфера или запись файла со снимком
└── web.rs
packages/core/src/vault/vault_settings.rs   # + ключ capture
packages/core/tests/common/capture_cases.rs · tests/capture_*.rs

packages/shell/src/lib.rs                   # capture_*, очередь захватов (native)
apps/mobile/src-tauri/plugins/capture/      # Tauri-плагин: Cargo.toml, src/lib.rs, android/ (Kotlin)
apps/mobile/src-tauri/gen/android/app/src/main/AndroidManifest.xml   # intent-filter SEND, shortcuts meta-data
apps/mobile/src-tauri/gen/android/app/src/main/res/xml/shortcuts.xml
apps/mobile/src-tauri/capabilities/*.json   # права плагина

ui/src/
├── capture/CaptureSheet.tsx · useCapture.ts · useDailyNote.ts     # D1–D4
├── settings/CaptureSection.tsx                                    # D6
├── workspace/Workspace.tsx                                        # «Сегодня», ‹ ›, команды
├── vault-client*.ts
└── i18n/locales/{ru,en}/capture.ts
apps/mobile/src/MobileWorkspace.tsx         # captureSource от плагина
```

## Complexity Tracking

| Решение | Почему так | Условие пересмотра |
|---|---|---|
| Свой Tauri-плагин на Kotlin | Приём `ACTION_SEND` и `moveTaskToBack` недоступны из Rust и JS без нативного кода | Появится поддерживаемый плагин share-target под Tauri 2 → заменить |
| Лист захвата поверх рабочей области приложения, а не отдельной прозрачной активностью | Одна активность `singleTask` — один вебвью и одно ядро; отдельная активность — второй вебвью | SC-001 (5 с) не выполняется на холодном запуске → лёгкий маршрут `?capture` без открытия дерева vault |
| Правки `gen/android` вручную | Каталог в git, `MainActivity.kt` уже правлен (001) | Перегенерация `tauri android init` — сверить по комментариям в файлах |
