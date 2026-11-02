# Data Model: Исследование

Дополнения к файлу урока 051.

| kind события | Поля |
| --- | --- |
| `ResearchStarted` | `at`, `id`, `kind` (`Web`/`Pdf`), `question`, `path?`, `by_user` |
| `ResearchDone` | `at`, `id`, `note` (путь заметки), `summary`, `sources[] {title, url?, pages?}` |
| `ResearchFailed` | `at`, `id`, `failure` (`AiFailure`), `cancelled` |

Производное `LessonView.research[]`: id, вид, вопрос, состояние (`Queued`/`Running`/`Done`/
`Failed`/`Cancelled`/`Interrupted`), источники, путь заметки, найдено источников.

Настройки ИИ: `web_research` (`false`), `research_parallel` (`2`).
