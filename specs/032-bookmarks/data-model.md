# Data Model: Закладки

```text
BookmarkTarget = Note { path } | Section { path, heading } | Search { query: SearchQuery } | Folder { path }
Bookmark       = { id, name, target }
BookmarkEntry  = Item(Bookmark) | Group { id, name, collapsed, items: Vec<Bookmark> }
vault.json     : "bookmarks": [BookmarkEntry…]
```

Вид (ответ ядра): `BookmarksView { entries: Vec<ViewEntry> }`, у закладки — `state`:
`Ready` | `Missing` | `SectionMissing`.

Ответы: `AddOutcome::Added { id, view } | Exists { id, view }`; отказы `BookmarkError`:
`ReadOnly`, `NotFound`, `Io`.

Перемещение: `BookmarkPlace { group: Option<id>, index }` — место среди закладок группы или
верхнего уровня (группы считаются записями верхнего уровня).
