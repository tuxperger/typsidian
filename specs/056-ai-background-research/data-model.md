# Data Model: Исследование

Дополнения к файлу урока 051.

| kind события | Поля |
| --- | --- |
| `ResearchStarted` | `at`, `id`, `research` (`Web`/`Pdf`; поле не `kind`: имя занято тегом события), `question`, `path?`, `by_user` |
| `ResearchDone` | `at`, `id`, `note` (путь заметки), `summary`, `sources[] {title, url?, pages?}`, `input_tokens?`, `output_tokens?`, `cost_usd?` |
| `ResearchFailed` | `at`, `id`, `failure` (`AiFailure`), `cancelled` |

Производное `LessonView.research[]`: id, вид, вопрос, состояние (`Queued`/`Running`/`Done`/
`Failed`/`Cancelled`/`Interrupted`), источники, путь заметки, найдено источников.

Настройки ИИ: `web_research` (`false`), `research_parallel` (`2`, от 1 до 4), `research_agent_pdf` (`false`: инструмент учителя `study_pdf`).
