# Contract: API ядра импорта

wasm (`import/web.rs`) и Tauri — 1:1; в UI — `VaultClient.import`. Версия после реализации ядра
(2026-10-15): сигнатуры ниже — фактические.

| Rust (`typsidian_core::import`) | wasm | Tauri | Что |
|---|---|---|---|
| `convert(md, &ConvertContext) -> Converted { typst, losses }` | `importConvert(markdown, ctx)` | — | один файл (тесты, предпросмотр) |
| `survey(&ReadOnly<S>, &V, &ImportJob) -> ImportSurvey` | `importSurvey(source, vault, job)` | `import_survey(job)` | C1, C4 |
| `import_begin(&ReadOnly<S>, &V, &ImportJob) -> ImportState` | `importBegin(source, vault, job)` | `import_begin(job)` | план |
| `import_step(&ReadOnly<S>, &mut V, ImportState) -> Result<ImportState, ImportError>` | `importStep(source, vault, state)` | `import_step(state)` | один файл |
| `import_finish(&mut V, &ImportState) -> Result<String, ImportError>` | `importFinish(vault, state)` | `import_finish(state)` | пишет журнал и отчёт, путь заметки отчёта |
| `ReadOnly::new(NativeBackend::open_read_only(path)?)` | `ImportSource.fromHandle(name, handle)` | `import_open_source(path)`, `import_close_source()` | источник только для чтения |

Источник в Rust — `&ReadOnly<S>`: запись в него невозможна по типу. Источник не открывается обычным
`open`/`from_handle` — они создают в папке служебное место (research.md §11).

Окно зовёт `import_step`, пока `next < total`, уступая UI-потоку между шагами; отмена — прекратить звать
и вызвать `import_finish` (ядро узнаёт отмену по `next < total`, отчёт получает пометку). План пакета живёт
в ядре под `state.id`, окно носит только `ImportState`; `ImportError::SessionLost` — план потерян
(перезагрузка), решение — снова `import_begin`: продолжение по журналу не даёт дублей.

## Тесты (оба пути, образцы в `tests/fixtures/obsidian/`)

- каждый образец переводится и собирается без ошибок (SC-001);
- таблица §2 — по случаю на строку; экранирование спецзнаков Typst;
- вики-ссылки всех форм, разрешение по правилу Obsidian, неоднозначность, нет цели;
- формула переводится; непереводимая → `raw` + потеря;
- источник не изменён: обёртка `ReadOnly` отвергает запись (SC-005);
- отмена посреди: перенесённые на месте, частичных файлов нет, отчёт есть;
- продолжение по журналу: дублей нет (FR-011);
- `[[путь|текст]]`: `link_sites`, индекс, перенос 023 сохраняет текст.
