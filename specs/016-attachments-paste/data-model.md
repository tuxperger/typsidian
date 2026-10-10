# Data Model: Вложения из буфера и перетаскиванием

Все типы — в `packages/core/src/attachments/` (кроме помеченных), с `Serialize`,
`Deserialize` и `Tsify` под wasm32. В UI попадают через `import type`.

## Настройка вложений vault

Поля `VaultOverrides` (`vault/vault_settings.rs`), таблица `attachments` файла
`.typsidian/vault.json`.

| Поле | Тип | Умолчание | Ключ в файле |
|---|---|---|---|
| `attachments_placement` | `Option<AttachmentsPlacement>` | `NextToNote` | `attachments.placement` |
| `attachments_folder` | `Option<String>` | `"attachments"` | `attachments.folder` |

```rust
#[serde(rename_all = "kebab-case")]
pub enum AttachmentsPlacement { NextToNote, VaultFolder }   // "next-to-note" | "vault-folder"

#[serde(tag = "kind")]
pub enum AttachmentsFolderProblem { Empty, Absolute, LeavesVault, ServiceFolder }
```

Правила (FR-016 … FR-019):
- `attachments_folder` хранится в нормальной форме: части через `/`, без краевых `/`.
- Запись негодного названия отклоняется `VaultSettingsError::InvalidAttachmentsFolder
  { problem }`, файл не тронут. Негодное значение в файле при чтении отбрасывается —
  действует умолчание.
- Смена настройки ничего не перемещает: она читается только при следующей вставке.

## Вложение

Файл изображения в vault. Отдельной записи о нём приложение не хранит: это файл.

```rust
pub enum ImageKind { Png, Jpeg, Gif, Webp, Svg }
// extension(): "png" | "jpg" | "gif" | "webp" | "svg"
// принимает расширения: png; jpg, jpeg; gif; webp; svg

// vault/mod.rs
pub struct ImageFile { pub path: PathBuf, pub size: u64 }   // путь от корня vault
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg"];
```

`is_image_file(path)` — по расширению без учёта регистра, вне служебного места. Вид
входящего файла определяется только содержимым (`sniff`); расширение служит для отбора
кандидатов на диске и для выбора чтения в предпросмотре.

## Момент вставки

```rust
pub struct InsertMoment { year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8 }
```

Местное время устройства, собирается в UI. Негодные значения (месяц 13) прижимаются к
границам при построении имени — имя обязано получиться всегда.

## Входящий файл и его судьба

```rust
pub enum Origin { Clipboard, Camera, File }     // File — перетащен, выбран, из галереи

pub struct IncomingItem {
    pub name: Option<String>,   // имя, как его дала платформа; для Clipboard/Camera игнорируется
    pub size: u64,              // File.size — известен до чтения байтов
    pub origin: Origin,
}

pub struct BatchItem {
    pub incoming: IncomingItem,
    pub status: ItemStatus,
}

#[serde(tag = "kind")]
pub enum ItemStatus {
    Queued,
    AwaitingConfirmation,                       // size > LARGE_IMAGE_BYTES
    Copying,
    Stored { path: String, markup_path: String, bytes: u64, saved_as: Option<String> },
    Reused { path: String, markup_path: String },
    Skipped,                                    // вид не поддерживается, файл не записан
    Declined,                                   // пользователь отказался от большого файла
    Failed { problem: StoreProblem },
    Cancelled,
}

#[serde(tag = "kind")]
pub enum StoreProblem {
    AccessDenied,        // запись отклонена: vault стал недоступен на запись
    Io,                  // запись не удалась (нет места и прочее)
    NameBusy,            // на месте файла папка либо свободного имени не нашлось
    SourceUnreadable,    // UI не смог прочитать исходный файл; ставится source_failed
}
```

- `path` — от корня vault; `markup_path` — от папки заметки, через `/`.
- `saved_as` — имя на диске, если оно отличается от исходного (суффикс при коллизии,
  заменённое расширение); `None` — имя сохранено.
- `Stored` и `Reused` — «вставляемые» статусы: только их пути попадают в текст.

Переходы одного элемента:

```text
                 ┌─ size > 20 МБ ─→ AwaitingConfirmation ─ confirm(true) ─→ Queued
start ──→ Queued ┘                                        └ confirm(false) → Declined
Queued ── step ─→ Copying ── store ─→ Stored | Reused | Skipped | Failed
                  Copying ── source_failed ─→ Failed { SourceUnreadable }
Queued | AwaitingConfirmation ── cancel ─→ Cancelled
Copying ── cancel ─→ Copying (доводится до конца, исход запишет store)
```

Запрещено: `store` для элемента не в `Copying` — состояние не меняется; `confirm` для
элемента не в `AwaitingConfirmation` — состояние не меняется; два элемента в `Copying`
одновременно.

## Пачка

```rust
pub struct BatchState {
    pub note_path: String,
    pub target_folder: String,            // от корня vault, для подписи прогресса и диалога
    pub items: Vec<BatchItem>,
    pub moment: InsertMoment,
    pub phase: BatchPhase,
    pub excluded_by: Vec<IgnoreFile>,     // FR-020; пусто — папка не исключена
    pub known: Vec<ImageFile>,            // кандидаты сопоставления (research.md §7)
}

#[serde(tag = "kind")]
pub enum BatchPhase {
    Refused { reason: RefusalReason },    // ничего не будет сделано
    Running,
    Cancelled,
    Finished,
}

pub enum RefusalReason { ReadOnly, NotANote }

#[serde(tag = "kind")]
pub enum NextAction { Confirm { index: u32 }, Copy { index: u32 } }
pub struct BatchStep { pub state: BatchState, pub next: Option<NextAction> }

pub struct BatchView {
    pub state: BatchState,
    pub processed: u32,                   // всё, что уже не Queued, не AwaitingConfirmation и не Copying
    pub total: u32,
    pub is_over: bool,
    pub summary: BatchSummary,
    pub markup_paths: Vec<String>,        // Stored и Reused в порядке элементов
}

pub struct BatchSummary { pub stored: u32, pub reused: u32, pub skipped: u32,
                          pub declined: u32, pub failed: u32, pub cancelled: u32 }
```

Правила пачки:
- `Refused { ReadOnly }` — `access_state() != ReadWrite` (FR-024); `Refused { NotANote }` —
  путь не является путём заметки Typst (`classify != Note`).
- `step` выдаёт `Confirm` для первого `AwaitingConfirmation`; пока такие есть, `Copy` не
  выдаётся. Очередь пуста и в работе никого нет → `Finished`.
- `cancel` в фазе `Running` → `Cancelled`; все `Queued` и `AwaitingConfirmation` →
  `Cancelled`.
- `known` пополняется каждым `Stored`: повтор содержимого внутри пачки даёт `Reused`.
- `excluded_by` считается один раз в `start` и к записи отношения не имеет: предупреждение
  не мешает вставке.
- `LARGE_IMAGE_BYTES = 20 * 1024 * 1024`; подтверждение требуется при `size` строго больше.

## Правка текста

Тип фичи 003 (`templates::InsertionEdit { replace: TextSpan, text, caret }`), смещения в
кодовых единицах UTF-16.

```rust
pub fn plan_markup(text_length: usize, point: &InsertionPoint, before: &str, after: &str,
                   markup_paths: &[String], block: bool) -> Option<InsertionEdit>;
```

`None` — путей нет: правки нет, текст не меняется. Правило переносов — research.md §9.

## Отношения

- Заметка → папка назначения: `target_folder(overrides, note_path)`.
- Папка назначения + имя → `path`; заметка + `path` → `markup_path`.
- `BatchState.known` ⊂ результат `VaultBackend::list_images`.
- `excluded_by` ← `.gitignore`, `.stignore` в корне vault (`sync::ignore::IgnoreFile`).
