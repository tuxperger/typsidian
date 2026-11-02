# Data Model: Практика

Дополнения к файлу урока 051.

## LessonHeader

| Поле | Тип | По умолчанию |
| --- | --- | --- |
| `kind` | `Lesson` / `Practice` | `Lesson` |

## Новые события

| kind | Поля |
| --- | --- |
| `Material` | `at`, `lessons[] {path, goal, chars}`, `notes[]` (через `NoteRead`), `description?` |
| `Problem` | `at`, `n`, `statement`, `solution?` |
| `Hint` | `at`, `n`, `text` |
| `HintRequest` | `at`, `n` |
| `GiveUp` | `at`, `n` |
| `Verdict` | `at`, `n`, `outcome` (`Correct`/`Partial`/`Wrong`/`Unsolved`), `review` |
| `NextProblem` | `at` |

## Производное

`LessonView.practice`: `problems[] {n, title, outcome?, hints, anchor, solution?}` —
`solution` только при `outcome`; `open_problem?` — номер задачи без итога; `last_review?`.
