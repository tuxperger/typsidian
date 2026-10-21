# Implementation Plan: Отказ от внешнего модального редактора

**Branch**: `026-drop-external-editor` | **Date**: 2026-10-21 | **Spec**: [spec.md](spec.md)

## Summary

Фича 004 заложила режим редактирования через внешний модальный редактор: предпочтение
режима и путь к конфигу в настройках, состояние режима и его недоступность в событиях и
диагностике, предусловия смены режима (сохранить открытую заметку, не переключать при
конфликте), раздел режима в настройках и надпись «NORMAL» в статус-баре. Сам редактор так
и не появился: на всех таргетах действовала заглушка «недоступно». Фича убирает всё это
целиком — из ядра, оболочки, UI, переводов, документации, конституции и среды
разработки, — сохраняя чтение старых файлов настроек.

Макет — не требуется: фича только убирает элементы интерфейса, новых экранов и состояний
нет.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`, `packages/shell`), TypeScript strict (`ui/`)
**Primary Dependencies**: без новых; `serde_json` (формат настроек)
**Storage**: `settings.json` устройства и `.typsidian/vault.json` — теряют два ключа
**Testing**: `cargo test`, `wasm-pack test` (`settings_native`, `settings_wasm`), `pnpm typecheck`, сценарий `scripts/dev/scenarios/rules-editor.mjs` как дымовой для настроек
**Target Platform**: web (wasm32), desktop и Android (Tauri)
**Project Type**: ядро + оболочка + общий UI
**Constraints**: старые файлы настроек открываются (FR-004), устаревшие ключи не пишутся (FR-005)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Удаление ключей при записи — в форматах настроек ядра; UI ничего не решает. |
| II. Паритет | PASS | Удаление одинаково для wasm и native; оболочка теряет заглушку. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы `Tsify` перегенерируются; UI-ветки по удалённым вариантам исчезают вместе с типами — пропуск ловит компилятор. |
| V. Внешние процессы | PASS | Принцип переформулирован (research.md §1): удалённая интеграция снята со списка санкционированных. |
| VI. Local-first | PASS | Файлы настроек остаются читаемыми; лишние ключи старых версий не ломают разбор. |
| VII. Офлайн | N/A | Сети нет. |
| VIII. Движки | PASS | Ссылка на удалённый мост как образец паттерна убирается; паттерн не меняется. |
| IX. Локальность | N/A | — |
| X. Массовые операции | N/A | — |

## Project Structure

```text
packages/core/src/settings/{editor_mode.rs (удаляется), mod.rs, apply.rs, format.rs,
  events.rs, logging.rs, diagnostics.rs, web.rs, native.rs, store.rs}
packages/core/src/vault/vault_settings.rs
packages/core/src/templates/mod.rs              — комментарий
packages/core/tests/settings_{native,wasm}.rs
packages/core/Cargo.toml                        — фича test-support
packages/shell/src/{lib.rs, sync.rs}
ui/src/settings/{KeybindingsSection,AdvancedSection,SettingsPanel,useSettings,
  useVaultSettings,useVaultList,applyWithNoteSave}.ts(x)
ui/src/workspace/useShortcuts.ts, ui/src/editor/NoteEditor.tsx, ui/src/styles/app.css
ui/src/templates/insertion/*, ui/src/note-panel/links/openNoteEditors.ts
ui/src/i18n/locales/{ru,en}/{editor,settings}.ts
nix/shells.nix, README.md, CLAUDE.md, .claude/agents/*, .claude/skills/flow-typsidian,
.specify/memory/constitution.md, speckit-prompts-revised.md, specs/001…025
```

## Complexity Tracking

Нет отклонений.
