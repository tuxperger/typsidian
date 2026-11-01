# Quickstart: проверка 050

```bash
.specify/scripts/typsidian/gates.sh core
.specify/scripts/typsidian/gates.sh ui
nix develop -c pnpm dev
nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
    scripts/dev/scenarios/ai-settings.mjs --opfs-vault --shot /tmp/ai.png
```

Ручная проверка с реальной моделью (стоит денег только в облаке):

1. Новый vault: раздел «Обучение с ИИ» выключен; сетевых запросов к провайдерам нет.
2. Включить, подтвердить, указать `http://127.0.0.1:<порт>/v1` локальной модели, «Проверить».
3. Журнал: запись с дословным текстом, стоимость «неизвестно».
4. Выключить сеть и проверить Anthropic: сообщение за 30 секунд, редактор работает.
5. Неверный ключ: сообщение о ключе, ключа в тексте нет.
6. `grep -r` значения ключа по vault: ноль совпадений (SC-004).
