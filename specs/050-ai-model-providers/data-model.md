# Data Model: Провайдеры моделей

## AiSettings (`.typsidian/ai.json`)

| Поле | Тип | Правила |
| --- | --- | --- |
| `enabled` | bool | `true` только после подтверждения в UI |
| `provider` | `AiProvider` | `Anthropic` \| `OpenAi` \| `Custom` |
| `custom_endpoint` | `Option<String>` | только для `Custom`, проверяется `classify_endpoint` |
| `insecure_confirmed_for` | `Option<String>` | адрес, для которого подтверждено незашифрованное соединение; другой адрес подтверждения не имеет |
| `model` | `Option<String>` | имя модели; пусто — проверка запрещена |
| `denied_folders` | `Vec<String>` | относительные пути папок, без дублей |

Ключа здесь нет.

## Секрет

`SecretKind::{AiAnthropicKey, AiOpenAiKey, AiCustomKey}`, запись `vault_id/kind`.

## JournalLine (`.typsidian/ai-journal.jsonl`, строка JSON)

- `sent`: `id`, `at`, `provider`, `model`, `endpoint`, `request` (дословный JSON без заголовков)
- `done`: `id`, `at`, `outcome`, `input_tokens?`, `output_tokens?`, `cost_usd?`

## JournalEntry (проекция для UI)

`sent` + необязательный `done`. Нет `done` — состояние «оборвано». Новые сверху, не больше 500.

## AiFailure

`Offline`, `Timeout`, `Cancelled`, `BadKey`, `Billing`, `Forbidden`, `ModelNotFound`, `RateLimited`,
`ProviderDown`, `BlockedByBrowser`, `Redirected`, `InsecureNotConfirmed`, `BadEndpoint`,
`CustomEndpointDisabled`, `BadResponse`, `JournalFailed`, `SecretsUnavailable`, `Disabled`.
Варианты без текста: сообщение строит UI из локалей; текст сервера и ключ в них не попадают.

## ModelPrice

`model_id`, `input_per_mtok`, `output_per_mtok`; `estimate(model, input, output) -> Option<f64>`.
