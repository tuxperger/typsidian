# Implementation Plan: Скачиваемые шаблоны не синхронизируются

**Branch**: `041-registry-templates-unsynced` | **Date**: 2026-10-26 | **Spec**: [spec.md](spec.md)

## Summary

Скачанные шаблоны Typst переезжают в собственную папку `.typsidian/downloads/` (раскладка
006, перечень `installed.json` там же); ручные остаются в `.typsidian/packages/`. Поиск
шаблона при сборке проверяет обе папки, ручная важнее. Блок правил приложения в `.gitignore`
и `.stignore` получает `/.typsidian/downloads`, `/.typsidian/packages/preview` и
`/.typsidian/packages/installed.json`. Существующие vault получают предложение дописать
правила (011 §FR-027) без отдельной работы.

Макет — не требуется: интерфейса нет; меняется только текст подсказки 007 о папке для ручных шаблонов.

## Technical Context

**Language/Version**: Rust 2021 (`packages/core`), TypeScript strict (тексты `ui/`)
**Primary Dependencies**: новых нет
**Storage**: `.typsidian/downloads/<ns>/<name>/<version>/`, `.typsidian/downloads/installed.json`
**Testing**: `cargo test` + `wasm-bindgen-test` — `package_cases` (установка в downloads,
поиск в обеих папках, ручная важнее, прежние установки), модульные тесты `ignore` и `service_dir`
**Target Platform**: все оболочки (общий `VaultBackend`)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Раскладка, поиск и правила — в ядре. |
| II. Паритет | PASS | Общие случаи на обоих путях. |
| III. Спека | PASS | Без путей и API. |
| IV. TS strict | PASS | Меняются только тексты. |
| V. Внешние процессы | PASS | Не затрагиваются; `.stignore` — только блок приложения с подтверждением. |
| VI. Local-first | PASS | Ручные шаблоны в vault; vault, собираемый обычным `typst`, по-прежнему находит ручные шаблоны. |
| VII. Офлайн | PASS | Без сети недостающий шаблон даёт названную причину (007); положенный вручную работает. |
| IX. Локальность | PASS | В сеть уходит только имя шаблона, как в 007. |
| X, VIII, XI | PASS | Не затрагиваются. |

## Project Structure

```text
packages/core/src/vault/service_dir.rs   — DOWNLOADS_DIR_NAME, downloads_dir, derived_paths
packages/core/src/preview/resolve.rs     — download_root, installed_root, поиск в обеих папках
packages/core/src/preview/packages.rs    — install/отпечаток/перечень в downloads
packages/core/src/sync/ignore.rs         — правила
packages/core/tests/common/package_cases.rs (+ native/wasm)
ui/src/i18n/locales/{ru,en}/app.ts       — подсказка о ручной папке
```

## Complexity Tracking

Нарушений нет.
