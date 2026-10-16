# Quickstart: импорт из Obsidian

## 1. Ядро

```bash
.specify/scripts/typsidian/gates.sh core     # import_native, import_wasm
```

## 2. Web — сценарий

```bash
nix develop -c pnpm dev
nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
    scripts/dev/scenarios/import.mjs --opfs-vault --shot /tmp/import.png
```

Сценарий кладёт `packages/core/tests/fixtures/obsidian/` в OPFS как источник, запускает импорт,
проверяет число заметок, открывает заметку со ссылкой, картинкой и формулой — предпросмотр
без ошибок; отчёт открыт и перечисляет потери образцов. Снимки — C1–C3.

## 3. Масштаб и отмена (рабочий стол, вручную)

Сгенерировать 2 000 заметок (`scripts/dev/gen-obsidian.sh`), импорт ≤2 мин, правка другой
заметки в это время без задержек; отмена на середине ≤2 с; повторный запуск — «Продолжить».
`diff -r` исходной папки с копией до импорта — пусто (SC-005).
