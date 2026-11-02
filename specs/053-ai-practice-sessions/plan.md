# Implementation Plan: Практика с ИИ

**Branch**: `learnharness-specs` | **Date**: 2026-11-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/053-ai-practice-sessions/spec.md`

## Summary

Сессия практики — файл урока 051 с `kind: Practice`: агент практики с инструментами
`pose_problem`, `give_hint`, `conclude`, решение фиксируется при постановке и скрыто ядром до
итога, следующая задача только по действию пользователя, сводка задач. Диалог «Новая
практика» с выбором уроков, заметок или описания.

Макет — временный (не Claude Design), https://claude.ai/artifact/9Gs9eBhoufHyiKJkmeMuxb, экраны A–E;
исходник — `specs/053-ai-practice-sessions/design/practice.html`.

## Technical Context

**Language/Version**: Rust 2021, TypeScript strict + React 19

**Primary Dependencies**: модуль `ai/lesson` (051); новых зависимостей нет

**Storage**: файл урока 051

**Testing**: `cargo test` и `wasm-bindgen-test` с поддельным потоком; сценарий браузера

**Target Platform**: wasm32, native

**Project Type**: кроссплатформенное приложение с общим ядром

**Performance Goals**: как у 051

**Constraints**: решение не попадает в UI до итога (SC-001)

**Scale/Scope**: десятки задач в сессии

## Constitution Check

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Инструменты, скрытие решения, сводка — ядро |
| II. Оба таргета | Чистый Rust, общие тесты |
| III. Спека о поведении | Да |
| IV. TS strict | tsify |
| V–VIII | Не затрагиваются |
| IX. Обучение с ИИ | Материал — только выбранный, через проверку доступа; журнал 050 |
| X. Массовые операции | Нет |
| XI. Сервер | Новые команды в белом списке; `ai_lesson_answer` уже ограничен |

Все гейты пройдены, отклонений нет.

## Project Structure

```text
packages/core/src/ai/lesson/{practice,turn,file,prompt,mod,web}.rs
packages/core/tests/common/lesson_cases.rs
packages/host/src/commands.rs, dispatch.rs; packages/shell/src/{commands,lib}.rs; apps/server/src/allowed.rs
ui/src/lessons/{PracticeFeed,NewPracticeDialog,PracticeSummary,LessonTab,LessonsScreen}.tsx
ui/src/vault-client{,-web,-tauri}.ts, ui/src/i18n/locales/{ru,en}/lessons.ts
scripts/dev/scenarios/ai-practice.mjs
```

## Complexity Tracking

Нет отклонений.
