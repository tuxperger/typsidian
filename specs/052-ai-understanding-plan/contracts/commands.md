# Contracts: замер и план

Изменения команд 051 (`specs/051-ai-teaching-session/contracts/commands.md`):

| Команда | Изменение |
| --- | --- |
| `ai_lesson_create` | новый аргумент `start: Probe \| Teach` |
| `ai_lesson_answer` | `answer.dont_know` |
| `ai_lesson_skip_probe` (новая) | `path` → `LessonView`; событие `Phase { Teach, skipped }` |
| `ai_lesson_plan_version` (новая) | `path`, `version` → план этой версии с раскладкой |

wasm: `lessonCreate` получает `start`, новые `lessonSkipProbe`, `lessonPlanVersion`.
Сервер: новые команды в `allowed.rs`; сетевых команд не добавляется.

Инструменты модели (описание в `ai/lesson/prompt.rs`): `probe_question`, `probe_done`,
`plan_set`, `plan_progress` (research §2, §3).
