# Contract: ядро — закладки

```text
bookmarks_view(backend) -> BookmarksView
bookmark_add(backend, target, name: Option<String>) -> Result<AddOutcome, BookmarkError>
bookmark_remove(backend, id) / bookmark_rename(backend, id, name)
bookmark_move(backend, id, place: BookmarkPlace)
bookmark_group_create(backend, name) / _rename(id, name) / _delete(id) / _toggle(id)
move_bookmarks(backend, from, to)           // из rename и переноса 023
```

Клиент: `client.bookmarks.{view, add, remove, rename, move, groupCreate, groupRename,
groupDelete, groupToggle}`; ответ правок — новый `BookmarksView`.
