# Implementation Plan: Граф знаний из уроков

**Branch**: `learnharness-specs` | **Date**: 2026-11-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/054-ai-knowledge-graph/spec.md`

## Summary

Модуль ядра `ai/knowledge`: файлы графа в vault (только дописывание), фоновое извлечение
утверждений и цепочек из объяснений учителя с объединением без дублей, локальный поиск,
удаление с надгробиями, чтение графа агентом и выбор утверждений для замера и практики. Вид
«Граф знаний» на месте текста: карта на `GraphCanvas` 025 и таблица цепочек.

Макет — временный (не Claude Design), https://claude.ai/artifact/DYKd7aRYDi5qoEj1WFLXPF, экраны A–F;
исходник — `specs/054-ai-knowledge-graph/design/knowledge.html`.

## Technical Context

**Language/Version**: Rust 2021, TypeScript strict + React 19

**Primary Dependencies**: `ai/lesson` (051, поток и ходы), `graph` 025 (форма графа, холст);
новых зависимостей нет

**Storage**: `<lessons_folder>/Граф знаний/*.jsonl`

**Testing**: `cargo test` и `wasm-bindgen-test` с поддельным потоком; сценарий браузера

**Target Platform**: wasm32, native

**Project Type**: кроссплатформенное приложение с общим ядром

**Performance Goals**: граф из 2000 утверждений открывается и ищется < 1 с (SC-003)

**Constraints**: ответы пользователя не уходят на извлечение (SC-004); урок не ждёт (SC-001)

**Scale/Scope**: тысячи утверждений

## Constitution Check

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Извлечение, объединение, удаление, форма графа — ядро |
| II. Оба таргета | Чистый Rust, общие тесты |
| III. Спека о поведении | Да |
| IV. TS strict | tsify |
| V, VIII | Не затрагиваются |
| VI. Local-first | Граф — файлы vault |
| VII. Офлайн | Обрыв извлечения сохраняет готовое, повтор без дублей |
| IX. Обучение с ИИ | В модель — только объяснения учителя, уже полученные от неё; граф уходит модели только по выбору или разрешённым чтением; поиск локальный; журнал 050 |
| X. Массовые операции | Извлечение в фоне, прогресс, отмена |
| XI. Сервер | Белый список, сетевая команда под пределом |

Все гейты пройдены, отклонений нет.

## Project Structure

```text
packages/core/src/ai/knowledge/{mod,store,extract,merge,jobs,view,web}.rs
packages/core/src/ai/lesson/{turn,prompt}.rs        # read_knowledge, Material из графа
packages/core/tests/common/knowledge_cases.rs, tests/ai_{native,wasm}.rs
packages/host/src/commands.rs, dispatch.rs; packages/shell/src/{commands,lib}.rs
apps/server/src/{allowed,invoke}.rs
ui/src/knowledge/{KnowledgeView,KnowledgeChains,PropositionPanel,KnowledgeJobs}.tsx
ui/src/settings/AiSection.tsx, ui/src/lessons/{LessonTab,NewPracticeDialog,NewLessonDialog}.tsx
ui/src/vault-client{,-web,-tauri}.ts, ui/src/i18n/locales/{ru,en}/knowledge.ts
scripts/dev/scenarios/ai-knowledge.mjs
```

## Complexity Tracking

Нет отклонений.
