# Quickstart: проверка синхронизации (011)

Тулчейн — только через `nix develop -c …` (CLAUDE.md). Пример vault —
`/home/apokryf/notes` (папка Syncthing, без истории). Для проверки берётся **копия**:

```bash
cp -a /home/apokryf/notes /tmp/typsidian-sync/notes-a
git init --bare /tmp/typsidian-sync/remote.git     # «удалённый репозиторий» для офлайн-проверок
```

## Автотесты

```bash
.specify/scripts/typsidian/gates.sh core     # cargo test + wasm-bindgen-test (sync::*)
.specify/scripts/typsidian/gates.sh ui       # strict typecheck + i18n
nix develop -c cargo bench -p typsidian-core --bench sync_status   # SC-002
```

Интеграционные тесты ядра (`packages/core/tests/sync_*.rs`) используют настоящий
`NativeGitEngine` на временных папках и `file://` удалённый; офлайн (FR-020, SC-004) —
адрес на закрытый порт loopback и `CancelToken`.

## Ручные сценарии

1. **US1** — открыть копию в desktop → индикатор «Syncthing, история выключена» →
   «Включить историю»: сначала предложение блока `.stignore` (`.git`,
   `.typsidian/cache.sqlite*`, `.typsidian/tmp`), затем `.gitignore`; принять →
   первый снимок. `git -C notes-a log --stat` показывает снимок; `.stignore` содержит
   блок `// typsidian:begin…end` в начале, повторное «исправить» дублей не даёт.
2. **US2** — изменить заметку трижды со снимками, открыть её историю, сравнить, откатить,
   отменить откат `Ctrl+Z`.
3. **US3** — подключить `/tmp/typsidian-sync/remote.git` (desktop) или GitHub с токеном,
   отправить; `clone_vault` в `notes-b`; правка одной строки в обоих → `pull` в `notes-b`
   показывает 3 колонки; «отменить получение» возвращает файлы. Выключить сеть → «отправить»
   завершается сообщением ≤ 30 с, редактирование не прерывается.
4. **US4** — `cp note.typ "note.sync-conflict-20261003-120000-ABCDEFG.typ"` → копия не
   в дереве/поиске/графе, есть в «Конфликтах»; «взять копию» → копия удалена. С запущенным
   Syncthing: «найти Syncthing» → подтверждение → индикатор показывает состояние папки.
5. **US5** — назначить хранителя на `notes-a`, включить автоснимки с паузой 30 с; в
   `notes-b` (второй профиль устройства) «Сделать снимок» объясняет, кто хранитель; история
   заметки в `notes-b` строится из зеркала удалённого. Конфликтная копия `vault.json` →
   автоснимки остановлены на обоих.
6. **US6** — web (`pnpm dev`): та же копия через выбор папки; снимок и история работают;
   отправка на GitHub без посредника → `CorsBlocked` с объяснением; на Android ssh-адрес
   отклоняется при подключении.

`git fsck` и `git log` в каждой копии после сценариев — без ошибок (SC-008);
`grep -r <токен> notes-a` — пусто.
