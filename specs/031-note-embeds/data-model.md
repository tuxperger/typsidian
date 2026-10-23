# Data Model: Встраивание заметок

## EmbedTarget (разбор строки)

| Поле | Значение |
|---|---|
| `note` | путь как написан (`Методика`, `/Справка/Метрология`) |
| `part` | `None` · `Section(заголовок)` · `Label(метка)` |

## EmbedEntry (значение `sys.inputs["typsidian-embed"][ключ]`, JSON)

- `{ "path": "/Диссертация/Методика.embed-0.typ" }` — встройка;
- `{ "error": "note" | "part" | "cycle" | "depth" | "board", "target": "…", "chain": [..] }`.

Ключ — строка аргумента в открытой заметке либо `"\u0001<n>"` в виртуальном файле.

## EmbedPlan (ядро → сборка)

| Поле | Тип |
|---|---|
| `inputs` | JSON-строка входов или `None` (вызовов нет) |
| `files` | `Vec<(путь vault, текст)>` — виртуальные файлы |
| `sources` | пути встроенных заметок — в `dependencies` |

## LinkSite (graph, дополнено)

`call: Option<(usize, usize)>` — у встраивания весь вызов `#embed(...)`; у `[[…]]` — `None`.

## MarkKind::MissingEmbedPart (018)

`{ target, part }` — заметка есть, раздела или метки в ней нет.

## Константы

`MAX_DEPTH = 5`; библиотека `.typsidian/lib/embed.typ`, версия 1.
