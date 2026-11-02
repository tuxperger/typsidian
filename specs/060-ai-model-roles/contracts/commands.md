# Contracts: модели по ролям

| Команда | Вход | Выход |
| --- | --- | --- |
| `ai_settings_set` (050) | `AiSettings` с `connections`, `roles` | — |
| `ai_connection_key_{state,set,delete}` | `vaultId`, `connection` | как у 050 |
| `ai_connection_approve` | `vaultId`, `connection`, `insecureOk` | — |
| `ai_connection_models` | `vaultId`, `connection`, `requestId` | `string[]` (сеть) |
| `ai_roles_resolved` | `vaultId` | `{ role, model, provider, fallback, cache_supported }[]` |

Сервер: свой адрес дополнительного подключения — под `ai_custom_endpoint`, как в 050;
`ai_connection_models` — в `AI_NETWORK_COMMANDS`.
