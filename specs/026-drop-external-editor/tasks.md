# Tasks: Отказ от внешнего модального редактора

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md)

## Phase 1: Foundational — конституция

- [X] T001 Поправка конституции через `/speckit-constitution`: принцип V «Внешние процессы — зона повышенного риска», список санкционированных интеграций без удалённого моста, шаг 3 рабочего процесса и принцип VIII без ссылки на мост, версия 2.0.0 (research.md §1) в `.specify/memory/constitution.md`

## Phase 2: User Story 2 — старые файлы настроек (P1)

- [X] T002 [US2] Тесты: файл настроек устройства и `vault.json` с `editor_mode_preference`/`editor_config_path` разбираются, прочие значения действуют, после записи ключей нет — в `packages/core/src/settings/format.rs` и `packages/core/src/vault/vault_settings.rs` (FR-004, FR-005)
- [X] T003 [US2] Удалить поля режима и пути к конфигу из `Settings`, `SettingsChange`, настроек vault и их форматов; `RETIRED_KEYS` удаляются при записи — `packages/core/src/settings/{mod,apply,format}.rs`, `packages/core/src/vault/vault_settings.rs`

## Phase 3: User Story 1 — интерфейс без режима (P1)

- [X] T004 [US1] Удалить `packages/core/src/settings/editor_mode.rs`, события и записи журнала режима, поля диагностики, `ApplyContext`, `ApplyPlan::RequiresNoteSave`, `RejectReason::{SaveFailed, UnresolvedConflict}` — `packages/core/src/settings/{mod,events,logging,diagnostics,apply,store,web,native}.rs`, комментарий `packages/core/Cargo.toml` (research.md §2)
- [X] T005 [US1] Переписать тесты настроек на обоих путях без режима и предусловий — `packages/core/tests/settings_native.rs`, `packages/core/tests/settings_wasm.rs`
- [X] T006 [US1] Оболочка: убрать заглушку моста и `ApplyContext` — `packages/shell/src/lib.rs`, `packages/shell/src/sync.rs`
- [X] T007 [US1] UI: блок режима в `ui/src/settings/KeybindingsSection.tsx`, строки режима в `ui/src/settings/AdvancedSection.tsx`, `applyWithNoteSave.ts` и ветки причин в `SettingsPanel.tsx`, `useSettings.ts`, `useVaultSettings.ts`, `useVaultList.ts`, `ui/src/workspace/useShortcuts.ts`; надпись `ts-vim` в `ui/src/editor/NoteEditor.tsx` и её стиль в `ui/src/styles/app.css`; ключи переводов в `ui/src/i18n/locales/{ru,en}/{editor,settings}.ts` (FR-001…FR-003)
- [X] T008 [US1] Порт вставки заготовок: реализация «ввод без раскладки» → «редактор» без изменения поведения — `ui/src/templates/insertion/*`, комментарий `ui/src/note-panel/links/openNoteEditors.ts`, `packages/core/src/templates/mod.rs` (FR-006)

## Phase 4: User Story 3 — документация и среда (P2)

- [X] T009 [P] [US3] Среда и правила: пакет из `nix/shells.nix`; агент моста к внешнему редактору в `.claude/agents/` удалить; `.claude/agents/{security-review-agent,speckit-agent}.md`, `.claude/skills/flow-typsidian/SKILL.md`, `CLAUDE.md`, `speckit-prompts-revised.md`, `README.md` (FR-007)
- [X] T010 [US3] Артефакты фич 001–025: Constitution Check и абзацы о мосте по research.md §5; шапка спеки 004 — пометка об отмене режима фичей 026 (FR-007)

## Phase 5: Polish

- [X] T011 Гейты `gates.sh all`, дымовой сценарий настроек в браузере, поиск по репозиторию без совпадений (SC-001), `**Status**: Implemented`
