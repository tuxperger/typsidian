# Implementation Plan: Проверка орфографии

**Branch**: `030-spellcheck` | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

## Summary

Ядро проверяет слова текста заметки по встроенным словарям Hunspell (`spellbook`, словари
`ru_RU` и `en_US` в UTF-8) и словарю vault `.typsidian/dictionary.txt`. Что считать текстом,
решает дерево сессии подсветки 024: проверяются листья текста разметки в видимых окнах,
формулы, код, метки, ссылки, адреса и `[[…]]` — нет. Ядро отдаёт диапазоны ошибок и
подсказки; UI подчёркивает, показывает исправления в контекстном меню редактора и раздел
настроек vault «Орфография» (переключатель, языки, словарь vault).

Макет — временный (не Claude Design), https://claude.ai/code/artifact/08a96207-ea01-47d7-a7ab-63f14d684d76, экраны O1–O3.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`), TypeScript strict (`ui/`)
**Primary Dependencies**: новая — `spellbook` 0.4.2 (чистый Rust, `no_std`, без нативных
библиотек — `nix-env-agent` не нужен); разбор — `typst::syntax` (уже в ядре)
**Storage**: словари — `packages/core/dictionaries/` (встроены); словарь vault —
`.typsidian/dictionary.txt`; настройки — таблица `spellcheck` в `.typsidian/vault.json`
**Testing**: `cargo test` (`spellcheck_native.rs`), `wasm-pack test` (`spellcheck_wasm.rs`),
бенч `spellcheck_scale`, сценарий `scripts/dev/scenarios/spellcheck.mjs`
**Target Platform**: web (wasm32), desktop и Android (Tauri)
**Performance Goals**: проверка окна (~150 строк) ≤ 5 мс нативно; ввод не ждёт проверки (SC-002)
**Constraints**: офлайн; словари +4 МБ — вшиты в нативное ядро, на web файлами рядом с модулем (research.md §1)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Границы текста, слова, языки, словари, словарь vault, подсказки — `spellcheck`. UI только расписание запросов, декорации и меню. |
| II. Паритет | PASS | Одни словари и один код на всех оболочках; тесты на обоих путях. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`. |
| V. Внешние процессы | N/A | Нет. |
| VI. Local-first | PASS | Словарь vault — обычный файл в vault. |
| VII. Офлайн | PASS | Словари встроены, сети нет. |
| VIII. Движки | N/A | — |
| IX. Локальность | PASS | Проверка на устройстве; словарь vault синхронизируется как настройки. |
| X. Массовые операции | N/A | Правка одного слова за раз. |

## Project Structure

```text
packages/core/dictionaries/{ru_RU,en_US}.{aff,dic}, NOTICE.md
packages/core/src/spellcheck/mod.rs        — SpellChecker, слова, языки, проверка окон, подсказки
packages/core/src/spellcheck/vault.rs      — словарь vault, spell_configure/add/remove
packages/core/src/spellcheck/web.rs        — wasm-класс SpellChecker
packages/core/src/highlight/{session,web}.rs — `spelling(windows, checker)`
packages/core/src/vault/vault_settings.rs  — таблица `spellcheck`
packages/core/tests/common/spellcheck_cases.rs, spellcheck_{native,wasm}.rs
packages/core/benches/spellcheck_scale.rs
packages/shell/src/lib.rs                  — состояние Speller, команды
ui/src/vault-client{,-web,-tauri}.ts       — SpellingClient, HighlightSession.spelling
ui/src/editor/highlight/typstHighlight.ts   — подчёркивание по сессии подсветки (CodeMirror)
ui/src/spellcheck/dictionaries.ts          — загрузка словарей на web (Cache API)
scripts/spell-dictionaries.mjs             — словари рядом с wasm-модулем (build:wasm)
ui/src/editor/editorContextMenu.ts, CodeEditor.tsx — исправления и «Добавить в словарь»
ui/src/settings/SpellcheckSection.tsx, SettingsPanel.tsx
ui/src/i18n/locales/{ru,en}/spellcheck.ts, ui/src/styles/app.css
scripts/dev/scenarios/spellcheck.mjs
```

## Complexity Tracking

Нет отклонений. Расхождение с макетом O2 (лист на телефоне) — research.md §7.
