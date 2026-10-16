# Data Model: Быстрый захват

## CaptureSettings (`vault.json` → `capture`)

`inbox: String`, `daily_folder: String`, `daily_template: Option<String>`,
`daily_name: DailyName (Iso | IsoNested | Dotted)`, `entry_style: List | Paragraph`,
`entry_time: bool`.

## Capture

| Поле | Тип | Смысл |
|---|---|---|
| `id` | `String` | uuid записи очереди |
| `content` | `Text { text } \| Link { url, title? } \| Image { file, mime, name }` | |
| `note` | `Option<String>` | приписка |
| `at` | `{ hour, minute }` + дата | время захвата по часам устройства |
| `target` | `Inbox \| Daily \| Note { path }` | назначение |

## CaptureOutcome

`EditBuffer(TextEdit)` · `Written { path }` · `Failed { reason: TooLarge | ReadOnly | NoVault | Conflict }`.

## DailyDate

`{ year, month, day }`; `daily_path(settings, date) -> String`;
`DailyNeighbours { previous: Option<String>, next: Option<String> }`.
