# Implementation Plan: Перетаскивание в дереве файлов

**Branch**: `023-tree-drag-drop` | **Date**: 2026-10-12 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/023-tree-drag-drop/spec.md`

## Summary

Заметки, доски и папки переносятся между папками vault перетаскиванием в дереве
(указателем и долгим нажатием) или действием «Переместить в…»; папка едет целиком;
открытый файл, несохранённые правки и раскрытость дерева следуют за путём; перед
переносом, разрывающим ссылки, показывается предупреждение; последний перенос
отменяется из сообщения.

Технически фича заводит в ядре операцию переноса — модуль `vault/relocate.rs` и метод
`VaultBackend::move_entry` (research.md §1). Существующего `rename` недостаточно: для
папки он оставляет индекс под старыми путями, а на web работает только с одним
текстовым файлом. Ядро решает, какие папки могут принять элемент (`move_targets`), что
сломается (`move_impact`, research.md §4), что переезжает вместе с заметкой
(research.md §2), и ведёт за путём индекс, правки, кэш ссылок и пометки разделов. На
native перенос — атомарный `rename`; на web — копирование с прогрессом, отменой и
откатом (research.md §5–§6). Интерфейс добавляет жест на Pointer Events (research.md
§8), окно выбора папки и сообщение с отменой — и один путь `useMoveEntry` для всех трёх.

Макет — временный (не Claude Design), https://claude.ai/code/artifact/f749a41c-2f82-40ef-a46f-3c016e30239f, экраны «Перетаскивание указателем», «После броска», «Без перетаскивания», «Сенсорный экран».

Claude Design в сессии планирования был недоступен (скилла `design` нет, вход отклонён).
Бриф для перерисовки — в отчёте по фиче; при перерисовке сверяется вёрстка, не спека.

## Technical Context

**Language/Version**: Rust 2021 (ядро, shell), TypeScript strict + React 19 (ui)

**Primary Dependencies**: новых нет. `wasm-bindgen`, `js-sys`, `tsify-next`, Tauri 2 —
уже в проекте; жест — на Pointer Events без библиотек

**Storage**: файлы vault (перенос); пометки разделов — настройки vault в служебной
папке (006); кэш ссылок (001). Нового хранимого состояния нет: отмена и раскрытость
папок — память окна

**Testing**: `cargo test` и `wasm-bindgen-test` на общих сценариях
`tests/common/relocate_cases.rs`; `pnpm typecheck`; сценарий headless-браузера
`scripts/dev/scenarios/tree-move.mjs`; ручная проверка desktop и Android по
quickstart.md (JS-тестов в репозитории нет)

**Target Platform**: web (wasm32, File System Access API / OPFS), desktop Linux (Tauri),
Android (Tauri)

**Project Type**: монорепо — Rust-ядро + общий UI + три оболочки

**Performance Goals**: SC-001 — перенос заметки ≤3 с от захвата до дерева; SC-005 —
папка в 1 000 файлов ≤5 с на desktop; SC-006 — подсветка цели без задержки при 5 000
элементов (проверка цели — принадлежность множеству, без вызова ядра на движение)

**Constraints**: без сети; ни одного потерянного или затёртого файла (SC-002); текст
заметок не переписывается (FR-019); на web ядро в главном потоке — копирование отдаёт
управление между файлами; вызовы wasm сериализованы очередью (`wasm-call-queue.ts`)

**Scale/Scope**: vault до предела 001 (FR-038); дерево до 5 000 элементов; один перенос
одновременно; один элемент за перенос

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Допустимость цели, список и фильтр папок, состав переносимого (папки снимков), разрываемые ссылки, обратный запрос для отмены, обновление индекса/правок/кэша/разделов — в `vault/relocate.rs` и backend. UI хранит ответ `move_targets` как множество, параметры жеста (пороги пикселей и миллисекунд) и рисует. Тесты — общие сценарии на обоих путях. |
| II. Паритет таргетов | PASS | Один метод трейта `move_entry` с одним контрактом исхода; различие исполнения (атомарный `rename` против копирования) скрыто за трейтом. Сценарии `relocate_cases` идут на обоих таргетах; биндинги wasm и команды Tauri 1:1 (contracts/core-move-api.md). |
| III. Спека без реализации | PASS | `spec.md` описывает поведение; `<заметка>.assets` в Clarifications — то, что пользователь видит в своей папке. |
| IV. TS strict, без any | PASS | Типы `Move*` — `Tsify` из Rust, в UI через `import type`. |
| V. Мост к Neovim | N/A | Мост не затрагивается: буфер редактора не пересоздаётся, меняется только путь файла. |
| VI. Local-first | PASS | Перенос — обычные операции с файлами vault; результат виден любому файловому менеджеру. Никаких служебных пометок о переносе в vault не пишется. |
| VII. Офлайн | N/A | Сети нет. |
| VIII. Внешние движки | PASS | Доска переносится как файл; открытая доска получает новый путь тем же путём, что заметка. Новые CSS-правила начинаются с `.ts-tree`/`.ts-palette`, в DOM Excalidraw не попадают. |
| IX. Локальность | PASS | Ничего не покидает устройство. |
| X. Массовые операции | PASS | Перенос папки на web — копирование с прогрессом, отменой между файлами и откатом скопированного (research.md §6): после отмены vault прежний. На native перенос — один атомарный системный вызов вне UI-потока: прерывать нечего, частичного состояния нет. |

Пересмотр после проектирования (Phase 1): статусы не изменились, нарушений нет.

## Project Structure

### Documentation (this feature)

```text
specs/023-tree-drag-drop/
├── plan.md · research.md · data-model.md · quickstart.md
├── contracts/core-move-api.md · contracts/frontend-tree-move.md
├── checklists/requirements.md
└── tasks.md            # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/
├── src/vault/relocate.rs           # типы Move*, move_targets, check_move, path_changes,
│                                   # broken_by_move, общая часть move_entry (apply_move)
├── src/vault/mod.rs                # pub mod relocate; реэкспорт типов
├── src/vault/backend.rs            # + move_targets, move_impact, move_entry в VaultBackend
├── src/vault/index.rs              # + NoteIndex::rename_prefix
├── src/vault/conflict.rs           # + ConflictRegistry::rename_prefix
├── src/vault/cache.rs              # переезд ссылок перенесённых заметок (оба backend кэша)
├── src/vault/native.rs             # move_entry: fs::rename + откат
├── src/vault/web.rs                # move_entry: копирование/удаление, MoveControl;
│                                   # биндинги moveTargets, moveImpact, moveEntry
└── tests/common/relocate_cases.rs · tests/relocate_native.rs · tests/relocate_wasm.rs

packages/shell/src/lib.rs           # команды move_targets, move_impact, move_entry

ui/src/
├── vault-client.ts · vault-client-web.ts · vault-client-tauri.ts   # + moveTargets, moveImpact, moveEntry
├── vault-tree/
│   ├── useMoveEntry.ts             # единый путь переноса: влияние → подтверждение → перенос → итог
│   ├── useTreeDrag.ts              # жест на Pointer Events, цель, раскрытие, прокрутка
│   ├── TreeDragGhost.tsx           # ярлык у указателя
│   ├── MoveToDialog.tsx            # «Переместить в…»
│   ├── MoveNotice.tsx              # сообщение об итоге с «Отменить», прогресс
│   ├── VaultTreeActions.tsx        # data-drop-folder, пункт меню, общая раскрытость папок
│   ├── BacklinksConfirmDialog.tsx  # текст для переноса
│   └── Sidebar.tsx                 # прогресс в нижней строке
├── workspace/Workspace.tsx         # applyPathChanges: открытый файл следует за путём
├── workspace/CommandPalette.tsx    # команда «Переместить в…»
├── i18n/locales/{ru,en}/tree.ts    # ключи tree.move.*
└── styles/app.css                  # .ts-tree--drop-root, .ts-tree__folder--drop,
                                    # .ts-tree__row--dragging, .ts-tree-drag*

scripts/dev/scenarios/tree-move.mjs # сценарий сверки с макетом
```

**Structure Decision**: перенос — часть vault-manager, поэтому модуль `relocate.rs`
рядом с `sections.rs` и `conflict.rs`, а не новый каталог. В UI всё живёт в
`ui/src/vault-tree/`: жест, окно и сообщение принадлежат дереву. `apps/*` не
меняются — оболочки получают фичу через общий UI и `packages/shell`.

## Complexity Tracking

Нарушений конституции нет. Условные решения:

| Решение | Почему | Что упрощает, если не подтвердится |
|---|---|---|
| Команда палитры без `CommandId` (research.md §10) | у переноса нет сочетания по умолчанию | если палитра не умеет действий без `CommandId` — завести `CommandId::MoveEntry` с пустым сочетанием |
| Собственный перенос не виден наблюдателю native (research.md §5) | индекс обновляется до доставки событий `notify`, как у `rename` | если тест покажет события удаления — пометка ожидаемых путей в конвейере изменений |
