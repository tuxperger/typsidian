# Implementation Plan: Оглавление заметки

**Branch**: `015-note-outline` | **Date**: 2026-10-10 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/015-note-outline/spec.md`

## Summary

У открытой заметки в колонке заметки появляется режим «Структура»: заголовки по уровням, по
желанию — рисунки, таблицы и метки под своими разделами; выбор пункта ставит каретку на него
и доводит предпросмотр до того же места; раздел под кареткой подсвечен; ветки сворачиваются,
пункты фильтруются.

Технически фича заводит общий модуль ядра `packages/core/src/structure/` — один разбор текста
заметки парсером Typst (`typst::syntax`, уже в зависимостях) в заголовки, рисунки, таблицы,
метки и ссылки `@имя` с местами (research.md §1–§5). Его переиспользуют 018 и 017
(contracts/core-structure-api.md § Расширение). Поверх разбора — чистые функции оглавления:
видимые строки с учётом свёрнутости и фильтра, текущий раздел по каретке, перенос
свёрнутости через правку (research.md §6–§8). Интерфейс только выбирает момент вызова:
разбор — по остановке набора, текущий раздел — с прореживанием (research.md §9). Переход в
предпросмотре — поиск заголовка в текстовом слое 005 и существующая прокрутка к прогону
(research.md §10). Колонку `ui/src/note-panel/` заводит 013; 015 добавляет в неё режим и
боковую шторку для узкого окна (research.md §11).

Макет — Claude Design, проект «Typsidian: Typst Editor Design»
(`b7a3b1f6-608f-403d-8c07-8101d15dce66`), файл `Typsidian Оглавление.dc.html`, экраны 1a–1c.

Расхождения макета со спекой разобраны в research.md §13–§19: решает спека.

## Technical Context

**Language/Version**: Rust 2021 (ядро, shell), TypeScript strict + React 19 (ui)

**Primary Dependencies**: новых нет. `typst::syntax` (`typst 0.15.1`, в `Cargo.lock` —
`typst-syntax 0.15.1`) уже используется в `elements/locate.rs`, `elements/edit.rs`,
`elements/values.rs`; `tsify-next`, `wasm-bindgen`, Tauri 2, CodeMirror 6

**Storage**: файлов vault фича не читает и не пишет. Показана ли колонка, её режим и
включённые виды пунктов — состояние устройства там же, где его хранит колонка 013; свёрнутые
ветки и текст фильтра — память окна, пока заметка открыта

**Testing**: `cargo test` и `wasm-bindgen-test` (ядро, оба таргета, общие сценарии в
`tests/common/structure_cases.rs`); `pnpm typecheck`; сценарий headless-браузера
`scripts/dev/scenarios/outline.mjs` со снимком и замерами; ручная проверка по quickstart.md
(JS-тестов в репозитории нет)

**Target Platform**: web (wasm32), desktop Linux (Tauri), Android (Tauri). iOS-оболочки нет —
на iOS не проверяется

**Project Type**: монорепо — Rust-ядро + общий UI + три оболочки

**Performance Goals**: SC-001 — оглавление заметки в 10 000 строк ≤200 мс после открытия;
SC-002 — каретка на месте ≤200 мс; SC-003 — правка заголовка видна ≤0,5 с после остановки
набора; SC-004 — задержка отклика на клавишу при 2 000 заголовков отличается ≤10 %. Бюджеты
шагов — research.md §9

**Constraints**: без сети; без записи в vault; разбор не зависит от сборки (FR-005); на web
ядро работает в главном потоке — разбор не должен попадать в обработку нажатия

**Scale/Scope**: заметка до предела размера 001 (FR-013), до 2 000 заголовков; один открытый
файл — подключённые файлы не разбираются

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Что считать заголовком, читаемый текст, вложенность, отбор по фильтру с родителями, свёрнутость и её перенос через правку, текущий раздел, поиск заголовка в текстовом слое — в `packages/core/src/structure/` и `preview/anchor.rs`. UI хранит ключи свёрнутых веток и текст фильтра как непрозрачное состояние, выбирает момент вызова и рисует строки. Тесты — на обоих путях. |
| II. Паритет таргетов | PASS | Чистые функции без файловой системы и состояния; `typst::syntax` уже собирается под wasm32 (005, 008). Одна поверхность: биндинги wasm и команды Tauri 1:1 (contracts/core-structure-api.md § Биндинги). |
| III. Спека без реализации | PASS | `spec.md` описывает поведение; `#heading(level: 2)[…]`, `=`, `#include`, `<итог>` в ней — запись пользователя в заметке, а не реализация. |
| IV. TS strict, без any | PASS | Все типы — `Tsify` из Rust, в UI через `import type`. |
| V. Внешние процессы | N/A | Мост не затрагивается: каретка ставится через `EditorView` CodeMirror тем же путём, что переход к ошибке (`reveal` в `Workspace.tsx`). |
| VI. Local-first | PASS | Vault не читается и не пишется; оглавление — производное от текста в редакторе. |
| VII. Офлайн | N/A | Сети нет вовсе (FR-015). |
| VIII. Внешние движки | N/A | У доски оглавления нет (спека, «Вне скоупа»); Excalidraw не вызывается. |
| IX. Локальность | PASS | Ничего не покидает устройство; индекса нет — разбор одной заметки в памяти. |
| X. Массовые операции | N/A | Разбирается один открытый файл; прохода по vault нет. Неблокирующий характер обеспечен моментом вызова (research.md §9), а не машиной состояний пакета. |

Пересмотр после проектирования (Phase 1): статусы не изменились. Нарушений, требующих
обоснования, нет; условные решения с порогом замера — в Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/015-note-outline/
├── plan.md · research.md · data-model.md · quickstart.md
├── contracts/core-structure-api.md · contracts/frontend-outline.md
├── checklists/requirements.md
└── tasks.md            # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/
├── src/lib.rs                      # pub mod structure
├── src/structure/
│   ├── mod.rs                      # NoteStructure, TextPlace, parse(); реэкспорт
│   ├── walk.rs                     # один обход дерева Typst: заголовки, рисунки, таблицы, метки, ссылки
│   ├── plain.rs                    # читаемый текст разметки без оформления
│   ├── outline.rs                  # Outline, OutlineItem, outline(), carry_collapsed()
│   ├── view.rs                     # OutlineRequest, OutlineView, outline_view(), outline_active()
│   └── web.rs                      # wasm-bindgen: noteOutline, noteOutlineView, noteOutlineActive, noteOutlineCarry
├── src/preview/anchor.rs           # + HeadingProbe, locate_heading()
├── src/preview/web.rs              # + previewLocateHeading
├── src/preview/compile.rs          # + заголовки собранного документа для тестов (feature test-support, SC-005)
├── src/settings/keybindings.rs     # + CommandId::ToggleOutline (Mod+Shift+O)
├── benches/structure_scale.rs      # 10 000 строк, 2 000 заголовков
└── tests/common/structure_cases.rs · tests/structure_native.rs · tests/structure_wasm.rs

packages/shell/src/lib.rs           # команды structure_outline, structure_outline_view,
                                    # structure_outline_active, structure_outline_carry,
                                    # preview_locate_heading

ui/src/
├── vault-client.ts · vault-client-web.ts · vault-client-tauri.ts   # + outline, outlineView, outlineActive, outlineCarry
├── preview-client.ts               # + locateHeading
├── note-panel/                     # колонку заводит 013; здесь — добавляемое 015
│   ├── useOutline.ts               # момент разбора, прореживание каретки, состояние заметки
│   ├── OutlineMode.tsx             # режим «Структура»: фильтр, дерево, пустые состояния, меню видов
│   ├── OutlineRow.tsx              # строка пункта
│   └── NotePanelDrawer.tsx         # боковая шторка колонки для узкого окна (макет 1b)
├── preview/useScrollAnchor.ts      # используется как есть (restore)
├── preview/PreviewPane.tsx         # приём запроса «показать заголовок»
├── workspace/Workspace.tsx         # команда, сочетание, кнопка «Оглавление», переход (reveal)
├── settings/ShortcutsSection.tsx   # название команды
├── i18n/locales/{ru,en}/outline.ts · i18n/locales/ru.ts · i18n/locales/en.ts
└── styles/app.css                  # .ts-outline*, .ts-notepanel-drawer*

scripts/dev/scenarios/outline.mjs   # сценарий сверки с макетом и замеров
```

**Structure Decision**: новый модуль ядра `structure/` рядом с `elements/` и `graph/` — общий
для 015, 017 и 018; собственных каталогов разбора у этих фич нет. В UI отдельного каталога
`outline/` не заводится: режим живёт внутри колонки заметки `ui/src/note-panel/`, которую
создаёт 013 (сквозное решение 1 общего брифа). Имена файлов колонки, созданных 013, сверяются
первой задачей Foundational: 013 планируется параллельно, и её `plan.md` на момент написания
этого плана пуст.

## Complexity Tracking

Нарушений конституции нет. Ниже — решения, принятые с порогом замера: они не отклоняются от
принципов, но меняются, если замер покажет иное.

| Решение | Почему так | Условие пересмотра |
|---|---|---|
| Разбор на web идёт в главном потоке, а не в воркере сборки | Вызов стоит вне нажатия клавиши (после 250 мс тишины); тот же разбор уже делает `useElementAtCaret` (008) на каждой остановке каретки и занимает единицы миллисекунд. Воркер сборки 005 занят сборкой и убивается по пределу времени — оглавление стало бы зависеть от сборки вопреки FR-005 | Замер T046: `noteOutline` на заметке в 10 000 строк в wasm > 50 мс или SC-004 не выполнен → отдельный лёгкий воркер со вторым экземпляром ядра только под `structure`; форма вызова в `VaultClient` не меняется |
| Значение `Outline` держит окно и передаёт обратно в ядро при каждом вопросе о строках и текущем разделе | Образец 009 (`BatchState` держит окно): поверхность без состояния одинакова в wasm и Tauri, оболочке не нужен кэш разбора по заметкам | Замер T046: `noteOutlineActive` при 2 000 заголовков > 8 мс → объект-сеанс разбора в ядре (класс wasm-bindgen и состояние в оболочке) либо сжатая форма входа; решение — отдельным Decision в research.md |
| Переход в предпросмотре ищет заголовок по тексту в текстовом слое, а не по точному положению элемента | Так задано постановкой: «через существующее сопоставление места 005». Спека требует только «в пределах возможностей 005» (FR-004) | Точности не хватает на практике (заголовки с формулами, повторяющиеся названия в оглавлении документа) → положения заголовков из интроспектора Typst в `CompiledDocument` — правка 005, отдельная задача (research.md §10, альтернатива) |
