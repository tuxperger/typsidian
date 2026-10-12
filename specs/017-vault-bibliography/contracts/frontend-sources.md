# Contract: источники в общем UI

Макет: Claude Design «Typsidian: Typst Editor Design» → `Typsidian Библиография.dc.html`,
экраны 1a (панель), 1b (попап выбора), 1c (отчёт и пустое). Расхождения — research.md §16.
Строки цитирующих заметок — подача макета `Typsidian Связи.dc.html` 1a (013).

## VaultClient

Расширение `ui/src/vault-client.ts`; обе реализации обязательны (`vault-client-web.ts`,
`vault-client-tauri.ts`). Типы — из `wasm-core` через `import type`.

```ts
sources: {
  refresh(registered: string[]): Promise<RefreshStep>;
  summary(): Promise<LibrarySummary>;
  search(query: string, limit: number, uncitedOnly: boolean): Promise<SourceHits>;
  forCompletion(prefix: string, limit: number): Promise<SourceHit[]>;       // для 018
  scanStart(): Promise<ScanView>;
  scanStep(maxNotes: number): Promise<ScanView>;
  scanCancel(): Promise<ScanView>;
  noteChanged(path: string): Promise<void>;
  noteRemoved(path: string): Promise<void>;
  noteRenamed(from: string, to: string): Promise<void>;
  citing(key: string): Promise<CitingNote[]>;
  missingKeys(): Promise<MissingKey[]>;
  checkNote(text: string, knownLabels: string[]): Promise<NoteCheck>;
  planCitation(text: string, caret: number, file: string, key: string): Promise<CitationPlanResult>;
  bibliographyOffer(text: string, notePath: string, file: string): Promise<BibliographyOffer>;
  createFile(): Promise<CreateSourceFileResult>;
};
```

## useSources (`ui/src/sources/useSources.ts`)

Живёт в `Workspace`. Правил здесь нет — только ритм вызовов.

```text
vault открыт                    → do { step = refresh(registered) } while step.pending > 0,
                                  уступая циклу событий между вызовами; revision++
событие SourceFilesChanged      → то же
настройка source_files изменена → то же
NoteCreated / NoteUpdated       → если проход начинался: noteChanged(path); revision++
NoteDeleted / NoteRenamed       → noteRemoved / noteRenamed; revision++
первый показ режима «Источники» → scanStart(); while view.phase == Running:
                                    view = scanStep(50); await yieldToEventLoop()
смена vault, размонтирование    → scanCancel()
```

Отдаёт: `summary`, `scan: ScanView`, `revision`, `refreshing`, действия `createFile()`,
`registerFile(path)` (пишет `source_files` через `useVaultSettings`).

## Отметки в редакторе — общий механизм (`ui/src/editor/marks.ts`)

Заводится этой фичей; 018 кладёт свои отметки тем же вызовом.

```ts
export interface EditorMark { from: number; to: number; tone: "error" | "warning"; message: string }
export const editorMarks: Extension;                       // поле состояния + подсказка по наведению
export function setMarks(view: EditorView, provider: string, marks: EditorMark[]): void;
```

- Отметки хранятся по поставщикам; `setMarks` заменяет набор одного поставщика. 017 —
  `"sources"`, 018 — `"references"`.
- Диапазоны переносятся через правки текста (`mapping`) до следующего `setMarks`; диапазон за
  пределами текста отбрасывается, а не роняет редактор.
- `.ts-mark--error` — красный текст и красный пунктир снизу; `.ts-mark--warning` — жёлтый
  пунктир. Подсказка — `message`, уже переведённый.
- `useCitationMarks` (`ui/src/sources/useCitationMarks.ts`): текст редактора изменился →
  пауза 300 мс → `checkNote(text, [])` → `setMarks(view, "sources", …)`; `revision` из
  `useSources` изменился → без паузы. Ответ на устаревший текст отбрасывается. Итог
  `NoteCheck` идёт и в статус-бар: «N цитат · M битых» (сегмента нет, если цитат нет).

## Палитра «Вставить цитату» (`ui/src/sources/CitePalette.tsx`)

Образец — `ui/src/editor/InsertMenu.tsx`: открывается у каретки (`CaretAnchor`), ничего не
вставляет сама, объявляет выбор наверх; закрывается по Escape (`useDismissOnEscape`), клику
мимо и после выбора.

- Поле запроса; на каждый ввод — `search(query, 50, false)`; ответ на устаревший запрос
  отбрасывается. Строка: «Авторы · год» + название второй строкой + моно-ключ справа; при
  совпадении ключей в разных файлах — имя файла под ключом. Активная строка — фон steel-800
  и оранжевая линия слева. Наведение и ↑↓ выделяют одну и ту же строку.
- Подвал: «↑↓ выбор · ↵ вставить · esc закрыть»; при `total_matches > 50` — «показаны 50 из N».
- Нет файлов источников → пустое состояние макета 1c: «Создать references.bib» и «Указать
  существующий…» (список `summary.yaml_candidates`; пуст — кнопки нет, есть подсказка положить
  файл в папку заметок). Vault только на чтение → обе кнопки недоступны с названной причиной.
- Файлы есть, источников нет → «В файлах источников нет записей» и путь файла.

Выбор → `NoteEditor`:

```text
plan = planCitation(editorText, caret, hit.source.file, hit.source.key)
отказ → сообщение с причиной, текст не меняется
applyEdits(plan.edits); каретка = plan.caret            // тот же applyEdits, что у 008
offer = bibliographyOffer(текст после вставки, notePath, hit.source.file)
offer != None и заметка не в списке «не сейчас» → BibliographyPrompt
```

`BibliographyPrompt` (`ui/src/sources/BibliographyPrompt.tsx`) — ненавязчивый запрос у нижнего
края редактора: «В заметке нет списка литературы. Добавить?» / «Файл … не подключён. Добавить?»
и кнопки «Добавить» (→ `applyEdits(offer.edits)`), «Не сейчас» (путь заметки запоминается до
конца сеанса). Без нажатия «Добавить» текст не меняется (FR-004).

## Панель «Источники» (`ui/src/sources/SourcesPanel.tsx`)

Третий режим колонки заметки (`ui/src/note-panel/`, 013 `contracts/note-panel.md`):
`NotePanelModeSpec { id: "Sources", title: «Источники», badge: «34 записи» }`. Колонка 248px;
на узком окне — шторка снизу.

Сверху вниз:
1. Поле «поиск по автору, году, ключу…» → `search(query, 200, uncitedOnly)`; рядом
   переключатель «Не цитируются» (состояние устройства; недоступен, пока `scan.phase != Done`).
2. Строка прогресса прохода «цитаты: 1 200 из 5 000 · отменить», пока `Running`; после отмены —
   «проход остановлен · продолжить».
3. Список: строка «Автор — Название» + тег с ключом; непроцитированный — приглушён, тег «не
   цитируется» (только при `Done`). Выбранный — карточка с оранжевой линией: авторы, название,
   моно-мета «контейнер · год / ключ · в тексте N раз / файл», кнопка-акцент «Вставить @ключ»
   (→ тот же путь, что выбор в палитре; недоступна без открытой заметки), под ней —
   `citing(key)`: заметка и строки-фрагменты с подсвеченной цитатой; клик →
   `revealPlace({ path, line, column })` (013).
4. Группа «Нет в файлах источников» — `missingKeys()`: красная строка «@ключ» + тег «битая» +
   число заметок; клик → `revealPlace` к первой цитате.
5. Группа «Ошибки в файлах» — `summary.problems`: «файл · строка N · ключ — причина», причина
   из переводов по `kind`, `detail` мелко под ней. Перехода нет: файл источников в
   приложении не открывается (research.md §6).
6. Пустые состояния — те же, что у палитры.

Список длиннее 200 строк не рисуется: «показаны 200 из N — уточните запрос».

## Workspace

- Команды `insert-citation` и `toggle-sources` — через `useShortcuts`, в палитре команд и в
  `ShortcutsSection.tsx`. «Вставить цитату» доступна только при открытой заметке Typst.
- Кнопка «Источники» в шапке: показывает колонку заметки в режиме «Источники»; повторное
  нажатие в этом режиме — скрывает.
- `useSources` и `useCitationMarks` монтируются здесь; `revision` передаётся панели.

## Тексты

`ui/src/i18n/locales/en/sources.ts` и `ru/sources.ts`, подключённые в `en.ts` и `ru.ts`:
названия команд и режима, подписи панели и палитры, причины `SourceProblemKind`,
`CitationPlanError`, `CreateSourceFileError`, `SourceFileProblem`, обе формулировки отметки
(`ambiguous`), названия видов записей Hayagriva (неизвестный вид показывается идентификатором),
формы числа («запись / записи / записей», «раз / раза»).
