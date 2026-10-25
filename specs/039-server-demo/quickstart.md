# Quickstart: Демо-режим сервера

```bash
nix develop -c cargo run -p typsidian-server -- --data /tmp/demo serve --demo
nix develop -c pnpm dev
nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
    scripts/dev/scenarios/server-demo.mjs --timeout 90000 --shot /tmp/demo.png
```

1. Экран входа — только «Попробовать без регистрации».
2. Кнопка → рабочая область vault «Демо» с примерами; пилюля «демо · осталось 60 мин».
3. Второй браузер — свой vault, правок первого нет.
4. `curl -X POST /api/login` → 404; «Подключить…» в синхронизации выключено.
5. Шестой старт с одного адреса за час → «попробуйте через N минут».
6. `[demo] ttl_minutes = 1` → через 1–3 минуты окно «Демо закончилось», на диске нет папок гостя.
