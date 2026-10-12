# Implementation Plan: Библиография vault

**Branch**: `017-vault-bibliography` | **Date**: 2026-10-10 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/017-vault-bibliography/spec.md`

**Note**: фича реализуется пятой из шести фич P2 (013 → 014 → 015 → 016 → 017 → 018) и
опирается на то, что заводят 013 (колонка заметки, `revealPlace`) и 015 (модуль `structure/`).
Спека не проходила отдельный `/speckit-clarify`; неоднозначности закрыты в research.md, три
из них меняют объём и помечены там **[объём]**.

## Summary

Источники vault — записи файлов BibTeX `.bib` и Hayagriva `.yml`, лежащих в папке заметок.
Пользователь вставляет цитату поиском по автору, названию, году и ключу; видит список
источников и заметки, которые каждый цитируют; цитаты с ключом, которого нет ни в одном
файле, отмечены в редакторе до сборки.

Технически это новый модуль ядра `packages/core/src/sources/`: чтение файлов крейтами
`hayagriva` и `biblatex`, которые уже приезжают с `typst` (research.md §1), с чтением по
записям при ошибке (§2); библиотека с поиском (§14); индекс цитат, который строится проходом
по vault порциями (§8); запись цитаты и предложение списка литературы (§12, §13); отметки
(§10). Цитаты в тексте находит общий модуль `structure/` фичи 015, которому 017 добавляет два
поля и одну функцию (§9). В UI — третий режим колонки заметки 013, палитра у каретки по
образцу меню вставки и общий с 018 механизм отметок CodeMirror.

Макет — Claude Design, проект «Typsidian: Typst Editor Design»
(`b7a3b1f6-608f-403d-8c07-8101d15dce66`), файл `Typsidian Библиография.dc.html`, экраны 1a–1c.

Расхождения с макетом — research.md §16 (десять решений).

## Technical Context

**Language/Version**: Rust 2021 (ядро, shell), TypeScript strict + React 19 (ui)

**Primary Dependencies**: `hayagriva 0.10.1`, `biblatex 0.12.0`, `serde_yaml 0.9` —
объявляются прямыми, уже есть в `Cargo.lock` транзитивно через `typst-library 0.15.1`, новых
крейтов в сборке нет (research.md §1); `typst::syntax` через `structure/` (015); CodeMirror 6
(`@codemirror/view`, `@codemirror/state` — уже в `ui/package.json`, новых пакетов нет)

**Storage**: файлы vault через `VaultBackend`; настройка — поле `VaultOverrides.source_files`
в `.typsidian/vault.json`; состояние панели — `Settings.note_panel` устройства (013);
библиотека и индекс цитат — только в памяти

**Testing**: `cargo test` и `wasm-bindgen-test` (ядро, оба таргета, общая таблица
`tests/common/sources_cases.rs`); бенчмарк `sources_scale`; `pnpm typecheck`;
`scripts/check-i18n.sh`; сценарий `scripts/dev/scenarios/sources.mjs` со снимком;
ручная проверка по quickstart.md (JS-тестов в репозитории нет)

**Target Platform**: web (wasm32), desktop Linux (Tauri), Android (Tauri). iOS-оболочки нет

**Project Type**: монорепо — Rust-ядро + общий UI + три оболочки

**Performance Goals**: SC-002 — выдача поиска по 10 000 источников ≤200 мс после нажатия;
SC-004 — список цитирующих ≤1 с в vault из 5 000 заметок (на построенном индексе, §8);
SC-001 — вставка известного источника ≤10 с

**Constraints**: без сети (FR-009); ничего не покидает устройство; первый проход по vault —
отменяемый, с прогрессом, не блокирует ввод (принцип X); файл источников приложением не
правится, кроме создания пустого

**Scale/Scope**: до 10 000 источников суммарно, до 10 000 заметок (предел vault, 001); не
больше 200 названных ошибок на файл

## Constitution Check

*GATE: пройден до Phase 0; перепроверен после Phase 1.*

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Какие файлы — источники, чтение и терпимость к ошибкам, поиск и порядок выдачи, запись цитаты, предложение списка литературы, индекс цитат и его переходы, правило отметок — в `packages/core/src/sources/` и `structure/`. UI выбирает момент вызова, крутит цикл прохода и рисует. Тесты — на обоих путях. |
| II. Паритет таргетов | PASS | Крейты чтения уже собираются под wasm32 в составе `typst`; обнаружение изменений — за трейтом `VaultBackend` (`list_source_candidates`) и одним событием `SourceFilesChanged` с реализацией на таргет; биндинги wasm и команды Tauri 1:1 (contracts/core-sources-api.md). Зонд T001 закрепляет прочитанное об API сборкой на обоих таргетах. |
| III. Спека без реализации | PASS | В спеке названы только форматы файлов, которые видит пользователь (`.bib`, `.yml`), — это наблюдаемое поведение. Крейтов, путей кода и схем в ней нет. |
| IV. TS strict, без any | PASS | Все типы границы — `Tsify` из Rust через `import type`. Единственный тип UI — `EditorMark` (вход механизма подачи, ядро его не знает). |
| V. Мост к Neovim | N/A | Мост не затрагивается. Цитата вставляется существующим `applyEdits` редактора — тем же, чем правка элементов 008; нового канала к редактору и новых обращений к мосту нет. |
| VI. Local-first | PASS | Источники — обычные `.bib`/`.yml` пользователя, читаемые без приложения; своего формата нет. Приложение пишет только пустой `references.bib` по явной команде и строку `#bibliography` по явному согласию. |
| VII. Офлайн | PASS | Сетевых вызовов нет вовсе (FR-009); проверяется тем, что тесты идут на `MemoryVault` без сети. |
| VIII. Внешние движки | N/A | Встраиваемых движков нет. CodeMirror — не чёрный ящик: отметки ставятся его публичным API декораций. |
| IX. Локальность | PASS | Библиотека и индекс цитат — в памяти процесса, на диск и в сеть не уходят. |
| X. Массовые операции | **PARTIAL** | Проход индекса цитат — порциями по 50 заметок, с прогрессом и отменой, UI уступает циклу событий; отмена ничего не пишет в vault. Разбор одного большого файла источников на web идёт в главном потоке — см. Complexity Tracking. |

Пересмотр после проектирования (Phase 1): статусы не изменились.

## Project Structure

### Documentation (this feature)

```text
specs/017-vault-bibliography/
├── plan.md · research.md · data-model.md · quickstart.md
├── contracts/core-sources-api.md · contracts/frontend-sources.md
├── checklists/requirements.md
└── tasks.md            # /speckit-tasks
```

### Source Code (repository root)

```text
packages/core/
├── Cargo.toml                       # + hayagriva, biblatex, serde_yaml (уже в lock); bench sources_scale
├── src/lib.rs                       # pub mod sources
├── src/sources/
│   ├── mod.rs                       # Source, SourceProblem, SourceFormat, create_source_file
│   ├── discover.rs                  # source_format, validate_source_file
│   ├── bibtex.rs                    # read_bibtex — чтение по записям
│   ├── yaml.rs                      # read_hayagriva — чтение по записям
│   ├── library.rs                   # SourceLibrary: refresh, summary, search
│   ├── citations.rs                 # CitationIndex: проход, правка, citing, missing_keys
│   ├── cite.rs                      # citation_text, plan_citation, bibliography_offer
│   ├── check.rs                     # check_note, CitationMark
│   ├── engine.rs                    # SourceEngine — фасад над библиотекой и индексом
│   └── web.rs                       # wasm-bindgen: класс SourceEngine
├── src/structure/walk.rs            # (015) + ветки cite(…) и bibliography(…)
├── src/structure/mod.rs             # (015) + CiteSite, BibliographySite, mode_at
├── src/vault/backend.rs             # + list_source_candidates
├── src/vault/native.rs              # обход кандидатов; вотчер шлёт SourceFilesChanged
├── src/vault/web.rs                 # обход кандидатов; poll_for_changes сверяет их снимок
├── src/vault/events.rs              # + VaultEvent::SourceFilesChanged
├── src/vault/vault_settings.rs      # + source_files, InvalidSourceFile
├── src/settings/keybindings.rs      # + InsertCitation, ToggleSources
├── src/settings/mod.rs              # (013) NotePanelMode::Sources, sources_uncited_only
├── benches/sources_scale.rs
└── tests/common/sources_cases.rs · tests/common/mod.rs (MemoryVault) ·
    tests/sources_native.rs · tests/sources_wasm.rs

packages/shell/src/lib.rs            # состояние Sources, команды sources_*

ui/src/
├── vault-client.ts · vault-client-web.ts · vault-client-tauri.ts   # + sources
├── sources/useSources.ts · useCitationMarks.ts · SourcesPanel.tsx ·
│   CitePalette.tsx · BibliographyPrompt.tsx · describe.ts
├── editor/marks.ts                  # общий механизм отметок (017 заводит, 018 пользуется)
├── editor/CodeEditor.tsx            # подключение editorMarks
├── editor/NoteEditor.tsx            # палитра, вставка через applyEdits, статус-бар
├── note-panel/modes.ts              # (013) режим «Источники»
├── workspace/Workspace.tsx          # команды, кнопка в шапке, useSources
├── settings/ShortcutsSection.tsx    # строки новых команд
├── i18n/locales/en/sources.ts · ru/sources.ts · en.ts · ru.ts
└── styles/app.css                   # .ts-sources*, .ts-cite*, .ts-mark--*

scripts/dev/scenarios/sources.mjs    # сценарий и снимок для сверки с макетом
```

**Structure Decision**: новый модуль ядра `sources/` рядом с `search/` и `export/`: у него
своё состояние (библиотека и индекс) и свой фасад, как у `export/batch.rs`. Разбор текста
заметки остаётся в `structure/` (015) — 017 добавляет туда поля, а не свой обход. Каталог UI
`ui/src/sources/` устроен по образцу `ui/src/export/`; отметки лежат в `ui/src/editor/`,
потому что принадлежат редактору, а не источникам.

## Зависимости от соседних фич

| Фича | Что берётся | Где описано |
|---|---|---|
| 013 | колонка заметки `ui/src/note-panel/` и `NotePanelModeSpec`; `Settings.note_panel` и `NotePanelMode`; `revealPlace({path, line, column})`; подача строк-фрагментов | `specs/013-backlinks-mentions/contracts/note-panel.md` |
| 015 | `structure::parse`, `NoteStructure.refs`/`labels`, `TextPlace`, правило расширения обхода | `specs/015-note-outline/contracts/core-structure-api.md` § «Расширение для 018 и 017» |
| 008 | `elements::edit::TextEdit` и `applyEdits` редактора | код |
| 009 | образец машины прохода и атомарная `write_bytes` | `specs/009-pdf-export/research.md` §4, §5 |
| 014 | после неё `revealPlace` оборачивается историей переходов; 017 этого не касается | — |
| 018 (потребитель) | `sources_for_completion`, механизм `ui/src/editor/marks.ts`, параметр `known_labels` | contracts/, research.md §10, §18 |

Если 017 начнётся раньше, чем готовы 013 или 015, задачи фаз US2 (панель) и T006 (поля
`structure/`) блокируются; ядро чтения, поиска и записи цитаты (фазы 2–3 без палитры) от них
не зависит.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| Принцип X на web: разбор одного файла источников блокирует главный поток на время разбора (ожидаемо десятки–сотни мс на 10 000 записей) | Ядро в wasm работает в главном потоке; исполнителя в Web Worker нет (005 T022 открыта) | Свой воркер для источников — второй экземпляр ядра и доступа к vault из воркера, то есть работа 005 T022 наполовину. Смягчение: `refresh` читает по одному файлу за вызов и UI уступает циклу событий между файлами; бюджет меряет бенчмарк `sources_scale`; повторный разбор идёт только для изменившегося файла. Снимается вместе с 005 T022 без изменения формы вызова. **Требует принятия пользователем.** |
| Ложная красная отметка у `@метка` на метку из связанного файла — до реализации 018 (research.md §10) | SC-003 требует 100 % отмеченных битых цитат, а отличить по тексту ссылку на чужую метку от цитаты нельзя | Не отмечать неоднозначные `@k` вовсе — пропуск битых цитат, прямое нарушение SC-003. Снимается, когда 018 передаёт `known_labels`. |
