# Implementation Plan: Свойства заметок и выборки

**Branch**: `020-note-properties-queries` | **Date**: 2026-10-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/020-note-properties-queries/spec.md`

## Summary

У заметки появляются свойства — теги и поля, — которые правятся формой в колонке заметки,
отбирают заметки в поиске и подставляются в таблицу-выборку внутри заметки.

Технически: свойства записываются в начале заметки обычным Typst —
`#metadata((tags: (…), статус: "…")) <properties>` — и в документ не выводятся (research.md §1).
Разбор и правка записи — новый модуль ядра `packages/core/src/properties/` на
`typst::syntax`, по правилу 008 «разбирается однозначно — правим, иначе только читаем»
(research.md §2–§3). Свойства всех заметок индексируются вместе со ссылками 013 в
`IndexCache` (SQLite на native, память и IndexedDB в web) при каждом обновлении записи
индекса; полный пересчёт — пакетной операцией с прогрессом и отменой (research.md §4–§5).
Отбор — чистая функция над индексом, она же питает фишки условий поиска (research.md §6).
Блок выборки — вызов `#select(…)` из файла `.typsidian/lib/select.typ`, который приложение
кладёт в vault один раз; результат приложение передаёт сборке через `sys.inputs`, а без
приложения `select` печатает замещающий текст с условиями (research.md §7–§8, Clarifications
2026-10-15). Форма блока — механизм параметров элемента 008 со своей схемой (research.md §9).

Макет — временный (не Claude Design), https://claude.ai/code/artifact/e74f2c04-a4d7-473a-9f18-a866396d4dc0, экраны B1–B6.

## Technical Context

**Language/Version**: Rust 2021 (ядро, shell), TypeScript strict + React 19 (ui)

**Primary Dependencies**: новых нет — `typst::syntax` (разбор записи), `typst` `Library::builder().with_inputs` (передача результата в сборку), `rusqlite` (кэш native, уже есть), IndexedDB (web, уже есть)

**Storage**: свойства — в тексте заметок (источник истины); индекс свойств — производный, в `IndexCache`; `.typsidian/lib/select.typ` — неизменяемый файл библиотеки с номером версии

**Testing**: `cargo test` + `wasm-bindgen-test` (`tests/common/properties_cases.rs`), `pnpm typecheck`, сценарий `scripts/dev/scenarios/properties.mjs`, проверка внешним `typst compile` в тесте ядра (SC-003) — сборка `PreviewWorld` без входов

**Target Platform**: web (wasm32), desktop Linux, Android

**Project Type**: монорепо — Rust-ядро + общий UI + три оболочки

**Performance Goals**: SC-001 — отбор по тегу на 10 000 заметок ≤300 мс; SC-002 — изменение свойства видно в выборке ≤2 с

**Constraints**: запись свойств — только литералы (иначе только чтение); результат выборки не пишется в vault; ничего не покидает устройство; пересчёт индекса — неблокирующий и отменяемый

**Scale/Scope**: до 10 000 заметок, до 50 свойств на заметку, выборка до 500 строк

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Разбор и правка записи, виды значений, сравнение и сортировка, условия отбора, результат выборки, сериализация во входы сборки — `packages/core/src/properties/`. UI рисует форму и фишки. |
| II. Паритет | PASS | Разбор и отбор — чистые функции; индекс — за существующим трейтом `IndexCache` с реализацией на таргет; биндинги wasm и команды Tauri 1:1. |
| III. Спека без реализации | PASS | Формат записи выбран здесь, а не в спеке. |
| IV. TS strict | PASS | Типы — `Tsify`. |
| V. Мост к Neovim | N/A | Правка записи — через план правки 008 (`EditOutcome`), как у параметров элементов. |
| VI. Local-first | PASS | Свойства — обычный Typst в файле; `select.typ` — обычный файл в vault; заметки собираются без приложения (SC-003). |
| VII. Офлайн | N/A | Сети нет. |
| VIII. Внешние движки | N/A | — |
| IX. Локальность | PASS | Индекс — только на устройстве; значения свойств не попадают в журнал и диагностику (тест на сообщения ошибок). |
| X. Массовые операции | PASS | Полный пересчёт индекса свойств — пакетная операция с прогрессом и отменой по образцу переиндексации 001; запись в кэш по заметке, отмена оставляет согласованный кэш. |

## Project Structure

```text
specs/020-note-properties-queries/
├── plan.md · research.md · data-model.md · quickstart.md
├── contracts/core-properties-api.md · contracts/select-library.md · contracts/frontend-properties.md
└── tasks.md

packages/core/
├── src/properties/
│   ├── mod.rs          # PropertyValue, Property, NoteProperties, реэкспорт
│   ├── read.rs         # разбор записи <properties>: литералы, места, однозначность
│   ├── write.rs        # план правки записи (добавить, изменить, удалить, создать запись)
│   ├── query.rs        # Selection, Condition, отбор и сортировка над индексом
│   ├── select.rs       # блоки #select в тексте, входы сборки, текст select.typ
│   └── web.rs          # wasm-bindgen
├── src/vault/cache.rs  # + свойства в IndexCache (SQLite: таблица properties; MemoryCache)
├── src/vault/mod.rs    # + обновление свойств вместе со ссылками, пакетный пересчёт
├── src/preview/world.rs · compile.rs   # + входы сборки (sys.inputs)
├── src/search/mod.rs   # + условия по свойствам в SearchQuery
├── src/elements/       # + схема формы «Выборка» для параметров 008
└── tests/common/properties_cases.rs · tests/properties_native.rs · tests/properties_wasm.rs

packages/shell/src/lib.rs   # команды properties_*

ui/src/
├── vault-client*.ts · preview-client.ts    # + properties.*, входы сборки
├── note-panel/properties/PropertiesMode.tsx · PropertyRow.tsx   # B1, B2
├── search/SearchOverlay.tsx                # + фишки условий (B3)
├── editor/ (меню вставки 008)              # + пункт «Выборка» (B4)
├── preview/useCompiledDocument.ts          # + входы выборок в запросе сборки
├── i18n/locales/{ru,en}/properties.ts
└── styles/app.css                          # .ts-props*, .ts-select-form*
```

**Structure Decision**: модуль `properties/` рядом с `structure/` и `elements/`; индекс — расширение
существующего `IndexCache`, отдельного хранилища не заводится.

## Complexity Tracking

| Решение | Почему так | Условие пересмотра |
|---|---|---|
| Результат выборки — через `sys.inputs`, а не запись в файл | Clarifications 2026-10-15: файлы vault ради результата не переписываются | — |
| Файл библиотеки `.typsidian/lib/select.typ` в vault | Без него `#select` не соберётся вне приложения; пишется один раз, меняется только с версией | Пользователь против файла в служебной папке → определение `select` вставляется в заметку рядом с блоком |
| Входы всех выборок заметки считаются перед каждой сборкой | Выборка — запрос к индексу в памяти (≤10 мс на 10 000 заметок) | Замер > 30 мс → кэш результата по отпечатку индекса |
