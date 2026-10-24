# Quickstart: Серверный режим web

1. `nix develop -c cargo run -p typsidian-server -- user add anna` (пароль спрашивается).
2. `nix develop -c cargo run -p typsidian-server -- serve --data /tmp/ts-data --listen 127.0.0.1:8787`
   (статика web — из `apps/web/out`, собрать `pnpm --filter web build:server`).
3. Браузер: http://127.0.0.1:8787 → «Сервер» → anna → «Создать vault» «Диссертация» →
   заметка, текст, перезагрузка — текст на месте; файл в `/tmp/ts-data/users/<id>/Диссертация/`.
4. Второй аккаунт `bob` не видит vault Анны, прямой адрес — 404.
5. Две вкладки — правка в одной видна в другой за ≤ 3 с.
6. Остановить сервер во время правки — баннер «Нет связи», после запуска правка сохранена.
7. Тесты: `cargo test -p typsidian-server`, `cargo test -p typsidian-host`;
   сценарий `scripts/dev/scenarios/server-mode.mjs`.
