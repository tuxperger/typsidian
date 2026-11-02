# Data Model: Схемы и демонстрации

- Событие урока `Figure { at, path, kind: Diagram|Demo, version }` — `version` растёт при
  исправлении учителем.
- Заметка схемы/демонстрации: Typst; у демонстрации первая строка — описание параметров
  (`DemoParams { params[] {name, label, min, max, step, value} }`).
- **FigureRender**: `svg?`, `errors[]`, `state` (`Ok`/`Failed`/`TimedOut`/`TooLarge`/
  `Disabled`).
- Настройка ИИ `demos_enabled: bool` (`true`).
