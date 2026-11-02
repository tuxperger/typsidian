# Contracts: практика

| Команда | Вход | Выход |
| --- | --- | --- |
| `ai_practice_create` (новая) | `title`, `lessons[]`, `notes[]`, `description?`, `language`, `now` | путь |
| `ai_lesson_answer` (051) | `answer` получает варианты действия: `Answer`, `HintRequest`, `GiveUp`, `NextProblem` | `LessonView` |

wasm: `practiceCreate`; `lessonBegin` принимает те же действия. Сервер: `ai_practice_create`
в `allowed.rs`. Инструменты модели: `pose_problem`, `give_hint`, `conclude` (research §2).
