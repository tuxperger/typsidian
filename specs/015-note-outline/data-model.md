# Data Model: Оглавление заметки

Все типы — в `packages/core/src/structure/` (кроме `HeadingProbe` — `preview/anchor.rs`),
`Serialize + Deserialize`, под wasm32 — `Tsify`. В UI приходят через `import type`.
Обоснования — [research.md](research.md).

## Разбор текста (общий для 015, 017, 018)

### TextPlace

Место в тексте заметки (research.md §3).

| Поле | Тип | Смысл |
|---|---|---|
| `line` | `u32` | строка, с единицы — как `DiagnosticLocation::line` |
| `column` | `u32` | колонка, с единицы — как `DiagnosticLocation::column` |
| `from`, `to` | `u32` | границы в кодовых единицах UTF-16 — как `ParsedElement::from/to` |

Инвариант: `from <= to`; `from` указывает на первый знак записи (`=` у заголовка, `#` у
вызова, `<` у метки, `@` у ссылки).

### NoteStructure

Результат `structure::parse(text)`. Все списки — в порядке текста.

| Поле | Тип |
|---|---|
| `headings` | `Vec<HeadingSite>` |
| `figures` | `Vec<FigureSite>` |
| `labels` | `Vec<LabelSite>` |
| `refs` | `Vec<RefSite>` |

### HeadingSite

| Поле | Тип | Смысл |
|---|---|---|
| `level` | `u8` | 1…; правила — research.md §4 |
| `text` | `String` | читаемый текст (research.md §6) |
| `text_is_source` | `bool` | читаемого текста не осталось, в `text` — начало исходной записи |
| `label` | `Option<String>` | привязанная метка, без угловых скобок |
| `place` | `TextPlace` | запись заголовка целиком, без метки |

### FigureSite

| Поле | Тип | Смысл |
|---|---|---|
| `kind` | `FigureKind` | `Figure` \| `Table` (research.md §5) |
| `caption` | `Option<String>` | читаемый текст подписи; `None` — подписи нет |
| `label` | `Option<String>` | привязанная метка |
| `place` | `TextPlace` | вызов `figure(…)` целиком |

### LabelSite

Определение метки (узел `Label` в разметке; употребления в коде сюда не попадают).

| Поле | Тип | Смысл |
|---|---|---|
| `name` | `String` | без угловых скобок |
| `attached` | `LabelOwner` | `Heading` \| `Figure` \| `Other` — к чему привязана |
| `place` | `TextPlace` | сама метка `<имя>` |

### RefSite

| Поле | Тип | Смысл |
|---|---|---|
| `target` | `String` | имя после `@` |
| `place` | `TextPlace` | ссылка целиком, с дополнением `[…]` |

## Оглавление

### Outline

Результат `structure::outline(text)`; значение держит окно и передаёт обратно в ядро
(research.md §9).

| Поле | Тип | Смысл |
|---|---|---|
| `items` | `Vec<OutlineItem>` | все пункты всех видов в порядке текста |

### OutlineItem

| Поле | Тип | Смысл |
|---|---|---|
| `key` | `String` | устойчивый ключ; для UI непрозрачен (research.md §7) |
| `kind` | `OutlineKind` | `Heading` \| `Figure` \| `Table` \| `Label` |
| `level` | `u8` | уровень отступа: у заголовка — свой, у остальных — уровень раздела + 1 |
| `text` | `String` | заголовок — читаемый текст; рисунок, таблица — подпись; метка — имя |
| `text_is_source` | `bool` | см. `HeadingSite` |
| `label` | `Option<String>` | метка при заголовке или рисунке |
| `parent` | `Option<u32>` | индекс родителя в `items` |
| `place` | `TextPlace` | куда ставится каретка: `place.from` |

Правила построения из `NoteStructure`:
- каждый `HeadingSite` → пункт `Heading`;
- `FigureSite` с подписью → пункт `Figure`/`Table`; без подписи — пункта нет;
- `LabelSite` с `attached = Other` → пункт `Label`; метка рисунка без подписи — тоже пункт
  `Label` (рисунка в оглавлении нет, показать её больше негде);
- `refs` в оглавление не попадают.

### OutlineKinds

Какие виды показывать кроме заголовков (FR-010). Сохраняется между запусками.

| Поле | Тип | По умолчанию |
|---|---|---|
| `figures` | `bool` — рисунки и таблицы | `false` |
| `labels` | `bool` — метки | `false` |

### OutlineRequest

| Поле | Тип | Смысл |
|---|---|---|
| `kinds` | `OutlineKinds` | |
| `collapsed` | `Vec<String>` | ключи свёрнутых заголовков |
| `filter` | `String` | пустая строка — фильтра нет; пробелы по краям не учитываются |
| `caret` | `Option<u32>` | каретка, UTF-16; `None` — подсветка не нужна |

### OutlineView

Результат `outline_view`: ровно то, что рисуется.

| Поле | Тип | Смысл |
|---|---|---|
| `rows` | `Vec<OutlineRow>` | видимые строки по порядку |
| `active` | `Option<String>` | ключ подсвечиваемой строки (research.md §8) |
| `counts` | `OutlineCounts` | `headings`, `figures`, `tables`, `labels` — всего в заметке, без учёта фильтра и свёрнутости |
| `state` | `OutlineState` | `Ready` \| `NoHeadings` \| `NoMatches` |

`state`: `NoHeadings` — в заметке нет ни одного заголовка (FR-012; пункты других видов при
этом всё равно показываются, если включены, пояснение стоит над ними); `NoMatches` — фильтр
задан и ничего не подошло.

### OutlineRow

| Поле | Тип | Смысл |
|---|---|---|
| `key` | `String` | |
| `kind` | `OutlineKind` | |
| `level` | `u8` | отступ |
| `segments` | `Vec<LineSegment>` | текст строки кусками; `highlighted` — совпадение фильтра (`search::LineSegment`) |
| `text_is_source` | `bool` | |
| `label` | `Option<String>` | показывается, только когда `kinds.labels` |
| `has_children` | `bool` | есть вложенные пункты включённых видов |
| `collapsed` | `bool` | ветка свёрнута (при фильтре всегда `false`) |
| `dimmed` | `bool` | строка не подходит под фильтр и показана как родитель |
| `parent` | `Option<String>` | ключ видимого родителя — для клавиш ← и → |
| `place` | `TextPlace` | |

## Переход в предпросмотре

### HeadingProbe (`preview/anchor.rs`)

| Поле | Тип | Смысл |
|---|---|---|
| `text` | `String` | читаемый текст заголовка |
| `occurrence` | `u32` | номер среди заголовков заметки с таким же текстом, с нуля |

`None` вместо пробы — у пункта, который не заголовок, и у заголовка с `text_is_source`.
Результат поиска — существующий `preview::anchor::ResolvedAnchor`.

## Состояние в интерфейсе

Не типы ядра — что и где держит UI (research.md §20).

| Что | Где | Срок жизни |
|---|---|---|
| колонка показана, режим | запись состояния колонки 013 | между запусками (FR-013) |
| `OutlineKinds` | та же запись, новые поля | между запусками (FR-010) |
| `collapsed`, `filter` | память окна, по пути заметки | пока заметка открыта (FR-008) |
| `Outline`, `OutlineView` | память окна | до следующего разбора |

## Переходы

```text
текст заметки ──parse──▶ NoteStructure ──▶ Outline
Outline + OutlineRequest ──outline_view──▶ OutlineView
Outline + OutlineRequest ──outline_active──▶ ключ строки | нет
Outline(прежнее) + Outline(новое) + collapsed ──carry_collapsed──▶ collapsed
Outline + ключ ──heading_probe──▶ HeadingProbe ──locate_heading(прогоны)──▶ ResolvedAnchor | нет
```

Запрещённые состояния: строка в `rows`, родитель которой свёрнут (при пустом фильтре);
`active`, которого нет среди `rows`; ключ в `collapsed` после `carry_collapsed`, которого нет
в новом `Outline`; два пункта с одним ключом.
