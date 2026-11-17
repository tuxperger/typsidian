# Contracts: исследование

| Команда | Вход | Выход |
| --- | --- | --- |
| `ai_research_start` | `vaultId`, `path` (урок), `requestId`, `kind`, `question`, `pdf?`, `now`, `allowCustom` | `ResearchState` (весь цикл; сеть) |
| `ai_research_estimate` | `pdf` | `{ units, cost_usd?, pages }` |
| `ai_jobs` | `vaultId` | задания (054 `ai_knowledge_jobs` переименовывается в общий) |

`ai_research_start` — в `AI_BACKGROUND_COMMANDS` (фоновое место аккаунта, место под запрос — на один запрос к модели); `ai_research_estimate` делит место с оценкой импорта. wasm: `researchBegin/Send/Finish`,
`researchEstimate`, `aiJobs`. Инструменты модели: `research`, `study_pdf`, `submit_research`.
