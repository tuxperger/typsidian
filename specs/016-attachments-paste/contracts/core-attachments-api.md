# Contract: поверхность ядра для вложений

Модуль `packages/core/src/attachments/`. Типы — см. [data-model.md](../data-model.md).
Условной компиляции в модуле нет, кроме `web.rs`.

## Чистые правила

```rust
// kind.rs
pub fn sniff(bytes: &[u8]) -> Option<ImageKind>;                       // FR-006, FR-007

// folder.rs
pub const DEFAULT_ATTACHMENTS_FOLDER: &str = "attachments";
pub fn validate_folder(raw: &str) -> Result<String, AttachmentsFolderProblem>;   // FR-018
pub fn attachments_folder(overrides: &VaultOverrides) -> String;       // с умолчанием
pub fn placement(overrides: &VaultOverrides) -> AttachmentsPlacement;  // с умолчанием
pub fn target_folder(overrides: &VaultOverrides, note_path: &Path) -> PathBuf;   // FR-016
pub fn markup_path(note_path: &Path, attachment_path: &Path) -> String;          // FR-001

// name.rs
pub fn generated_name(note_path: &Path, moment: &InsertMoment, kind: ImageKind) -> String;
pub fn kept_name(original: &str, kind: ImageKind) -> String;           // FR-012, research §4, §5
pub fn free_name(name: &str, is_taken: impl Fn(&str) -> bool) -> Option<String>;  // FR-009

// markup.rs
pub fn image_markup(markup_path: &str) -> String;                      // #image("…")
pub fn plan_markup(text_length: usize, point: &InsertionPoint, before: &str, after: &str,
                   markup_paths: &[String], block: bool) -> Option<InsertionEdit>;

// sync/ignore_match.rs
pub fn excludes(file: IgnoreFile, text: &str, path: &str) -> bool;     // FR-020
```

Обязательства, проверяемые тестами на обоих таргетах:

1. `validate_folder` даёт тот же результат, что `export::folder::validate_folder`, для
   любой строки: правило одно, отличается только тип причины.
2. `target_folder`: `NextToNote` → `<родитель заметки>/<папка>`; `VaultFolder` → `<папка>`.
   Заметка в корне vault при `NextToNote` → `<папка>`.
3. `markup_path` всегда через `/`; для общей папки — с нужным числом `../`.
4. `generated_name` → `<основа заметки>-ГГГГ-ММ-ДД-ЧЧММСС.<расширение вида>`.
5. `kept_name` сохраняет основу; расширение меняет, только если оно не принадлежит виду;
   недопустимые символы заменяет на `-`; пустое имя → `image.<расширение>`.
6. `free_name`: свободное имя возвращается как есть; иначе `-2`, `-3`, … перед расширением;
   `None` после 9 999 попыток.
7. `image_markup` экранирует `\` и `"`.
8. `plan_markup`: `block = false` → запись в точке, каретка сразу за ней; `block = true` →
   по записи на строку, ведущий и завершающий `\n` только там, где в строке есть
   непробельный текст; выделение заменяется; пустой список путей → `None`.
9. `excludes`: таблица случаев из research.md §12, включая порядок («последнее выигрывает»
   у `.gitignore`, «первое» у `.stignore`) и исключение папки через папку над ней.

## Трейт `VaultBackend`

```rust
/// Изображения в vault — для сопоставления вложений по содержимому (фича 016).
/// `None` — весь vault, по правилам обхода шрифтов; `Some(folder)` — файлы одной папки
/// без вложенных, включая скрытую. Читаются только метаданные.
async fn list_images(&self, _folder: Option<&Path>) -> Vec<ImageFile> { Vec::new() }
```

Реализуется в `NativeBackend`, `WebBackend` и `MemoryVault` тестов. Результат одинаков на
обоих таргетах для одного и того же дерева файлов (тест `vault_native`/`vault_wasm`).

## Пачка

```rust
// модуль attachments::batch
pub async fn start<B: VaultBackend>(backend: &B, overrides: &VaultOverrides, note_path: &str,
                                    items: Vec<IncomingItem>, moment: InsertMoment) -> BatchState;
pub fn step(state: BatchState) -> BatchStep;                 // { state, next }
pub fn confirm(state: BatchState, index: u32, accept: bool) -> BatchState;       // FR-013
pub async fn store<B: VaultBackend>(backend: &mut B, state: BatchState, index: u32,
                                    bytes: &[u8]) -> BatchState;
pub fn source_failed(state: BatchState, index: u32) -> BatchState;   // UI не прочитал File
pub fn cancel(state: BatchState) -> BatchState;              // FR-023
pub fn view(state: BatchState) -> BatchView;
```

`start`, по порядку:
1. Путь не заметка Typst → `Refused { NotANote }`. Vault не `ReadWrite` →
   `Refused { ReadOnly }`. В обоих случаях к диску больше не обращается.
2. `target_folder` из настроек.
3. `known` = `list_images(Some(target_folder))`; если среди элементов есть `Origin::File` —
   плюс записи `list_images(None)`, у которых имя и размер совпали с каким-нибудь входящим.
4. `excluded_by` = файлы правил в корне vault, исключающие `target_folder`.
5. Статусы: `size > LARGE_IMAGE_BYTES` → `AwaitingConfirmation`, иначе `Queued`. Пустой
   список → `Finished`.

`store` для элемента в `Copying`, по порядку — и порядок проверяется тестами:
1. `sniff(bytes)` → `None`: `Skipped`. Запись не выполняется (FR-006).
2. Имя: `generated_name` для `Clipboard`/`Camera`, `kept_name` для `File`.
3. Для `Origin::File`: кандидаты `known` с тем же именем и размером где угодно → чтение
   `read_bytes_any_size` → байты равны → `Reused` (FR-011).
4. Кандидаты `known` в папке назначения с тем же размером → чтение → равны → `Reused`
   (FR-010).
5. `free_name`: занято, если `file_mark(папка/имя)` есть или в `known` в этой папке есть
   то же имя без учёта регистра. Нет свободного → `Failed { NameBusy }`.
6. `ensure_folder(папка)`, затем `write_bytes(путь, bytes)`.
   `WriteError::AccessDenied` → `Failed { AccessDenied }`, прочее → `Failed { Io }`.
7. Успех → `Stored`, запись добавляется в `known`.

Гарантии:
- существующий файл не изменяется ни в одном исходе (SC-003): `write_bytes` вызывается
  только по пути, для которого шаг 5 подтвердил отсутствие файла;
- при `Skipped`, `Reused`, `Failed` число файлов в vault не растёт (SC-004, FR-014);
- функции не паникуют и не возвращают `Result`: каждый исход — значение состояния;
- сетевых обращений нет.

## Настройки vault

`VaultOverrides` получает `attachments_placement` и `attachments_folder`.
`write_vault_settings` вызывает `validate_folder` до записи; отказ —
`VaultSettingsError::InvalidAttachmentsFolder { problem }`, файл не тронут. `parse`
отбрасывает негодное значение молча. Круговая проверка «записал → прочитал» включает оба
поля; неизвестные ключи таблицы `attachments` сохраняются.

## Предпросмотр

`preview::resolve::read_dependency`: путь проекта с расширением изображения
(`vault::is_image_file`) читается `read_bytes_any_size`; остальное — как раньше
(research.md §13).

## Команда клавиатуры

`CommandId::InsertImage` (`"insert-image"`, `Mod+Alt+I`, «Вставить изображение…») — в конец
`COMMANDS`; тесты уникальности умолчаний обновляются.

## Биндинги (принцип II: одинаковая поверхность)

| Операция | web (wasm-bindgen, `attachments/web.rs`) | desktop/Android (Tauri, `packages/shell`) |
|---|---|---|
| начало пачки | `attachmentsBatchStart(backend, overrides, notePath, items, moment)` | `attachments_batch_start` |
| шаг | `attachmentsBatchNext(state)` | `attachments_batch_next` |
| подтверждение | `attachmentsBatchConfirm(state, index, accept)` | `attachments_batch_confirm` |
| запись файла | `attachmentsBatchStore(backend, state, index, bytes)` | `attachments_batch_store` — сырое тело |
| источник не прочитан | `attachmentsBatchSourceFailed(state, index)` | `attachments_batch_source_failed` |
| отмена | `attachmentsBatchCancel(state)` | `attachments_batch_cancel` |
| вид | `attachmentsBatchView(state)` | `attachments_batch_view` |
| правка текста | `attachmentsPlanMarkup(textLength, point, before, after, paths, block)` | `attachments_plan_markup` |
| проверка названия папки | `attachmentsValidateFolder(raw)` | `attachments_validate_folder` |

Тело `attachments_batch_store` (research.md §11): `u32` little-endian — длина JSON; JSON
`{"state": BatchState, "index": u32}` в UTF-8; затем байты файла до конца тела. Тело короче
заголовка или с неразбираемым JSON — отказ команды, состояние у вызывающего остаётся
прежним.

На web вызовы с `backend` идут через `wasm-call-queue`.
