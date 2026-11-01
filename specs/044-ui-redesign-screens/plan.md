# Implementation Plan: Остальные экраны по новому дизайну

**Branch**: `044-ui-redesign-screens` | **Date**: 2026-11-01 | **Spec**: [spec.md](spec.md)

## Summary

Перевёрстка `ui/` по артбордам 3–10 нового дизайна без изменений ядра и поведения:
палитра команд, колонка заметки вкладками справа от текста, карточка ошибки сборки над
бледным прошлым листом, панель синхронизации колонкой и слияние «свои / их», граф внутри
рабочей области с колонкой фильтров, настройки под шапкой, окно экспорта. Низ дерева
сверяется с артбордом 1: «+ Заметка», граф, задачи, источники, настройки.

Макет — Claude Design, экспорт артбордов в `specs/044-ui-redesign-screens/design/*.dc.html` (выгрузка `new_design/*.zip`): Palette (3), TextPanel (4), BuildError (5), Sync (6), Graph (7), Settings (9), Export (10); Main (1) и Welcome (2) — сверка с 043.

## Technical Context

**Language/Version**: TypeScript strict (`ui/`)
**Primary Dependencies**: новых нет
**Storage**: не меняется
**Testing**: сценарий `scripts/dev/scenarios/redesign-screens.mjs` (снимки экранов 3–10), гейт `ui`
**Target Platform**: web, рабочий стол, Android (общий `ui/`)
**Constraints**: только токены дизайн-системы; тексты из `ui/src/i18n/locales/`; новых сочетаний нет; элементы без поведения не рисуются (spec, Clarifications)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Меняется только показ: команды, диагностика, синхронизация, граф, настройки, экспорт — прежние вызовы ядра. |
| II. Паритет | PASS | Ядро не меняется; общий `ui/` во всех оболочках. |
| III. Спека | PASS | Поведение без API; новое из макетов вынесено в 045–049. |
| IV. TS strict | PASS | Гейт `ui`. |
| V–IX | PASS | Не затрагиваются. |
| X. Массовые операции | PASS | Массовое решение конфликтов (040) сохраняется как есть. |

## Project Structure

```text
ui/src/workspace/CommandPalette.tsx   — «Esc» в поле, место заметки справа, выделение совпадения
ui/src/note-panel/NotePanel.tsx       — вкладки режимов со счётчиком
ui/src/workspace/Workspace.tsx        — колонка заметки справа; граф и настройки под шапкой; название экрана в шапке
ui/src/vault-tree/Sidebar.tsx         — низ дерева: задачи вместо оглавления
ui/src/preview/PreviewPane.tsx        — карточки ошибок, бледный прошлый лист
ui/src/editor/                        — подчёркивание места ошибки, «N ошибок» в строке состояния
ui/src/sync/SyncPanel.tsx, MergeEditor.tsx — колонка синхронизации, слияние «свои / их»
ui/src/graph/GraphView.tsx, GraphPanel.tsx — колонка «Показывать» / «Обозначения», сводка внизу
ui/src/settings/SettingsPanel.tsx     — разделы слева, строки «название — поле — подсказка»
ui/src/export/ExportCenter.tsx        — окно «Экспорт»: формат, «Что», «Куда»
ui/src/styles/app.css                 — стили по токенам
ui/src/i18n/locales/{ru,en}/*.ts      — тексты
```

## Complexity Tracking

Нарушений нет.
