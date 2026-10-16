# Contract: API ядра свойств

Модуль `packages/core/src/properties/`; wasm (`properties/web.rs`) и Tauri
(`packages/shell/src/lib.rs`) — 1:1. В UI — `VaultClient.properties`.

| Rust | wasm | Tauri | Что |
|---|---|---|---|
| `read(text) -> NoteProperties` | `propertiesRead` | `properties_read` | свойства заметки (B1, B2) |
| `edit(text, edit) -> EditOutcome` | `propertiesEdit` | `properties_edit` | правка формой |
| `select(index, selection) -> SelectionResult` | — (внутри vault) | — | |
| `VaultSession::select(selection)` | `vaultSelect` | `vault_select` | отбор (B3) |
| `VaultSession::property_keys()` | `vaultPropertyKeys` | `vault_property_keys` | ключи и их виды для подсказок |
| `select_blocks(text) -> Vec<SelectBlock>` | `selectBlocks` | `select_blocks` | блоки в заметке (форма) |
| `select_inputs(index, text) -> Dict` | — | — | внутри сборки 005 |
| `select_library() -> (path, text, version)` | — | — | §10 |
| `VaultSession::reindex_properties()` | `vaultReindexProperties` | `vault_reindex_properties` | пакетно, с отменой |

`SearchQuery` получает `properties: Vec<Condition>` (B3).

## Тесты (оба пути)

- разбор: все виды значений; кириллические ключи; строковые ключи; не-литерал → `Unknown`,
  `editable = false`; повтор ключа; две записи; запись не первой — всё равно находится;
- правка: добавить, изменить, удалить, удалить последнее (запись исчезает), создать запись;
  остальной текст побайтно тот же;
- запись собирается `PreviewWorld` без входов и ничего не выводит (SC-003);
- отбор: тег, сравнения по видам, несравнимые не проходят, папка, даты, сортировка с
  пустыми в конце, `limit`, `truncated`;
- блок: с входом — таблица со ссылками; пустой результат — пояснение; без входа — замещающий
  текст с условиями; одинаковые аргументы в другом порядке → один ключ;
- индекс: обновление заметки меняет результат; пересчёт с отменой оставляет кэш согласованным;
- значения свойств не попадают в тексты ошибок (IX).
