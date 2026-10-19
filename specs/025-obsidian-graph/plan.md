# Implementation Plan: Граф связей в духе Obsidian

**Branch**: `025-obsidian-graph` | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/025-obsidian-graph/spec.md`

## Summary

Граф становится живым: силовая раскладка, перетаскивание узлов, подсветка соседей,
подписи по масштабу, размер по числу связей, локальный граф глубины 1–3 на экране графа и
в колонке заметки, панель с фильтром, показом заметок без связей / вложений / тегов,
группами по цвету, стрелками и силами раскладки; всё плавно на 5 000 заметок.

Технически граф **строит ядро** (принцип I прямо называет «построение графа»): новый
`packages/core/src/graph/view.rs` одним вызовом отдаёт узлы и связи vault из индекса
ссылок 001 (`IndexCache`), тегов 020 и вставок файлов, уже применив фильтр, показ видов,
группы и локальную выборку по глубине (research.md §1–§4). Настройки графа — ключ `graph`
в `.typsidian/vault.json` (006, research.md §5). Интерфейс только **рисует**: силовая
раскладка — `d3-force` в Web Worker (Barnes–Hut, research.md §6), отрисовка — Canvas 2D
с пакетной заливкой (research.md §7); положения узлов переживают обновление графа по
идентификатору узла (research.md §8). Существующий `ui/src/graph/GraphView.tsx` с его N
вызовами `backlinks` заменяется (закрывает записанный в нём долг).

Макет — временный (не Claude Design), https://claude.ai/code/artifact/e51fcf9f-d746-43aa-89d2-5ea3f02b9895, экраны G1–G5.

## Technical Context

**Language/Version**: Rust 2021 (ядро, shell), TypeScript strict + React 19 (ui)

**Primary Dependencies**: новая JS-зависимость `d3-force` 3.x (+ `@types/d3-force`) в `ui/` —
чистый JS без нативных частей; новая запись в `pnpm-lock.yaml` меняет хэш зависимостей
nix-сборки — сверить с `nix-env-agent` первой задачей. В ядре новых крейтов нет

**Storage**: настройки графа — `graph` в `.typsidian/vault.json`; положение вида и глубина
локального графа — состояние окна; граф — производное от индекса (принцип VI)

**Testing**: `cargo test` + `wasm-bindgen-test` (`tests/common/graph_cases.rs`), бенч
`packages/core/benches/graph_view.rs` (5 000 заметок, 15 000 связей), `pnpm typecheck`,
сценарий `scripts/dev/scenarios/graph.mjs` с замером кадров

**Target Platform**: web, desktop Linux, Android

**Performance Goals**: SC-001 ≤2 с до движения на 5 000 / 15 000; SC-002 ≥30 кадров/с,
покой ≤10 с; SC-003 подсветка ≤50 мс; SC-004 обновление ≤2 с. Бюджеты — research.md §9

**Constraints**: раскладка не в главном потоке; ядро не зовётся на каждый кадр; без сети

**Scale/Scope**: до 10 000 узлов (5 000 — с запасом по кадрам)

## Constitution Check

| Принцип | Статус | Как соблюдается |
|---|---|---|
| I. Логика в ядре | PASS | Какие узлы и связи есть, виды узлов, вложения, теги, фильтр по языку условий 020, группы и их порядок, локальная выборка по глубине, число связей — `graph/view.rs`. Силовая раскладка и отрисовка — представление (как вёрстка строк редактором), в UI; правил отбора UI не заводит. |
| II. Паритет | PASS | `vault_graph` — одна функция над `VaultBackend`/`IndexCache`, wasm и Tauri 1:1; раскладка и отрисовка — общий UI во всех оболочках. |
| III. Спека | PASS | — |
| IV. TS strict | PASS | Типы — `Tsify`; `d3-force` имеет типы. |
| V. Мост | N/A | — |
| VI. Local-first | PASS | Граф — производное; настройки — обычный JSON в vault. |
| VII. Офлайн | N/A | Сети нет. |
| VIII. Движки | N/A | `d3-force` — библиотека вычислений, а не встраиваемый движок с DOM. |
| IX. Локальность | PASS | Всё на устройстве. |
| X. Массовые операции | N/A | Граф строится из уже готового индекса одним проходом (≤100 мс на 5 000, research.md §9), без обхода файлов. |

## Project Structure

```text
packages/core/
├── src/graph/mod.rs              # + pub mod view; link_sites/резолв — как есть
├── src/graph/view.rs             # GraphRequest, GraphSettings, VaultGraph, vault_graph(), local
├── src/graph/web.rs              # wasm: vaultGraph
├── src/vault/vault_settings.rs   # + ключ graph (GraphSettings)
├── src/settings/mod.rs           # + NotePanelMode::Graph
├── benches/graph_view.rs
└── tests/common/graph_cases.rs · tests/graph_native.rs · tests/graph_wasm.rs

packages/shell/src/lib.rs         # vault_graph

ui/
├── package.json                  # + d3-force, @types/d3-force
└── src/graph/
    ├── GraphView.tsx             # экран графа (G1, G2, G4): верхняя полоса, холст, панель
    ├── GraphCanvas.tsx           # Canvas 2D, указатель, жесты, подсветка, подписи
    ├── GraphPanel.tsx            # настройки (G1, G5 — шторка)
    ├── layout.worker.ts          # d3-force: тики, перетаскивание, силы
    ├── useGraphLayout.ts         # мост к воркеру, перенос положений по id
    ├── useVaultGraph.ts          # запрос vault_graph, обновление по событиям vault
    ├── LocalGraphMode.tsx        # режим «Граф» колонки заметки (G3)
    └── GraphA11yList.tsx         # список узлов для клавиатуры и чтения с экрана
ui/src/vault-client*.ts           # + graph()
ui/src/i18n/locales/{ru,en}/graph.ts
ui/src/styles/app.css             # .ts-graph*
scripts/dev/scenarios/graph.mjs
```

## Complexity Tracking

| Решение | Почему так | Условие пересмотра |
|---|---|---|
| Раскладка в UI (воркер), а не в ядре | 30 кадров/с на desktop потребовали бы гонять 5 000 положений через IPC Tauri на каждый кадр; раскладка — представление, не правило отбора | Нужна одинаковая раскладка на всех устройствах (сохранённые положения) → перенести в ядро, wasm-воркер на web |
| Новая зависимость `d3-force` | Barnes–Hut своими силами — отдельная фича с собственными ошибками. С T028 отталкивание — своя сила `forceRepel` (тик 5 000 узлов 29 → 12 мс, research.md §14), симуляция, связи и центр — по-прежнему d3 | `pnpm-lock` ломает nix-сборку и `nix-env-agent` не может поправить хэш → свой Barnes–Hut (~300 строк) |
| Canvas 2D, не WebGL | Без новой зависимости; пакетная отрисовка путей держит 15 000 линий | Замер T-кадров < 30 на 5 000 узлах → WebGL-рендер узлов и линий |
