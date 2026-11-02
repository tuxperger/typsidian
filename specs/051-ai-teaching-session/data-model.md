# Data Model: Сессия урока

## Файл урока (`*.lesson`, JSON Lines)

Первая строка — `LessonHeader`, дальше — `LessonEvent`. Нечитаемые строки пропускаются.

### LessonHeader

| Поле | Тип | Правила |
| --- | --- | --- |
| `format` | число | `1`; больше известного — урок открывается только для чтения |
| `goal` | строка | непустая, до 500 символов |
| `language` | `ru` \| `en` | язык общения учителя |
| `created_at` | мс | |

### LessonEvent (`kind`)

| kind | Поля | Кто пишет |
| --- | --- | --- |
| `User` | `at`, `text`, `choice?` (индекс), `confidence?` (`Guess`/`Likely`/`Sure`), `thinking?` | ответ пользователя, до сети |
| `Teacher` | `at`, `text`, `question?` (`{question, choices?}`), `interrupted`, `failure?`, `model`, `input_tokens?`, `output_tokens?`, `cost_usd?` | итог хода |
| `NoteRead` | `at`, `path`, `by_user`, `outcome` (`Sent{chars,truncated}` / `Denied` / `Missing`) | прикрепление или инструмент |
| `Summary` | `at`, `text`, `covers` (число событий) | сжатие |
| `Language` | `at`, `language` | смена языка |
| `State` | `at`, `state` (`Finished` / `Stopped` / `Active`) | пользователь |

## Производное

- **LessonView**: шапка, события, `waiting` (ждёт ответа / ответ оборван / завершён),
  `last_question`, сумма объёма и стоимости (`cost_usd: None`, если хоть одна цена неизвестна),
  `events_len` для проверки §10.
- **LessonSummary**: путь, цель, время последнего события, число шагов (`User` + `Teacher`),
  состояние.
- **PreparedTurn**: id запроса, путь, провайдер, модель, тело запроса без ключа, вид хода
  (`Teach` / `Compact`), число событий на момент начала.
- **TurnReply**: текст, вызовы инструментов, объёмы.
- **Next**: `Continue` (автопродолжение) / `WaitUser`.

## Настройки ИИ (дополнение к 050)

| Поле | По умолчанию |
| --- | --- |
| `lessons_folder` | `Уроки` |
| `context_budget` | 100 000 единиц |

## Устройство

`.typsidian/lesson-partial/<hash>.txt` — накопленный текст идущего ответа; в `derived_paths`.
