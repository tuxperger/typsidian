# Contract: поверхность ядра — строение текста и оглавление

Модуль `packages/core/src/structure/`; поиск заголовка в предпросмотре —
`packages/core/src/preview/anchor.rs`. Типы — [data-model.md](../data-model.md), обоснования —
[research.md](../research.md).

Все функции чистые: ни файловой системы, ни сети, ни состояния между вызовами; не паникуют
на любом тексте, включая незакрытые скобки и обрыв посреди записи.

## Разбор текста — общий для 015, 017, 018

```rust
pub fn parse(text: &str) -> NoteStructure;
```

Правила, и каждое проверяется тестом:

1. Разбор — `typst::syntax::parse`; один обход дерева собирает все четыре списка.
2. Обход не заходит в `Closure`, `ForLoop`, `WhileLoop`, `ShowRule`, `SetRule`,
   `ModuleImport`, `ModuleInclude`; заходит в блоки содержимого аргументов и в ветки `#if`
   (research.md §4).
3. Заголовок — узел `Heading` либо вызов `heading(…)`; уровень — research.md §4.
4. Рисунок или таблица — вызов `figure(…)`; вид и подпись — research.md §5.
5. `labels` — только определения (узел `Label` в разметке). `Label` в коде
   (`#cite(<k>)`, `#link(<a>)`) определением не является.
6. Метка привязана к предыдущему соседу в разметке через пробел без разрыва абзаца.
7. `refs` — узлы `Ref`; цель без `@`.
8. Места — `TextPlace`; пересчёт байт через `typst::syntax::Lines`, один раз построенный на
   вызов (research.md §3).
9. Текст с ошибками разбора даёт всё, что удалось распознать; узлы-ошибки пропускаются.

`parse` и `NoteStructure` — Rust-поверхность. 015 наружу (wasm, Tauri) их не выводит: её
интерфейсу нужно только оглавление. Биндинги к `parse` добавляет та фича, которой он нужен
в UI.

### Расширение для 018 и 017

Правило одно: **новый вопрос к тексту — новое поле `NoteStructure` и новая ветка в
`walk.rs`, а не новый обход и не новый `parse`**. Существующие поля и их смысл не меняются;
тесты `structure_cases` остаются зелёными без правок.

| Фича | Что берёт готовым | Что добавляет |
|---|---|---|
| 018 метки и ссылки | `labels` (имя, место, к чему привязана), `refs` (цель, место), `headings`/`figures` с `label` и текстом — подписи вариантов дополнения | производные функции поверх `NoteStructure`: дубли меток, ссылки без метки; узел под кареткой для «перейти к определению». Разбор не трогает |
| 017 цитаты | `refs` — `@ключ` в тексте; отличить цитату от ссылки на метку можно только сверкой цели с `labels` и ключами `.bib` | поле `cites: Vec<CiteSite>` — вызовы `cite(<ключ>)` (метка в коде, правило 5 уже отделяет её от определений); поле `bibliographies: Vec<BibliographySite>` — вызовы `bibliography(…)` с путями-литералами и местом |

Проход по vault (017 — цитаты во всех заметках, 018 — метки других заметок) вызывает тот же
`parse` на тексте каждого файла внутри своей машины состояний (сквозное решение 5); кэш
разборов, если понадобится, заводит та фича.

## Оглавление

```rust
pub fn outline(text: &str) -> Outline;                       // = Outline::from(&parse(text))
pub fn outline_view(outline: &Outline, request: &OutlineRequest) -> OutlineView;
pub fn outline_active(outline: &Outline, request: &OutlineRequest) -> Option<String>;
pub fn carry_collapsed(prev: &Outline, next: &Outline, collapsed: &[String]) -> Vec<String>;
pub fn heading_probe(outline: &Outline, key: &str) -> Option<HeadingProbe>;
```

### `outline`

- Пункты всех видов в порядке текста; виды отбирает `outline_view`, а не разбор, — включение
  вида не требует повторного разбора.
- Ключи уникальны в пределах оглавления; у двух одинаковых заголовков одного уровня ключи
  различаются порядковым номером.
- `parent` — research.md §7.

### `outline_view`

Порядок, и он проверяется тестами:

1. Отобрать виды по `request.kinds`; заголовки — всегда.
2. Фильтр пуст → скрыть потомков свёрнутых заголовков (`request.collapsed`; неизвестные
   ключи игнорируются).
3. Фильтр не пуст → оставить подходящие пункты и всех их предков; свёрнутость не
   применяется; предок, не подходящий сам, — `dimmed`; совпадение размечено в `segments`.
4. `has_children` считается по включённым видам; `parent` — ключ ближайшего **видимого**
   предка.
5. `active` — как у `outline_active`.
6. `counts` — по всей заметке; `state` — data-model.md.

### `outline_active`

Тот же ответ, что `outline_view(...).active`, без построения строк — для движения каретки.
`request.caret == None` → `None`.

Правило (research.md §8): последний заголовок с `place.from <= caret`; скрыт — ближайший
видимый предок; нет — `None`. Каретка левее первого заголовка → `None`.

### `carry_collapsed`

Ключ есть в `next` → остаётся. Нет, но число заголовков в `prev` и `next` равно → заменяется
ключом заголовка с тем же порядковым номером. Иначе снимается. Результат не содержит ключей,
которых нет в `next`, и повторов.

### `heading_probe`

`None` для пункта не-заголовка, неизвестного ключа и заголовка с `text_is_source`.

## Поиск заголовка в предпросмотре

```rust
// preview/anchor.rs
pub fn locate_heading(runs: &[Vec<TextRun>], probe: &HeadingProbe) -> Option<ResolvedAnchor>;
```

`runs[i]` — прогоны текстового слоя страницы `i` в порядке чтения, как в
`RenderedPage::text_runs`. Правило — research.md §10: склейка соседних прогонов страницы
равна тексту пробы без учёта пробелов; из совпадений — наибольший кегль; из них —
`probe.occurrence`-е по порядку чтения (номер за концом → последнее). `ResolvedAnchor`
указывает на первый прогон совпадения, `exact: true`. Совпадений нет → `None`.

Существующая `resolve(anchor, pages)` не меняется.

## Команда клавиатуры

`CommandId::ToggleOutline` (`"toggle-outline"`, `Mod+Shift+O`) — в конец `COMMANDS`
(после команд 013 и 014, если они уже добавлены). Тест уникальности умолчаний обновляется.

## Биндинги (принцип II: одинаковая поверхность)

| Операция | web (wasm-bindgen, свободные функции) | desktop / Android (Tauri) |
|---|---|---|
| оглавление текста | `noteOutline(text)` | `structure_outline` |
| видимые строки | `noteOutlineView(outline, request)` | `structure_outline_view` |
| текущий раздел | `noteOutlineActive(outline, request)` | `structure_outline_active` |
| перенос свёрнутости | `noteOutlineCarry(prev, next, collapsed)` | `structure_outline_carry` |
| заголовок в предпросмотре | `previewLocateHeading(runs, outline, key)` | `preview_locate_heading` |

`previewLocateHeading` = `heading_probe` + `locate_heading`: проба наружу не выводится.

Функции свободные, объектов ядра не заимствуют — через `wasm-call-queue` не идут (как
`WebBackend.elementAt`). Команды Tauri — без состояния оболочки (как `export_batch_*`).

## Тесты

Общие сценарии — `packages/core/tests/common/structure_cases.rs`; раннеры —
`tests/structure_native.rs` и `tests/structure_wasm.rs` (образец — `element_cases`,
`elements_native.rs`, `elements_wasm.rs`). `locate_heading` проверяется там же на прогонах,
собранных руками, и на документе, собранном настоящим компилятором (как `textlayer_cases`).
