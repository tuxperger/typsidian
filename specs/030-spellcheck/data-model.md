# Data Model: Проверка орфографии

## SpellLanguage

`Ru` | `En`. Определяется азбукой слова.

## Настройки (vault.json, таблица `spellcheck`)

| Поле | Тип | Умолчание | Правило |
|---|---|---|---|
| `enabled` | bool | `true` | неверное значение — «не задано» |
| `languages` | `["ru", "en"]` | оба | неизвестные коды отбрасываются; пустой список — оба |

В `VaultOverrides`: `spellcheck_enabled: Option<bool>`, `spellcheck_languages: Option<Vec<String>>`.

## Словарь vault

`.typsidian/dictionary.txt` — UTF-8, слово в строке. Чтение: обрезка пробелов, пустые и
строки с `#` в начале пропускаются. Запись: отсортировано, без повторов (сравнение без учёта
регистра, сохраняется первое написание). Слово — 1…64 букв/дефисов/апострофов без пробелов;
иное — отказ `InvalidWord`.

## SpellConfig (ответ `spell_configure`, `spell_add_word`, `spell_remove_word`)

| Поле | Тип |
|---|---|
| `enabled` | bool |
| `languages` | `Vec<SpellLanguage>` |
| `words` | `Vec<String>` — словарь vault |

## Ответ проверки

`Uint32Array` пар `[from, to]` в UTF-16 текста редактора — подчёркнутые слова в окнах, по
возрастанию. `undefined` — сессия разошлась с редактором (как у подсветки 024).

## Подсказки

`Vec<String>` до 5 исправлений; регистр первой буквы как у слова.
