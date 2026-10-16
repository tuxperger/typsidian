# Implementation Plan: Режим показа слайдов

**Branch**: `019-slides-mode` | **Date**: 2026-10-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/019-slides-mode/spec.md`

## Summary

Открытая заметка показывается слайдами: на рабочем столе и в web — в отдельном окне, которое
переносится на проектор, с видом докладчика во втором окне; на Android — слоем на весь экран
поверх приложения. Показ стартует со слайда под кареткой, листается клавишами, касанием и
кликером, переходит по номеру, на выходе ставит каретку на показанный слайд и следует за
пересборкой, не теряя номера.

Технически фича добавляет к сборке 005 два сведения о каждой странице, которых сейчас нет
(research.md §1–§3): диапазон текста заметки, попавшего на страницу, — для сопоставления
каретки и слайда (в 005 оно вынесено из скоупа, 005 spec «Вне скоупа»), — и метаданные
`<pdfpc>`, через которые пакеты презентаций (touying, polylux) отдают заметки докладчика и
номер логического слайда. Новый модуль ядра `packages/core/src/slides/` держит чистые
функции колоды: слайд под кареткой и каретка у слайда, листание, переход по номеру,
перенос номера через пересборку, подписи «слайд/шаг», раскладка клавиш показа
(research.md §4–§6). Состоянием показа владеет окно редактора; окна показа и докладчика
только рисуют присланные страницы и шлют действия обратно по каналу `ShowChannel` —
`BroadcastChannel` в web, события Tauri на рабочем столе (research.md §7–§9). Страницы
уходят в окна по отпечатку: перерисовывается только изменившееся (research.md §10).

Макет — временный (не Claude Design), https://claude.ai/code/artifact/e09bc800-3794-4acd-9101-68ec412a934a, экраны A1–A8.

Бриф для перерисовки в Claude Design — research.md §14.

## Technical Context

**Language/Version**: Rust 2021 (ядро, shell), TypeScript strict + React 19 (ui)

**Primary Dependencies**: новых крейтов и пакетов нет. `typst 0.15.1` (интроспектор,
`Label`, `MetadataElem`, спаны глифов `TextItem`) и `typst-svg` уже в ядре (005);
`@tauri-apps/api` 2.9 (`WebviewWindow`, `emitTo`, `listen`, `availableMonitors`) уже в `ui/`

**Storage**: vault не читается и не пишется. Состояние показа — память окна редактора на время
показа; выбранный вариант запуска не запоминается

**Testing**: `cargo test` и `wasm-bindgen-test` (ядро, общие сценарии
`tests/common/slides_cases.rs`), `pnpm typecheck`, сценарий headless-браузера
`scripts/dev/scenarios/slides.mjs` (web: окно показа, листание, переход по номеру, вид
докладчика, замеры SC-001/SC-002), ручная проверка рабочего стола и Android по quickstart.md
(JS-тестов в репозитории нет, `tauri-driver` нет)

**Target Platform**: web (wasm32), desktop Linux (Tauri), Android (Tauri)

**Project Type**: монорепо — Rust-ядро + общий UI + три оболочки

**Performance Goals**: SC-001 — первый слайд ≤2 с для 100 слайдов; SC-002 — смена слайда
≤100 мс; SC-004 — правка видна в показе ≤2 с. Бюджеты шагов — research.md §11

**Constraints**: без сети (FR-009); без записи в vault; окна показа и докладчика не держат
второго экземпляра ядра (research.md §7); сбор сведений о страницах не должен заметно
удлинять сборку 005 (≤5 % на презентации из 100 слайдов, research.md §11)

**Scale/Scope**: презентация до 300 страниц; один показ на приложение одновременно

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Какой текст на какой странице, где заметки докладчика и номер слайда, слайд под кареткой, листание и его границы, переход по номеру, перенос номера через пересборку, подписи «7 / 24 · шаг 2 из 3», какая клавиша что делает — в `packages/core/src/slides/` и сборке 005. UI держит состояние как непрозрачное значение, пересылает страницы и рисует. Тесты — на обоих путях. |
| II. Паритет таргетов | PASS | Сбор сведений — часть общей сборки, она уже идёт под wasm32 (воркер 005) и нативно. Функции колоды чистые. Биндинги wasm и команды Tauri 1:1 (contracts/core-slides-api.md). Различие окон спрятано за `ShowChannel` с реализацией на оболочку; вызывающий код не ветвится. |
| III. Спека без реализации | PASS | `spec.md` описывает поведение; «pdfpc» в Assumptions — имя формата, видимого пользователю пакета презентаций. |
| IV. TS strict, без any | PASS | Типы ядра — `Tsify`; сообщения канала — тип-объединение в `ui/src/slides/show-channel.ts` поверх этих типов, без дубликатов. |
| V. Мост к Neovim | N/A | Мост не затрагивается; каретка ставится тем же `reveal`, что переход к ошибке. |
| VI. Local-first | PASS | Vault не трогается; показ — производное от сборки. |
| VII. Офлайн | PASS | Сети нет (FR-009). Пакет, которого нет в кэше, — обычная ошибка сборки, показ называет её (FR-008, артборд A5). |
| VIII. Внешние движки | N/A | Движков нет. |
| IX. Локальность | PASS | Страницы передаются только между окнами одного приложения на устройстве. |
| X. Массовые операции | N/A | Прохода по vault нет. |

Пересмотр после проектирования (Phase 1): статусы не изменились. Нарушений нет; условные
решения — в Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/019-slides-mode/
├── plan.md · research.md · data-model.md · quickstart.md
├── contracts/core-slides-api.md · contracts/show-channel.md · contracts/frontend-show.md
├── checklists/requirements.md
└── tasks.md            # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/
├── src/lib.rs                      # pub mod slides
├── src/preview/compile.rs          # + CompiledDocument.slides (сбор при Render::Pages)
├── src/slides/
│   ├── mod.rs                      # PageSlideInfo, реэкспорт
│   ├── collect.rs                  # из PagedDocument: диапазоны текста, метаданные <pdfpc>
│   ├── deck.rs                     # ShowState, ShowAction, navigate, carry, page_at_caret, caret_at_page, slide_label
│   ├── keys.rs                     # show_action_for_key: клавиши и кликеры → ShowAction
│   └── web.rs                      # wasm-bindgen: slidesStart, slidesNavigate, slidesCarry, slidesCaret, slidesLabel, slidesActionForKey
├── src/settings/keybindings.rs     # + CommandId::StartShow (Mod+Alt+P)
└── tests/common/slides_cases.rs · tests/slides_native.rs · tests/slides_wasm.rs

packages/shell/src/lib.rs           # команды slides_* 1:1 с web.rs

ui/src/
├── vault-client.ts · vault-client-web.ts · vault-client-tauri.ts   # + slides.* (contracts/core-slides-api.md)
├── slides/
│   ├── show-channel.ts             # интерфейс ShowChannel, сообщения
│   ├── show-channel-web.ts         # window.open + BroadcastChannel
│   ├── show-channel-tauri.ts       # WebviewWindow + emitTo/listen, второй монитор
│   ├── useShowSession.ts           # окно редактора: состояние, вызовы ядра, рассылка страниц
│   ├── useShowWindow.ts            # окно показа/докладчика: приём страниц, отправка действий
│   ├── SlideImage.tsx              # страница SVG, вписанная в рамку
│   ├── ShowView.tsx                # окно показа: слайд, панель, номер, плашки (A2–A6)
│   ├── PresenterView.tsx           # вид докладчика (A7)
│   └── ShowOverlay.tsx             # Android: слой поверх приложения (A8)
├── workspace/Workspace.tsx         # кнопка «Показ» с меню (A1), команда, сочетание, выход → reveal
├── settings/ShortcutsSection.tsx   # название команды
├── i18n/locales/{ru,en}/slides.ts · i18n/locales/ru.ts · i18n/locales/en.ts
└── styles/app.css                  # .ts-show*, .ts-presenter*

apps/web/app/show/page.tsx          # маршрут окна показа и докладчика
apps/desktop/show.html · apps/desktop/src/show.tsx · apps/desktop/vite.config.ts   # вторая точка входа
apps/desktop/src-tauri/capabilities/default.json   # окна show/presenter, права на создание окна и события
apps/mobile/src/MobileWorkspace.tsx # канал «в том же окне»

scripts/dev/scenarios/slides.mjs    # сценарий сверки с макетом и замеров
```

**Structure Decision**: новый модуль ядра `slides/` рядом с `structure/` (015) и `export/`
(009); сбор сведений о страницах — в `slides/collect.rs`, а вызывается из
`preview/compile.rs`, потому что нужен свёрстанный документ и мир сборки, которые живут
только там. В UI — каталог `ui/src/slides/`; окна показа монтируются отдельными точками
входа оболочек, чтобы не тянуть в них рабочую область и ядро.

## Complexity Tracking

Нарушений конституции нет. Решения с порогом замера:

| Решение | Почему так | Условие пересмотра |
|---|---|---|
| Сведения о страницах собираются при каждой сборке предпросмотра, а не по запросу показа | Сопоставление каретки нужно в момент запуска (FR-001), а сборка уже держит документ и мир; повторная сборка ради показа нарушила бы SC-001 | Замер: сбор > 5 % времени сборки на 100 слайдах → собирать только когда показ открыт (флаг в запросе сборки) |
| Окна показа не держат ядро, состоянием владеет окно редактора | Одна точка истины даёт синхронность FR-005 без согласования; второе ядро в web — второй wasm на 30+ МБ шрифтов | Задержка действия из окна показа > 50 мс на рабочем столе → состояние дублируется в окне показа, редактор получает итог |
| Страницы — SVG той же сборки, что у предпросмотра, без растра | Тот же вывод 005, резкий на любом проекторе, без второго рендера | Смена слайда > 100 мс на тяжёлой странице (SC-002) → растровый кэш соседних страниц в окне показа |
