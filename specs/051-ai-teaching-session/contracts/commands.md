# Contracts: команды урока

Все команды — в `packages/host/src/commands.rs` (рабочий стол, Android, сервер) и
wasm-биндинги в `packages/core/src/ai/lesson/web.rs`. Отказы — `AiFailure` 050 плюс
`LessonChanged`, `LessonUnreadable`, `GoalEmpty`.

| Команда | Вход | Выход | Заметки |
| --- | --- | --- | --- |
| `ai_lesson_list` | — | `LessonSummary[]` | все `.lesson` vault |
| `ai_lesson_create` | `goal`, `notes[]`, `language`, `now` | путь | выключено → `Disabled`; имя из цели, уникальное |
| `ai_lesson_open` | `path` | `LessonView` | восстанавливает оборванный ответ (§5) |
| `ai_lesson_answer` | `vaultId`, `path`, `requestId`, `answer?`, `expected`, `now`, `allowCustom` | `LessonView` | весь цикл хода с автопродолжениями; `answer: null` — повтор |
| `ai_lesson_progress` | `vaultId`, `requestId` | `{ text, reading? }` \| null | опрос раз в 150 мс |
| `ai_lesson_checkpoint` | `vaultId`, `path`, `requestId` | — | раз в секунду |
| `ai_lesson_set_state` | `path`, `state` | `LessonView` | |
| `ai_lesson_set_language` | `path`, `language` | `LessonView` | |
| `ai_cancel` (050) | `vaultId`, `requestId` | — | останавливает ход |

wasm разбивает `ai_lesson_answer` на `lessonBegin` → `lessonSend` → `lessonFinish`
(решение о продолжении в ответе `lessonFinish`).

Сервер: все команды — в `allowed.rs`, `ai_lesson_answer` — в `AI_NETWORK_COMMANDS`.
