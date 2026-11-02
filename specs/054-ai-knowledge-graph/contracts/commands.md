# Contracts: граф знаний

| Команда | Вход | Выход |
| --- | --- | --- |
| `ai_knowledge_graph` | `query?` | `KnowledgeGraph` |
| `ai_knowledge_chains` | `query?` | `Chain[]` |
| `ai_knowledge_proposition` | `id` | `Proposition` с цепочками |
| `ai_knowledge_extract` | `vaultId`, `source`, `requestId`, `now`, `allowCustom` | `KnowledgeJob` (сеть, весь цикл пачек) |
| `ai_knowledge_jobs` | `vaultId` | `KnowledgeJob[]` |
| `ai_knowledge_remove` | `id` \| `source`, `keep` | — |

`ai_knowledge_extract` — в `AI_NETWORK_COMMANDS` сервера; остальные — в `allowed.rs`.
wasm: `knowledgeGraph`, `knowledgeChains`, `knowledgeProposition`, `knowledgeBegin`/`Send`/
`Finish` (цикл пачек в `vault-client-web.ts`, решения — в ядре), `knowledgeJobs`,
`knowledgeRemove`. Удаление урока (`ai_lesson_delete`, новая) принимает `keep_knowledge`.
