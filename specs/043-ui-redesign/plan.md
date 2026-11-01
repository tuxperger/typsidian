# Implementation Plan: Оболочка интерфейса по дизайну Typsidian.pdf

**Branch**: `043-ui-redesign` | **Date**: 2026-10-28 | **Spec**: [spec.md](spec.md)

## Summary

Перестройка оболочки `ui/` по экранам 1–4 Typsidian.pdf без изменений ядра: шапка рабочей
области (переключатель vault, поле палитры, вид «Текст / Рядом / Документ», «Экспорт PDF»),
низ дерева со значками, полоса предпросмотра со страницей, статусом сборки и масштабом,
«История» в строке состояния, стартовый экран с недавними vault, экраны телефона.

Макет — Typsidian.pdf в корне репозитория (экраны 1–4); для поверхностей вне PDF — временный (не Claude Design) макет того же направления https://claude.ai/code/artifact/3d3485cf-c8de-4d6f-a6d3-db0432208bd6. Холст Claude Design https://claude.ai/artifact/Nc2R8hmwGDhBx3bE3fsXj8 прочитать не удалось (research.md, Decision 1).

## Technical Context

**Language/Version**: TypeScript strict (`ui/`, оболочки `apps/*`)
**Primary Dependencies**: новых нет
**Storage**: вид рабочей области — в раскладке колонок устройства (`usePaneLayout`, localStorage)
**Testing**: сценарий `scripts/dev/scenarios/redesign.mjs` (снимки экранов 1–4), гейт `ui`
**Target Platform**: web, рабочий стол, Android (общий `ui/`)
**Constraints**: только токены дизайн-системы; тексты из `ui/src/i18n/locales/`; новых сочетаний нет

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Меняется только показ; список vault, сборка, история — из существующих вызовов ядра. |
| II. Паритет | PASS | Ядро не меняется; общий `ui/` во всех оболочках. |
| III. Спека | PASS | Поведение без API. |
| IV. TS strict | PASS | Гейт `ui`. |
| V–IX, XI | PASS | Не затрагиваются. |
| X. Массовые операции | PASS | Не затрагиваются. |

## Project Structure

```text
ui/src/workspace/Workspace.tsx        — шапка, вид, стартовый экран, телефон
ui/src/workspace/VaultSwitcher.tsx    — новое: меню vault в шапке
ui/src/workspace/ViewSwitch.tsx       — новое: «Текст / Рядом / Документ»
ui/src/workspace/WelcomeScreen.tsx    — новое: стартовый экран с недавними
ui/src/workspace/usePaneLayout.ts     — поле вида
ui/src/vault-tree/Sidebar.tsx         — низ дерева
ui/src/preview/PreviewPane.tsx        — полоса предпросмотра, без линеек
ui/src/editor/NoteEditor.tsx          — «История» в строке состояния
ui/src/styles/app.css                 — стили по токенам
ui/src/i18n/locales/{ru,en}/*.ts      — тексты
apps/{web,desktop,mobile}             — открытие известного vault по id
```

## Complexity Tracking

Нарушений нет.
