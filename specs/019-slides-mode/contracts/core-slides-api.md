# Contract: API ядра показа слайдов

Модуль `packages/core/src/slides/`. Функции чистые; wasm-биндинги (`slides/web.rs`) и
команды Tauri (`packages/shell/src/lib.rs`) — одна поверхность 1:1. В UI — методы
`VaultClient.slides` (обе реализации клиента).

## Сбор при сборке (005)

```rust
// slides/collect.rs — вызывается из preview/compile.rs при Render::Pages
pub(crate) fn collect(
    document: &PagedDocument,
    world: &dyn World,
    main: FileId,
    source: &str,          // текст главного файла — для перевода в UTF-16
) -> Vec<PageSlideInfo>;
```

`CompiledDocument` получает `pub slides: Vec<PageSlideInfo>` (`#[serde(default)]`). Контракт
005 в остальном не меняется: `pages`, `digest`, `anchor` те же.

## Колода

| Rust | wasm | Tauri | Возврат |
|---|---|---|---|
| `start(slides, caret: Option<u32>, now_ms) -> Result<ShowState, SlidesError>` | `slidesStart` | `slides_start` | состояние или `EmptyDeck` |
| `navigate(state, slides, action) -> ShowState` | `slidesNavigate` | `slides_navigate` | новое состояние |
| `carry(state, slides) -> Result<ShowState, SlidesError>` | `slidesCarry` | `slides_carry` | после пересборки |
| `caret_at_page(slides, page) -> Option<SlidePlace>` | `slidesCaret` | `slides_caret` | куда поставить каретку на выходе; `None` — на странице и до неё нет текста заметки |
| `slide_label(slides, page) -> SlideLabel` | `slidesLabel` | `slides_label` | подписи |
| `show_keymap() -> ShowKeymap` | `slidesKeymap` | `slides_keymap` | таблица клавиш показа |

У окна показа нет ядра (research.md §7), поэтому правило клавиш (research.md §6) приезжает в
окно таблицей: `ShowKeymap` — список `{ key, action }` по `KeyboardEvent.key`, построенный
ядром. Окно редактора кладёт его в сообщение `hello` (contracts/show-channel.md); окно
показа сверяет нажатие с таблицей и своего правила не заводит. Набор цифр и `Enter`/`Escape`
при наборе — поведение поля номера (A3), а не таблицы.

## Ошибки

`SlidesError::EmptyDeck` — у сборки нет страниц. Сообщение — `AppMessage` (010).

## Тесты (оба пути, `tests/common/slides_cases.rs`)

- три страницы, каретка во второй → `start` даёт страницу 1; каретка до первого текста → 0;
- пошаговый слайд (3 страницы с одинаковым текстом) → каретка ведёт на первый шаг;
- `#metadata((t: "Note", v: "…")) <pdfpc>` на странице 2 → `notes` только у неё; две заметки на
  странице склеены; `v` — содержимое `[*жирно*]` → «жирно»;
- `LogicalSlide` 1,1,2,3 → `slide_label` на странице 1 — слайд 1 из 3, шаг 2 из 2, дальше —
  страница 2;
- без `<pdfpc>` → подписи по страницам, шагов нет;
- `navigate` на границах; `GoToSlide(99)` → последняя; `GoToSlide(2)` при логических номерах →
  первая страница слайда 2;
- `carry` при уменьшении страниц → последняя; до нуля страниц → `EmptyDeck`;
- `caret_at_page` у страницы-картинки → `source_start` предыдущей;
- страница с `#include` из другого файла → `source_start = None`;
- сбор `slides` не меняет `pages` и `digest` (регрессия 005).
