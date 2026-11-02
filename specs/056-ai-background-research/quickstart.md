# Quickstart: исследование

```bash
nix develop -c cargo test -p typsidian-core --test ai_native research
```

Поддельный транспорт: веб-исследование при выключенном поиске — 0 запросов поиска (тело
запроса без инструмента поиска); включённое — запрос с инструментом, `submit_research` →
заметка Typst с экранированным текстом и источниками; PDF из запрещённой папки — отказ;
незавершённое при открытии — `Interrupted`.

Браузер: `scripts/dev/scenarios/ai-research.mjs`, сверка с артбордами A–E.
