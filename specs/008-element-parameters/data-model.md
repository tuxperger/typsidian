# Data Model: Element Parameters

**Фича**: `008-element-parameters` | **Дата**: 2026-09-26

Сущности фичи и их форма в ядре. Поведение — в [spec.md](./spec.md), функции — в
[contracts/element-params.md](./contracts/element-params.md), обоснования — в
[research.md](./research.md).

## Схема параметров — `ElementSchema` (файл `packages/core/src/elements/schema.json`)

Единственный источник полей формы (FR-001, FR-001a). Поставляется в составе ядра
(`include_str!`), одна на все клиенты (FR-001c).

```json
{
  "typst": "0.15.1",
  "elements": [
    {
      "id": "code-block",
      "typst": "raw",
      "syntax": "markup-raw",
      "label": null,
      "params": [
        { "name": "lang", "title": "Язык", "main": true,
          "kind": { "type": "string" }, "default": "none",
          "positional": false, "required": false, "markup": true }
      ]
    },
    {
      "id": "figure",
      "typst": "figure",
      "syntax": "call",
      "label": { "prefix": "fig" },
      "params": [ … все параметры figure … ]
    }
  ]
}
```

### Элемент схемы — `ElementSpec`

| Поле | Тип | Смысл |
|------|-----|-------|
| `id` | строка | тот же id, что у записи каталога вставки в UI (`typstElements.ts`) |
| `typst` | строка | путь функции Typst: `figure`, `math.frac`, `polygon.regular` |
| `syntax` | перечисление | как элемент выглядит в тексте — см. ниже |
| `label` | `{ prefix } \| null` | допускает ли элемент метку `<prefix:…>` после себя |
| `params` | список `ParamSpec` | **все** параметры функции Typst, в порядке справочника |
| `form_on_insert` | bool | вычисляется ядром при загрузке: открывать ли форму при вставке (research §12) |

`syntax`:

| Значение | Вид в тексте | Что правится |
|----------|--------------|--------------|
| `call` | `#name(args)[body]` или `name(args)` внутри кода/формулы | все параметры |
| `set` | `#set name(args)` | только `settable`-параметры |
| `markup-heading` | `== Текст` | уровень (профиль разметки, research §7) |
| `markup-raw` | ```` ```lang ```` | язык |
| `markup-equation` | `$ … $` / `$…$` | строчная или выключная |

### Параметр — `ParamSpec`

| Поле | Тип | Откуда | Смысл |
|------|-----|--------|-------|
| `name` | строка | Typst | имя аргумента; для позиционных — имя из справочника (`body`, `path`) |
| `title` | строка | руками | как параметр называется пользователю; пустым быть не может |
| `main` | bool | руками | основной — виден сразу; иначе в блоке «Все параметры» (FR-012b) |
| `kind` | `ParamKind` | Typst (`CastInfo`) | вид значения → виджет формы |
| `default` | строка \| null | Typst (`default()`) | умолчание в записи Typst (`"auto"`, `"1em"`); `null` — обязательный |
| `positional` | bool | Typst | позиционный аргумент |
| `required` | bool | Typst | обязательный |
| `settable` | bool | Typst | допускается в `#set` |
| `markup` | bool | руками | входит в профиль разметки (только для `markup-*`) |

### Вид значения — `ParamKind`

| `type` | Виджет | Литералы в тексте (research §5) |
|--------|--------|------|
| `choice` + `options: [..]` | выбор из списка (FR-014) | строка или идентификатор из `options` |
| `bool` | переключатель | `true` / `false` |
| `int`, `float` | число | `Int`, `Float` |
| `length`, `ratio`, `relative`, `fraction`, `angle` | число + единица | `Numeric` (`2cm`, `80%`, `1fr`, `45deg`) |
| `string` | строка | `Str` |
| `content` | текст | `[…]` |
| `path` | строка-путь | `Str` |
| `auto-or` + `inner` | «авто» или вложенный вид | `auto` либо литерал `inner` |
| `none-or` + `inner` | «нет» или вложенный вид | `none` либо литерал `inner` |
| `expression` | поле «как есть» | любой текст Typst — для типов, которых форма не рисует (массивы, словари, функции, `stroke`) |

**Валидация** (тест схемы, SC-001a):
- у каждого элемента набор `name` совпадает с `Func::params()` соответствующей
  функции Typst (для `markup-*` — с профилем разметки);
- `kind`, `default`, `positional`, `required`, `settable` совпадают с тем, что
  отдаёт Typst;
- `title` непуст; у каждого элемента с параметрами хотя бы один `main`;
- `id` уникальны.

## Разобранный элемент — `ParsedElement`

Результат `element_at(text, offset)` (FR-006, FR-007).

| Поле | Тип | Смысл |
|------|-----|-------|
| `id` | строка | элемент схемы |
| `from`, `to` | usize | границы элемента в тексте, **в UTF-16** (как у CodeMirror), метка включена |
| `values` | список `ParamValue` | прочитанные значения, по одному на заданный в тексте параметр |
| `label` | строка \| null | метка элемента без префикса |
| `taken_labels` | список строк | метки заметки того же префикса, кроме собственной |
| `revision` | строка | отпечаток текста элемента — для FR-020 |

### `ParamValue`

| Поле | Смысл |
|------|-------|
| `name` | имя параметра |
| `value` | `{ literal: "<текст Typst>" }` либо `{ expression: "<текст Typst>" }` (FR-012a) |
| `span` | диапазон значения в тексте — адрес точечной замены (research §4) |

Незнакомые аргументы в `values` не попадают и **никогда** не адресуются заменой —
этим они и сохраняются (FR-012).

## Отказ — `ElementAtResult`

```text
None                                  — каретка не в элементе схемы (US2 AS5)
Found(ParsedElement)                  — можно править
Unavailable { id, reason }            — элемент есть, но правка не предлагается (FR-011)
```

`reason` — различимые причины: `Unclosed` (незакрытая скобка/строка), `DuplicateParam
{ name }`, `SpreadArgs` (`..args`), `UnknownPositional` (позиционных больше, чем знает
схема). Каждая называется словами в UI.

## Форма — `ParamForm` (состояние UI)

| Поле | Смысл |
|------|-------|
| `schema` | `ElementSpec` из ядра |
| `values` | текущие значения полей (строки в записи Typst) |
| `errors` | ошибки по полям от ядра (FR-015) |
| `preview` | «Будет вставлено» — текст от `render_element` (FR-004, FR-005) |
| `mode` | `insert` \| `edit { parsed }` |
| `showAll` | раскрыт ли блок «Все параметры»; при правке истина, если задан хоть один не-основной (FR-012b) |

Переходы: `закрыта → открыта (insert | edit) → применена | отменена (Escape)`. Применение
заперто, пока `errors` непуст (FR-015) или `revision` элемента в тексте не совпадает с
открытым (FR-020).
