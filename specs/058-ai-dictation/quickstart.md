# Quickstart: диктовка

```bash
nix develop -c cargo test -p typsidian-core --test ai_native speech
```

Поддельный транспорт: multipart с полями `model`, `language` и файлом; журнал без байтов
звука, с описанием и текстом; свой адрес без одобрения — `EndpointNotApproved` до журнала;
`provider: None` — `Disabled`.

Руками: локальный сервер распознавания с совместимым API, поле «думаю вслух» → запись →
текст в поле; поиск `*.webm`/`*.mp4` в vault и `.typsidian` — пусто (SC-002).
