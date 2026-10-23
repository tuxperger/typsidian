# Tasks: Проверка орфографии

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/core-spellcheck-api.md](contracts/core-spellcheck-api.md)

## Phase 1: Основа

- [X] T001 Зависимость `spellbook` 0.4.2 в `packages/core/Cargo.toml` и `Cargo.lock`; словари `packages/core/dictionaries/{ru_RU,en_US}.{aff,dic}` в UTF-8 с `NOTICE.md` (лицензии, отметка о переводе `ru_RU` в UTF-8) (research.md §1)
- [X] T002 Таблица `spellcheck` (`enabled`, `languages`) в `vault/vault_settings.rs`: `spellcheck_enabled`, `spellcheck_languages` в `VaultOverrides`, разбор и запись с тестом (data-model.md)

## Phase 2: User Story 1 — опечатки подчёркнуты (P1)

- [X] T003 [US1] Тесты ядра `packages/core/tests/common/spellcheck_cases.rs`, вызовы в `spellcheck_native.rs` и `spellcheck_wasm.rs`: эталонная заметка — подчёркнуты ровно слова текста, не формулы/код/`#функции`/метки/`@ключи`/`[[…]]`/адреса/комментарии (SC-001, FR-002); ru и en в одной заметке (FR-004); дефис, ё/е, аббревиатуры, числа, смешанные азбуки (Edge Cases); подсказки содержат исправление (FR-003); выключенный язык и выключенная проверка; окна; ≥ 95% опечаток эталона подчёркнуты (SC-003)
- [X] T004 [US1] `packages/core/src/spellcheck/mod.rs`: `SpellChecker`, словари `OnceLock`, разбиение на слова, язык по азбуке, проверка дефиса и ё, `misspelled_in`, `spell_suggest`; `HighlightSession::spelling` по дереву сессии с вычетом `graph::link_sites`; экспорт в `lib.rs` (research.md §2–§3)
- [X] T005 [US1] Привязки: wasm-класс `SpellChecker` (`spellcheck/web.rs`), `SyntaxSession.spelling` (`highlight/web.rs`); состояние `Speller` и команды Tauri в `packages/shell/src/lib.rs`; `SpellingClient` и `HighlightSession.spelling` в `ui/src/vault-client{,-web,-tauri}.ts`
- [X] T006 [US1] UI: подчёркивание в `ui/src/editor/highlight/typstHighlight.ts` (расписание 400 мс по сессии подсветки), меню — `NoteEditor.tsx`; словари web — `ui/src/spellcheck/dictionaries.ts`, `scripts/spell-dictionaries.mjs`; исправления сверху контекстного меню и «Нет вариантов» в `editorContextMenu.ts`; тексты `ui/src/i18n/locales/{ru,en}/spellcheck.ts`; стиль волнистой линии в `app.css` (макет O1, O2)

## Phase 3: User Story 2 — свои слова (P2)

- [X] T007 [US2] Тесты ядра: добавить слово — файл `.typsidian/dictionary.txt` отсортирован без повторов, слово не подчёркнуто без учёта регистра; убрать — снова подчёркнуто; неверное слово — `InvalidWord`; vault только для чтения — `ReadOnly`; `spell_configure` читает файл, изменённый снаружи (FR-005) — `spellcheck_cases.rs`
- [X] T008 [US2] `packages/core/src/spellcheck/vault.rs`: `spell_configure`, `spell_add_word`, `spell_remove_word`; пункт «Добавить в словарь» в меню редактора, перерисовка подчёркиваний во всех открытых редакторах

## Phase 4: User Story 3 — выключить и языки (P3)

- [X] T009 [US3] Раздел настроек vault «Орфография» `ui/src/settings/SpellcheckSection.tsx` в `SettingsPanel.tsx`: переключатель, языки, словарь vault списком с добавлением и «убрать», пустое состояние (макет O3); смена настроек перенастраивает проверку без перезапуска

## Phase 5: Polish

- [X] T010 Бенч `packages/core/benches/spellcheck_scale.rs`: проверка окна 150 строк в заметке на 10 000 строк ≤ 5 мс (SC-002), загрузка словарей; запись в `Cargo.toml`
- [X] T011 Сценарий `scripts/dev/scenarios/spellcheck.mjs` по quickstart, сверка снимков с макетом O1–O3, README, `**Status**: Implemented`
