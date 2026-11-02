# Implementation Plan: Диктовка ответов в уроке

**Branch**: `learnharness-specs` | **Date**: 2026-11-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/058-ai-dictation/spec.md`

## Summary

Подключение распознавания речи (OpenAI или свой совместимый адрес), запись в интерфейсе с
пределом 2 мин, распознавание в ядре (multipart), текст вставляется в поле для правки, звук
нигде не сохраняется, журнал 050 — с описанием звука и текстом. Разрешения микрофона в
оболочках рабочего стола и Android.

Макет — временный (не Claude Design), https://claude.ai/artifact/KgUXrKy13osG8KLDs5KHHd, экраны A–E;
исходник — `specs/058-ai-dictation/design/dictation.html`.

## Technical Context

**Language/Version**: Rust 2021, TypeScript strict + React 19

**Primary Dependencies**: `ai` 050; `MediaRecorder` браузера; новых Rust-зависимостей нет
(multipart собирается вручную)

**Storage**: настройки ИИ; секреты; звук — только память

**Testing**: `cargo test` и `wasm-bindgen-test`; ручная проверка микрофона на оболочках

**Target Platform**: wasm32, native (desktop WebKitGTK, Android WebView, сервер)

**Project Type**: кроссплатформенное приложение с общим ядром

**Performance Goals**: текст ≤ ответ подключения + 1 с (SC-001)

**Constraints**: 0 звуковых файлов на диске (SC-002)

**Scale/Scope**: запись ≤ 2 мин

## Constitution Check

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Запрос, multipart, журнал, одобрение — ядро; запись звука — ввод устройства в UI |
| II. Оба таргета | Да |
| III. Спека о поведении | Да |
| IV. TS strict | tsify |
| V. Внешние процессы | Нет |
| VI, VII | Обрыв — запись в памяти до повтора |
| VIII | Нет |
| IX. Обучение с ИИ | Только подключение пользователя; сторонних сервисов нет; журнал |
| X | Нет |
| XI. Сервер | Свой адрес под флагом, звук только в памяти, предел сетевых команд |

Гейты пройдены.

## Project Structure

```text
packages/core/src/ai/{speech,settings,transport,protocol,journal,client}.rs
packages/core/src/sync/secrets.rs
packages/core/tests/common/speech_cases.rs
packages/host/src/commands.rs, dispatch.rs; packages/shell/src/{commands,lib}.rs; apps/server/src/{allowed,invoke}.rs
apps/desktop/src-tauri/, apps/mobile/src-tauri/ (разрешения микрофона)
ui/src/lessons/{DictationButton,useRecorder}.tsx, ui/src/settings/AiSpeechSection.tsx
ui/src/vault-client{,-web,-tauri}.ts, ui/src/i18n/locales/{ru,en}/ai.ts
```

## Complexity Tracking

Нет отклонений.
