# Implementation Plan: Заметки в граф знаний

**Branch**: `learnharness-specs` | **Date**: 2026-11-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/055-ai-notes-to-graph/spec.md`

## Summary

Импорт выбранных заметок, папок и PDF в граф знаний 054 тем же заданием извлечения:
оценка до отправки с пропусками и стоимостью, части больших файлов, покрытие по хэшу
частей (повтор без дублей и без лишних запросов), итог по файлам с повтором. Вход — меню
дерева и кнопка в виде графа знаний.

Макет — временный (не Claude Design), https://claude.ai/artifact/TUbgxmBDvL9TZsouBfrFxG, экраны A–D;
исходник — `specs/055-ai-notes-to-graph/design/import.html`.

## Technical Context

**Language/Version**: Rust 2021, TypeScript strict + React 19

**Primary Dependencies**: `ai/knowledge` (054); `hayro-interpret` (уже в дереве) для текста
PDF (research §3)

**Storage**: файлы графа 054

**Testing**: `cargo test` и `wasm-bindgen-test`; сценарий браузера

**Target Platform**: wasm32, native

**Project Type**: кроссплатформенное приложение с общим ядром

**Performance Goals**: импорт 50 заметок не прерывает редактирование (SC-003)

**Constraints**: только явный выбор, запрещённые папки не уходят (SC-001)

**Scale/Scope**: десятки–сотни файлов за импорт

## Constitution Check

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Оценка, части, покрытие, итог — ядро |
| II. Оба таргета | Чистый Rust; PDF — тоже |
| III. Спека о поведении | Да |
| IV. TS strict | tsify |
| V, VIII | Не затрагиваются |
| VI, VII | Обрыв сохраняет готовое, повтор частей |
| IX. Обучение с ИИ | Только явный выбор, оценка и подтверждение до отправки, проверка доступа, журнал; всего vault нет |
| X. Массовые операции | Фон, прогресс, отмена |
| XI. Сервер | Сетевая команда под пределом 050 |

Все гейты пройдены, отклонений нет.

## Project Structure

```text
packages/core/src/ai/knowledge/{import,pdf_text,extract,jobs,web}.rs
packages/core/tests/common/knowledge_cases.rs
packages/host/src/commands.rs, dispatch.rs; packages/shell/src/{commands,lib}.rs; apps/server/src/{allowed,invoke}.rs
ui/src/knowledge/{ImportConfirm,ImportJob}.tsx, ui/src/vault-tree/VaultTreeActions.tsx, ui/src/knowledge/KnowledgeView.tsx
ui/src/vault-client{,-web,-tauri}.ts, ui/src/i18n/locales/{ru,en}/knowledge.ts
```

## Complexity Tracking

Нет отклонений.
