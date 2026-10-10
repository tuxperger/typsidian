# Contract: поверхность ядра для дополнения

Модуль `packages/core/src/completion/`. Типы — [data-model.md](../data-model.md). Каждая
функция имеет wasm-биндинг и команду Tauri 1:1 (таблица внизу) и случаи в
`tests/common/completion_cases.rs` на обоих путях.

## Перечень меток связанных файлов

```rust
pub async fn linked_index<B: VaultBackend>(
    backend: &B,
    packages: &PackageCache,
    note_path: &str,
    dependencies: &[String],     // CompiledDocument.dependencies последней сборки
) -> LinkedIndex
```

- Берутся только пути с расширением `typ`, кроме самой заметки.
- Байты — из `PackageCache::vault_files_of(note_path)`, если `file_mark` совпал; иначе
  `VaultBackend::read_bytes`. Других чтений нет (принципы I, VII).
- Отказ чтения или не-UTF-8 → путь в `unreadable`, функция не паникует и не возвращает
  `Result`.
- Разбор — `structure` (015).

## Список предложений

```rust
pub async fn completions<B: VaultBackend>(backend: &B, request: &CompletionRequest) -> CompletionList
```

Порядок:
1. `context::at(text, offset, manual)` (чистая функция, research.md §2). `None` → пустой ответ
   с причиной, `backend` не трогается.
2. `Label` → `labels::candidates` (чистая): метки заметки из `structure`, метки `linked`,
   `sources`; фильтр и порядок — research.md §5.
3. `Note` → `backend.list()` и `notes::candidates` (чистая): research.md §6; `insert` и
   `preview` — через функцию кратчайшей записи `graph` (013).

`backend` нужен только для списка заметок; чистые части проверяются отдельно.

## Цель перехода

```rust
pub async fn definition_at<B: VaultBackend>(backend: &B, request: &DefinitionRequest) -> DefinitionOutcome
```

- Ссылка под кареткой — `@имя` из `structure` либо вхождение `[[…]]` из `graph`, чей диапазон
  содержит каретку (включая позицию сразу после последнего знака).
- `@имя`: метка в заметке → в связанных файлах → ключ источника (`Source`) → подтверждена
  `evidence` (`LabelFromBuild`) → `LabelMissing`.
- `[[цель]]`: `graph::resolve_link_target`; `None` → `OutsideVault`; заметки нет в
  `backend.list()` → `NoteMissing`; иначе `Place` с началом файла (строка 1, колонка 1).

## Отметки

```rust
pub async fn reference_marks<B: VaultBackend>(backend: &B, request: &MarksRequest) -> Vec<ReferenceMark>
```

Правила — data-model.md «MarksRequest / ReferenceMark» и research.md §4, §8. `backend` нужен
только для проверки существования заметок-целей; чистая часть — `marks::label_marks(...)`.

Гарантии, проверяемые тестами:
- документ, который собирается без ошибок, при переданном `evidence` его сборки не получает
  ни одной `MissingLabel` (SC-002) — на метках из цикла, из функции шаблона, из
  `#label(…)` и при правиле `show ref`;
- без `evidence` каждая ссылка на имя, которого нет в тексте, отмечена (SC-002, первая
  половина);
- при непустом `linked.unreadable` ни одной `MissingLabel` не выдаётся (research.md §3).

## Свидетельство сборки (расширение 005)

`CompiledDocument.evidence: Option<BuildEvidence>` заполняется в
`preview/compile.rs::compile_with` у удачной сборки вида `Render::Pages`:
`labels` — из `document.introspector().query_labelled()`, `references` — из `structure` над
текстом заметки. Сигнатуры публичных функций предпросмотра не меняются; поле сериализуется с
`serde(default, skip_serializing_if = "Option::is_none")`.

## Настройка и команды

- `Settings.editor_completion_auto` и `SettingsChange.editor_completion_auto` — по образцу
  `editor_font_size` в `settings/apply.rs`.
- `CommandId::GotoDefinition`, `CommandId::TriggerCompletion` — `keybindings.rs`; тест
  уникальности умолчаний обновляется.

## Биндинги

| Ядро | wasm (`js_name`) | Команда Tauri |
|---|---|---|
| `linked_index` | `PreviewEngine.completionLinkedIndex(backend, notePath, dependencies)` | `completion_linked_index` |
| `completions` | `WebBackend.completions(request)` | `completions` |
| `definition_at` | `WebBackend.definitionAt(request)` | `definition_at` |
| `reference_marks` | `WebBackend.referenceMarks(request)` | `reference_marks` |

`linked_index` стоит на `PreviewEngine`, потому что кэш связанных файлов (`PackageCache`)
принадлежит ему; в оболочке Tauri — то же состояние, что у команды сборки.
