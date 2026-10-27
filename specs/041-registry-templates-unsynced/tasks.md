# Tasks: Скачиваемые шаблоны не синхронизируются

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md)

## Phase 1: User Story 1 — скачанное не едет на другие устройства (P1)

- [X] T001 [US1] `DOWNLOADS_DIR_NAME`, `downloads_dir()` в `packages/core/src/vault/service_dir.rs`; `/.typsidian/downloads`, `/.typsidian/packages/preview`, `/.typsidian/packages/installed.json` в правилах (`derived_paths` или `packages/core/src/sync/ignore.rs`) (FR-002, FR-003)
- [X] T002 [US1] `download_root`, `installed_root` в `packages/core/src/preview/resolve.rs`; `read_dependency` ищет в обеих папках, ручная важнее (FR-004, FR-005)
- [X] T003 [US1] `install`, отпечаток, сверка, перечень установленного в `packages/core/src/preview/packages.rs` — в `downloads`, чтение — там, где найден шаблон (FR-001)
- [X] T004 [US1] Случаи в `packages/core/tests/common/package_cases.rs` и обёртки native/wasm: установка в downloads; поиск в обеих папках; ручная важнее; прежняя установка в packages собирается; модульные тесты правил
- [X] T005 [P] [US1] Подсказка о ручной папке в `ui/src/i18n/locales/{ru,en}/app.ts` (FR-006) — проверено: подсказка уже называет `.typsidian/packages/`, это и есть папка ручных шаблонов; правка не нужна

## Phase 2: Polish

- [X] T006 Гейт `all`; README (раздел синхронизации/шаблонов); `**Status**: Implemented`
