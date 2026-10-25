# Contract: API демо-сервера

## `GET /api/info`

```json
{ "name": "Typsidian", "password": false, "demo": { "ttlMinutes": 60, "quotaMb": 50 } }
```

На обычном сервере `demo` нет.

## `POST /api/demo/start` (без сессии)

- `200` + cookie сессии: `{ login, displayName, hasPassword: false, csrf, expiresAt, vault: "Демо" }`.
- `429 {"kind":"TooManyDemos","retry_after_s":N}` — предел по адресу.
- `404` — сервер не в демо-режиме.

## `GET /api/demo/usage`

`{ "used": 13002342, "quota": 52428800 }` — байты vault гостя.

## Отказы

- `401 {"kind":"DemoExpired"}` — срок гостя истёк (любой запрос с его сессией).
- `400 {"kind":"DemoQuota","quota_mb":50}` — запись превысила бы квоту.
- `403` — закрытые точки (§3 research).
