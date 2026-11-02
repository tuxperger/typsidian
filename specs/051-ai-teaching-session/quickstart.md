# Quickstart: проверка урока

## Тесты ядра

```bash
nix develop -c cargo test -p typsidian-core --test ai_native lesson
env -u LD_LIBRARY_PATH CARGO_PROFILE_DEV_DEBUG=0 nix develop -c \
  wasm-pack test --headless --chrome packages/core --test ai_wasm
```

Поддельный транспорт отдаёт заготовленный поток: вопрос с вариантами, чтение заметки,
обрыв посреди текста, отказ 429.

## В браузере

```bash
nix develop -c pnpm dev
nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
    scripts/dev/scenarios/ai-lesson.mjs --opfs-vault --shot /tmp/lesson.png
```

Сценарий подменяет `fetch` к модели заготовленным потоком и проверяет: урок создаётся в
«Уроки/», вопрос с вариантами отвечается, текст появляется частями, после перезагрузки
переписка на месте. Снимок сверяется с артбордами A, B, G макета.

## Руками

1. Включить ИИ (050), подключить локальную модель.
2. Палитра → «Новый урок», цель, прикрепить заметку → в журнале 050 её текст.
3. Посреди ответа отключить сеть → реплика помечена оборванной, «Запросить ответ снова».
4. Запретить папку, попросить учителя прочесть заметку из неё → отказ в уроке.
