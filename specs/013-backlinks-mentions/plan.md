# Implementation Plan: Обратные ссылки и упоминания

**Branch**: `013-backlinks-mentions` | **Date**: 2026-10-10 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/013-backlinks-mentions/spec.md`

## Summary

У открытой заметки появляется колонка со списком обратных ссылок (заметки-источники,
фрагменты, переход к месту) и раскрываемым списком упоминаний названия без ссылки с
действием «Связать».

Технически: разбор `[[…]]` в `graph` начинает возвращать места вхождений (research.md §2);
новый модуль ядра `links/` строит страницу обратных ссылок из индекса и текущего текста
источников (§4), ищет упоминания отменяемым проходом порциями (§7) и превращает упоминание в
ссылку одной функцией, которая сама решает — писать файл со снимком `expected` или вернуть
правку для редактора (§9). Правка открытой заметки идёт через порт (§11). Фича заводит общую
колонку заметки `ui/src/note-panel/` с реестром режимов и состоянием в настройках устройства
(§12); 015 и 017 добавляют в неё свои режимы.

Макет — Claude Design, проект «Typsidian: Typst Editor Design»
(`b7a3b1f6-608f-403d-8c07-8101d15dce66`), файл `Typsidian Связи.dc.html`, экраны 1a–1c.

## Technical Context

**Language/Version**: Rust 2021 (ядро, shell), TypeScript strict + React 19 (ui)

**Primary Dependencies**: новых нет. Существующие `regex` (поиск названия, как у `search`),
`serde`, `tsify-next`, `wasm-bindgen`, Tauri 2, CodeMirror 6

**Storage**: файлы vault через `VaultBackend`; индекс ссылок 001 (`vault::cache`, без
изменения схемы); состояние колонки — поле `Settings.note_panel` в настройках устройства
(004)

**Testing**: `cargo test` и `wasm-bindgen-test` (ядро, оба таргета) на общей таблице
`tests/common/links_cases.rs`; `pnpm typecheck`; сценарий `scripts/dev/browser.mjs` со
снимком и ручная проверка по quickstart.md (JS-тестов в репозитории нет)

**Target Platform**: web (wasm32), desktop Linux (Tauri), Android (Tauri). iOS-оболочки нет

**Project Type**: монорепо — Rust-ядро + общий UI + три оболочки

**Performance Goals**: SC-001 — первая порция обратных ссылок ≤ 300 мс на 5 000 заметок;
SC-003 — правка видна ≤ 2 с после сохранения; SC-004 — проход упоминаний ≤ 3 с на 5 000
заметок без задержки набора

**Constraints**: «Связать» меняет в файле только упоминание (SC-005); ни одно действие не
теряет несохранённых правок (SC-006); без сети; ничего не покидает устройство

**Scale/Scope**: vault до 10 000 заметок (предел 001), заметка до 10 МиБ; сотни источников
на одну цель

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Места ссылок, правила упоминания, порядок и порции, фрагменты, запись ссылки, проверка «текст на месте», выбор «файл или редактор», состояние колонки — в `packages/core`. UI держит ритм запросов и рисует. Тесты — на обоих путях. |
| II. Паритет таргетов | PASS | Всё поверх трейта `VaultBackend`, без платформенного кода; биндинги wasm и команды Tauri 1:1 (contracts/core-links-api.md). Подачу «колонка или шторка» выбирает ширина окна, а не таргет. |
| III. Спека без реализации | PASS | В спеке нет крейтов, путей кода и схем; `[[путь]]` — запись, которую пишет пользователь. |
| IV. TS strict, без any | PASS | Новые типы — `Tsify` из Rust, в UI через `import type`; ручных дубликатов нет. |
| V. Внешние процессы | **PARTIAL** | Моста в коде нет, но замена текста в открытой заметке (FR-016) — операция, которую мост обязан будет проводить сам. Вынесена за порт с одной реализацией — см. Complexity Tracking. Остальная фича мост не затрагивает. |
| VI. Local-first | PASS | Ссылка — обычный текст в файле заметки; панель читается из файлов и восстановимого индекса. |
| VII. Офлайн | PASS | Сетевых обращений нет (FR-019); проверять в офлайне нечего сверх обычных тестов ядра. |
| VIII. Внешние движки | PASS | При открытой доске колонка стоит рядом с контейнером `BoardCanvas` и в его DOM не входит; содержимое досок не читается. |
| IX. Локальность | PASS | Индекс и проход — на устройстве; содержимое и фрагменты никуда не отправляются. |
| X. Массовые операции | PASS | Проход упоминаний — порциями, с прогрессом, отменяется сворачиванием; ничего не пишет, поэтому отмена не может оставить vault в промежуточном состоянии. «Связать» — одна атомарная запись одной заметки, «Связать все» вне скоупа. |

Пересмотр после проектирования (Phase 1): статусы не изменились. Нарушение по принципу V
обосновано ниже и требует явного принятия пользователем.

## Project Structure

### Documentation (this feature)

```text
specs/013-backlinks-mentions/
├── plan.md · research.md · data-model.md · quickstart.md
├── contracts/core-links-api.md · contracts/note-panel.md · contracts/editor-edit-port.md
├── checklists/requirements.md
└── tasks.md            # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/
├── src/lib.rs                      # pub mod links
├── src/graph/mod.rs                # link_sites, link_text; resolve_link_target хранит .excalidraw
├── src/links/
│   ├── mod.rs                      # OpenText, SitePlace, SiteRow, SourceGroup, title_of, порядок по близости
│   ├── snippet.rs                  # место (строка, колонка) и фрагмент-окно
│   ├── backlinks.rs                # BacklinksPage, backlinks_page
│   ├── mentions.rs                 # availability, mentions_in_text, MentionScan: start/step/cancel, in_notes
│   ├── link.rs                     # LinkRequest, LinkOutcome, plan_link, link_mention
│   └── web.rs                      # wasm-bindgen: links*
├── src/settings/mod.rs             # NotePanelState, NotePanelMode, Settings.note_panel
├── src/settings/apply.rs           # SettingsChange.note_panel, route → устройство
├── src/settings/keybindings.rs     # CommandId::ToggleNotePanel
└── tests/common/mod.rs             # MemoryVault: expected в write, несохранённые правки, счётчик записей
    tests/common/links_cases.rs · tests/links_native.rs · tests/links_wasm.rs
    tests/common/note_panel_cases.rs (раннеры — settings_native.rs, settings_wasm.rs)

packages/shell/src/lib.rs           # команды links_*

ui/src/
├── vault-client.ts · vault-client-web.ts · vault-client-tauri.ts   # links: {…}
├── note-panel/
│   ├── NotePanel.tsx               # оболочка: шапка режимов, колонка | шторка
│   ├── modes.ts                    # NotePanelModeSpec, реестр режимов
│   ├── useNotePanelState.ts        # Settings.note_panel ↔ устройство
│   ├── useNarrowViewport.ts
│   └── links/
│       ├── LinksMode.tsx           # два раздела, состояния макета 1b
│       ├── useNoteLinks.ts         # ритм запросов, цикл прохода
│       ├── openNoteEditors.ts      # порт правки открытой заметки
│       ├── plainEditors.ts         # реализация порта для обычного ввода
│       └── describe.ts             # исходы «Связать» → Phrase
├── workspace/Workspace.tsx         # проп device, кнопка, сочетание, палитра, linksRevision, revealPlace
├── editor/NoteEditor.tsx           # onSaved, сегмент статус-бара linksSummary
├── settings/ShortcutsSection.tsx   # подпись команды
├── i18n/locales/en/links.ts · ru/links.ts · en.ts · ru.ts
└── styles/app.css                  # .ts-notepanel*, .ts-links*

apps/web/app/VaultWorkspace.tsx · apps/desktop/src/DesktopWorkspace.tsx ·
apps/mobile/src/MobileWorkspace.tsx     # device={device} в Workspace

scripts/dev/scenarios/note-links.mjs    # сценарий и снимки для сверки с макетом
```

**Structure Decision**: разбор и запись самой ссылки остаются в `graph` (там уже живут
`extract_links` и `resolve_link_target`); всё, что относится к панели, — новый модуль
`links/` рядом с `search/`, по образцу `export/` (чистые функции + тонкие биндинги в
`web.rs`). Общая колонка — новый каталог `ui/src/note-panel/`; режим «Связи» — его
подкаталог `links/`, чтобы 015 (`outline/`) и 017 (`sources/`) легли рядом.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| Принцип V: замена текста в открытой заметке (FR-016) спланирована без отдельного `/speckit-clarify` по мосту к внешнему редактору | FR-016 требует, чтобы «Связать» в заметке с несохранёнными правками попадало в текст редактора. Моста в коде сейчас нет, единственный режим редактора — обычный ввод CodeMirror | Прямой `view.dispatch` из панели стёр бы границу гейта, как это описано у 003. Поэтому операция вынесена за порт `OpenNoteEditors` (contracts/editor-edit-port.md) с одной реализацией `plainEditors.ts`; для режимов через мост реализаций нет, порт отвечает `NoEditor`, панель показывает отказ и ничего не пишет. **Условие снятия**: отдельный `/speckit-clarify` по мосту (тот же, что ждёт порт вставки 003) — после него рядом с `plainEditors.ts` дописываются реализации остальных режимов без изменения панели. До принятия пользователем — открытый пункт. |

## Зависимости от соседних фич

- **014** (вкладки): передаёт в `useNoteLinks` тексты всех открытых вкладок (`openTexts`),
  расширяет `plainEditors.ts` на все вкладки, оборачивает `revealPlace` историей переходов.
  До 014 сценарий FR-016 в интерфейсе недостижим (открыта одна заметка, и она же цель
  панели) и покрыт тестами ядра.
- **015** (оглавление): добавляет `NotePanelMode::Outline`, режим в `modes.ts`, свои поля в
  `NotePanelState`; команда 015 (`Mod+Shift+O`) открывает колонку в своём режиме.
- **017** (библиография): третий режим `Sources`; подачу строк-фрагментов берёт из
  `.ts-links__*` (бриф, пересказ макета 017).
- **018** (дополнение): не зависит; `graph::link_sites` доступен ей для набора `[[`.
