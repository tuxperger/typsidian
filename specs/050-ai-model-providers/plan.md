# Implementation Plan: Провайдеры моделей для обучения с ИИ

**Branch**: `050-ai-model-providers` | **Date**: 2026-11-01 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/050-ai-model-providers/spec.md`

## Summary

Новый модуль ядра `ai`: настройки vault, проверка адреса и доступа к файлам, клиенты Anthropic и
совместимого протокола, журнал отправленного, справочник цен. Команды идут через `typsidian-host`
на рабочий стол, Android и сервер, в web — через wasm-биндинги. В настройках появляется раздел
«Обучение с ИИ». Уроков здесь нет: единственный запрос к модели — пробный.

Макет — временный (не Claude Design), https://claude.ai/artifact/THhvsXAbinBudJvjoU9EvR, экраны A–I.

## Technical Context

**Language/Version**: Rust 2021 (ядро, host, shell, server), TypeScript strict + React 19 (ui)

**Primary Dependencies**: существующие `reqwest`, `serde`, `serde_json`; новые чистые на Rust
`futures-util`, `futures-channel` (research §9); новых нативных зависимостей нет

**Storage**: `.typsidian/ai.json`, `.typsidian/ai-journal.jsonl`, `SecretStore`

**Testing**: `cargo test` и `wasm-bindgen-test` с поддельным `AiTransport`; реальный HTTP на
loopback — только native; сценарий `scripts/dev/browser.mjs`

**Target Platform**: wasm32 (web), native (desktop, Android, server)

**Project Type**: кроссплатформенное приложение с общим ядром

**Performance Goals**: сообщение об отказе не позднее 30 секунд (SC-005)

**Constraints**: без сетевых обращений при выключенной функции; ключ не покидает `SecretStore`,
кроме заголовка запроса к настроенному адресу

**Scale/Scope**: 500 записей журнала, 3 вида подключения

## Constitution Check

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Протоколы, проверки, журнал, цены — `core/ai`; UI только показывает |
| II. Оба таргета | `reqwest` работает на обоих, тесты общие с поддельным транспортом |
| III. Спека о поведении | Названия провайдеров — состав продукта, технологий в спеке нет |
| IV. TS strict | Типы через tsify, без `any` |
| V. Внешние процессы | Не добавляются, CLI исключены FR-004 |
| VI. Local-first | Без подключения всё работает; сервер не требуется |
| VII. Офлайн | Предел ожидания, отмена, отказы; тест отказа соединения |
| VIII. Внешние движки | Не затрагивается |
| IX. Обучение с ИИ | Выключено по умолчанию, на vault, журнал, запреты, индексов и фоновой отправки нет |
| X. Массовые операции | Нет |
| XI. Сервер | Белый список, флаг своего адреса, секреты по пользователям, security-review |

Все гейты пройдены, отклонений нет.

## Project Structure

```text
packages/core/src/ai/{mod,settings,endpoint,access,protocol,transport,client,journal,price,web}.rs
packages/core/tests/common/ai_cases.rs, tests/ai_native.rs, tests/ai_wasm.rs
packages/host/src/commands.rs, dispatch.rs (генерируется)
packages/shell/src/{commands,lib}.rs
apps/server/src/{allowed,config,invoke}.rs
ui/src/settings/AiSection.tsx, ui/src/settings/AiJournal.tsx
ui/src/vault-client{,-web,-tauri}.ts, ui/src/i18n/locales/{ru,en}/ai.ts
scripts/dev/scenarios/ai-settings.mjs
```

## Complexity Tracking

Нет отклонений.
