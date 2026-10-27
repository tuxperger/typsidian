# Data Model: 040

Все типы — в `packages/core/src/sync`, `Serialize`/`Deserialize`, на wasm — Tsify.

## LineOrigin (`merge.rs`)

`enum LineOrigin { Same, Ours, Theirs, Both, Manual }`, сериализуется строкой.

| Значение | В колонке версии | В результате |
|---|---|---|
| `Same` | строка не тронута | общий участок без правок |
| `Ours` | правка своей стороны | взято из своей версии |
| `Theirs` | правка их стороны | взято из их версии |
| `Both` | (только общая колонка) тронута обеими | одинаковая правка обеих |
| `Manual` | — | изменено вручную |

## Segment (`merge.rs`)

`#[serde(tag = "kind")] enum Segment`:

- `Agreed { text: String, origin: LineOrigin }`: участок без спора. Соседние участки с
  одинаковым `origin` склеены. `origin` бывает `Same`, `Ours`, `Theirs` или `Both`.
- `Disputed { base: String, ours: String, theirs: String }`: спорный участок, тот же,
  что `Hunk::Conflict`.

Инвариант: склейка `Agreed.text` и `ours` спорных участков даёт `text_preferring(true)`.
Число `Disputed` равно числу `Hunk::Conflict` в `auto_merge`.

## MergeView (`merge.rs`)

```text
MergeView {
  ours:   Vec<LineOrigin>,          // по строке своей версии
  theirs: Vec<LineOrigin>,          // по строке их версии
  base:   Option<Vec<LineOrigin>>,  // по строке общей; None — общей нет
  segments: Option<Vec<Segment>>,   // None — общей нет, совмещать нечего (как auto_merge)
}
```

Строки — `split_inclusive('\n')`, как в `merge3`. Длины векторов равны числу строк версии.

## Conflict (`session.rs`, дополнение)

`view: Option<MergeView>` — `Some`, когда своя и их версии текстовые; `None` для двоичных и
отсутствующих.

## Pick, Annotated (`merge.rs`)

- `enum Pick { Ours, Theirs, Both }`: выбор в спорном участке, по одному на `Disputed`
  в порядке текста. Недостающие считаются `Ours`.
- `Annotated { text: String, lines: Vec<LineOrigin> }`: `lines.len()` равно числу строк
  `text`.

`merge_annotate(segments: &[Segment], picks: &[Pick], edited: Option<&str>) -> Annotated`.
Запрос интерфейса и команды — `AnnotateRequest { segments, picks, edited? }`.

## BulkResolution, BulkReport (`session.rs`)

- `#[serde(tag = "kind")] enum BulkResolution { KeepOurs, TakeTheirs, AcceptClean }`
- `BulkReport { resolved: Vec<String>, skipped: Vec<String>, failed: Vec<BulkFailure>,
  cancelled: bool }`: в списках id конфликтов.
- `BulkFailure { id: String, path: String, error: SyncError }`

Переходы: каждый id из запроса попадает ровно в один список. `resolved` — конфликта больше
нет. `skipped` — не трогали (нечистый при `AcceptClean` или после отмены). `failed` —
конфликт остался, причина в `error`.

## Operation (`mod.rs`, дополнение)

`Operation::Resolve` — ход пачки в `SyncProgress { done, total }`.
