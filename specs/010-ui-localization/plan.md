# Implementation Plan: Мультиязычный интерфейс

**Branch**: `010-ui-localization` | **Date**: 2026-09-29 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/010-ui-localization/spec.md`

## Summary

Интерфейс получает выбор языка (русский, английский; расширяемо) с автоопределением по
системе и немедленным применением. Ядро становится языконезависимым: сообщения, которые
оно формирует само, уходят в UI структурно — вид + параметры (`AppMessage`), а текст
собирается в общем UI из типизированных каталогов. Выбор языка — значение настроек
устройства в ядре. Полнота переводов обеспечивается компилятором TypeScript и скриптом
проверки хардкода, который же подключён хуком агента. Библиотек не добавляется.

## Technical Context

**Language/Version**: Rust 2021 (стабильный из `nix develop`), TypeScript 5.9 strict, React 19

**Primary Dependencies**: без новых. `serde`/`tsify-next` (ядро), стандартный `Intl`
(PluralRules, NumberFormat, DateTimeFormat) в UI; `@excalidraw/excalidraw` 0.18.1 — проп
`langCode`

**Storage**: `settings.json` устройства (фича 004) — новый ключ `language`; в vault ничего

**Testing**: `cargo test` + `wasm-bindgen-test` (ядро); `tsc --noEmit` во всех пакетах;
`scripts/check-i18n.sh`; стенд headless Chromium для раскладки (JS-тестов в репо нет)

**Target Platform**: web (Next.js, wasm32), desktop (Tauri, Linux/deb), Android (Tauri)

**Project Type**: монорепо: ядро на Rust + общий UI + три оболочки

**Performance Goals**: смена языка ≤1 с на всём видимом интерфейсе (SC-003); без
пересоздания редактора, доски, предпросмотра

**Constraints**: офлайн (FR-030); без новых зависимостей; без `any`; паритет native/wasm32

**Scale/Scope**: ≈1250 строк в 88 файлах `ui/src`, ≈20 файлов оболочек, 7 кодов
`AppMessage` с вложенными причинами, 2 языка × ≈700 ключей

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Принцип | Оценка |
|---|---|
| I. Логика в ядре | ✅ с оговоркой. Хранение и валидация выбора языка, маршрут в устройство, структурные сообщения — в ядре. Каталоги и выбор действующего языка — в UI: это представление, а не бизнес-логика; см. Complexity Tracking. Ядро ничего не «перепроверяет» в UI и наоборот. |
| II. Паритет таргетов | ✅ `AppMessage`, `Settings.language`, `LanguageChoice` — общий код без cfg; тесты парные (`*_native.rs` / `*_wasm.rs`). |
| III. Спека — только поведение | ✅ spec.md без технологий; решения — здесь и в research.md. |
| IV. TS strict, без any | ✅ каталоги типизированы от эталона; `AppMessage` — из tsify; исчерпывающий `switch` с `never`. |
| V. Мост к Neovim | ✅ не затрагивается (подписи режима редактирования — только тексты). |
| VI. Local-first | ✅ всё локально, ни сервера, ни загрузки переводов. |
| VII. Офлайн | ✅ сетевых путей нет (FR-030). |
| VIII. Внешние движки | ✅ редактор досок получает язык пропом публичного API; внутренний DOM не трогается. |
| IX. Локальность поиска | ✅ не затрагивается. |
| X. Массовые операции | ✅ не затрагивается (подписи прогресса экспорта — только тексты). |

**Post-design re-check (после Phase 1)**: без изменений — data-model и контракты не вводят
новых нарушений.

## Project Structure

### Documentation (this feature)

```text
specs/010-ui-localization/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── i18n-module.md
│   └── core-messages.md
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/src/
├── messages.rs                  # НОВЫЙ: AppMessage + вложенные причины, log_text()
├── preview/diagnostics.rs       # Diagnostic.app
├── preview/compile.rs           # замечания приложения → AppMessage
├── export/mod.rs                # отказ экспорта → AppMessage
├── settings/mod.rs              # Settings.language, InvalidSettings::LanguageTag
├── settings/apply.rs            # SettingsChange.language, маршрут, сброс
└── settings/format.rs           # ключ "language" в settings.json
packages/core/tests/
├── settings_{native,wasm}.rs    # язык: разбор, запись, маршрут, сброс, неизвестный тег
└── preview_{native,wasm}.rs     # app у диагностик приложения, нет у Typst

ui/src/i18n/                     # НОВЫЙ
├── index.ts                     # I18nProvider, useI18n, LANGUAGES
├── resolve.ts                   # resolveLanguage
├── format.ts                    # plural, числа, даты, байты (Intl)
├── messages.ts                  # AppMessage / Diagnostic / отказы с kind → текст
└── locales/
    ├── en.ts                    # эталон, тип Catalog
    └── ru.ts                    # : Catalog
ui/src/**                        # все экраны: литералы → t(...)
ui/src/plural.ts                 # удаляется (заменён i18n/format.ts)
ui/src/settings/AppearanceSection.tsx   # выбор языка
apps/{web/app,desktop/src,mobile/src}   # провайдер в корне, строки оболочек → t(...)

scripts/check-i18n.sh            # НОВЫЙ: хардкод-литералы с кириллицей
.claude/hooks/check-i18n-edit.sh # НОВЫЙ: хук агента поверх того же скрипта
.claude/settings.json            # НОВЫЙ: регистрация хука PostToolUse
.specify/extensions.yml          # хуки фаз Spec Kit (уже заведены)
```

**Structure Decision**: раскладка существующего монорепо; новое — модуль `messages` в ядре,
модуль `i18n` в общем UI, скрипт проверки и хук агента.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Выбор действующего языка (`resolveLanguage`) и каталоги — в UI, а не в ядре (буква принципа I) | Вход функции — состав каталогов, существующих только в UI; язык нужен до первого кадра, а на desktop/Android ядро доступно только через IPC | Функция в ядре потребовала бы передавать через границу список каталогов и задержала бы первый кадр IPC-вызовом; каталоги в ядре — второй источник текстов рядом с UI и повторные вызовы ядра ради перевода уже показанного (FR-014). Бизнес-логики (vault, сборка, ссылки, граф) здесь нет — это выбор представления того же рода, что тема. |
