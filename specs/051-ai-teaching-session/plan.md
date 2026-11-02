# Implementation Plan: Сессия урока с ИИ-учителем

**Branch**: `learnharness-specs` | **Date**: 2026-11-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/051-ai-teaching-session/spec.md`

## Summary

Модуль ядра `ai/lesson`: файл урока `.lesson` (JSON Lines, только дописывание), учитель с
инструментами `ask_question` и `read_note`, потоковый разбор ответов Anthropic и совместимого
протокола, сжатие длинной переписки, восстановление оборванного ответа. Новый вид файла
`Lesson` открывается вкладкой урока; список уроков и диалог «Новый урок» в `ui/src/lessons/`.
Команды — через `typsidian-host` (рабочий стол, Android, сервер) и wasm.

Макет — временный (не Claude Design), https://claude.ai/artifact/5HgfE8QuCSKK5eRAeioLEY, экраны A–J;
исходник — `specs/051-ai-teaching-session/design/lesson.html`.

## Technical Context

**Language/Version**: Rust 2021 (ядро, host, shell, server), TypeScript strict + React 19 (ui)

**Primary Dependencies**: существующие `reqwest`, `serde_json`, `futures-util`; `web-sys`
получает фичи `ReadableStream`, `ReadableStreamDefaultReader` (чистый wasm, без нативных
библиотек)

**Storage**: файлы `*.lesson` в vault; `.typsidian/lesson-partial/` (устройство); журнал 050

**Testing**: `cargo test` и `wasm-bindgen-test` с поддельным `AiTransport`, отдающим поток;
сценарий `scripts/dev/scenarios/ai-lesson.mjs`

**Target Platform**: wasm32 (web), native (desktop, Android, server)

**Project Type**: кроссплатформенное приложение с общим ядром

**Performance Goals**: первая часть ответа на экране ≤ 1 с после её получения (SC-005,
опрос 150 мс)

**Constraints**: ни одного запроса при выключенной функции; ответ пользователя на диске до
сети; блокировка vault на время сети не держится

**Scale/Scope**: урок до сотен шагов, сжатие с 100 тыс. единиц

## Constitution Check

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Формат, сборка запроса, разбор потока, решение о продолжении и сжатии — `core/ai/lesson`; UI показывает и опрашивает |
| II. Оба таргета | Разбор потока общий; чтение тела — `reqwest` и `ReadableStream`; общие тесты |
| III. Спека о поведении | Спека без технологий; формат файла — здесь |
| IV. TS strict | Типы через tsify |
| V. Внешние процессы | Нет |
| VI. Local-first | Урок — файл vault; без подключения приложение работает |
| VII. Офлайн | Ответ на диске до сети, оборванный ответ сохраняется, повтор |
| VIII. Внешние движки | Не затрагивается |
| IX. Обучение с ИИ | Только при включении; заметки — по выбору или инструменту, через проверку доступа; журнал до отправки; фоновой отправки нет |
| X. Массовые операции | Нет |
| XI. Сервер | Белый список, предел сетевых команд, `vault_id` от сервера; security-review до «готово» |

Все гейты пройдены, отклонений нет.

## Project Structure

```text
packages/core/src/ai/lesson/{mod,file,turn,stream,prompt,partial,web}.rs
packages/core/src/ai/{transport,settings,mod}.rs          # потоковая отправка, поля настроек
packages/core/src/vault/{mod,service_dir}.rs              # FileKind::Lesson, lesson-partial
packages/core/src/workspace/{model,view,persist}.rs       # вкладка урока
packages/core/tests/common/lesson_cases.rs, tests/ai_{native,wasm}.rs
packages/host/src/commands.rs, dispatch.rs; packages/shell/src/{commands,lib}.rs
apps/server/src/{allowed,invoke}.rs
ui/src/lessons/{LessonTab,LessonQuestion,LessonsScreen,NewLessonDialog}.tsx
ui/src/workspace/{Workspace,CommandPalette}.tsx, ui/src/workspace/tabs/EditorGroup.tsx
ui/src/vault-client{,-web,-tauri}.ts, ui/src/i18n/locales/{ru,en}/lessons.ts
scripts/dev/scenarios/ai-lesson.mjs
```

## Complexity Tracking

Нет отклонений. Edge Case «вторая вкладка только для чтения» заменён проверкой числа событий
(research §10); спека поправлена.
