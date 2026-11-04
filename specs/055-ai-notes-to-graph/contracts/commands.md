# Contracts: импорт заметок

| Команда | Вход | Выход |
| --- | --- | --- |
| `ai_knowledge_import_estimate` | `paths[]` | `ImportEstimate` |
| `ai_knowledge_import` | `vaultId`, `requestId`, `estimate`, `now`, `allowCustom` | `ImportJob` (= `KnowledgeJob` с `files`) |

`ai_knowledge_import` — фоновая команда сервера (`AI_BACKGROUND_COMMANDS`): одно фоновое
место на аккаунт, место под запрос берётся на одну часть (`RequestGate`), как у
`ai_knowledge_extract` 054; прогресс по файлам — `ai_knowledge_jobs` 054, отмена —
`ai_cancel`. `ai_knowledge_import_estimate` — чтение vault без сети.

Отказы: корень vault, путь вне vault, больше 200 путей — `Forbidden`; оценка не совпала с
пересчитанной — `LessonChanged` (до журнала и задания; клиент показывает новую оценку); второй
импорт в vault — `Cancelled`.

wasm: `knowledgeImportEstimate(paths)`, цикл частей — `knowledgeImportBegin(vaultId,
requestId, estimate, now)` → `knowledgeSend(vaultId, prepared)` (тот же, что у 054) →
`knowledgeImportFinish(vaultId, prepared, now, reply, failure)`; `ExtractDone.next` решает,
продолжать ли (`Continue` / `WaitUser`). `PreparedExtract.part {index, hash}` у частей
заметок.
