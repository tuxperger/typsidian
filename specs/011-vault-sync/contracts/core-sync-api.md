# Contract: API синхронизации ядра (011)

Одна поверхность на оба таргета (принцип II): native — команды Tauri в
`packages/shell` поверх `SyncSession<NativeGitEngine>`, wasm — `WebSyncSession`
(`#[wasm_bindgen]`) поверх `SyncSession<JsGitEngine>`. Формы вызовов совпадают;
UI видит их через `SyncClient` (`ui/src/sync/sync-client.ts`). Ошибки — `SyncError`
(data-model).

## Трейт движка (внутренний шов ядра)

```rust
#[allow(async_fn_in_trait)]
pub trait GitEngine {
    async fn discover(&self, root: &Path) -> Result<RepoProbe, EngineError>;   // Absent | Local | ForeignParent
    async fn init(&self, branch: &str) -> Result<(), EngineError>;
    async fn status(&self) -> Result<Vec<Change>, EngineError>;               // учитывает .gitignore
    async fn commit(&self, paths: &[PathBuf], message: &str, author: &Author,
                    parents: Parents) -> Result<String, EngineError>;          // Parents::Head | Merge(theirs)
    async fn log(&self, path: Option<&Path>, cursor: Option<&str>, limit: u32)
                    -> Result<Page<Snapshot>, EngineError>;
    async fn read_blob(&self, rev: &str, path: &Path) -> Result<Option<Vec<u8>>, EngineError>;
    async fn tree_diff(&self, from: &str, to: &str) -> Result<Vec<Change>, EngineError>;
    async fn merge_base(&self, a: &str, b: &str) -> Result<Option<String>, EngineError>;
    async fn head(&self) -> Result<HeadInfo, EngineError>;                    // ветка, oid, in_progress
    async fn set_head(&self, oid: &str) -> Result<(), EngineError>;            // + индекс по дереву
    async fn fetch(&self, remote: &RemoteSpec, auth: &Auth, cancel: &CancelToken)
                    -> Result<Option<String>, EngineError>;                    // oid удалённой ветки
    async fn push(&self, remote: &RemoteSpec, auth: &Auth, cancel: &CancelToken)
                    -> Result<(), EngineError>;                                // non-ff → RejectedNonFastForward
    async fn clone_into(&self, remote: &RemoteSpec, auth: &Auth, bare: bool,
                    cancel: &CancelToken) -> Result<(), EngineError>;          // clone в vault | зеркало
}
```

Движок **не пишет рабочие файлы vault**, кроме `clone_into` в пустую папку: слияние,
откат и обновление после `pull` пишет ядро через `VaultBackend` (research §1, §9).

## Операции сессии (`SyncSession`)

| Операция | Вход | Выход | FR |
| --- | --- | --- | --- |
| `overview()` | — | `SyncOverview` | FR-035 |
| `ignore_reports()` | — | `Vec<IgnoreReport>` | FR-003, FR-026 |
| `apply_ignore(file)` / `decline_stignore()` | `IgnoreFile` | `()` | FR-027, FR-028 |
| `enable_history()` | — | `Snapshot` (первый) | FR-001, FR-028 |
| `changes()` | — | `Vec<Change>` | FR-004 |
| `diff(path, from, to)` | пути/ревизии (`Working` — рабочий файл) | `Vec<DiffLine>` \| `Binary{..}` | FR-010 |
| `snapshot(paths, message)` | `Option<Vec<PathBuf>>`, `String` | `Snapshot` | FR-005…FR-007, FR-030 |
| `history(path, cursor)` | `Option<PathBuf>` | `Page<Snapshot>` | FR-009, FR-033 |
| `restore(path, snapshot)` | | `FileVersion` (записан) | FR-011 |
| `connect_remote(url, credentials)` | `Credentials { user?, token? }` | `RemoteStatus` | FR-013, FR-014, FR-014a |
| `disconnect_remote()` | | `()` | FR-013, FR-038 |
| `clone_vault(url, credentials, target)` | пустая папка | `VaultId` | FR-013 |
| `push()` / `pull()` | `CancelToken` | `RemoteStatus` / `PullOutcome { applied, conflicts }` | FR-015…FR-020 |
| `abort_pull()` | | `()` | FR-016 |
| `conflicts()` | | `Vec<Conflict>` | FR-016, FR-024, FR-034 |
| `resolve(conflict_id, resolution)` | `Resolution` | `()` | FR-016, FR-025 |
| `set_keeper(this_device)` / `clear_keeper()` | | `Keeper` | FR-029, FR-032 |
| `update_settings(patch)` | `VaultSyncSettingsPatch` | `VaultSyncSettings` | FR-019, FR-031 |
| `syncthing_discover()` | | `Option<SyncthingEndpoint>` | FR-022a (desktop) |
| `syncthing_connect(endpoint, api_key)` | | `SyncthingFolder` | FR-022 |
| `tick(now)` | | `AutoDecision` + выполненное действие | FR-019, FR-031 |

Правила, общие для всех операций:
- Операции записи при `HistoryState::Local { in_progress: Some(_) }` → `InProgressOperation`.
- `snapshot`/`push` на не-хранителе без флага `confirm_not_keeper` → `KeeperElsewhere`.
- `snapshot`/`push` при неразрешённых конфликтах получения → `PullConflictsPending`.
- Перед `snapshot` UI вызывает сохранение открытых заметок (FR-006): `snapshot` принимает
  `flush_token` от `VaultClient.flushAll()` — без него операция отклоняется.

## События

`SyncEvent` в общий поток событий vault: `OverviewChanged(SyncOverview)`,
`ConflictsChanged(u32)`, `Progress { op, done, total }`, `AutoSnapshotDone(Snapshot)`,
`ExchangeFailed(SyncError)` (FR-019: показывается в индикаторе, не модальным окном).
