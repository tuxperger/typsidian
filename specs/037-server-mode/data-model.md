# Data Model: Серверный режим web

## SQLite сервера (`<данные>/server.sqlite`)

| Таблица | Поля |
|---|---|
| `accounts` | `id` (uuid), `login` (уникальный), `display_name`, `password_hash` (nullable), `blocked`, `created_at` |
| `oidc_links` | `account_id`, `issuer`, `subject` — уникальная пара |
| `sessions` | `id` (uuid, PK — в списке сессий), `token_hash` (уникальный), `account_id`, `csrf`, `created_at`, `last_seen`, `user_agent`, `ip` |

Попытки входа — в памяти процесса (сброс при перезапуске допустим).

## Папка данных

```text
<данные>/server.sqlite
<данные>/users/<account id>/<vault>/…        — обычные vault
<данные>/trash/<время>-<account id>-<vault>/  — удалённые vault
```

## API (JSON, тот же origin)

| Метод и путь | Что |
|---|---|
| `GET /api/info` | имя сервера, есть ли вход паролем, кнопка провайдера |
| `POST /api/login` | `{login, password}` → cookie сессии |
| `GET /api/oidc/start`, `GET /api/oidc/callback` | вход через провайдера |
| `POST /api/logout`, `POST /api/logout-all` | выход; `logout-all` — все сессии аккаунта, текущая тоже |
| `GET /api/me` | аккаунт, CSRF-токен |
| `POST /api/password` | `{current, next}` |
| `GET /api/sessions`, `DELETE /api/sessions/{id}` | сессии |
| `GET/POST /api/vaults`, `GET/PATCH/DELETE /api/vaults/{name}` | свои vault; `GET` одного — число заметок для подтверждения удаления |
| `POST /api/vaults/{name}/invoke/{command}` | команда ядра (белый список) |
| `GET /api/vaults/{name}/events?client={id вкладки}` | SSE: `ready`, `vault` (пачка `VaultEvent`), `lagged`, `progress` (пересчёт свойств) |
| `GET /api/vaults/{name}/file/{путь}` | скачать файл vault (экспорт) |
