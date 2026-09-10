# Контракт: расчёт расположения файлов вставок

**Фича**: `006-vault-service-layout` | **Модуль**: `packages/core/src/boards/placement.rs`

Единственное место, знающее, где лежит файл вставки. Ни `snapshot.rs`, ни `embed.rs`, ни UI не
воспроизводят это правило у себя.

## Форма

```rust
pub fn assets_root(
    placement: AssetsPlacement,
    note_path: &Path,
    sections: &[SectionMark],
) -> PathBuf;

pub fn snapshot_path(
    placement: AssetsPlacement,
    note_path: &Path,
    sections: &[SectionMark],
    embed_id: &EmbedId,
) -> Option<PathBuf>;

pub fn markup_path(
    placement: AssetsPlacement,
    note_path: &Path,
    sections: &[SectionMark],
    embed_id: &EmbedId,
) -> Option<String>;
```

Все три — **чистые**: ни чтения файлов, ни записи, ни кэша. Отсюда следует, что вся таблица
входов и выходов проверяется одинаково под `cargo test` и `wasm-bindgen-test`, без файловой
системы.

`None` — путь не является путём заметки (`classify(note_path) != FileKind::Note`). Тот же
контракт, что у существующей `assets_folder_name`.

## Правила

### Корень

| `placement` | Корень |
|-------------|--------|
| `NextToNote` | `note_path.parent()`, либо корень vault для заметки в корне |
| `VaultRoot` | корень vault (пустой путь) |
| `SectionRoot` | ближайший помеченный предок `note_path`; если помеченных предков нет — корень vault |

«Ближайший» = самый длинный путь из `sections`, являющийся префиксом пути заметки по
компонентам. Сравнение **по компонентам**, а не по строке: `Работа` не должен считаться
префиксом `Работа-архив`.

Помеченный корень vault (`path == ""`) допустим и делает `SectionRoot` тождественным
`VaultRoot`.

### Раскладка внутри корня

```text
<корень>/assets/<имя заметки без расширения>/<embed_id>.svg
```

Одинакова для всех трёх вариантов — различается только корень (FR-004). Имя `embed_id`
выводится по прежнему правилу из полного пути заметки и пути доски (`boards/snapshot.rs`), и
эта фича его не меняет.

### Адрес в тексте заметки

`markup_path` возвращает путь **относительно папки заметки**, с `../` там, где нужно
(research §7). Разделитель — `/` на обоих таргетах, включая Windows: это путь внутри документа
Typst, а не путь файловой системы.

## Обязательства

1. **Никакого ветвления по таргету.** Условная компиляция в модуле запрещена.
2. **Никакого доступа к файлам.** Функция, которой понадобился `VaultBackend`, объявлена не
   здесь.
3. **Обратная совместимость не здесь.** Распознавание старых `*.assets` живёт в
   `boards/embed.rs`; этот модуль отвечает только за то, куда класть **новое**.
4. **Нормализация на входе.** `sections` приходят нормализованными (без `./`, без хвостового
   `/`); нормализацию делает хранилище, а не расчёт.

## Проверяемые сценарии

| Вход | Ожидаемый `snapshot_path` |
|------|--------------------------|
| `NextToNote`, `Проекты/Идея.typ` | `Проекты/assets/Идея/board-….svg` |
| `NextToNote`, `Идея.typ` | `assets/Идея/board-….svg` |
| `VaultRoot`, `Проекты/Идея.typ` | `assets/Идея/board-….svg` |
| `SectionRoot`, `Работа/Проекты/Идея.typ`, sections `["Работа"]` | `Работа/assets/Идея/board-….svg` |
| `SectionRoot`, та же заметка, sections `["Работа", "Работа/Проекты"]` | `Работа/Проекты/assets/Идея/board-….svg` |
| `SectionRoot`, `Личное/Мысли.typ`, sections `["Работа"]` | `assets/Мысли/board-….svg` |
| `SectionRoot`, `Работа-архив/Идея.typ`, sections `["Работа"]` | `assets/Идея/board-….svg` — префикс по компонентам, не по строке |
| любой, `Схема.excalidraw` | `None` — доска не заметка |

| Вход | Ожидаемый `markup_path` |
|------|------------------------|
| `NextToNote`, `Проекты/Идея.typ` | `assets/Идея/board-….svg` |
| `VaultRoot`, `Проекты/Идея.typ` | `../assets/Идея/board-….svg` |
| `VaultRoot`, `a/b/c/Идея.typ` | `../../../assets/Идея/board-….svg` |
