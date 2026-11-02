# Contracts: исследование

| Команда | Вход | Выход |
| --- | --- | --- |
| `ai_research_start` | `vaultId`, `path` (урок), `requestId`, `kind`, `question`, `pdf?`, `now`, `allowCustom` | `ResearchState` (весь цикл; сеть) |
| `ai_research_estimate` | `pdf` | `{ units, cost_usd?, pages }` |
| `ai_jobs` | `vaultId` | задания (054 `ai_knowledge_jobs` переименовывается в общий) |

`ai_research_start` — в `AI_NETWORK_COMMANDS`. wasm: `researchBegin/Send/Finish`,
`researchEstimate`. Инструменты модели: `research`, `study_pdf`, `submit_research`.
