# Quickstart: схемы и демонстрации

```bash
nix develop -c cargo test -p typsidian-core --test ai_native figure
```

`FigureWorld`: `read("Личное/Дневник.typ")` и `image("x.png")` — ошибка; бесконечный цикл —
`TimedOut` за ≤ 5 с; 300 КБ — `TooLarge`; демонстрация с параметром — SVG меняется с
значением. Поддельный поток: `draw` с ошибкой → ошибка ушла модели, в виде урока нет SVG;
исправленная — есть.

Браузер: `scripts/dev/scenarios/ai-figures.mjs`, сверка с артбордами A–D.
