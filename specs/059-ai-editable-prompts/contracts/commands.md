# Contracts: инструкции агентов

| Команда | Вход | Выход |
| --- | --- | --- |
| `ai_prompts_get` | `language` | `PromptView[]` |
| `ai_prompt_set` | `agent`, `language`, `text` | `PromptView` (пустой → `PromptEmpty`) |
| `ai_prompt_reset` | `agent`, `language` | `PromptView` |
| `ai_prompt_keep` | `agent`, `language` | `PromptView` (принять обновление исходной, оставив свою) |

Сервер: все — в `allowed.rs`, без сети. wasm: `promptsGet`, `promptSet`, `promptReset`,
`promptKeep`.
