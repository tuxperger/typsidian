# Contract: перенос в интерфейсе дерева

Макет — временный, https://claude.ai/code/artifact/f749a41c-2f82-40ef-a46f-3c016e30239f.
Интерфейс не решает, что допустимо и что сломается: спрашивает ядро
(contracts/core-move-api.md) и рисует ответ.

## Один путь переноса: `useMoveEntry`

`ui/src/vault-tree/useMoveEntry.ts` — им пользуются жест, «Переместить в…» и отмена.

```text
requestMove(request, { confirm = true })
  1. moveImpact(request)        → отказ SamePlace: молча выйти; иной отказ → сообщение
  2. broken.length > 0 и confirm → BacklinksConfirmDialog; отказ пользователя → выйти
  3. moveEntry(request, control) → прогресс в нижней строке панели
  4. Done     → applyPathChanges(moved, folders); refresh(); сообщение «Перенесено в …» + «Отменить»
     Partial  → то же + сообщение «Перенесено не всё: …»
     Cancelled→ ничего
     ошибка   → сообщение по причине, refresh()
```

Пока идёт шаг 3, новый жест и «Переместить в…» недоступны (`pendingPath`, как у
существующих действий дерева).

## Жест: `useTreeDrag`

`ui/src/vault-tree/useTreeDrag.ts`, Pointer Events (research.md §8).

| Событие | Поведение | FR |
|---|---|---|
| захват | `moveTargets(from, "")` → множество целей; строка-источник `ts-tree__row--dragging`; ярлык `ts-tree-drag` у указателя | FR-009, FR-010 |
| движение | цель: строка папки → она; строка заметки → её папка; пустое место дерева → корень; вне множества → цели нет | FR-011, FR-012 |
| цель есть | `li.ts-tree__folder--drop` (или `.ts-tree--drop-root` на дереве) | FR-010 |
| цели нет | ярлык `ts-tree-drag--denied`, курсор `not-allowed` | FR-012 |
| 600 мс над свёрнутой папкой | раскрыть | FR-013 |
| указатель в 28 px от края дерева | прокрутка `requestAnimationFrame`, скорость от глубины захода | FR-013 |
| отпускание над целью | `requestMove` | FR-001 |
| Escape, отпускание вне цели, `blur` окна, `pointercancel` | отмена, фокус на исходной строке | FR-014 |

Не ломает существующее (FR-015): клик без сдвига открывает заметку или раскрывает
папку; кнопка «⋯», поле переименования и контекстное меню жест не начинают; во время
переименования захват его отменяет; на сенсорном экране сдвиг до 400 мс — прокрутка
(`touch-action: pan-y` на дереве, `none` только после начала жеста).

Раскрытость папок — множество путей в `useVaultTreeActions` (`expanded`,
`toggleExpanded`, `expand`), а не `useState` в `FolderRow` (research.md §9).

## «Переместить в…»: `MoveToDialog`

- Открывается из меню «⋯» и контекстного меню заметки, доски и папки (после
  «Переименовать»; у папки — после пункта раздела) и командой палитры для открытого файла.
- Разметка и классы палитры (`ts-palette-scrim`, `ts-palette`, `ts-palette__row`);
  заголовок — что переносится.
- Каждое изменение запроса → `moveTargets(from, query)`; строки рисуются как пришли.
- ↑/↓ — выбор, Enter — `requestMove`, Escape — закрыть и вернуть фокус на строку дерева.
- Пусто → текст «Нет папки с таким именем».

## Сообщения

В `ts-notices`, существующим видом (`role="status"` / `role="alert"`), тексты — ключи
`tree.move.*` в `ui/src/i18n/locales/{ru,en}/tree.ts`:

| Ключ | ru |
|---|---|
| `tree.move.menu` | Переместить в… |
| `tree.move.done` | Перенесено в {folder} |
| `tree.move.undo` | Отменить |
| `tree.move.undone` | Перенос отменён |
| `tree.move.refused.nameConflict` | В «{folder}» уже есть {name}. Переименуйте одну из заметок и повторите. |
| `tree.move.refused.accessDenied` | Vault открыт только для чтения — перенос недоступен. |
| `tree.move.refused.notFound` | «{name}» больше нет в vault. |
| `tree.move.refused.targetNotFound` | Папки «{folder}» больше нет. |
| `tree.move.refused.io` | Не удалось перенести «{name}». Ничего не изменилось. |
| `tree.move.partial` | Перенесено не всё: {count} остались в «{folder}». |
| `tree.move.undoRefused` | Отменить не получилось: в «{folder}» появилась «{name}». |
| `tree.move.progress` | перенос · {done} из {total} |
| `tree.move.confirm.title` | Перенести «{name}» в «{folder}»? |
| `tree.move.confirm.body` | После переноса перестанут работать ссылки в этих заметках: |
| `tree.move.dialog.title` | Переместить {name} |
| `tree.move.dialog.placeholder` | Папка… |
| `tree.move.dialog.root` | корень vault |
| `tree.move.dialog.empty` | Нет папки с таким именем |
| `tree.move.a11y.grabbed` | Переносится {name} |
| `tree.move.a11y.over` | В папку {folder} |
| `tree.move.a11y.overRoot` | В корень |
| `tree.move.a11y.denied` | Сюда нельзя |

Объявления `a11y.*` и итог — в `aria-live="polite"` области дерева (FR-026). Сообщение
об итоге держится 8 с или до следующего переноса; отмена — `requestMove(outcome.undo,
{ confirm: false })`.

## Стили

Только токены; все правила начинаются с `.ts-tree` / `.ts-tree-drag` / `.ts-palette`.
Подсветка цели: фон `rgba(255,106,0,.10)` + внутренняя рамка `--orange-500`; источник —
`opacity: .4`; ярлык — `--steel-800`, рамка `--border-strong`, `--shadow-lg`; поднятая
строка на сенсорном экране — `--steel-800`, `--shadow-lg`, `scale(1.03)`;
`prefers-reduced-motion` отключает масштаб и плавную прокрутку.
