# Implementation Plan: Разные модели для разных задач

**Branch**: `learnharness-specs` | **Date**: 2026-11-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/060-ai-model-roles/spec.md`

## Summary

Дополнительные подключения vault со своими ключами и одобрением адреса, назначение
подключения и модели каждой роли агентов, единая точка выбора `resolve_role`, роль и объёмы
кэша в журнале, предпочтение хранения кэша у провайдера.

Макет — временный (не Claude Design), https://claude.ai/artifact/YBt7cyCsbjUwPriLC4mP5n, экраны A–D;
исходник — `specs/060-ai-model-roles/design/roles.html`.

## Technical Context

**Language/Version**: Rust 2021, TypeScript strict + React 19

**Primary Dependencies**: `ai` 050, ходы 051–058

**Storage**: `.typsidian/ai.json`; секреты устройства

**Testing**: `cargo test` и `wasm-bindgen-test`

**Target Platform**: wasm32, native

**Project Type**: кроссплатформенное приложение с общим ядром

**Performance Goals**: —

**Constraints**: при умолчаниях запросы не меняются (SC-003); ключ подключения не уходит на
чужой адрес

**Scale/Scope**: единицы подключений, 7 ролей

## Constitution Check

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Разрешение ролей, кэш, журнал — ядро |
| II. Оба таргета | Да |
| III–VIII | Соблюдаются / не затрагиваются |
| IX. Обучение с ИИ | Только подключения пользователя; одобрение адресов; журнал с ролью |
| X | Нет |
| XI. Сервер | Свой адрес под флагом администратора; сетевая команда под пределом |

Гейты пройдены; security-review обязателен (новые секреты и адреса).

## Project Structure

```text
packages/core/src/ai/{roles,settings,client,protocol,journal,price}.rs, packages/core/src/sync/secrets.rs
packages/core/tests/common/roles_cases.rs
packages/host/src/commands.rs, dispatch.rs; packages/shell/src/{commands,lib}.rs; apps/server/src/{allowed,invoke}.rs
ui/src/settings/{AiConnections,AiRoles}.tsx, ui/src/settings/{AiSection,AiJournal}.tsx
ui/src/vault-client{,-web,-tauri}.ts, ui/src/i18n/locales/{ru,en}/ai.ts
```

## Complexity Tracking

Нет отклонений.
