# Implementation Plan: Element Parameters

**Branch**: `008-element-parameters` | **Date**: 2026-09-26 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/008-element-parameters/spec.md`

## Summary

Форма параметров, которая сейчас есть только у рисунка и таблицы, становится общим
правилом каталога вставки. Три опоры:

1. **Схема параметров** — читается из самого Typst (`Func::params()`) в момент
   обращения: **все** параметры любой встроенной функции, подписи — имена Typst,
   подсказки — из справочника. Файла схемы нет (research §1, пересмотрено 2026-09-26).
2. **Разбор парсером Typst** — элемент под кареткой ищется по синтаксическому дереву
   `typst::syntax`, а не регулярками в UI (research §2). Нынешний разбор рисунка из
   `ui/src/editor/typstElements.ts` переезжает в ядро (принцип I).
3. **Правка точечными заменами** — меняется только значение изменённого параметра;
   незнакомые аргументы, выражения и форматирование не трогаются вовсе (research §4).

Форма в UI одна на все элементы и строится по схеме, подгружаемой при открытии.

## Technical Context

**Language/Version**: Rust 2021 (ядро), TypeScript 5 strict (UI)

**Primary Dependencies**: новых нет. `typst 0.15.1` — парсер (`typst::syntax`) и описания
функций (`Func::params()`); `serde_json` — чтение схемы; `tsify-next` — типы границы;
CodeMirror 6 — применение замен одной транзакцией.

**Storage**: схема — файл в составе ядра (`include_str!`); заметки — обычный текст, формат не
меняется

**Testing**: `cargo test` + `wasm-bindgen-test` одной таблицей (`tests/common/element_cases.rs`);
тест схемы против `Func::params()`; стенд UI на chromium headless (JS-тестов в репозитории
нет — см. память проекта)

**Target Platform**: web (wasm), desktop и Android (Tauri) — все через `VaultClient`

**Project Type**: монорепо — Rust-ядро + общий UI + три клиента

**Performance Goals**: разбор заметки 10 000 строк — 3,7 мс release native (замер, research
§3); доступность «Параметров» проверяется после паузы каретки 150 мс; «Будет вставлено» — на
каждое изменение поля

**Constraints**: без сети (FR-023); правка без изменений = пустой список замен (FR-009);
смещения в UTF-16 (соглашение `templates::InsertionPoint`)

**Scale/Scope**: 111 записей каталога; схема — у тех, что вызывают функцию Typst или являются
разметкой с параметрами. Записи-конструкции языка (`#let`, `#show`, `#import`, `#include`,
`#context`, палитра символов) параметров функции не имеют и остаются сниппетами (FR-003)

## Constitution Check

*GATE: пройден до Phase 0, перепроверен после Phase 1.*

| Принцип | Как соблюдается | Статус |
|---------|-----------------|--------|
| **I. Логика в ядре** | Схема, поиск элемента, сборка текста, план замен, проверка значений — `packages/core/src/elements/`. Регулярный разбор рисунка уходит из UI. UI держит только состояние формы и показывает | ✅ |
| **II. Паритет таргетов** | Функции чистые, без `cfg`; одна таблица тестов гоняется `cargo test` и `wasm-bindgen-test` | ✅ |
| **III. Спека — только поведение** | Схема, крейты, форматы — только здесь и в research; спека говорит «схема параметров» как понятие (проверено: ни пути, ни формата в ней нет) | ✅ |
| **IV. TS strict, без `any`** | Типы `ElementSpec`, `ParsedElement`, `FieldValue`, `TextEdit` порождаются `tsify-next` | ✅ |
| **V. Внешние процессы** | Не затрагивается | ✅ |
| **VI. Данные пользователя** | Заметка остаётся обычным Typst; правка не касается того, чего не поняла (FR-012) | ✅ |
| **VII. Офлайн** | Сети нет вовсе (FR-023) | ✅ |
| **VIII. Внешние движки** | Не затрагивается (доски — Out of Scope) | ✅ |
| **IX. Локальность** | Не затрагивается | ✅ |
| **X. Массовые операции** | Не вводятся | ✅ |

**Нарушений нет.** Complexity Tracking пуст.

## Project Structure

### Documentation (this feature)

```text
specs/008-element-parameters/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/element-params.md
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/
├── src/elements/                 # НОВЫЙ модуль
│   ├── mod.rs                    # публичные функции контракта
│   ├── schema.rs                 # ElementSpec/ParamSpec из Func::params(), ключи, кэш
│   ├── locate.rs                 # element_at: LinkedNode → ParsedElement / Unavailable
│   ├── values.rs                 # литерал / выражение, проверка по ParamKind
│   ├── render.rs                 # render_element
│   └── edit.rs                   # plan_element_edit: точечные TextEdit
├── src/settings/keybindings.rs   # + CommandId::ElementParams, умолчание Mod+Shift+E
├── src/vault/web.rs              # + 4 статических метода WebBackend
└── tests/
    ├── common/element_cases.rs   # таблица сценариев контракта
    ├── elements_native.rs
    └── elements_wasm.rs

packages/shell/src/lib.rs         # + 4 команды Tauri

ui/src/
├── vault-client.ts, vault-client-web.ts, vault-client-tauri.ts   # + 4 метода
├── editor/
│   ├── ParamForm.tsx             # НОВЫЙ: общая форма по схеме (заменяет BlockParamsDialog)
│   ├── useElementAtCaret.ts      # НОВЫЙ: доступность «Параметров» с паузой 150 мс
│   ├── NoteEditor.tsx            # вставка/правка через форму, применение замен
│   ├── InsertPanel.tsx           # кнопка «Параметры» + причина недоступности
│   ├── InsertMenu.tsx            # элемент со схемой → форма, без схемы → сниппет
│   ├── editorContextMenu.ts      # пункт «Параметры…»
│   ├── typstElements.ts          # − BlockParams, blockAtOffset, blockTemplate, labelsInText
│   └── BlockParamsDialog.tsx     # удаляется
├── workspace/Workspace.tsx       # команда палитры «Параметры элемента»
└── styles/app.css                # стили формы
```

**Structure Decision**: новый модуль ядра `elements/` рядом с `templates/` (фича 003) — тот же
вид работы: чистые функции над текстом заметки. Новых пакетов нет.

## Порядок работы

1. **Схема и сверка** — `schema.rs`, генератор, `schema.json` для 2–3 элементов, тест против
   `Func::params()`. Дальше схема наполняется до всего каталога.
2. **Ядро контракта** — `element_at`, `render_element`, `plan_element_edit` с таблицей
   тестов на обоих таргетах.
3. **Граница** — wasm, Tauri, `VaultClient`.
4. **Форма** — `ParamForm` на месте `BlockParamsDialog`: сначала рисунок и таблица
   (поведение не должно ухудшиться), затем весь каталог.
5. **Вызов из четырёх мест** — панель, контекстное меню, палитра, сочетание.

## Известные ограничения, принятые сознательно

| Ограничение | Причина |
|-------------|---------|
| Разметка правится только в пределах своего профиля (уровень заголовка, язык кода) | Превращение `==` в `#heading(…)` переписало бы стиль заметки (research §7) |
| Массивы, словари, функции, `stroke` — поле «как есть» | Виджет для каждого типа Typst — отдельная работа; текст Typst сохраняется без потерь |
| `..args` в вызове — правка недоступна | Какие параметры задал разворот, известно только при вычислении |
| Подсказки к параметрам только русские подписи, без описаний | Описания Typst (`docs`) английские; перевод — отдельная задача |

## Constitution Check — повторная оценка после Phase 1

Артефакты Phase 1 (data-model, contracts, quickstart) проверены против таблицы выше. Новых
швов нет, логика в UI не просочилась (форма держит только состояние), типы границы
порождаются. Гейты пройдены.
