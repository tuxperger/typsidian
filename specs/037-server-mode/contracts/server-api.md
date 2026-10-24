# Contract: API сервера

Все ответы — JSON. Отказы: `401 {"kind":"Unauthenticated"}`, `403 {"kind":"Forbidden"}`
(в том числе чужой vault — тот же ответ, что несуществующий: `404 {"kind":"NoVault"}`),
`429 {"kind":"TooManyAttempts","retry_after_s":N}`, `400 {"kind":"Invalid","field":…}`.
Изменяющие запросы требуют `X-Typsidian-CSRF`. Ответ `invoke` — ровно то, что вернула бы
команда Tauri (успех — значение, отказ — `400` с телом отказа ядра), поэтому клиент
Tauri работает поверх HTTP без изменений в разборе.

Вызов команды несёт `X-Typsidian-Client` — id вкладки; та же вкладка подписывается на
события с `?client=` в адресе (`EventSource` заголовков не шлёт). После изменяющей команды
(`write`, `create`, `rename`, `delete`, `write_board`, `create_board`, `move_entry`) сервер
рассылает остальным вкладкам vault те же `VaultEvent`, что пришли бы от внешней правки.
`attachments_batch_store` принимает сырое тело (`application/octet-stream`) в том же формате,
что у Tauri; `vault_reindex_properties` отдаёт ход событием SSE `progress` своей вкладке.
Поток событий идёт с `Cache-Control: no-cache, no-transform` и `X-Accel-Buffering: no`:
прокси, сжимающий или копящий ответ, задержал бы события до закрытия потока.
