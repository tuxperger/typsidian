# Contracts: схемы и демонстрации

| Команда | Вход | Выход |
| --- | --- | --- |
| `ai_figure_render` | `path`, `values` (имя → число) | `FigureRender` |
| `ai_figure_params` | `path` | `DemoParams` \| null |

Сборка native — в потоке с пределом 5 с; web — `figureRender` в воркере предпросмотра.
Инструменты модели: `draw`, `demonstrate`. Сервер: команды в `allowed.rs` (сборка на
сервере — тот же предел; демонстрация не исполняет код вне Typst).
