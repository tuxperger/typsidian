# Implementation Plan: Фоновое исследование в уроке

**Branch**: `learnharness-specs` | **Date**: 2026-11-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/056-ai-background-research/spec.md`

## Summary

Фоновые исследования в уроке: веб — встроенным поиском провайдера модели (выключено по
умолчанию), PDF из vault — текстом страниц. Общий реестр заданий, карточки в ленте урока,
результат — заметка Typst с источниками, собранная ядром с экранированием.

Макет — временный (не Claude Design), https://claude.ai/artifact/6vgEiE4xUT9kdvoJJxwZKX, экраны A–E;
исходник — `specs/056-ai-background-research/design/research.html`.

## Technical Context

**Language/Version**: Rust 2021, TypeScript strict + React 19

**Primary Dependencies**: `ai/lesson` (051), реестр заданий (054), `pdf_text` (055); новых нет

**Storage**: события урока; заметки в `<lessons_folder>/Исследования/`

**Testing**: `cargo test` и `wasm-bindgen-test`; сценарий браузера

**Target Platform**: wasm32, native

**Project Type**: кроссплатформенное приложение с общим ядром

**Performance Goals**: урок отзывчив во время исследования (SC-001)

**Constraints**: при выключенном поиске 0 запросов поиска (SC-003)

**Scale/Scope**: единицы одновременных исследований

## Constitution Check

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Задания, сборка заметки, источники — ядро |
| II. Оба таргета | Чистый Rust |
| III. Спека о поведении | Да |
| IV. TS strict | tsify |
| V, VIII | Не затрагиваются |
| VI, VII | Заметка в vault; прерванное видно и повторяется |
| IX. Обучение с ИИ | Поиск — только у настроенного провайдера, выключен по умолчанию; PDF через проверку доступа и с оценкой; журнал |
| X. Массовые операции | Фон, очередь, отмена |
| XI. Сервер | Сетевая команда под пределом |

Все гейты пройдены, отклонений нет.

## Project Structure

```text
packages/core/src/ai/{jobs,research}.rs, packages/core/src/ai/lesson/{turn,prompt,file}.rs
packages/core/src/ai/protocol.rs                     # инструменты поиска провайдеров
packages/core/tests/common/research_cases.rs
packages/host/src/commands.rs, dispatch.rs; packages/shell/src/{commands,lib}.rs; apps/server/src/{allowed,invoke}.rs
ui/src/lessons/{ResearchCard,ResearchInput,LessonTab}.tsx, ui/src/settings/AiSection.tsx
ui/src/vault-client{,-web,-tauri}.ts, ui/src/i18n/locales/{ru,en}/lessons.ts
```

## Complexity Tracking

Нет отклонений.
