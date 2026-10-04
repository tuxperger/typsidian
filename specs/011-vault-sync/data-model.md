# Data Model: Синхронизация vault (011)

Все типы — в `packages/core/src/sync/`, сериализуются `serde`, на wasm32 — `tsify`
(принцип IV). Пути — относительно корня vault (как в 006).

## Устройство

| Тип | Поля | Где хранится |
| --- | --- | --- |
| `DeviceIdentity` | `id: String` (32 hex, случайный), `name: String` | настройки устройства (004), создаётся при первом запуске |

## Настройки синхронизации

`VaultSyncSettings` — в `.typsidian/vault.json`, ключ `sync` (доставляется с vault):

| Поле | Тип | По умолчанию | Правила |
| --- | --- | --- | --- |
| `remote_url` | `Option<String>` | `None` | без учётных данных (research §10); схемы `https`, `http` (только loopback), `ssh`, `user@host:path` |
| `keeper` | `Option<Keeper>` | `None` | FR-029 |
| `auto_snapshot` | `bool` | `false` | действует только на хранителе (FR-030, FR-031) |
| `auto_snapshot_pause_secs` | `u32` | `300` | 30…86 400 |
| `auto_push` | `bool` | `false` | FR-019 |
| `auto_pull_interval_secs` | `Option<u32>` | `None` | ≥ 60; `None` — только при открытии, если `auto_pull_on_open` |
| `auto_pull_on_open` | `bool` | `false` | FR-019 |

`Keeper { device_id: String, device_name: String, since: Timestamp }`.

`DeviceVaultSync` — в настройках устройства, по `vault_id` (не доставляется):

| Поле | Тип | Назначение |
| --- | --- | --- |
| `author` | `Option<Author { name, email }>` | FR-007 |
| `cors_proxy` | `Option<String>` | только web, FR-037 |
| `syncthing` | `Option<SyncthingEndpoint { address, tls, cert_fingerprint: Option<String> }>` | ключ API — в `SecretStore` |
| `last_seen_keeper` | `Option<String>` | детект спора хранителей (research §4) |
| `stignore_declined` | `bool` | пользователь явно отказался от правил (FR-028) |
| `last_seen_history` | `bool` | история была при прошлом открытии — отличает «удалена» от «не было» (Edge Cases) |

Секреты (`SecretStore`, research §10): `(vault_id, RemoteToken)`, `(vault_id, RemoteUser)`,
`(vault_id, SyncthingApiKey)`.

## Состояние истории

```text
HistoryState
  ├─ Absent                                  // нет истории
  ├─ ForeignParent { repo_root }             // vault внутри чужого репозитория (Edge Cases)
  ├─ Mirror { fetched_at }                   // не-хранитель, история из зеркала (FR-033)
  └─ Local {
       branch: Option<String>,               // None — отделённый HEAD
       in_progress: Option<InProgressOp>,    // Merge | Rebase | CherryPick | Revert | Bisect
       changes: u32,
       ahead: Option<u32>, behind: Option<u32>, counted_at: Option<Timestamp>,
       pull_conflicts: u32,
     }
```

`Change { path, kind: Added | Modified | Deleted | Renamed { from }, binary: bool }`.

`Snapshot { id: String (hex), summary: String, message: String, author: Author,
time: Timestamp, parents: Vec<String>, paths: Vec<Change> }` — `paths` заполняется для
истории файла с учётом переименований (FR-009).

`FileVersion { snapshot: String, path: String, content: Content }`, `Content = Text(String) |
Binary { size: u64 }`.

## Удалённый репозиторий и обмен

`RemoteStatus { url, reachable: Option<bool>, ahead, behind, last_exchange: Option<Timestamp>,
last_error: Option<SyncError> }`.

`SyncError` (`#[serde(tag = "kind")]`, тексты — через i18n 010):
`Offline`, `Timeout`, `AuthRequired`, `AuthRejected`, `NotFound`, `RejectedNonFastForward`
(FR-018), `CorsBlocked` (web без посредника, FR-037), `UnsupportedScheme { scheme }`
(FR-014a), `KeeperElsewhere { keeper }` (FR-030), `KeeperDisputed`, `InProgressOperation`,
`PullConflictsPending` (FR-017), `Io`, `Cancelled`.

## Конфликты

```text
Conflict
  ├─ source: Pull { theirs_snapshot } | Syncthing { copy_path, time, device: DeviceRef }
  ├─ path
  ├─ ours: Content            // текущий файл
  ├─ theirs: Content          // удалённая версия / конфликтная копия
  ├─ base: Option<Content>    // общая версия из истории (FR-016, FR-034)
  └─ auto_merge: Option<MergeResult>
MergeResult = Clean(String) | Hunks(Vec<Hunk>)   // Hunk = Same(lines) | Conflict { ours, theirs, base }
Resolution = KeepOurs | TakeTheirs | Merged(String) | KeepBothAsFile { new_path }  // последнее — только Syncthing
```

Жизненный цикл конфликта получения: `fetch → planned → (resolved | aborted)`; пока есть
неразрешённые — `PullConflictsPending` блокирует снимки и отправку (FR-017). Конфликт
Syncthing: `detected → resolved` (копия удалена или сохранена как файл).

## Syncthing

`SyncthingFolder { detected: bool, link: LinkState, folder_id: Option<String>,
state: Option<FolderState>, progress: Option<f32>, connected_devices: Option<u32>,
devices: Vec<DeviceRef> }`, где `LinkState = NotConfigured | Unreachable | Unauthorized |
UnavailableInShell | Connected`, `FolderState = Idle | Scanning | Syncing | Paused | Error {
message }`.

## Правила исключения

`IgnoreReport { file: StIgnore | GitIgnore, missing: Vec<String>, proposed_block: String,
user_lines: u32 }` — FR-026, FR-027; применение заменяет только блок между маркерами.

## Сводка индикатора (FR-035)

`SyncOverview { history: HistoryState, remote: Option<RemoteStatus>, syncthing:
SyncthingFolder, keeper: Option<Keeper>, this_device_is_keeper: bool, last_snapshot_at:
Option<Timestamp>, conflicts: u32, ignore_warnings: Vec<IgnoreReport>, capabilities:
ShellCapabilities }`.

`ShellCapabilities { ssh: bool, direct_remote: Directness, syncthing_status: bool }`,
`Directness = Always | CorsOnly` (web) — для FR-036.

## Автоснимок

`AutoDecision = Wait { until } | Snapshot | Blocked { reason: NotKeeper | KeeperDisputed |
SyncthingBusy | PullConflictsPending | InProgressOperation | Disabled }`.
