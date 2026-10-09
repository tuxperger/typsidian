# Data Model: Редактор правил исключения

Все типы — в `packages/core/src/sync/ignore.rs`, сериализуются `serde` и выводятся в
TypeScript через `tsify`. Постоянного состояния фича не добавляет: единственные данные на
диске — сами файлы правил.

## IgnoreDocument

Файл правил, каким его видит редактор.

| Поле | Тип | Смысл |
| --- | --- | --- |
| `file` | `IgnoreFile` (011) | `GitIgnore` или `StIgnore` |
| `exists` | `bool` | есть ли файл на диске |
| `text` | `String` | содержимое с окончаниями `\n`; для отсутствующего файла — предложенный блок приложения (FR-005); для нередактируемого — пусто |
| `line_ending` | `LineEnding` | `Lf` \| `CrLf` — чем писать обратно |
| `base` | `Option<String>` | отметка версии на диске (blake3, hex); `None` — файла нет |
| `editable` | `Editability` | можно ли править и почему нет |
| `in_effect` | `bool` | действуют ли правила сейчас: история включена (`GitIgnore`) / Syncthing обнаружен (`StIgnore`) — FR-007 |
| `foreign_parent` | `bool` | vault внутри чужого репозитория — действуют и правила выше (Edge Cases); только для `GitIgnore` |
| `check` | `IgnoreCheck` | проверка `text` |

## Editability

`Editable` \| `NotText` \| `TooLarge` \| `NotAFile` \| `ReadOnly` (FR-016).

## IgnoreCheck

Результат проверки текста (FR-010, FR-013).

| Поле | Тип | Смысл |
| --- | --- | --- |
| `missing` | `Vec<String>` | обязательные правила, которых нет или которые дословно отменены |
| `block` | `Option<LineRange>` | строки блока приложения в тексте (с маркерами), с нуля, конец не включён |
| `proposed_block` | `String` | точный текст блока приложения |

Инвариант: `missing` совпадает с `ignore::report(file, Some(text)).missing`.

## IgnoreSaveRequest

| Поле | Тип | Смысл |
| --- | --- | --- |
| `file` | `IgnoreFile` | |
| `text` | `String` | текст редактора, `\n` |
| `line_ending` | `LineEnding` | из документа |
| `base` | `Option<String>` | отметка версии, с которой начата правка |
| `confirm_incomplete` | `bool` | пользователь подтвердил сохранение без обязательных правил (FR-012) |
| `overwrite` | `bool` | пользователь выбрал «записать свою» при расхождении или пересоздание удалённого файла (FR-015) |

## IgnoreSaveOutcome

Перечисление с тегом `kind`:

- `Saved { document }` — записано; `document` — файл после записи (новая `base`).
- `Diverged { disk, diff }` — версия на диске не та, с которой начата правка; ничего не
  записано. `disk: IgnoreDocument` — что на диске сейчас (`exists: false` — файл удалён),
  `diff: Vec<DiffLine>` — различие текста редактора и версии с диска.
- `NeedsConfirmation { missing }` — обязательных правил не хватает, подтверждения нет;
  ничего не записано.
- `Refused { reason }` — `reason: IgnoreRefusal`: `NotText` \| `TooLarge` \| `NotAFile` \|
  `ReadOnly` (те же причины, что в `Editability`) \| `HistoryBusy`
  (FR-017); ничего не записано.

Порядок проверок в `ignore_save`: возможность правки → занятость истории → расхождение
(если не `overwrite`) → обязательные правила (если не `confirm_incomplete`) → запись.

## Переходы состояния диалога (UI, без правил)

```text
закрыт → загрузка → (нередактируемый | чистый)
чистый ⇄ изменён                       набор текста / отмена правки
чистый → чистый (обновлён извне)       revision вырос, base другая
изменён → изменён (устарел)            revision вырос, base другая — предупреждение
изменён → сохранение → чистый          Saved
                     → подтверждение   NeedsConfirmation → сохранение(confirm) | изменён
                     → расхождение     Diverged → взять с диска (чистый)
                                                | сохранение(overwrite) | изменён
                     → изменён+причина Refused / SyncError
изменён → закрытие? → сохранение | закрыт | изменён      (FR-006)
```
