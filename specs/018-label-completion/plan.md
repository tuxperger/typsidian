# Implementation Plan: Дополнение меток и ссылок

**Branch**: `018-label-completion` | **Date**: 2026-10-10 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/018-label-completion/spec.md`

**Note**: фича реализуется последней из шести фич P2 (013 → 014 → 015 → 016 → 017 → 018) и
опирается на соседей; ссылки на них даны с номером фичи.

## Summary

При наборе `@` редактор предлагает метки открытой заметки и связанных файлов (и ключи
источников 017), при наборе `[[` — заметки vault; команда «Перейти к определению» открывает
место метки или заметку; ссылки в никуда и повторные метки отмечаются в тексте до сборки.

Технически: новый модуль ядра `packages/core/src/completion/` — контекст каретки, кандидаты с
ранжированием и фильтром, цель перехода и набор отметок — поверх разбора `structure` (015) и
`graph` (013). Связанные файлы берутся из списка зависимостей последней сборки 005. Ложные
отметки исключает **свидетельство сборки**: удачная сборка 005 отдаёт перечень меток
собранного документа и ссылок, которые в ней разрешились; ссылка, подтверждённая сборкой, не
отмечается (research.md §4). UI — свой попап у каретки по образцу `InsertMenu.tsx` без новой
зависимости, отметки — в механизме отметок редактора 017.

Макет — Claude Design, проект «Typsidian: Typst Editor Design»
(`b7a3b1f6-608f-403d-8c07-8101d15dce66`), файл `Typsidian Дополнение.dc.html`, экраны 1a–1c.

Расхождения с макетом собраны в research.md §15; при расхождении решает спека.

## Technical Context

**Language/Version**: Rust (stable из `nix/toolchain.nix`, edition 2021) — ядро; TypeScript
5.9 `strict` — UI.

**Primary Dependencies**: `typst` 0.15.1 (его `typst::syntax` — через модуль `structure`
фичи 015; интроспектор `PagedDocument` — для свидетельства сборки); CodeMirror 6 из уже
подключённых пакетов (`@codemirror/state`, `@codemirror/view`). **Новых зависимостей нет** —
ни крейтов, ни npm-пакетов: `pnpm-lock.yaml` и хэш `pnpmDeps` в `nix/desktop.nix` не меняются
(research.md §9).

**Storage**: файлов не пишет. Выключатель автопоказа — одно поле в настройках устройства
(`settings.json`, фича 004). Единственная запись в vault — создание заметки по действию
«Создать заметку», существующей операцией `VaultBackend::create`.

**Testing**: `cargo test` и `wasm-bindgen-test` на одной таблице случаев
(`packages/core/tests/common/completion_cases.rs` + раннеры `completion_native.rs`,
`completion_wasm.rs`); бенчмарк `packages/core/benches/completion_scale.rs`; UI — `pnpm
typecheck` и сценарий `scripts/dev/scenarios/completion.mjs` со снимком.

**Target Platform**: web (wasm32, Next.js), desktop и Android (Tauri). iOS-оболочки нет.

**Project Type**: ядро-библиотека + общий UI на три оболочки.

**Performance Goals**: SC-001 — список не позже 100 мс после открывающего знака при 500
метках (бюджет ядра ≤ 20 мс на нативном таргете, остальное — граница процесса и отрисовка);
SC-003 — переход ≤ 1 с; SC-004 — путь нажатия клавиши без открытого списка не вызывает ядро
вовсе; отметки — не позже 1 с после набора (пересчёт через 300 мс паузы, удержание новых
отметок о метке не дольше 800 мс).

**Constraints**: без сети; одинаково на трёх оболочках; места в тексте редактора — в кодовых
единицах UTF-16 (как у `elements`, 008), места в чужих файлах — `DiagnosticLocation` (005);
тексты — из `ui/src/i18n/locales/`.

**Scale/Scope**: заметка до 10 000 строк и 500 меток; vault до 5 000 заметок (потолок 001);
в списке показывается не больше 50 строк с общим числом подходящих.

## Constitution Check

*GATE: пройден до Phase 0 и перепроверен после Phase 1.*

| Принцип | Оценка |
|---|---|
| **I. Логика в ядре** | Соблюдён. Контекст каретки, набор кандидатов, ранжирование, фильтр, подставляемый текст, цель перехода, набор отметок и правило сверки со сборкой — в `completion/`. UI решает только «когда спросить» (набран открывающий знак, прошла пауза) и рисует ответ. Второго разбора Typst не заводится: метки и ссылки — из `structure` (015), `[[…]]` — из `graph` (013). Собственная раскраска `CodeEditor.tsx` для решений не используется. |
| **II. Паритет таргетов** | Соблюдён. Модуль — чистые функции и чтение через `VaultBackend`; платформенного кода нет, кроме тонкого `completion/web.rs` с биндингами. Каждая функция имеет wasm-биндинг и команду Tauri 1:1 и случаи в общей таблице тестов на обоих путях. |
| **III. Спека — поведение** | Соблюдён: `spec.md` не называет крейтов, путей и схем. Спека не переписывается; закрытые по коду неоднозначности — в research.md. |
| **IV. TS strict** | Соблюдён. Все типы границы — `Tsify` из Rust, в UI только `import type`. |
| **V. Внешние процессы** | Не затронут. Моста в кодовой базе нет (`settings/editor_mode.rs` — только `NullBridge`); фича работает с буфером CodeMirror и протокол моста не трогает. |
| **VI. Local-first** | Соблюдён. Ничего проприетарного не хранится; настройка — в существующем файле настроек устройства. |
| **VII. Офлайн** | Соблюдён построением: сетевых обращений нет. Проверяется тестом «нет чтения вне `VaultBackend`» и случаем недоступной папки vault (связанный файл не читается → его метки не предлагаются, отметок из-за него нет). |
| **VIII. Чёрный ящик** | Не применим: стороннего движка нет. CodeMirror расширяется штатными расширениями (`StateField`, `Decoration`, `keymap`), его DOM не патчится. |
| **IX. Локальность индексов** | Соблюдён. Перечень меток связанных файлов — производное состояние в памяти сеанса, восстановимо из файлов, устройство не покидает. |
| **X. Массовые операции** | Не применим: проходов по vault нет. Чтение связанных файлов ограничено списком зависимостей одной заметки и идёт асинхронно вне пути нажатия клавиши; список заметок для `[[` — готовый индекс 001. |

Нарушений нет; `Complexity Tracking` — одна запись о расширении типа фичи 005.

## Project Structure

### Documentation (this feature)

```text
specs/018-label-completion/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── core-completion-api.md
│   └── frontend-completion.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/
├── Cargo.toml                         # [[bench]] completion_scale
├── benches/completion_scale.rs        # 500 меток, 10 000 строк (SC-001)
├── src/
│   ├── lib.rs                         # pub mod completion;
│   ├── completion/
│   │   ├── mod.rs                     # типы границы, реэкспорт
│   │   ├── context.rs                 # контекст каретки (FR-004)
│   │   ├── linked.rs                  # перечень меток связанных файлов
│   │   ├── labels.rs                  # кандидаты-метки и источники, ранжирование, фильтр
│   │   ├── notes.rs                   # кандидаты-заметки, подставляемая запись
│   │   ├── definition.rs              # цель перехода (FR-005)
│   │   ├── marks.rs                   # отметки и сверка со сборкой (FR-006, FR-007)
│   │   └── web.rs                     # wasm-биндинги
│   ├── preview/
│   │   ├── evidence.rs                # BuildEvidence: метки и ссылки удачной сборки
│   │   ├── compile.rs                 # заполнение CompiledDocument.evidence
│   │   └── web.rs                     # PreviewEngine.completionLinkedIndex
│   ├── settings/
│   │   ├── mod.rs                     # Settings.editor_completion_auto
│   │   ├── apply.rs                   # SettingsChange.editor_completion_auto
│   │   └── keybindings.rs             # GotoDefinition, TriggerCompletion
│   ├── structure/                     # заводит 015; 018 только читает
│   └── graph/mod.rs                   # 013: места вхождений, кратчайшая запись; 018 читает
└── tests/
    ├── common/completion_cases.rs
    ├── common/preview_cases.rs        # случаи свидетельства сборки
    ├── completion_native.rs
    └── completion_wasm.rs

packages/shell/src/lib.rs              # команды Tauri 1:1

ui/src/
├── completion/
│   ├── useCompletion.ts               # сеанс списка: когда спросить ядро
│   ├── CompletionPopup.tsx            # попап у каретки (макет 1a, 1b)
│   ├── completionKeys.ts              # расширение CodeMirror: клавиши открытого списка
│   ├── useLinkedIndex.ts              # перечень меток связанных файлов по сборке
│   ├── useReferenceMarks.ts           # отметки → механизм отметок 017
│   ├── MarkExplanation.tsx            # пояснение отметки с действием (макет 1c)
│   └── useGotoDefinition.ts           # переход и отказ в строке состояния
├── editor/
│   ├── CodeEditor.tsx                 # onCompletionTrigger, слот расширений
│   ├── NoteEditor.tsx                 # подключение списка, отметок, строки состояния
│   └── editorContextMenu.ts           # «Перейти к определению»
├── settings/AppearanceSection.tsx     # выключатель «Предлагать при наборе»
├── workspace/Workspace.tsx            # команды, палитра, переход через reveal (014)
├── vault-client.ts, vault-client-web.ts, vault-client-tauri.ts, preview-client.ts
├── i18n/locales/{ru,en}/editor.ts, settings.ts, palette.ts
└── styles/app.css                     # .ts-complete*, .ts-mark-note*

scripts/dev/scenarios/completion.mjs   # сверка с макетом снимком
```

**Structure Decision**: отдельный модуль ядра `completion/`, а не расширение `structure`:
`structure` отвечает на вопрос «что есть в тексте», `completion` — «что предложить, куда
перейти и что отметить», и ему дополнительно нужны vault, сборка и источники. Каталог UI
`ui/src/completion/` — по образцу `ui/src/export/` (009).

## Зависимости от соседних фич

| Фича | Что 018 берёт | Где |
|---|---|---|
| 015 | разбор текста: метки `<имя>` с видом и текстом помеченного элемента, ссылки `@имя`, корень разбора | `packages/core/src/structure/` |
| 013 | места вхождений `[[…]]` и функция кратчайшей записи ссылки | `packages/core/src/graph/mod.rs` |
| 014 | открытие цели вкладкой и история переходов («Назад») вокруг `reveal` | `ui/src/workspace/Workspace.tsx`, `packages/core/src/workspace/` |
| 017 | ключи источников для списка `@`; механизм отметок редактора; признак «ключ найден» | `specs/017-vault-bibliography/contracts/` |
| 005 | список зависимостей сборки, кэш связанных файлов, `DiagnosticLocation` | `packages/core/src/preview/` |
| 004 | настройки устройства, команды и сочетания | `packages/core/src/settings/` |

Точные имена функций соседей на момент планирования не существуют (планы пишутся
параллельно); первая задача фазы Foundational сверяет их с фактическим кодом
(tasks.md T002).

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Расширение типа чужой фичи: `CompiledDocument` (005) получает необязательное поле `evidence` | FR-007 и SC-002 требуют знать, какие метки существуют в **собранном** документе: метку, созданную кодом, циклом или шаблоном, в тексте не найти | Разбирать текст диагностик («label … does not exist») — сверка с формулировкой чужого крейта, ломается при его обновлении и ничего не говорит об удачной сборке. Отдельная сборка ради меток — второй цикл резолва и двойная цена на каждое нажатие. Поле необязательное (`serde(default)`), прежние потребители его не видят |
