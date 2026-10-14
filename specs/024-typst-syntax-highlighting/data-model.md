# Data Model: Подсветка синтаксиса Typst

Все типы — в `packages/core/src/highlight/`, наружу через `tsify` (wasm) и serde (Tauri).
Состояние одно — сессия редактора (research.md §2); раскраска сессии всегда равна разбору
того же текста с нуля.

## SyntaxRole

Роль фрагмента. Сериализуется числом (`u8`), порядок значений — часть контракта: интерфейс
сопоставляет число классу CSS. Новые роли добавляются только в конец.

| № | Роль | Режим | Источник (research §4) |
|---|---|---|---|
| 0 | `Comment` | все | `Tag::Comment` |
| 1 | `Punctuation` | все | `Tag::Punctuation`, `MathGroupingParens` |
| 2 | `Escape` | разметка, строка | `Tag::Escape` (экранирование, перенос, сокращения) |
| 3 | `MarkupMarker` | разметка | `*`/`_` выделения, ограничители raw |
| 4 | `HeadingMarker` | разметка | `HeadingMarker` |
| 5 | `ListMarker` | разметка | `Tag::ListMarker` (`-`, `+`, `/`, `1.`) |
| 6 | `ListTerm` | разметка | `Tag::ListTerm` |
| 7 | `Link` | разметка | `Tag::Link` |
| 8 | `Raw` | разметка | `Tag::Raw` |
| 9 | `Label` | разметка | `Tag::Label` |
| 10 | `Ref` | разметка | `Tag::Ref` |
| 11 | `MathDelimiter` | формула | `Tag::MathDelimiter` |
| 12 | `MathOperator` | формула | `Tag::MathOperator` |
| 13 | `MathIdent` | формула | `MathIdent` без вызова |
| 14 | `Keyword` | код | `Tag::Keyword` без `Bool`/`None`/`Auto` |
| 15 | `Constant` | код | `Bool`, `None`, `Auto` |
| 16 | `Operator` | код | `Tag::Operator` |
| 17 | `Number` | код | `Tag::Number` (с единицей) |
| 18 | `String` | код | `Tag::String` |
| 19 | `Function` | код, формула | `Tag::Function`, весь путь `FieldAccess` перед вызовом |
| 20 | `ArgName` | код | `Ident` — первый ребёнок `Named` |
| 21 | `Variable` | код | `Tag::Interpolated`, прочие `Ident` |
| 22 | `Plain` | все | текст без роли, но с флагами начертания |

`Tag::Error`, `Text` и `Space` без флагов в ответ не попадают.

## StyleFlags

Битовая маска начертания, унаследованная от предков узла.

| Бит | Флаг | Предок |
|---|---|---|
| 0 | `STRONG` | `Strong` |
| 1 | `EMPH` | `Emph` |
| 2 | `HEADING` | `Heading` |

## HighlightWindow

Диапазон `[from, to)` в кодовых единицах UTF-16. Правила:
- `from <= to <= len_utf16(text)`; окно за концом текста прижимается к концу;
- пересекающиеся и соседние окна сливаются;
- пустой список окон означает весь текст (для тестов и коротких заметок).

## HighlightSpans

Ответ: `packed: Vec<u32>` — тройки `[from, to, role | flags << 8]`.

Инварианты (проверяются тестами ядра):
1. Фрагменты отсортированы по `from` и не пересекаются: `to[i] <= from[i+1]`.
2. `from < to`, оба на границе символа (не внутри суррогатной пары).
3. Каждый фрагмент пересекает хотя бы одно окно. Фрагмент, выходящий за окно, отдаётся
   целиком, а не обрезается.
4. Для одного текста и окон ответ одинаков на native и wasm32 (FR-010).
5. Роль каждой позиции внутри окна не зависит от того, какие ещё окна запрошены: ответы для
   двух окон по отдельности и для их объединения дают внутри окон одни и те же роли.

## HighlightEdit

Правка редактора: `[from, to)` в UTF-16 заменяется на `insert`. Список правок одного вызова
применяется по очереди, координаты каждой — в тексте после предыдущих (клиент шлёт правки
транзакции справа налево).

## HighlightSession

Разобранный текст одного редактора. Переходы:

| Из | Событие | В |
|---|---|---|
| — | `new(text)` | открыта, текст = текст редактора |
| открыта | `update(edits, length, windows)`, длина сошлась | открыта, ответ — фрагменты окон |
| открыта | правка вне текста или длина не сошлась | разошлась: ответ `None`, клиент открывает новую |
| открыта | вид закрыт | закрыта (`free` на web, `syntax_session_close` на Tauri) |

## Связи

`typstHighlight` (UI) → `VaultClient.openHighlight(text)` → wasm `SyntaxSession` / Tauri
`syntax_session_open`, `syntax_session_update`, `syntax_session_close` →
`highlight::HighlightSession`.
