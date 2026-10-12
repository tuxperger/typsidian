# Implementation Plan: Вложения из буфера и перетаскиванием

**Branch**: `016-attachments-paste` | **Date**: 2026-10-10 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/016-attachments-paste/spec.md`

## Summary

Картинка попадает в заметку одним действием — вставкой из буфера, перетаскиванием, выбором
файла, а на Android ещё снимком с камеры и из галереи. Файл ложится в папку вложений
(`attachments` рядом с заметкой по умолчанию либо одна папка vault — настройка vault), в
текст встаёт `#image("…")` с путём от заметки.

Технически это новый модуль ядра `packages/core/src/attachments/`: определение вида по
сигнатуре, имя, папка и путь, сопоставление по содержимому, запрет перезаписи и машина
состояний пачки по образцу `export/batch.rs` (research.md §10). Все оболочки отдают файлы
одним и тем же путём — объектами `File` из событий `paste`/`drop` и из `<input type="file">`
(research.md §1, §2), поэтому ни плагинов Tauri, ни нативных зависимостей не добавляется.
Правка текста уходит в редактор через существующий порт вставки фичи 003 одной транзакцией
(research.md §9).

Макет — Claude Design, проект «Typsidian: Typst Editor Design»
(`b7a3b1f6-608f-403d-8c07-8101d15dce66`), файл `Typsidian Вложения.dc.html`, экраны 1a–1c.

## Technical Context

**Language/Version**: Rust 2021 (ядро, shell), TypeScript strict + React 19 (ui)

**Primary Dependencies**: новых нет. Существующие `regex` (сопоставление правил исключения),
`serde`, `tsify-next`, `wasm-bindgen`, Tauri 2 (`wry 0.57`), CodeMirror 6. `blake3` для
сопоставления содержимого не используется — сравниваются сами байты (research.md §7)

**Storage**: файлы vault через `VaultBackend` (`write_bytes` атомарна с 009); настройка —
таблица `attachments` в `.typsidian/vault.json` (два поля `VaultOverrides`). Индексов и
кэшей фича не заводит

**Testing**: `cargo test` и `wasm-bindgen-test` (ядро, оба таргета, общая таблица
`tests/common/attachment_cases.rs`); `pnpm typecheck`; сценарий headless-браузера
`scripts/dev/scenarios/attachments.mjs`; ручная проверка по quickstart.md (JS-тестов в
репозитории нет)

**Target Platform**: web (wasm32, Chromium), desktop Linux (Tauri), Android (Tauri). iOS нет

**Project Type**: монорепо — Rust-ядро + общий UI + три оболочки

**Performance Goals**: SC-001 — от вставки до картинки в предпросмотре ≤2 с для снимка до
5 МБ; SC-005 — отклик ввода не меняется во время копирования 50 файлов, отмена ≤1 с

**Constraints**: существующий файл не перезаписывается никогда (SC-003); без полузаписанных
файлов при отказе и отмене (FR-014); сети фича не касается вовсе (research.md §15)

**Scale/Scope**: пачка до 50 файлов за действие; порог подтверждения 20 МБ на файл; vault до
10 000 заметок (001)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Вид файла, имя, папка, путь вставки, текст вставки, сопоставление содержимого, разрешение коллизий, порог 20 МБ, переходы пачки, «папка исключена правилами» — в `packages/core/src/attachments/` и `sync/ignore_match.rs`. В UI остаются маршрутизация событий DOM (research.md §3) и цикл пачки. |
| II. Паритет таргетов | PASS | Модуль чистый поверх `VaultBackend`; одна публичная поверхность, биндинги wasm и Tauri 1:1 (contracts/core-attachments-api.md). Новый метод трейта `list_images` реализуется на обоих бэкендах. Общая таблица тестов на обоих таргетах. |
| III. Спека без реализации | PASS | В спеке нет крейтов, путей кода и схем; `#image(...)` упомянут только в исходной реплике пользователя. |
| IV. TS strict, без any | PASS | Все типы — `Tsify` из Rust, в UI через `import type`. |
| V. Мост к Neovim | **PARTIAL** | Вставка текста одной отменяемой правкой в режимах `FullBackend` и `SimplifiedEmulation` — операция через мост. Здесь она не планируется: правка идёт через порт `InsertionTarget` фичи 003, у которого есть только реализация `PlainInput`. См. Complexity Tracking. |
| VI. Local-first | PASS | Вложения — обычные файлы в папке пользователя, видны внешним программам, попадают в историю и доставку на общих основаниях (FR-015). |
| VII. Офлайн | PASS | Сетевых обращений нет: ссылка вместо файла не скачивается ни при какой сети (research.md §15). Поверхность ядра принимает только `VaultBackend`, сетевого клиента в модуле нет; случай «ссылка вместо файла» проверяется сценарием браузера (T066: сетевых запросов нет, текст не изменён) и шагом 13 quickstart. |
| VIII. Внешние движки | N/A | Доска (Excalidraw) не затрагивается: обработчики вставки ставятся только в редактор заметки. |
| IX. Локальность | PASS | Ничего не покидает устройство; индексов нет. |
| X. Массовые операции | PASS | Пачка — машина состояний в ядре, цикл в UI уступает циклу событий между файлами, отмена действует между файлами, запись файла атомарна. Байты одного файла читаются и пишутся асинхронно; синхронная часть шага — определение вида и сравнение байтов (миллисекунды). |

Пересмотр после проектирования (Phase 1): статусы не изменились. Гейт принципа V остался
PARTIAL с тем же условием снятия.

## Project Structure

### Documentation (this feature)

```text
specs/016-attachments-paste/
├── plan.md · research.md · data-model.md · quickstart.md
├── contracts/core-attachments-api.md · contracts/frontend-attachments.md
├── checklists/requirements.md
└── tasks.md            # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/
├── src/lib.rs                      # pub mod attachments
├── src/attachments/
│   ├── mod.rs                      # публичная поверхность, LARGE_IMAGE_BYTES
│   ├── kind.rs                     # ImageKind, sniff
│   ├── folder.rs                   # AttachmentsPlacement, validate_folder, target_folder, markup_path
│   ├── name.rs                     # InsertMoment, generated_name, kept_name, free_name
│   ├── markup.rs                   # image_markup, plan_markup → InsertionEdit
│   ├── store.rs                    # store_item: сопоставление, коллизии, запись
│   ├── batch.rs                    # BatchState и переходы
│   └── web.rs                      # wasm-bindgen
├── src/sync/ignore_match.rs        # excludes(file, text, path) — сопоставление правил
├── src/sync/mod.rs                 # pub mod ignore_match
├── src/vault/mod.rs                # ImageFile, IMAGE_EXTENSIONS, is_image_file
├── src/vault/backend.rs            # VaultBackend::list_images
├── src/vault/native.rs · web.rs    # реализации list_images
├── src/vault/vault_settings.rs     # attachments_placement, attachments_folder, InvalidAttachmentsFolder
├── src/boards/placement.rs         # relative_to → pub(crate)
├── src/preview/resolve.rs          # изображения читаются без предела размера заметки
├── src/settings/keybindings.rs     # CommandId::InsertImage
└── tests/common/attachment_cases.rs · tests/attachments_native.rs · tests/attachments_wasm.rs
    tests/common/mod.rs             # MemoryVault::list_images, pub mod attachment_cases

packages/shell/src/lib.rs           # команды attachments_*

ui/src/
├── vault-client.ts · vault-client-web.ts · vault-client-tauri.ts   # attachments: AttachmentOps
├── attachments/
│   ├── useAttachments.ts           # сбор File[], цикл пачки, отмена, одна правка в конце
│   ├── editorExtension.ts          # paste/dragover/drop в CodeMirror, индикатор, слежение за позицией
│   ├── pickFiles.ts                # <input type="file">: файлы, галерея, камера
│   ├── AttachmentNotices.tsx       # прогресс, итог, предупреждение, отказ
│   ├── LargeImageDialog.tsx        # подтверждение >20 МБ
│   ├── InsertImageSheet.tsx        # шторка источников на телефоне
│   └── describe.ts                 # исходы → тексты
├── editor/CodeEditor.tsx           # проп extensions
├── editor/NoteEditor.tsx           # подключение расширения, статус «вставлено: …»
├── editor/InsertMenu.tsx           # пункт «Изображение из файла…»
├── workspace/Workspace.tsx         # команда, сочетание, палитра, ряд уведомлений, проп imageSources
├── settings/VaultSection.tsx       # раздел «Вложения»
├── i18n/locales/{ru,en}/attachments.ts · {ru,en}.ts · {ru,en}/settings.ts · {ru,en}/palette.ts
└── styles/app.css                  # .ts-attach-*

apps/desktop/src-tauri/tauri.conf.json   # dragDropEnabled: false
apps/mobile/src/MobileWorkspace.tsx      # imageSources: камера и галерея
scripts/dev/scenarios/attachments.mjs    # сценарий сверки с макетом
```

**Structure Decision**: новый модуль ядра `attachments/` рядом с `export/` — та же форма
(чистые правила + машина пачки + `web.rs`). Сопоставление правил исключения живёт в `sync/`
рядом с `ignore.rs`, потому что это знание о файлах правил, а не о вложениях. Каталог UI
`ui/src/attachments/` устроен по образцу `ui/src/export/`. Оболочки получают по одной
правке конфигурации; кода на Kotlin и плагинов нет.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| Принцип V: фича вставляет текст в заметку одной отменяемой правкой, а в режимах `FullBackend` и `SimplifiedEmulation` это операция через msgpack-rpc мост; отдельного `/speckit-clarify` по мосту не было | В `PlainInput` — единственном режиме, работающем сегодня во всех трёх оболочках, — буфер принадлежит браузеру, и вставка моста не касается. Правка уходит через порт `InsertionTarget` (`specs/003-templates-snippets/contracts/insertion-target.md`); реализации порта для двух режимов моста здесь **не планируются и не реализуются**. Обработчики `paste`/`drop` ставятся расширением CodeMirror и вне `PlainInput` не существуют. | «Спланировать вставку через мост заодно» запрещено принципом V прямо. «Отложить фичу до clarify по мосту» задержало бы работу, не снижая риска: риск сосредоточен в двух ненаписанных реализациях порта. **Условие снятия**: сессия `/speckit-clarify` по мосту (вопросы 1–4 из контракта 003) и реализации порта для двух режимов; тогда же решается, как мост узнаёт о `paste`/`drop`. До этого в режимах моста действует только команда «Вставить изображение…», и она отвечает отказом порта `NoEditor` без изменения текста. |
