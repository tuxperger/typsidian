# Contract: поверхность ядра для связей заметки

Модули `packages/core/src/graph/` (разбор и запись ссылок) и `packages/core/src/links/`
(панель). Типы — [data-model.md](../data-model.md).

## Разбор и запись ссылок (`graph`)

```rust
pub fn link_sites(content: &str) -> Vec<LinkSite>;
pub fn extract_links(content: &str) -> Vec<String>;      // = link_sites(..).map(raw), поведение прежнее
pub fn resolve_link_target(source: &Path, raw: &str) -> Option<PathBuf>; // + сохраняет `.excalidraw`
pub fn link_text(source: &Path, target: &Path) -> Option<String>;
```

`link_text` (FR-017, research.md §9):

- цель в той же папке, что источник → `[[название]]`; иначе `[[/путь/название]]`;
- `.typ` опускается, `.excalidraw` остаётся;
- гарантия, проверяемая тестом на таблице путей: `resolve_link_target(source, внутренность
  результата) == Some(target)`;
- `None`, если путь цели содержит `]]` или обратная проверка не сошлась.

## Обратные ссылки

```rust
pub const BACKLINKS_PAGE_MAX: u32 = 50;

pub async fn backlinks_page<B: VaultBackend>(
    backend: &B,
    target: &str,
    open_texts: &[OpenText],
    offset: u32,
    limit: u32,              // прижимается к 1..=BACKLINKS_PAGE_MAX
) -> BacklinksPage
```

Порядок и правила — research.md §4. Не пишет в vault. Не возвращает `Result`: нечитаемый
источник — группа с `problem`, несуществующая цель — пустая страница.

## Упоминания

```rust
pub const MIN_MENTION_TITLE_CHARS: usize = 3;
pub const MENTION_STEP_NOTES: usize = 64;
pub const MENTION_STEP_BYTES: usize = 1024 * 1024;

pub fn title_of(path: &str) -> String;
pub fn availability(target: &str) -> MentionsAvailability;
pub fn mentions_in_text(text: &str, title: &str) -> Vec<SiteRow>;   // чистая; правила research.md §5

// модуль links::mentions
pub async fn start<B: VaultBackend>(backend: &B, target: &str) -> Result<MentionScan, MentionsAvailability>;
pub async fn step<B: VaultBackend>(backend: &B, state: MentionScan, open_texts: &[OpenText]) -> MentionStep;
pub fn cancel(state: MentionScan) -> MentionScan;
pub async fn in_notes<B: VaultBackend>(
    backend: &B, target: &str, paths: &[String], open_texts: &[OpenText],
) -> Vec<SourceGroup>;
```

- `start` отвечает `Err(TitleTooShort)` и ничего не читает (FR-011).
- `step` читает не больше бюджета, не пишет в vault, после `cancel` не читает ничего.
- Досок, служебных файлов и самой цели среди источников нет.

## «Связать»

```rust
pub fn plan_link(text: &str, request: &LinkRequest, link: &str) -> Result<LinkPlan, Stale>; // чистая
pub async fn link_mention<B: VaultBackend>(
    backend: &mut B,
    request: &LinkRequest,
    editor_text: Option<&str>,   // Some → заметка открыта в редакторе
) -> LinkOutcome
```

Порядок шагов `link_mention` — research.md §9, проверяется тестами в этом порядке. Гарантии:

1. Единственная запись в vault — `write(source, new_text, Some(expected))` в исходе
   `Written`; во всех прочих исходах vault не тронут (FR-015, FR-018, SC-006).
2. `new_text` отличается от прочитанного только диапазоном `request.place`, заменённым на
   `link` (FR-014, SC-005).
3. При `editor_text = Some` на диск не пишется ничего (FR-016).
4. Функция не паникует; каждый исход — вариант `LinkOutcome`.

## Состояние колонки (`settings`)

`Settings.note_panel: NotePanelState`, `SettingsChange.note_panel: Option<NotePanelState>`,
конструктор `SettingsChange::note_panel(state)`. `apply::route` отправляет поле в устройство
при любом `vault_is_open`; `reset_to_defaults` и `device_defaults_change` его не содержат.

## Команда клавиатуры

`CommandId::ToggleNotePanel` (`"toggle-note-panel"`, `Mod+Shift+L`) — в конец `COMMANDS`.

## Биндинги (принцип II: одинаковая поверхность)

| Операция | web (wasm-bindgen, `links/web.rs`) | desktop/Android (Tauri, `packages/shell/src/lib.rs`) |
|---|---|---|
| страница обратных ссылок | `linksBacklinksPage(backend, target, openTexts, offset, limit)` | `links_backlinks_page` |
| доступность упоминаний | `linksMentionsAvailability(target)` | `links_mentions_availability` |
| проход | `linksMentionsStart(backend, target)` / `linksMentionsStep(backend, state, openTexts)` / `linksMentionsCancel(state)` | `links_mentions_start` / `links_mentions_step` / `links_mentions_cancel` |
| точечно | `linksMentionsIn(backend, target, paths, openTexts)` | `links_mentions_in` |
| «Связать» | `linksLinkMention(backend, request, editorText?)` | `links_link_mention` |

`start` наружу отдаёт размеченное объединение `{ kind: "Started", state } | { kind:
"Unavailable", reason }`, а не `Result`: отказ здесь — штатный ответ, а не ошибка вызова.
На web все вызовы с `backend` идут через `wasm-call-queue`, как `search` и `exportPdf`.

## Тесты (оба таргета)

`packages/core/tests/common/links_cases.rs` + `links_native.rs` + `links_wasm.rs`, на
`MemoryVault`. Двойнику добавляются: проверка `expected` в `write`, объявляемые
несохранённые правки (`has_unsaved_edits`) и счётчик записей — без них гарантии 1 и 3 и
исход `Stale` по `Conflict` проверить нечем. Состояние колонки —
`packages/core/tests/common/` + существующие `settings_native.rs` / `settings_wasm.rs`.
