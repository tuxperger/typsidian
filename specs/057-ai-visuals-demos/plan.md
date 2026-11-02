# Implementation Plan: Наглядные объяснения и демонстрации

**Branch**: `learnharness-specs` | **Date**: 2026-11-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/057-ai-visuals-demos/spec.md`

## Summary

Учитель рисует схему (`draw`) — заметку Typst — и собирает демонстрацию (`demonstrate`) —
Typst с параметрами из `sys.inputs`, которые пользователь меняет ползунками. Всё собирается
в мире без файлов vault и без сети, с пределом 5 с; несобираемое возвращается учителю.

Макет — временный (не Claude Design), https://claude.ai/artifact/Kj7dBxxrJMADVpxRNQF2cW, экраны A–D;
исходник — `specs/057-ai-visuals-demos/design/visuals.html`.

## Technical Context

**Language/Version**: Rust 2021, TypeScript strict + React 19

**Primary Dependencies**: Typst (уже в ядре), `preview` (005), `ai/lesson` (051); новых нет

**Storage**: заметки `<lessons_folder>/Схемы/*.typ`, события урока

**Testing**: `cargo test` и `wasm-bindgen-test`; сценарий браузера

**Target Platform**: wasm32, native

**Project Type**: кроссплатформенное приложение с общим ядром

**Performance Goals**: зависшая демонстрация останавливается ≤ 5 с (SC-002)

**Constraints**: 0 обращений демонстрации к сети и vault (SC-001)

**Scale/Scope**: исходник ≤ 200 КБ

## Constitution Check

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Мир, сборка, параметры, пределы — ядро; UI рисует ползунки и SVG |
| II. Оба таргета | Typst собирается на обоих; предел — поток (native) и воркер (web) |
| III. Спека о поведении | Да; Typst — продуктовое решение (clarify спеки) |
| IV. TS strict | tsify |
| V. Внешние процессы | Нет: JavaScript-демонстраций нет (research §2) |
| VI, VII | Файлы vault, сборка локальная |
| VIII. Внешние движки | Typst — уже встроенный движок |
| IX. Обучение с ИИ | Новых путей отправки нет |
| X | Нет |
| XI. Сервер | Сборка с пределом; кода вне Typst нет |

Гейты пройдены.

## Project Structure

```text
packages/core/src/preview/figure_world.rs, packages/core/src/ai/figure.rs
packages/core/src/ai/lesson/{turn,prompt,file}.rs
packages/core/tests/common/figure_cases.rs
packages/host/src/commands.rs, dispatch.rs; packages/shell/src/{commands,lib}.rs; apps/server/src/allowed.rs
ui/src/lessons/{LessonFigure,DemoControls,LessonTab}.tsx, ui/src/preview/compile-worker.ts
ui/src/settings/AiSection.tsx, ui/src/vault-client{,-web,-tauri}.ts, ui/src/i18n/locales/{ru,en}/lessons.ts
```

## Complexity Tracking

| Отклонение | Почему | Что отвергнуто |
| --- | --- | --- |
| На native зависшая сборка оставляет поток до выхода из приложения | Rust не прерывает поток; отзывчивость важнее; повтор этой демонстрации блокируется | Отдельный процесс сборки — внешний процесс (принцип V) ради редкого случая |
