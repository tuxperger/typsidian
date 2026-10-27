# Implementation Plan: Свернуть и развернуть папки разом

**Branch**: `042-tree-collapse-all` | **Date**: 2026-10-27 | **Spec**: [spec.md](spec.md)

## Summary

Свёрнутость папок уже живёт в состоянии дерева (`collapsed` — множество путей в
`useVaultTreeActions`, 001/023). Фича добавляет к нему три операции: свернуть все папки,
развернуть все, задать свёрнутость ветки (папка и все вложенные). Перечень папок дерево
уже строит из путей заметок (`buildTree`), поэтому ядро не меняется. В строку подписи
«Vault» встают две кнопки-значка, в меню папки с подпапками — группа «Развернуть вложенные /
Свернуть вложенные».

Макет — временный (не Claude Design), https://claude.ai/code/artifact/38f5a7fc-03b9-449e-bc6b-309703b9ad41, экраны «Кнопки над деревом», «Всё свёрнуто», «Меню папки», «Папок нет».

## Technical Context

**Language/Version**: TypeScript strict (`ui/`)
**Primary Dependencies**: новых нет
**Storage**: нет — состояние показа в памяти, как и прежде
**Testing**: сценарий `scripts/dev/scenarios/tree-collapse.mjs` в браузере; проверка типов
**Target Platform**: все оболочки (общий `ui/`)
**Performance Goals**: SC-002 — 500 папок, результат ≤ 0,2 с: одна замена множества путей и
одна отрисовка
**Constraints**: тексты только из `ui/src/i18n/locales/`; сочетаний клавиш не добавляется

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Свёрнутость — состояние показа, не правило vault; перечень папок — из уже построенного дерева показа. Ядро в этом не участвует и логику не дублирует. |
| II. Паритет | PASS | Ядро не меняется; общий `ui/` во всех оболочках. |
| III. Спека | PASS | Спека без API и структур. |
| IV. TS strict | PASS | Без `any`, гейт `ui`. |
| V. Внешние процессы | PASS | Не затрагиваются. |
| VI. Local-first | PASS | Файлы vault не меняются (FR-006). |
| VII. Офлайн | PASS | Сети нет. |
| IX. Локальность | PASS | Ничего не уходит наружу. |
| X. Массовые операции | PASS | Свёртка не трогает данных — отменять нечего; мгновенна. |
| VIII, XI | PASS | Не затрагиваются. |

## Project Structure

```text
ui/src/vault-tree/buildTree.ts           — folderPaths(дерево | ветка)
ui/src/vault-tree/VaultTreeActions.tsx   — collapseAll/expandAll/setBranchCollapsed, кнопки, пункты меню
ui/src/icons/Icons.tsx                   — значки collapseAll/expandAll
ui/src/styles/app.css                    — строка подписи с кнопками
ui/src/i18n/locales/{ru,en}/tree.ts      — тексты
scripts/dev/scenarios/tree-collapse.mjs  — сценарий проверки
```

## Complexity Tracking

Нарушений нет.
