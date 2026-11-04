# Data Model: импорт заметок в граф

Дополнения к 054 (`specs/054-ai-knowledge-graph/data-model.md`). Поля сериализуются как
есть (snake_case), как у `KnowledgeJob`.

- **ImportEstimate**: `requested[]`, `files[] {path, kind: Note/Text/Pdf, chars, parts,
  parts_new, chars_new, hash}`, `skipped[] {path, reason: Denied/Unsupported/NoText/Missing/
  TooLarge}`, `skipped_more`, `parts_new`, `chars_new`, `units` (оценка входных токенов:
  символы / 4 и служебная часть каждого запроса), `cost_usd?` (нет цены — `null`, не ноль),
  `model?`.
- **ImportJob** — это `KnowledgeJob` с непустым `files[] {path, state: Pending/Running/Done/
  Partial/Failed/Skipped, reason?, failure?, added_propositions, parts_done, parts}`; `source`
  пуст, `batches_total/batches_done` считают части. Файлы идут в порядке оценки, пропущенные —
  после них.
- `sources.jsonl`: `Covered { source, kind: Note, part, hash, at }` — покрытие по частям;
  у строк урока `part` и `hash` нет.
