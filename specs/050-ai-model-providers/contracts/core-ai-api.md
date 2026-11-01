# Контракт: команды обучения с ИИ

Логика — `packages/core/src/ai`. Команды — `packages/host/src/commands.rs`, обёртки в
`packages/shell`, сервер через белый список, web через `ai/web.rs`.

| Команда | Вход | Выход | Отказ |
| --- | --- | --- | --- |
| `ai_settings_get` | — | `AiSettings?` | vault не открыт |
| `ai_settings_set` | `AiSettings` | — | `BadEndpoint` |
| `ai_key_state` | `provider` | `bool` | `SecretsUnavailable` |
| `ai_key_set` | `provider, key` | — | `SecretsUnavailable` |
| `ai_key_delete` | `provider` | — | `SecretsUnavailable` |
| `ai_models` | `requestId` | `string[]` | `AiFailure` |
| `ai_probe` | `requestId` | `ProbeResult` | `AiFailure` |
| `ai_approve_endpoint` | `insecureOk` | — | `BadEndpoint`, `InsecureNotConfirmed`, `SecretsUnavailable` |
| `ai_cancel` | `requestId` | — | — |
| `ai_journal` | — | `JournalEntry[]` | чтение |
| `ai_journal_clear` | — | `u32` | запись |
| `ai_check_access` | `path` | `bool` | — |

Гарантии:

- При `enabled == false` любая сетевая команда отвечает `Disabled` без запроса (FR-002).
- `ai_key_*` не возвращают значение ключа (FR-008).
- `ai_probe` пишет `sent` до запроса; ошибка записи — `JournalFailed` без запроса (FR-012).
- На сервере `Custom` без `ai_custom_endpoint` — `CustomEndpointDisabled`.
- `Custom` с адресом, не одобренным на этом устройстве (`ai_approve_endpoint`), —
  `EndpointNotApproved` без журнала и сети (research.md §14, M1).
- На сервере адрес во внутренней сети без `sync_private_remotes` — `InternalAddress`
  (адрес числом) или `Offline` (имя, разрешившееся во внутренний адрес).
- `ai_check_access` закрыт при выключенной функции и для путей через ссылки.
