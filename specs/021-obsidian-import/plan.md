# Implementation Plan: Импорт из Obsidian

**Branch**: `021-obsidian-import` | **Date**: 2026-10-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/021-obsidian-import/spec.md`

## Summary

Папка Markdown переносится в vault: заметки Typst с тем же деревом, ссылки vault, картинки
во вложениях, формулы Typst; всё непереводимое остаётся видимым исходным текстом и попадает в
отчёт; импорт идёт пакетно с прогрессом и отменой и продолжается без дублей.

Технически: новый модуль ядра `packages/core/src/import/` — перевод одного файла Markdown в
Typst (`pulldown-cmark` с расширениями Obsidian поверх него, research.md §1–§4), перевод
формул LaTeX → Typst (`mitex`, research.md §5), план имён и разрешение вики-ссылок по
правилу Obsidian (research.md §6), пакетная операция по образцу пакетного экспорта 009
(`BatchState` держит окно, шаг — одна заметка, запись атомарная, research.md §7) и журнал
импорта в служебной папке для продолжения (research.md §8). Исходная папка открывается
вторым `VaultBackend` в режиме только чтения (research.md §9). Разбор ссылок 001 учится
форме `[[путь|текст]]` (Clarifications 2026-10-15, research.md §10).

Макет — временный (не Claude Design), https://claude.ai/code/artifact/86095c4a-833f-4b13-bd79-90497e7672fd, экраны C1–C5.

## Technical Context

**Language/Version**: Rust 2021, TypeScript strict + React 19

**Primary Dependencies**: новые крейты ядра — `pulldown-cmark` (CommonMark + таблицы, сноски, флажки, математика; чистый Rust, wasm32) и `mitex` (LaTeX → Typst; чистый Rust, собирается в wasm32). Нативных библиотек нет — `nix-env-agent` не нужен, сверить сборку wasm32 первой задачей

**Storage**: заметки, вложения и отчёт — файлы vault; журнал импорта — `.typsidian/imports/<id>.json`

**Testing**: `cargo test` + `wasm-bindgen-test` (`tests/common/import_cases.rs` с набором образцовых файлов Obsidian в `tests/fixtures/obsidian/`), каждая переведённая заметка собирается `compile_note` (SC-001); `pnpm typecheck`; сценарий `scripts/dev/scenarios/import.mjs`

**Target Platform**: web (wasm32, Chromium — выбор папки), desktop Linux, Android

**Performance Goals**: SC-004 — 2 000 заметок ≤2 мин, отмена ≤2 с

**Constraints**: исходная папка только читается (FR-002, SC-005); без сети; ни одного частично записанного файла

**Scale/Scope**: до 10 000 файлов, файл Markdown до 5 МБ

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Перевод, имена, ссылки, отчёт, журнал, пакет — в `import/`. UI — диалог, карточка хода. |
| II. Паритет | PASS | Новые крейты — чистый Rust; первая задача сверяет wasm32. Источник — `VaultBackend` на таргет. Биндинги 1:1. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`. |
| V. Внешние процессы | N/A | — |
| VI. Local-first | PASS | Результат — обычные файлы Typst. |
| VII. Офлайн | N/A | Сети нет (FR-013). |
| VIII. Внешние движки | N/A | — |
| IX. Локальность | PASS | Ничего не покидает устройство. |
| X. Массовые операции | PASS | Отмена между заметками, шаг — одна заметка с атомарной записью; прогресс; UI-поток не блокируется (шаги по одному, как 009). |

## Project Structure

```text
packages/core/
├── src/import/
│   ├── mod.rs          # ImportJob, ImportReport, реэкспорт
│   ├── markdown.rs     # Markdown → Typst одного файла, список потерь с местами
│   ├── obsidian.rs     # вики-ссылки, вставки ![[…]], выделение ==…==, комментарии %%…%%, свойства YAML
│   ├── math.rs         # формулы через mitex, запасной путь — исходный текст
│   ├── names.rs        # имена файлов, недопустимые знаки, разрешение ссылок по правилу Obsidian
│   ├── batch.rs        # ImportState, шаг, отмена, журнал
│   ├── report.rs       # заметка отчёта
│   └── web.rs
├── src/graph/mod.rs    # + [[путь|текст]] (research.md §10)
├── src/links/          # + перенос и «Связать» сохраняют текст ссылки
├── Cargo.toml          # + pulldown-cmark, mitex
└── tests/common/import_cases.rs · tests/import_*.rs · tests/fixtures/obsidian/

packages/shell/src/lib.rs   # import_* и открытие источника только для чтения

ui/src/
├── import/ImportDialog.tsx · ImportProgress.tsx · useImport.ts     # C1–C5
├── vault-client*.ts                                                # + import.*
├── workspace/Workspace.tsx                                         # команда палитры
└── i18n/locales/{ru,en}/import.ts
apps/web · apps/desktop · apps/mobile   # выбор исходной папки — тем же жестом, что openVaultFolder
```

## Complexity Tracking

| Решение | Почему так | Условие пересмотра |
|---|---|---|
| Два новых крейта | Свой парсер CommonMark и LaTeX — недели работы и источник расхождений | `mitex` не собирается под wasm32 или тянет нативное → свой перевод подмножества (дроби, индексы, греческие, `\text`, матрицы), остальное — исходным текстом |
| Шаги импорта на web — в главном потоке, по одному файлу между кадрами | Как пакетный экспорт 009; типовая заметка переводится за единицы миллисекунд | Замер T023: шаг > 50 мс на файле 1 МБ в wasm → шаги в отдельном воркере со вторым экземпляром ядра; форма вызова `VaultClient.import` не меняется |
| Расширение формы ссылок 001 в чужой фиче | Clarifications 2026-10-15; без этого FR-005 не выполним | — |
