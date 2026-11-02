# Implementation Plan: Видимые и редактируемые инструкции агентов

**Branch**: `learnharness-specs` | **Date**: 2026-11-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/059-ai-editable-prompts/spec.md`

## Summary

Исходные инструкции всех агентов линейки собираются в ядре в одну точку с версиями;
правки vault — в `.typsidian/ai-prompts.json` (синхронизируются), действуют со следующего
запроса, сбрасываются, получают пометку при обновлении исходной. Подраздел «Инструкции
агентов» в настройках ИИ.

Макет — временный (не Claude Design), https://claude.ai/artifact/UyXxv4562czWrr3tWiYcJt, экраны A–D;
исходник — `specs/059-ai-editable-prompts/design/prompts.html`.

## Technical Context

**Language/Version**: Rust 2021, TypeScript strict + React 19

**Primary Dependencies**: `ai/lesson` (051) и агенты 052–056 по мере готовности

**Storage**: `.typsidian/ai-prompts.json`

**Testing**: `cargo test` и `wasm-bindgen-test`

**Target Platform**: wasm32, native

**Project Type**: кроссплатформенное приложение с общим ядром

**Performance Goals**: —

**Constraints**: правка не влияет на проверки 050 (SC-003)

**Scale/Scope**: до 6 агентов × 2 языка

## Constitution Check

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Выбор инструкции, хранение, версии — ядро |
| II. Оба таргета | Да |
| III–VIII | Соблюдаются / не затрагиваются |
| IX. Обучение с ИИ | Прозрачность отправляемого; гарантии доступа вне инструкций |
| X | Нет |
| XI. Сервер | Команды без сети, в белом списке |

Гейты пройдены.

## Project Structure

```text
packages/core/src/ai/prompts.rs, packages/core/src/ai/lesson/prompt.rs (перенос в prompts.rs)
packages/core/tests/common/prompts_cases.rs
packages/host/src/commands.rs, dispatch.rs; packages/shell/src/{commands,lib}.rs; apps/server/src/allowed.rs
ui/src/settings/{AiPrompts,PromptCompare}.tsx, ui/src/settings/AiSection.tsx
ui/src/vault-client{,-web,-tauri}.ts, ui/src/i18n/locales/{ru,en}/ai.ts
```

## Complexity Tracking

Нет отклонений.
