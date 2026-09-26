# Контракт: схема и параметры элементов Typst

**Фича**: `008-element-parameters` | **Модуль**: `packages/core/src/elements/`

Сущности — в [data-model.md](../data-model.md). Все функции **чистые**: ни файловой
системы, ни сети (FR-023), ни состояния между вызовами. Смещения — в кодовых
единицах UTF-16, как в `templates::InsertionPoint` (фича 003).

## Форма

```rust
/// Схема элемента каталога (FR-001a). `None` — у элемента нет параметров:
/// он вставляется сразу (FR-003).
pub fn element_schema(id: &str) -> Option<ElementSpec>;

/// Элемент, непосредственно содержащий каретку (FR-006, FR-016).
/// Каретка на границе элемента (перед `#`, сразу после `)`/`]`/метки) считается внутри.
pub fn element_at(text: &str, offset: usize) -> ElementAtResult;

/// Текст элемента по значениям формы — «Будет вставлено» (FR-004, FR-005).
/// Необязательные параметры, равные умолчанию, не пишутся (FR-013).
pub fn render_element(id: &str, values: &[FieldValue], label: Option<&str>)
    -> Result<RenderedElement, Vec<FieldError>>;

/// Точечные замены для правки на месте (FR-008, FR-009, FR-012, FR-012a).
/// Пустой список — ничего не изменилось. `Stale` — текст элемента не тот, что был
/// при открытии формы (FR-020).
pub fn plan_element_edit(
    text: &str,
    parsed: &ParsedElement,
    values: &[FieldValue],
    label: Option<&str>,
) -> Result<Vec<TextEdit>, PlanEditError>;
```

`FieldValue { name, value: Literal(String) | Expression(String) | Default }` — то, что
форма держит в полях. `FieldError { name, reason }` — значение, которое элемент не
примет (FR-015): не тот вид, вне списка, пустое обязательное, занятая метка.

`RenderedElement { text, caret: Option<usize> }` — текст и позиция первого пустого
места (US1 AS3).

## Когда форма

`opens_form_on_insert(spec)` — правило ядра, UI его только спрашивает (поле
`ElementSpec.form_on_insert`, вычисляется при загрузке схемы):

- **Вставка** открывает форму, если у элемента ≥ 2 параметров (для `markup-*` — в
  профиле разметки) **или** есть обязательный параметр без умолчания (SC-001). Иначе
  элемент вставляется сразу, как сейчас (FR-003): `= Заголовок` — без формы.
- **Правка** («Параметры») доступна у любого элемента схемы с ≥ 1 параметром — уровень
  заголовка правится формой, хотя вставляется он без неё.

## Обязательства

1. **Одна схема на вставку и правку** (FR-001a). `render_element` и `plan_element_edit`
   читают одну и ту же `ElementSpec`; второго описания полей нет ни в ядре, ни в UI.
2. **Правка без изменений — пустой список** (FR-009, SC-003). Проверяется по каждому
   элементу схемы: `element_at` → значения как есть → `plan_element_edit` = `[]`.
3. **Незнакомое не адресуется** (FR-012, SC-004). Ни одна замена не пересекает
   диапазон аргумента, которого нет в схеме, комментария или пробелов между
   аргументами.
4. **Выражение — как есть** (FR-012a). Нетронутое поле `Expression` не порождает замены.
5. **Отказ вместо догадки** (FR-011). Если граница элемента не установлена или
   параметр повторён — `Unavailable` с причиной, а не частичный `Found`.
6. **Паритет** (FR-022). Каждая функция объявлена на wasm-границе (`WebBackend`,
   статически) и командой Tauri; UI вызывает её через `VaultClient`.

## Граница wasm / Tauri / UI

| Ядро | wasm (`WebBackend`, static) | Tauri (`packages/shell`) | `VaultClient` |
|------|------|------|------|
| `element_schema` | `elementSchema(id)` | `element_schema` | `elementSchema(id)` |
| `element_at` | `elementAt(text, offset)` | `element_at` | `elementAt(text, offset)` |
| `render_element` | `renderElement(id, values, label)` | `render_element` | `renderElement(…)` |
| `plan_element_edit` | `planElementEdit(text, parsed, values, label)` | `plan_element_edit` | `planElementEdit(…)` |

Типы границы порождаются `tsify-next` (принцип IV).

## Проверяемые сценарии (таблица `tests/common/element_cases.rs`, оба таргета)

| Сценарий | Ожидание |
|----------|----------|
| схема против `Func::params()` для каждого `call`/`set`-элемента | совпадение имён, видов, умолчаний (SC-001a) |
| каждый элемент: `render_element` с умолчаниями → `element_at` | элемент найден, значения совпадают (круг вставка → правка) |
| каждый элемент: правка без изменений | `[]` (FR-009) |
| `#figure(image("a.png"), caption: [a (b) [c]], foo: 1)`, меняем `caption` | меняется только подпись; `foo: 1` на месте |
| `#columns(n * 2)[…]` | `count` = `Expression("n * 2")`; правка `gutter` его не трогает |
| `#image("a.png", width: 40%, width: 60%)` | `Unavailable(DuplicateParam)` |
| `#figure(image("a.png"` без `)` | `Unavailable(Unclosed)` |
| `#columns(2)[ #figure(…) ]`, каретка внутри figure / вне | figure / columns (FR-016) |
| каретка перед `#` и после `)` | элемент найден |
| кириллица до элемента | смещения в UTF-16 верны |
| `render_element` с пустым обязательным | `FieldError` по этому полю |
| занятая метка | `FieldError`; собственная метка правимого элемента — не ошибка |
| `plan_element_edit` после изменения текста элемента | `Stale` (FR-020) |
| `== Итоги`, уровень 3 | замена `==` на `===`, текст не тронут |
