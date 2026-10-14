# Contract: редактор — подсветка

Каталог `ui/src/editor/highlight/`. Решения — [research.md](../research.md) §6–§7, макет —
временный, https://claude.ai/code/artifact/7c1a68f4-9a9c-4a5c-9aef-165fc970637f.

## Расширение

```ts
export function typstHighlight(client: Pick<VaultClient, "openHighlight">): Extension;
export function typstHighlightFor(client): Extension; // одно расширение на клиента
```

- `ViewPlugin`, у которого `decorations` — `DecorationSet` из `Decoration.mark`. Сессия
  открывается в конструкторе с текстом вида и закрывается в `destroy()`.
- Правки копятся в `ChangeSet` от текста сессии; в запрос уходят справа налево.
- Окна — `view.visibleRanges` (до первого измерения — `view.viewport`), расширенные до строк
  и ещё на 50 строк в каждую сторону (research §2).
- **web** (`updateNow` есть): раскраска считается прямо в `update()` и в конструкторе — в той
  же транзакции, что и правка (research §2).
- **Tauri**: запрос в микрозадаче, не больше одного в пути; правки за время запроса — один
  следующий запрос. Пока ответа нет, `decorations = decorations.map(update.changes)`, а
  пришедший ответ сдвигается правками, накопленными за время запроса.
- Ответ `undefined` (сессия разошлась) — новая сессия с текущим текстом. Исключение вызова
  не ломает редактор: остаётся прежняя раскраска, ошибка пишется в консоль один раз за
  сеанс, повтор — только по новой правке.

## Что уходит из `CodeEditor.tsx`

`typstLanguage` (`StreamLanguage`), `typstHighlight` (`HighlightStyle`), `KEYWORDS`,
`TypstState`, `syntaxHighlighting(…)` и импорты `@lezer/highlight`, если они больше не
нужны. `commentTokens` переезжают в `EditorState.languageData.of(() => [{ commentTokens:
{ line: "//", block: { open: "/*", close: "*/" } } }])`: без этого перестанет работать
`Mod-/` (research §6).

Расширение передаётся в `CodeEditor` пропом `highlight?: Extension`. Он попадает в
`stateFor` через отдельный `Compartment`, потому что сохранённое состояние вкладки (014)
должно узнавать конфигурацию. `NoteEditor` передаёт его только для Typst-заметок (FR-012).

## Классы и цвета

Каждой роли из data-model соответствует класс `ts-syn-<kebab-имя>`, флагам —
`ts-syn-strong`, `ts-syn-emph`, `ts-syn-heading`. Цвета задаются только в `app.css` и только
через алиасы:

| Алиас (`.ts-editor`) | Значение | Роли |
|---|---|---|
| `--ts-syn-comment` | `var(--steel-500)` + italic | `Comment` |
| `--ts-syn-punct` | `var(--steel-400)` | `Punctuation`, `Operator`, `MathOperator`, `MarkupMarker` |
| `--ts-syn-escape` | `color-mix(in srgb, var(--red-600), var(--red-100))` | `Escape` |
| `--ts-syn-heading-marker` | `var(--orange-500)` + bold | `HeadingMarker` |
| `--ts-syn-heading` | `var(--steel-100)` + bold | флаг `heading` |
| `--ts-syn-strong` | `var(--steel-100)` + bold | флаг `strong` |
| — | italic | флаг `emph` |
| `--ts-syn-list-marker` | `var(--orange-400)` | `ListMarker` |
| `--ts-syn-term` | `var(--steel-100)` + semibold | `ListTerm` |
| `--ts-syn-link` | `var(--steel-300)` + подчёркивание `var(--steel-600)` | `Link` |
| `--ts-syn-raw` / `--ts-syn-raw-bg` | `var(--steel-300)` на `var(--steel-900)` | `Raw` |
| `--ts-syn-label` | `var(--steel-400)` | `Label`, `Ref` |
| `--ts-syn-math` | `color-mix(in srgb, var(--blue-600), var(--blue-100))` | `MathIdent`, `MathDelimiter` (+ bold) |
| `--ts-syn-keyword` | `var(--orange-400)` | `Keyword` |
| `--ts-syn-constant` | `color-mix(in srgb, var(--green-600), var(--green-100))` | `Constant`, `Number` |
| `--ts-syn-string` | `var(--hivis-400)` | `String` |
| `--ts-syn-function` | `var(--orange-200)` | `Function` |
| `--ts-syn-arg` | `var(--steel-300)` | `ArgName` |
| — | цвет текста полотна | `Variable`, `Plain` |

Все правила начинаются с `.ts-editor` (селекторы без префикса проваливаются в Excalidraw).
Литералов цвета в правилах нет (FR-009).
