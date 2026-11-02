# Data Model: Замер и план

Дополнения к файлу урока 051 (`specs/051-ai-teaching-session/data-model.md`).

## LessonHeader

| Поле | Тип | По умолчанию |
| --- | --- | --- |
| `start` | `Probe` / `Teach` | `Teach` (уроки 051 читаются как раньше) |

## Новые события

| kind | Поля |
| --- | --- |
| `Phase` | `at`, `phase` (`Probe`/`Teach`), `skipped` |
| `ProbeResult` | `at`, `parts[] {name, state: Known/Partial/Unknown, note}`, `start` |
| `Plan` | `at`, `version`, `steps[] {id, title, after[], known}`, `goal`, `current`, `reason?` |
| `PlanProgress` | `at`, `current`, `done[]` |

`Teacher.question` получает необязательное `part` (вопрос замера). `User` получает
`dont_know: bool`.

## Производное (`LessonView`)

- `phase`, `probe_count` (вопросов замера), `probe_result?`.
- `plan?`: последняя версия; шаги с `state` (`Done`/`Current`/`Todo`/`Known`), `added`
  (появился в этой версии), `anchor?` (номер события), координаты `x`, `y`; `edges[]`;
  `changes` — число версий минус один; `last_reason?`.
- `plan_history[]`: версия, время, число шагов, причина.

## Проверки `plan_set`

уникальные id · ссылки существуют · без циклов · ≤ 40 шагов · `goal` и `current` существуют.
