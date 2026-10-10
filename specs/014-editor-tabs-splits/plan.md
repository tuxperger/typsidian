# Implementation Plan: Вкладки и разделение редактора

**Branch**: `014-editor-tabs-splits` | **Date**: 2026-10-10 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/014-editor-tabs-splits/spec.md`

## Summary

Рабочая область перестаёт быть одной заметкой: открытые заметки и доски стоят вкладками, их
можно поставить в две группы рядом, по истории переходов группы есть «Назад» и «Вперёд», а
набор вкладок vault переживает перезапуск.

Технически — новый модуль ядра `packages/core/src/workspace/`: чистая машина состояний с
одним входом `apply(state, action, facts)` и одним показом `view(state, facts)`
(research.md §1, §2). Значение состояния держит окно; UI отправляет действия и рисует ответ.
`Workspace.tsx` теряет `openNote`/`editorText` — все потребители берут заметку из активной
вкладки активной группы. Текст заметки, чтение и автосохранение уезжают из компонента
`NoteEditor` в документ на путь (`NoteDocument`), общий для всех видов заметки, — этим
обеспечено единое содержимое в двух группах (research.md §3). Набор вкладок хранится в
настройках устройства по образцу `vault_sync` фичи 011 (research.md §11).

Макет — Claude Design, проект «Typsidian: Typst Editor Design»
(`b7a3b1f6-608f-403d-8c07-8101d15dce66`), файл `Typsidian Вкладки.dc.html`, экраны 1a–1c.

## Technical Context

**Language/Version**: Rust 2021 (ядро, shell), TypeScript strict + React 19 (ui)

**Primary Dependencies**: новых нет. Существующие `serde`, `tsify-next`, `wasm-bindgen`,
Tauri 2; в UI — `@codemirror/state` ^6.7, `@codemirror/view` ^6.43 (`scrollSnapshot`,
аннотации транзакций), `@codemirror/commands`

**Storage**: набор вкладок — `Settings.workspaces` в файле настроек устройства
(`NativeStore` / `WebStore`, фича 004); содержимое заметок — как раньше, через `VaultBackend`

**Testing**: `cargo test` и `wasm-bindgen-test` на общей таблице
`packages/core/tests/common/workspace_cases.rs`; `pnpm typecheck`; сценарий
`scripts/dev/scenarios/tabs.mjs` для `scripts/dev/browser.mjs`; ручная проверка по
quickstart.md (JS-тестов в репозитории нет)

**Target Platform**: web (wasm32), desktop Linux (Tauri), Android (Tauri). iOS-оболочки нет —
проверка на iOS не выполняется и в quickstart помечена «не проверено»

**Project Type**: монорепо — Rust-ядро + общий UI + три оболочки

**Performance Goals**: SC-001 — переключение на открытую вкладку ≤100 мс; SC-002 — при 50
вкладках задержка ввода в пределах 10 % от одной; SC-006 — правка видна во второй группе
≤100 мс

**Constraints**: ни один путь закрытия не теряет правок (SC-004); набор вкладок не попадает
в vault, его историю и доставку (FR-022); набор текста не обращается к модели вкладок
(FR-027); на Android путь vault фиксирован — сценариев с выбором папки нет

**Scale/Scope**: 50+ вкладок, 2 группы, история до 100 записей на группу; vault до 10 000
заметок (001)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Порядок и активность вкладок, группы, история, отказ закрытия, различение одноимённых, подача по ширине, восстановление набора — `packages/core/src/workspace/`, тесты на обоих путях. В UI остаётся только принадлежащее редактору: `EditorState` и прокрутка вкладки, пересылка правок между видами одного документа (research.md §3, §4). Правила автосохранения и конфликтов не меняются и остаются за ядром 001. |
| II. Паритет таргетов | PASS | Модуль без платформенного кода; одна таблица тестов, два раннера; биндинги wasm и команды Tauri 1:1 (contracts/core-workspace-api.md). Телефон — это ширина окна, а не таргет: ветвления по платформе нет (research.md §10). |
| III. Спека без реализации | PASS | В `spec.md` нет крейтов, путей кода и схем; ссылки на 001/004/005/010 — на поведение. |
| IV. TS strict, без any | PASS | Все типы модели — `Tsify`, в UI через `import type`; `WorkspaceAction` — размеченное объединение. |
| V. Мост к Neovim | N/A | Редактор — CodeMirror; мост не затрагивается. Отдельный `/speckit-clarify` не требуется. |
| VI. Local-first | PASS | Заметки — те же файлы; набор вкладок — производное состояние на устройстве, его утрата ничего не ломает (FR-023). |
| VII. Офлайн | N/A | Сети нет. |
| VIII. Внешние движки | PASS | Доска — вкладка с тем же `BoardCanvas`; в DOM движка обвязка не входит. Одна доска — один экземпляр движка на окно: синхронизировать два можно было бы только через его внутренности (research.md §8). |
| IX. Локальность | PASS | Ничего не покидает устройство; набор вкладок не уходит и в доставку 011 (FR-022). |
| X. Массовые операции | N/A | Проходов по vault нет; «закрыть остальные» ограничено числом вкладок. |

Пересмотр после проектирования (Phase 1): статусы не изменились. Нарушений, требующих
обоснования, нет.

## Project Structure

### Documentation (this feature)

```text
specs/014-editor-tabs-splits/
├── plan.md · research.md · data-model.md · quickstart.md
├── contracts/core-workspace-api.md · contracts/frontend-workspace.md
├── checklists/requirements.md
└── tasks.md            # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/
├── src/lib.rs                      # pub mod workspace
├── src/workspace/
│   ├── mod.rs                      # типы состояния, действий, ответа; константы
│   ├── model.rs                    # apply: вкладки и группы
│   ├── history.rs                  # история переходов группы
│   ├── view.rs                     # WorkspaceView: подписи, признаки, подача по ширине
│   ├── facts.rs                    # facts_of, apply_with, view_with, restore_with
│   ├── persist.rs                  # SavedWorkspace, snapshot, restore
│   └── web.rs                      # wasm-bindgen: workspaceApply/View/Snapshot/Restore/Empty
├── src/settings/mod.rs             # Settings.workspaces, workspace_for
├── src/settings/apply.rs           # SettingsChange.workspace, route → Device
├── src/settings/format.rs          # ключ workspaces
├── src/settings/logging.rs         # набор из одного workspace в журнал не идёт
├── src/settings/vaults.rs          # забыть vault → убрать его набор
├── src/settings/keybindings.rs     # 7 команд; Shortcut::code() для знаков
└── tests/common/workspace_cases.rs · tests/workspace_native.rs · tests/workspace_wasm.rs

packages/shell/src/lib.rs           # workspace_apply/view/snapshot/restore/empty, discard_edit

ui/src/
├── vault-client.ts · vault-client-web.ts · vault-client-tauri.ts   # workspace*, discardEdit
├── editor/
│   ├── noteDocument.ts             # документ на путь: текст, чтение, автосохранение, виды
│   ├── noteDocuments.ts            # реестр документов
│   ├── useNoteDocument.ts          # привязка документа к React
│   ├── tabEditorStates.ts          # EditorState и прокрутка по TabId
│   ├── syncViews.ts                # расширение CodeMirror: правки между видами документа
│   ├── CodeEditor.tsx              # создание из снимка, document вместо value; без Mod-Alt-\
│   ├── NoteEditor.tsx              # document, tabId, active; без заголовка и «Закрыть»
│   ├── useNoteAutoSave.ts          # правила переезжают в noteDocument.ts, файл удаляется
│   └── useConflictResolution.ts    # показ конфликта по пути активной вкладки
├── boards/BoardTab.tsx             # доска во вкладке: useBoardDocument, flush, beforeunload
├── workspace/
│   ├── Workspace.tsx               # вкладки вместо openNote/editorText; команды, сочетания
│   ├── usePaneLayout.ts            # ширина окна для подачи
│   └── tabs/
│       ├── useWorkspaceTabs.ts     # состояние-значение, очередь действий, события vault, сохранение
│       ├── EditorGroups.tsx · EditorGroup.tsx · TabStrip.tsx
│       ├── OpenNotesSheet.tsx · CloseRefusedNotice.tsx
│       └── tabs-store.ts · tabs-store-web.ts · tabs-store-tauri.ts
├── settings/ShortcutsSection.tsx   # подписи новых команд
├── i18n/locales/ru/workspace.ts · i18n/locales/en/workspace.ts   # workspace.tabs.*
└── styles/app.css                  # .ts-tabs*, .ts-tab*, .ts-groups*, .ts-open-notes*

apps/web/app/VaultWorkspace.tsx · apps/desktop/src/DesktopWorkspace.tsx ·
apps/mobile/src/MobileWorkspace.tsx  # проп tabsStore

scripts/dev/scenarios/tabs.mjs      # сценарий сверки с макетом и замеров
```

**Structure Decision**: новый модуль ядра `workspace/` рядом с `export/`, устроенный так же —
значение состояния и чистые переходы. В UI вкладки живут в `ui/src/workspace/tabs/`, а всё,
что относится к документу и редактору, — в `ui/src/editor/`. `Workspace.tsx` остаётся
композицией: вкладочная часть вынесена в хук и компоненты, чтобы файл не рос.

## Порядок и соседние фичи

- **013** реализуется раньше и заводит колонку заметки `ui/src/note-panel/` поверх `openNote`.
  014 переводит её вход на `view.active` (tasks.md, задача интеграции в US3): колонка и
  предпросмотр следуют за активной вкладкой активной группы (FR-009).
- **013/015/018** открывают заметку «с кареткой на месте» через `revealDiagnostic`; 014
  сохраняет его сигнатуру и проводит через `Open { target }`, то есть через историю
  (research.md §5). «Назад» для 018 — команда `navigate-back` этой фичи.
- **015/017** добавляют режимы той же колонки — для них вход уже `view.active`.
- 013 и 014 обе добавляют поля в `Settings`/`SettingsChange` (состояние колонки и набор
  вкладок): поля независимы, но правятся одни и те же файлы — `settings/mod.rs`,
  `apply.rs`, `format.rs`.
- Расширение `Shortcut::code()` на знаки препинания (Phase 2) делает физическую сверку
  доступной и сочетаниям соседей.

## Complexity Tracking

Нарушений конституции нет.
