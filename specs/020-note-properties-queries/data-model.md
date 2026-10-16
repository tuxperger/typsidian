# Data Model: Свойства заметок и выборки

**Feature**: 020-note-properties-queries | **Date**: 2026-10-15

## PropertyValue

`Text(String)` · `Number(f64)` · `Date { year, month, day }` · `Flag(bool)` · `List(Vec<String>)` ·
`Unknown { source: String }` (не литерал — только показ, B2).

## Property

| Поле | Тип | Смысл |
|---|---|---|
| `key` | `String` | имя как в записи; `tags` — теги |
| `value` | `PropertyValue` | |
| `place` | `TextPlace` | место значения в тексте (переход из B2) |

## NoteProperties

| Поле | Тип | Смысл |
|---|---|---|
| `properties` | `Vec<Property>` | в порядке записи |
| `editable` | `bool` | запись однозначна (research.md §2) |
| `problem` | `Option<PropertiesProblem>` | `NotLiteral { key }`, `DuplicateKey`, `SecondRecord`, `NotADictionary` |
| `record` | `Option<TextRange>` | где запись; `None` — записи нет |

## PropertyEdit

`Set { key, value }` · `Remove { key }` · `Rename { from, to }` → `EditOutcome` 008.

## Selection

| Поле | Тип | Смысл |
|---|---|---|
| `conditions` | `Vec<Condition>` | все должны выполняться |
| `sort` | `Option<SortKey { key, descending }>` | |
| `columns` | `Vec<String>` | `название`, `папка`, `изменена`, `создана` или ключ свойства |
| `limit` | `u32` | ≤500 |
| `view` | `Table \| List` | |

`Condition`: `TagHas(String)` · `Compare { key, op: Eq/Ne/Lt/Gt/Le/Ge, value }` ·
`Exists(key)` · `Folder(String)` · `Modified(DateRange)` · `Created(DateRange)`;
`DateRange`: `LastDays(u32)` · `Between(Date, Date)`.

## SelectionResult

| Поле | Тип |
|---|---|
| `rows` | `Vec<{ path, cells: Vec<Option<PropertyValue>> }>` |
| `columns` | `Vec<String>` |
| `truncated` | `bool` |

## SelectBlock

Блок `#select(…)` в тексте: `selection`, `key` (канонический текст аргументов), `range`,
`editable` (аргументы — литералы).

## Индекс

`IndexCache` хранит на заметку `Vec<(key, PropertyValue)>` и версию формата. Состояние
пересчёта — `ReindexProgress { done, total, cancelled }` по образцу 001.
