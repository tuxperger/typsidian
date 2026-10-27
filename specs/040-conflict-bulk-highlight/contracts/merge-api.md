# Contract: команды и методы клиента синхронизации (040)

## Ядро

```text
merge::merge_view(base: Option<&str>, ours: &str, theirs: &str) -> MergeView
merge::merge_annotate(segments: &[Segment], picks: &[Pick], edited: Option<&str>) -> Annotated
merge::AnnotateRequest { segments, picks, edited? }.run() -> Annotated
Session::resolve_many(&mut self, backend, ctx, ids: &[String], how: BulkResolution)
    -> Result<BulkReport, SyncError>
```

`resolve_many` возвращает `Err` только в одном случае: пачка не может начаться (нет
сессии). Сбои отдельных файлов попадают в `BulkReport.failed`.

## wasm (`packages/core/src/sync/web.rs`)

```text
WebSyncSession.resolveMany(backend, ctx, ids: string[], how: BulkResolution) -> Promise<BulkReport>
mergeAnnotate(request: AnnotateRequest) -> Annotated   // свободная функция
```

## host / Tauri / сервер

| Команда | Аргументы (camelCase) | Ответ |
|---|---|---|
| `sync_resolve_many` | `{ ids: string[], how: BulkResolution }` | `BulkReport` / `SyncError` |
| `sync_merge_annotate` | `{ request: AnnotateRequest }` | `Annotated` |

`sync_resolve_many` идёт под блокировкой vault и сессии, как `sync_resolve`.
`sync_merge_annotate` не обращается ни к vault, ни к сессии. Обе команды входят в
`apps/server/src/allowed.rs`.

## SyncClient (`ui/src/sync/sync-client.ts`)

```ts
resolveMany(ids: string[], how: BulkResolution): Promise<BulkReport>;
mergeAnnotate(request: AnnotateRequest): Promise<Annotated>;
```

Ход пачки показывает прежний `progress()` с `operation: "Resolve"`, отменяет прежний
`cancel()`.
