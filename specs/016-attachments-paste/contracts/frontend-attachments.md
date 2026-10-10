# Contract: вложения в общем UI

Макет: Claude Design «Typsidian: Typst Editor Design» → `Typsidian Вложения.dc.html`,
экраны 1a–1c. Расхождения — research.md §16.

## VaultClient

Расширение `ui/src/vault-client.ts`; обязательно в обеих реализациях
(`vault-client-web.ts`, `vault-client-tauri.ts`). Типы — из `wasm-core` через `import type`.

```ts
attachments: {
  start(overrides: VaultOverrides, notePath: string, items: IncomingItem[],
        moment: InsertMoment): Promise<BatchState>;
  next(state: BatchState): Promise<BatchStep>;
  confirm(state: BatchState, index: number, accept: boolean): Promise<BatchState>;
  store(state: BatchState, index: number, bytes: Uint8Array): Promise<BatchState>;
  sourceFailed(state: BatchState, index: number): Promise<BatchState>;
  cancel(state: BatchState): Promise<BatchState>;
  view(state: BatchState): Promise<BatchView>;
  planMarkup(textLength: number, point: InsertionPoint, before: string, after: string,
             paths: string[], block: boolean): Promise<InsertionEdit | undefined>;
  validateFolder(raw: string): Promise<string>;   // отказ — AttachmentsFolderProblem
};
```

## Расширение редактора (ui/src/attachments/editorExtension.ts)

`attachmentExtension(port): Extension` — передаётся в `CodeEditor` новым пропом
`extensions`. Ставится только `NoteEditor` для заметки Typst; у доски и редактора правил
его нет (FR-024, Edge Case «нетекстовый редактор»).

| Событие | Условие перехвата | Действие |
|---|---|---|
| `paste` | `text/plain` пуст и есть файлы (research §3) | `preventDefault`; `port.onFiles(files, { origin: "Clipboard", at: "caret" })` |
| `dragenter`/`dragover` | в `dataTransfer.types` есть `Files` | `preventDefault`; индикатор: строка под указателем, число файлов |
| `dragleave`, `drop`, `dragend` | — | индикатор снят |
| `drop` | есть файлы | `preventDefault`; `port.onFiles(files, { origin: "File", at: началоСтроки })` |
| `drop` | файлов нет, в `text/html` есть `<img` | `preventDefault`; `port.onLinkDropped()` (research §15) |

Прочие `paste` и `drop` не трогаются — поведение прежнее (FR-002).

Индикатор (макет 1a): оранжевая линия 2px на границе строки, под ней тег
«N изображений — сюда, строка L»; рамка редактора `inset 0 0 0 1px var(--orange-500)`; тег
«перетаскивание» в шапке панели. Рисуется декорацией CodeMirror из `StateField`.

Слежение за позицией: `trackPosition(view, pos)` кладёт позицию в `StateField` и переносит
её через каждую правку (`mapPos`); `current()` возвращает `null`, если редактор уничтожен.

## useAttachments (ui/src/attachments/useAttachments.ts)

Живёт в `Workspace`. Вход: `VaultClient`, `VaultOverrides`, путь открытой заметки, порт
вставки `InsertionTarget` (фича 003), состояние доступа vault.

```text
insert(files, { origin, at }):
  если уже идёт пачка → сообщение «дождитесь окончания», выход
  moment = местное время; items = files.map(имя, размер, origin)
  state = start(overrides, notePath, items, moment)
  если state.phase == Refused → уведомление с причиной, выход              (FR-024)
  позиция = at == "caret" ? точка порта : trackPosition(view, at)
  цикл:
    { state, next } = next(state); если next == null: выход из цикла
    Confirm{index} → ответ диалога LargeImageDialog → state = confirm(state, index, ответ)
    Copy{index}    → bytes = await files[index].arrayBuffer()
                     state = store(state, index, bytes)
    await yieldToEventLoop()                                               (SC-005)
  view = view(state)
  если view.markup_paths не пуст и редактор жив:
     edit = planMarkup(длина, точка, before, after, view.markup_paths, block)
     target.apply(edit)                                                    (FR-005)
  уведомление по view                                                       (макет 1b)
```

- `block = at != "caret" || files.length > 1`.
- `cancel()` меняет состояние сразу; цикл увидит его на следующем `next`. Отмена в
  диалоге подтверждения — `confirm(…, false)`, а не отмена пачки.
- Не прочитался сам `File` (`arrayBuffer` отклонён) — `state = sourceFailed(state, index)`:
  ядро записывает `Failed { SourceUnreadable }`, файл назван в отчёте, пачка идёт дальше.
- Смена открытой заметки, смена клиента vault, размонтирование → `cancel`; правка не
  применяется; уведомление «сохранены, но не вставлены» (research §10).
- Прогресс показывается, если элементов больше одного либо копирование длится дольше
  400 мс.

## Выбор файлов (ui/src/attachments/pickFiles.ts)

`pickFiles(source: "files" | "gallery" | "camera"): Promise<File[]>` — скрытый
`<input type="file">` с атрибутами из research.md §2. Отмена диалога — пустой массив;
текст не меняется (FR-022).

Точки вызова «Вставить изображение…» (FR-004):
- палитра ⌘K — пункт «Вставить изображение…»;
- `Mod+Alt+I` через `useShortcuts` (`insert-image`);
- меню вставки ⌘E (`InsertMenu.tsx`) — пункт «Изображение из файла…» рядом с существующим
  элементом `image`;
- на телефоне — шторка `InsertImageSheet` (макет 1c) с действиями по `imageSources`.

Команда недоступна, когда открыта не заметка Typst. В vault только на чтение пункт палитры
показан недоступным с причиной, а сочетание и меню отвечают уведомлением «Vault открыт
только на чтение» (FR-024).

## Уведомления (ui/src/attachments/AttachmentNotices.tsx)

В том же ряду, что `ExportToast` (макет 1b). Тексты — `describe.ts` из каталога
`attachments.*`.

| Панель | Когда | Содержимое |
|---|---|---|
| прогресс (`panel--live`) | пачка идёт | «Копирую вложения: N из M», текущий файл → папка, hazard-полоса, «Отменить», «уже скопированные останутся и будут вставлены» |
| итог | пачка закончена, было больше одного файла или есть что сказать | «Вставлено N из M»; по файлам: имя · «уже лежит в vault, копия не создана» · «сохранён как …»; красным — «Пропущено — вид не поддерживается: …», отказы с причиной, «отменено: K» |
| предупреждение (`panel--warn`) | `excluded_by` не пуст и хоть один файл записан | «Папка вложений не попадает в историю» / «…не доставляется»; кнопка «Открыть правила» → `IgnoreEditor` (012) |
| отказ (`panel--err`) | `Refused`, либо единственный файл не записан | «Изображение не вставлено» + причина + «Текст заметки не изменён» |
| ссылка | `onLinkDropped` | «Перетащена ссылка, а не файл…» |

Одиночная удачная вставка панели не показывает: статус-бар редактора пишет
«вставлено: <имя>» (макет 1a).

`LargeImageDialog` (макет 1b): «Вставить большое изображение?», имя и размер
(`formatBytes`), «будет скопирован как есть, без сжатия», путь назначения, «Отмена» /
«Вставить». `Escape` — «Отмена».

## Настройки vault (ui/src/settings/VaultSection.tsx)

Раздел «Вложения» рядом с «Папкой экспорта» (макет 1c): две радиокнопки с пояснениями и
примером пути, поле «Название папки», строка «Настройка хранится вместе с vault…». Отказ
ядра `InvalidAttachmentsFolder` показывается под полем, прежнее значение остаётся (FR-018).

## Оболочки

`Workspace` получает проп `imageSources?: { camera: boolean; gallery: boolean }` (умолчание
— оба `false`). Мобильная оболочка передаёт его из `apps/mobile/src/MobileWorkspace.tsx`
(research.md §2). Ветвлений по платформе в `ui/` нет.

## Стили и тексты

Все правила CSS — с префиксом `.ts-attach-*` (селекторы без `.ts-*` залетают в Excalidraw,
принцип VIII). Тексты — `ui/src/i18n/locales/{ru,en}/attachments.ts`, ключи настроек — в
`settings.ts`, пункт палитры — в `palette.ts` (FR-026).
