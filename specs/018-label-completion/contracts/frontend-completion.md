# Contract: интерфейс дополнения

Каталог `ui/src/completion/`. Правил здесь нет (принцип I): UI выбирает момент вопроса и
рисует ответ ядра. Типы — только `import type` из `ui/src/wasm-core/typsidian_core.js`.

## Клиент

В `VaultClient` (`ui/src/vault-client.ts`, обе реализации):

```ts
completions(request: CompletionRequest): Promise<CompletionList>;
definitionAt(request: DefinitionRequest): Promise<DefinitionOutcome>;
referenceMarks(request: MarksRequest): Promise<ReferenceMark[]>;
```

В `PreviewClient` (`ui/src/preview-client.ts`):

```ts
completionLinkedIndex(notePath: string, dependencies: string[]): Promise<LinkedIndex>;
```

Web-реализация ходит через `wasm-call-queue`, как остальные вызовы ядра.

## `CodeEditor.tsx`

Новые пропы (остальное поведение редактора не меняется):

- `onCompletionTrigger?: (offset: number) => void` — только что набран `@` или второй `[`
  подряд (пользовательский ввод, как у `onSlashMenu`). Ничего больше редактор не решает.
- `extensions?: Extension[]` в отдельном `Compartment` — клавиши открытого списка и
  отметки подключаются снаружи без пересоздания редактора.

## `useCompletion`

Вход: клиент, путь заметки, текст, каретка, `LinkedIndex`, ключи источников (017), настройка
`editor_completion_auto`, доступ к `EditorView`.

- Автопоказ: `onCompletionTrigger` при включённой настройке → запрос `manual: false`.
- Ручной вызов: команда `trigger-completion` → запрос `manual: true` (работает и при
  выключенной настройке, FR-009).
- Пока список открыт, каждое изменение текста или каретки → новый запрос; ответ с номером
  меньше последнего отправленного отбрасывается.
- `target.kind === "None"` или пустые группы → закрыть.
- Выбор строки → одна транзакция CodeMirror: заменить `from..to` на `insert`, каретка — за
  вставленным; список закрыть.
- Смена заметки, потеря фокуса редактором, Escape → закрыть.

## `completionKeys.ts`

Расширение CodeMirror с `Prec.highest`; перехватывает `ArrowUp`, `ArrowDown`, `Enter`, `Tab`,
`Escape` **только** когда список открыт (иначе возвращает `false`, и клавиша идёт своим
путём — FR-003, поля сниппета 003 не страдают).

## `CompletionPopup.tsx` (макет 1a, 1b)

- Привязка к `CodeEditorHandle.caretCoords()`, положение — `useMenuPosition` из
  `InsertMenu.tsx` (вынести в общий модуль, а не копировать).
- Группы с заголовком и числом («Метки · 3», «Источники · 1»); строка: имя с подсветкой
  `matched`, тег вида, текст элемента с обрезкой, имя файла справа.
- Подвал: подсказки клавиш и `preview` выбранной строки.
- Узкий экран: над строкой набора на всю ширину редактора, без подвала с клавишами; выбор
  касанием (`pointerdown` с `preventDefault`, чтобы редактор не терял фокус и клавиатуру).
- `role="listbox"`, строки — `role="option"`, `aria-selected`; у поля редактора —
  `aria-activedescendant` на время показа.

## `useLinkedIndex`

Пересчитывает `LinkedIndex`, когда меняется путь заметки, список `dependencies` последней
сборки или её отметка зависимостей (сигнал — из `useCompiledDocument`). Опоздавший ответ
чужой заметки отбрасывается.

## `useReferenceMarks`

- Запрос отметок — через 300 мс после последней правки, а также при смене `LinkedIndex`,
  свидетельства сборки, ключей источников и по событию vault (создана или удалена заметка).
- `held` = сборка текущего текста ещё не вернулась **и** с последней правки прошло меньше
  800 мс; по приходу сборки или истечении 800 мс запрос повторяется с `held: false`.
- `evidence` — из `PreviewState.shown` (последняя удачная сборка открытой заметки).
- Ответ передаётся в механизм отметок редактора фичи 017 своими видами; подача — жёлтый
  пунктир.
- Счётчик отметок — в строке состояния `NoteEditor.tsx`.

## `MarkExplanation.tsx` (макет 1c)

Пояснение у каретки и по наведению: заголовок-причина, строка-описание, для `MissingNote` —
кнопка «Создать заметку» (`client.create(create_path)`; отказ создания показывается здесь же
словами из `errors`). Для `DuplicateLabel` — перечень остальных мест.

## `useGotoDefinition`

Команда `goto-definition` (сочетание, палитра, контекстное меню редактора):

- `Place` → `revealDiagnostic(location)` из `Workspace.tsx` (014 добавляет запись в историю
  и открытие вкладкой);
- `Source` → показать источник в панели 017;
- `Nothing` → текст причины в строке состояния редактора до следующего движения каретки;
  ничего не открывается.

## Настройки

Выключатель «Предлагать метки и заметки при наборе» с подписью о ручном вызове (сочетание —
из `ShortcutsState.displayOf("trigger-completion")`, не литералом) — `AppearanceSection.tsx`.

## Тексты

Все строки — ключи `editor.complete.*`, `editor.marks.*`, `editor.goto.*` в
`ui/src/i18n/locales/{ru,en}/editor.ts`; настройка — `settings.ts`; команды палитры —
`palette.ts`.
