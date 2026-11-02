# Contracts: импорт заметок

| Команда | Вход | Выход |
| --- | --- | --- |
| `ai_knowledge_import_estimate` | `paths[]` | `ImportEstimate` |
| `ai_knowledge_import` | `vaultId`, `requestId`, `estimate`, `now`, `allowCustom` | `ImportJob` |

`ai_knowledge_import` — в `AI_NETWORK_COMMANDS`; прогресс — `ai_knowledge_jobs` 054,
отмена — `ai_cancel`. wasm: `knowledgeImportEstimate`, цикл частей через
`knowledgeBegin/Send/Finish` 054.
