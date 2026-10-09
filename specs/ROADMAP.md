# Roadmap

<!-- СГЕНЕРИРОВАНО .specify/scripts/typsidian/roadmap.sh — руками не править.
     Стадия выводится из артефактов каталога фичи; приоритет и суть —
     строки `**Priority**` / `**Summary**` её spec.md. -->

Все фичи проекта ведутся через Spec Kit: одна фича — один каталог `specs/NNN-имя/`.
Стадия: `spec` → `plan` → `tasks` → `implement N/M` → `done`.

| № | Фича | Приоритет | Стадия | Вопросы | Суть |
|---|------|-----------|--------|---------|------|
| 001 | [Vault Manager](001-vault-manager/spec.md) | — | implement 128/135 | — | — |
| 002 | [Drawing Boards](002-drawing-boards/spec.md) | — | implement 72/78 | — | — |
| 003 | [Templates and Snippets](003-templates-snippets/spec.md) | — | implement 44/50 | — | — |
| 004 | [Application Settings](004-app-settings/spec.md) | — | implement 105/114 | — | — |
| 005 | [Note Rendering and Live Preview](005-live-preview/spec.md) | — | implement 111/122 | — | — |
| 006 | [Vault Service Layout](006-vault-service-layout/spec.md) | — | implement 68/75 | — | — |
| 007 | [Template Download](007-template-download/spec.md) | — | implement 49/50 | — | — |
| 008 | [Element Parameters](008-element-parameters/spec.md) | — | implement 57/58 | — | — |
| 009 | [PDF Export](009-pdf-export/spec.md) | — | done | — | — |
| 010 | [Мультиязычный интерфейс](010-ui-localization/spec.md) | — | implement 57/59 | — | — |
| 011 | [Синхронизация vault — история версий и доставка между устройствами](011-vault-sync/spec.md) | P2 | implement 71/80 | — | История версий и удалённый репозиторий (как синхронизация с GitHub в typst.app) плюс совместная работа с Syncthing: файлы между устройствами доставляет Syncthing, историю и резервную копию ведёт git, конфликты разрешаются в приложении. |
| 012 | [Редактор правил исключения](012-ignore-rules-editor/spec.md) | P3 | implement 18/19 | — | Правила исключения истории и Syncthing (`.gitignore`, `.stignore`) открываются и правятся прямо в приложении: виден весь файл, чужие правки не затираются, а правила, нужные приложению, нельзя сломать незаметно. |

Активная фича: `.specify/scripts/typsidian/feature.sh`.
