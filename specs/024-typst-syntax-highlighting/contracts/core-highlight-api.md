# Contract: поверхность ядра — подсветка

Модуль `packages/core/src/highlight/`. Типы — [data-model.md](../data-model.md),
обоснования — [research.md](../research.md).

Функция чистая: ни файловой системы, ни сети, ни состояния между вызовами. Не паникует на
любом тексте: незакрытые скобки, обрыв посреди строки, суррогатные пары, пустой текст.

## Rust

```rust
pub fn highlight(text: &str, windows: &[HighlightWindow]) -> HighlightSpans;

pub struct HighlightSession;
impl HighlightSession {
    pub fn new(text: impl Into<String>) -> Self;
    pub fn text(&self) -> &str;
    pub fn edit(&mut self, edits: &[HighlightEdit], len_utf16: usize) -> bool;
    pub fn highlight(&self, windows: &[HighlightWindow]) -> HighlightSpans;
    pub fn update(&mut self, edits: &[HighlightEdit], len_utf16: usize,
                  windows: &[HighlightWindow]) -> Option<HighlightSpans>;
}
```

Правила, и каждое проверяется тестом (`tests/common/highlight_cases.rs`):

1. Разбор — `typst::syntax::parse` (без сессии) или инкрементальный `Source::edit` (сессия).
   Роль узла — `typst::syntax::highlight` с уточнениями research §4. Флаги начертания — от
   предков `Strong`/`Emph`/`Heading`.
2. Обход листьев дерева. Лист без роли и без флагов в ответ не попадает.
3. Фрагмент попадает в ответ, если пересекает окно. Обход не заходит в поддерево, целиком
   лежащее вне окон.
4. Смещения — UTF-16: без сессии — одним проходом по тексту (`text_offsets::Utf16Index`),
   в сессии — индексом строк `Source`. Общие `utf16_at`/`byte_at` живут в
   `packages/core/src/text_offsets.rs`, `elements` берёт их оттуда.
5. Соседние листья с одинаковой ролью и флагами склеиваются в один фрагмент.
6. Незакрытая пара (скобки, `$`, строка) обрезается концом своего абзаца (research §9).
7. Раскраска сессии после любой цепочки правок равна `highlight` того же текста.
8. Правка вне текста или несошедшаяся длина — `update` отвечает `None`.

## wasm (`highlight/web.rs`)

```rust
#[wasm_bindgen(js_name = SyntaxSession)]
pub struct SyntaxSessionJs(HighlightSession);
// new SyntaxSession(text); session.update(edits, length, windows): Uint32Array | undefined; session.free()
```

Объект своей памяти, `WebBackend` не заимствует: очередь вызовов wasm его не ждёт.

## Tauri (`packages/shell/src/lib.rs`)

```rust
fn syntax_session_open(text: String) -> u32;
fn syntax_session_update(id: u32, edits: Vec<HighlightEdit>, length: usize,
                         windows: Vec<HighlightWindow>) -> Option<Vec<u32>>;
fn syntax_session_close(id: u32);
```

Сессии — состояние окна `HighlightSessions` (`std::sync::Mutex`, не tokio: блокировка
короткая и синхронная). Неизвестный номер — `None`, как расхождение.

## VaultClient (`ui/src/vault-client.ts`)

```ts
openHighlight(text: string): HighlightSession;

interface HighlightSession {
  update(edits, length, windows): Promise<Uint32Array | undefined>;
  updateNow?: (edits, length, windows) => Uint32Array | undefined; // только web
  close(): void;
}
```

Типы `HighlightEdit` и `HighlightWindow` — `import type` из `wasm-core`, перечень ролей —
константа в `ui/src/editor/highlight/roles.ts`. Константа сверяется с Rust тестом ядра
`roles_match_frontend_table`: тест читает файл и сравнивает порядок имён. Тест только
нативный: он сверяет два файла репозитория, а не логику, и в браузере файлов нет.

## Тесты на обоих путях

`tests/highlight_native.rs` и `tests/highlight_wasm.rs` прогоняют одну таблицу
`tests/common/highlight_cases.rs` (принцип II):

- образец из макета, артборд «Редактор»: ожидаемая роль каждого фрагмента (SC-001);
- пять ошибок текущей подсветки, артборд «Было · стало» (SC-004);
- вложенность разметка → код → разметка → формула → код (AS 2.5);
- незакрытые `(`, `"`, `$`, `*` не меняют роли следующего абзаца (AS 3.2, FR-006);
- после ошибки роли восстанавливаются со следующего абзаца (AS 3.4);
- кириллица и эмодзи до фрагмента: смещения в UTF-16 (research §3);
- окна: инварианты 1–5 data-model;
- путь вызова `calc.pow`, `heading.where`, `text.with` — `Function` целиком (AS 2.3);
- сессия: после цепочки правок (вставки, удаления, две правки одной транзакции, кириллица и
  эмодзи) текст и раскраска совпадают с разбором с нуля; расхождение распознаётся.
