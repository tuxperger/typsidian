# Quickstart: граф связей

## 1. Ядро

```bash
.specify/scripts/typsidian/gates.sh core          # graph_native, graph_wasm
nix develop -c cargo bench -p typsidian-core --bench graph_view
```

## 2. Web — сценарий

```bash
nix develop -c pnpm dev
nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
    scripts/dev/scenarios/graph.mjs --opfs-vault --shot /tmp/graph.png
```

Сценарий: 50 связанных заметок → «Граф» → узлы движутся и останавливаются ≤10 с; наведение
на узел — соседи выделены; перетаскивание узла; щелчок открывает заметку; «Вокруг заметки»
глубина 1 → 2 добавляет узлы; группа `папка:проекты` красит узлы; фильтр `тег:проект`;
перезапуск — настройки на месте; колонка заметки → «Граф». Снимки — G1–G4.

## 3. Масштаб (SC-001, SC-002)

Сценарий с генерацией 5 000 заметок и 15 000 связей: время до первого движения ≤2 с, кадры
при перетаскивании ≥30/с (замер `requestAnimationFrame` в сценарии), покой ≤10 с.

## 4. Рабочий стол и Android (вручную)

Те же шаги; на телефоне — шторка G5, щипок, долгое нажатие на узел.
