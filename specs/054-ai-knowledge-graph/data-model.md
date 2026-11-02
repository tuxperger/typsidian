# Data Model: Граф знаний

## Файлы (`<lessons_folder>/Граф знаний/`)

| Файл | Строки |
| --- | --- |
| `propositions.jsonl` | `Phrasing { id, text, source, event?, at }`, `Removed { id, at }` |
| `chains.jsonl` | `Chain { id, premises[], conclusion, source, event?, at }` |
| `sources.jsonl` | `Covered { source, kind: Lesson/Note, upto, at }`, `SourceRemoved { source, keep: bool, at }` |

`source` — путь урока или заметки в vault; при переносе в приложении — строка `Moved` в
`sources.jsonl` (как запрет папки 050 едет за папкой).

## Производное

- **Proposition**: id, формулировки, источники (путь, событие, удалён ли).
- **Chain**: id, посылки, вывод, источники, `incomplete`.
- **KnowledgeGraph**: узлы, рёбра, группы в форме `VaultGraph` 025; счётчики.
- **KnowledgeJob**: id, источник, пачек всего/готово, состояние (`Running`/`Done`/`Failed`/
  `Cancelled`), добавлено утверждений и цепочек.

## Настройки ИИ (дополнение)

| Поле | По умолчанию |
| --- | --- |
| `knowledge_auto` | `true` |
| `knowledge_agent_reads` | `false` |
| `knowledge_map` | `true` |
