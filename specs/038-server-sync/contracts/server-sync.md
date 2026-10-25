# Contract: синхронизация через сервер

Команды идут тем же `POST /api/vaults/{v}/invoke/{command}` (037), аргументы и ответы — как у
команд Tauri 011 (`ui/src/sync/sync-client-tauri.ts`).

## Белый список (добавляется)

`sync_open`, `sync_capabilities`, `sync_overview`, `sync_poll_events`, `sync_progress`,
`sync_note_change`, `sync_cancel`, `sync_ignore_reports`, `sync_apply_ignore`,
`sync_decline_stignore`, `sync_ignore_open`, `sync_ignore_check`, `sync_ignore_restore`,
`sync_ignore_save`, `sync_enable_history`, `sync_changes`, `sync_snapshot`, `sync_history`,
`sync_diff`, `sync_snapshot_changes`, `sync_file_at`, `sync_restore`, `sync_connect_remote`,
`sync_disconnect_remote`, `sync_push`, `sync_pull`, `sync_restore_history`, `sync_abort_pull`,
`sync_conflicts`, `sync_resolve`, `sync_set_keeper`, `sync_clear_keeper`,
`sync_update_settings`, `sync_refresh_mirror`, `sync_tick`, `sync_device_values`,
`sync_set_author`.

Не входят: `sync_syncthing_*`, `sync_clone_vault`, `device_settings`, `apply_device_settings`.

## Особые случаи

- `sync_open` — аргумент `vaultId` игнорируется; сервер подставляет id vault аккаунта.
- `sync_connect_remote` — адрес с внутренним хостом без `sync_private_remotes` →
  `400 {"kind":"UnsupportedScheme","scheme":"internal"}` до вызова ядра.
- Ни один ответ не содержит токена: `RemoteStatus` несёт адрес без учётных данных (ядро
  нормализует адрес), секреты читаются только ядром.

## Новые команды host

- `sync_device_values() -> DeviceVaultSync` — значения синхронизации открытого vault.
- `sync_set_author(author: Option<Author>) -> ()` — автор снимков этого vault.
