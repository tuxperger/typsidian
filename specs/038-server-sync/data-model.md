# Data Model: Синхронизация на своём сервере

## ShellCapabilities (ядро, расширение)

| Поле | Тип | Смысл |
|---|---|---|
| `server` | bool, по умолчанию `false` | Ядро работает на сервере: адреса «этого устройства» (`file://`, путь, `http://localhost`) отклоняются |

## vault_sync_ids (SQLite сервера)

| Поле | Тип | |
|---|---|---|
| `account_id` | TEXT, FK `accounts(id)` ON DELETE CASCADE | |
| `name` | TEXT | имя vault (папки) |
| `id` | TEXT UNIQUE | 32 hex, id синхронизации |

PK `(account_id, name)`.

## Папка синхронизации аккаунта

```text
<данные>/sync/<аккаунт>/          0700
  device/                         настройки устройства (NativeStore): тождество, значения синхронизации vault
  secrets.json                    0600; ключ — entry_name(id vault, вид), значение — секрет
  mirrors/<id>.git                зеркало истории vault
```

## Конфигурация сервера

| Ключ | По умолчанию | |
|---|---|---|
| `sync_private_remotes` | `false` | Разрешить удалённые репозитории по внутренним адресам |
