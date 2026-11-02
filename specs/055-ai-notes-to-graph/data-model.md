# Data Model: импорт заметок в граф

Дополнения к 054 (`specs/054-ai-knowledge-graph/data-model.md`).

- **ImportEstimate**: `files[] {path, kind: Note/Text/Pdf, chars, parts}`, `skipped[] {path,
  reason: Denied/Unsupported/NoText/Missing}`, `units`, `cost_usd?`, `model`.
- **ImportJob** (вид `KnowledgeJob`): `files[] {path, state: Pending/Running/Done/Partial/
  Failed/Skipped, reason?, added_propositions, parts_done, parts}`.
- `sources.jsonl`: `Covered { source, kind: Note, part, hash, at }` — покрытие по частям.
