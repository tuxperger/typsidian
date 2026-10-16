# Data Model: Граф связей

## GraphRequest

`filter: String`, `show_orphans: bool`, `show_attachments: bool`, `show_tags: bool`,
`groups: Vec<GraphGroup>`, `local: Option<LocalGraph { center: String, depth: u8 /* 1..=3 */ }>`.

## GraphGroup

`query: String` (язык условий 020), `color: GroupColor` (токен палитры: `Orange`, `Blue`,
`Green`, `Amber`, `Red`, `Hivis`, `Steel`).

## VaultGraph

| Поле | Тип | Смысл |
|---|---|---|
| `nodes` | `Vec<GraphNode>` | видимые узлы, по `id` |
| `edges` | `Vec<GraphEdge>` | видимые связи |
| `center` | `Option<u32>` | индекс центра локального графа |
| `total_notes` | `u32` | заметок в vault до фильтра (для подписи) |

`GraphNode { id: String, kind: Note | Board | Attachment | Tag, label: String, path:
Option<String>, degree: u32, group: Option<u32> }`.
`GraphEdge { from: u32, to: u32, both_ways: bool }` — индексы в `nodes`, одна запись на пару.

## GraphSettings (`vault.json` → `graph`)

`filter`, `show_orphans` (true), `show_attachments` (false), `show_tags` (false), `arrows`
(false), `groups` ([]), `forces { center: 0.3, repel: 0.6, link: 0.5, distance: 0.45 }`
(0..=1), `label_zoom` (0.4), `node_size` (0.5).
