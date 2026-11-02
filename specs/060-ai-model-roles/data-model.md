# Data Model: Модели по ролям

- **AiConnection**: `id`, `provider`, `endpoint?`, `insecure_confirmed_for?`.
- **AiRole**: `Teacher` / `Probe` / `Practice` / `Extract` / `Compact` / `Research` / `Speech`.
- **RoleAssignment**: `connection?` (id; пусто — основное), `model?`, `cache`
  (`Provider`/`Short`/`Long`).
- **ResolvedRole**: провайдер, базовый адрес, модель, кэш, ключ-секрет, `fallback`.
- Журнал 050: `Sent.role?`, `Done.cache_write_tokens?`, `Done.cache_read_tokens?`.
- Секреты: `AiConnectionKey(id)`, `AiConnectionOrigin(id)`.
