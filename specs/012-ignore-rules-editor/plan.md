# Implementation Plan: Редактор правил исключения

**Branch**: `012-ignore-rules-editor` | **Date**: 2026-10-09 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/012-ignore-rules-editor/spec.md`

## Summary

В панели синхронизации появляется раздел «Правила исключения» с входом в редактор
`.gitignore` и `.stignore` корня vault. Редактор — диалог с текстовым полем на весь файл:
блок приложения подсвечен, под полем — проверка обязательных правил по мере набора,
действие «вернуть правила приложения» и сохранение.

Все решения принимает ядро (`packages/core/src/sync`): чтение файла с причиной, по которой
его нельзя править; проверка текста; возврат блока; сохранение со сверкой версии на диске,
подтверждением неполных правил и отказом посреди операции истории. Новые методы
`SyncSession` выводятся в web через `wasm-bindgen`, в рабочий стол и Android — командами
Tauri; общий UI зовёт их через `SyncClient`. Расчёта действия правил (какие файлы
исключаются) нет — он вне скоупа (Clarifications 2026-10-09).

## Technical Context

**Language/Version**: Rust (edition и toolchain — как в `packages/core`), TypeScript strict

**Primary Dependencies**: уже имеющиеся — `serde`, `tsify-next`, `wasm-bindgen`, `blake3`
(ядро); React, CodeMirror 6 (`ui`). Новых зависимостей нет.

**Storage**: файлы `.gitignore` и `.stignore` в корне vault через `VaultBackend`

**Testing**: `cargo test` и `wasm-bindgen-test` — общие случаи в
`packages/core/tests/common/sync_cases.rs`, запускаемые из `sync_native.rs` и
`sync_wasm.rs`; модульные тесты чистых функций в `ignore.rs`

**Target Platform**: web (wasm32 + File System Access API), рабочий стол и Android (Tauri)

**Project Type**: ядро-библиотека + общий UI + три оболочки

**Performance Goals**: проверка текста по мере набора — чистая функция над строкой до
предела размера заметки, без обращения к диску; ощутимой задержки набора нет

**Constraints**: без сети (FR-019); атомарная запись (FR-004); правила пользователя не
меняются приложением ни на символ (SC-004)

**Scale/Scope**: 2 файла; 4 новых метода ядра, по 4 привязки на оболочку, 1 диалог и
1 раздел панели, ~35 строк локализации на язык

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Можно ли править файл, чего не хватает, что считается расхождением, когда нужно подтверждение, как вернуть блок и какими окончаниями строк писать — решает `packages/core/src/sync/ignore.rs` и `session.rs`. UI показывает ответ ядра и передаёт выбор пользователя флагами (`confirm_incomplete`, `overwrite`). |
| II. Паритет таргетов | Новые методы — на общем `SyncSession` поверх `VaultBackend`, без `cfg`. Случаи сохранения, расхождения и отказов — в `tests/common/sync_cases.rs`, идут и через `cargo test`, и через `wasm-bindgen-test`. Единственная платформенная часть — `VaultBackend::is_symlink` (умолчание `false`, реализация только в native: в File System Access API ссылок нет). |
| III. Спека — поведение | spec.md не называет ни типов, ни библиотек; выбор редактора и формата отметки версии — в research.md. |
| IV. Strict TS | `IgnoreDocument`, `IgnoreCheck`, `IgnoreSaveOutcome` приходят из Rust через `tsify`; в Tauri-клиенте — те же типы (с приведением `null` → отсутствие поля, как у остальных команд `sync_*`). |
| V. Внешние процессы | Не затрагивается: редактор правил — отдельное поле, не редактор заметок. |
| VI. Local-first | Правятся обычные файлы в папке пользователя, в их родном формате; приложение не добавляет в них ничего, чего нет в тексте редактора (FR-003). |
| VII. Офлайн | Сети нет вовсе. |
| VIII. Внешние движки | Git и Syncthing не вызываются: правила — файлы vault, ядро пишет их через `VaultBackend`, движки читают сами. |
| IX. Локальность индексов | Индексы не меняются; файлы правил в индекс заметок по-прежнему не попадают. |
| X. Массовые операции | Массовых операций нет: одно чтение и одна запись файла до предела размера заметки. |

Отклонений нет. Повторная проверка после Phase 1: без изменений.

## Project Structure

### Documentation (this feature)

```text
specs/012-ignore-rules-editor/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── core-ignore-api.md
│   └── frontend-ignore.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/src/
├── sync/ignore.rs           # check, restore_block, окончания строк, отметка версии
├── sync/session.rs          # ignore_open, ignore_save (+ события)
├── sync/mod.rs              # реэкспорт новых типов
├── sync/web.rs              # привязки wasm: ignoreOpen/Check/Restore/Save
├── vault/backend.rs         # VaultBackend::is_symlink (умолчание false)
└── vault/native.rs          # реализация is_symlink
packages/core/tests/common/sync_cases.rs   # общие случаи для native и wasm
packages/shell/src/sync.rs, lib.rs         # команды sync_ignore_*
ui/src/sync/
├── sync-client.ts           # методы SyncClient
├── sync-client-web.ts, sync-client-tauri.ts
├── IgnoreEditor.tsx         # диалог редактора
├── useIgnoreEditor.ts       # состояние диалога: документ, правка, сохранение
└── SyncPanel.tsx            # раздел «Правила исключения», вход из предупреждения
ui/src/i18n/locales/{ru,en}/sync.ts
ui/src/styles/               # стили диалога
```

**Structure Decision**: расширение модуля синхронизации 011 на месте — новые типы и
функции рядом с `ignore::report`/`apply`, которые они переиспользуют; новых пакетов и
модулей верхнего уровня нет.

## Complexity Tracking

Отклонений от конституции нет.
