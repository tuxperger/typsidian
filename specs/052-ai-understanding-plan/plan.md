# Implementation Plan: Замер понимания и план урока

**Branch**: `learnharness-specs` | **Date**: 2026-11-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/052-ai-understanding-plan/spec.md`

## Summary

Урок 051 получает стадию замера с инструментами `probe_question` и `probe_done` и план —
инструменты `plan_set` и `plan_progress` с проверкой структуры в ядре, версиями в файле
урока и раскладкой карты слоями. Во вкладке урока: метка стадии, итог замера, боковая
колонка плана (карта / список), история версий.

Макет — временный (не Claude Design), https://claude.ai/artifact/UP51s8BrDGsV2UrBSK6L12, экраны A–G;
исходник — `specs/052-ai-understanding-plan/design/plan.html`.

## Technical Context

**Language/Version**: Rust 2021, TypeScript strict + React 19

**Primary Dependencies**: модуль `ai/lesson` фичи 051; новых зависимостей нет

**Storage**: события в файле урока 051

**Testing**: `cargo test` и `wasm-bindgen-test` с поддельным потоком 051; сценарий браузера

**Target Platform**: wasm32 (web), native (desktop, Android, server)

**Project Type**: кроссплатформенное приложение с общим ядром

**Performance Goals**: раскладка плана до 40 шагов — мгновенно

**Constraints**: план с ошибкой структуры не показывается (SC-002); оценок способностей вне
урока нет (FR-008)

**Scale/Scope**: до 40 шагов плана, до 15 вопросов замера

## Constitution Check

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Стадии, проверка плана, раскладка, якоря — `core/ai/lesson` |
| II. Оба таргета | Чистый Rust, общие тесты |
| III. Спека о поведении | Да |
| IV. TS strict | tsify |
| V–VIII | Не затрагиваются |
| IX. Обучение с ИИ | Новых путей отправки нет: те же ходы урока, журнал 050 |
| X. Массовые операции | Нет |
| XI. Сервер | Новые команды — в белом списке, без сети |

Все гейты пройдены, отклонений нет.

## Project Structure

```text
packages/core/src/ai/lesson/{plan,turn,file,prompt,web}.rs
packages/core/tests/common/lesson_cases.rs
packages/host/src/commands.rs, dispatch.rs; packages/shell/src/{commands,lib}.rs; apps/server/src/allowed.rs
ui/src/lessons/{LessonTab,LessonPlan,ProbeResult,NewLessonDialog}.tsx
ui/src/vault-client{,-web,-tauri}.ts, ui/src/i18n/locales/{ru,en}/lessons.ts
scripts/dev/scenarios/ai-lesson-plan.mjs
```

## Complexity Tracking

Нет отклонений.
