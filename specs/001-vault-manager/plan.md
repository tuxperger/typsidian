# Implementation Plan: Vault Manager

**Branch**: `001-vault-manager` | **Date**: 2026-08-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-vault-manager/spec.md`

## Summary

Пользователь открывает папку на диске как vault; ядро рекурсивно индексирует заметки,
показывает их в виде дерева, отслеживает внешние изменения без перезапуска, позволяет
create/rename/delete из интерфейса, предупреждает о конфликтах редактирования и о
битых ссылках перед удалением/переименованием. Вся эта логика реализуется один раз в
`packages/core` за единым трейтом `VaultBackend` с двумя реализациями —
`NativeBackend` (нативная файловая система + `notify`) и `WebBackend` (File System
Access API + периодическая сверка) — так что desktop, mobile и web получают
идентичное поведение при разных механизмах доступа к диску и обнаружения изменений.
Персистентный кэш (SQLite/`sqlx` нативно, `wa-sqlite` в вебе) избавляет от полного
пересканирования при повторном открытии; конфликты разрешаются диалогом с выбором
пользователя, а не автоматически.

## Technical Context

**Language/Version**: Rust (stable toolchain, зафиксирован через `flake.nix`/fenix) —
вся бизнес-логика этой фичи. TypeScript `strict` — только UI-слой, потребляющий
контракт из `contracts/frontend-events.md`.

**Primary Dependencies**:
- `notify` (+ дебаунсинг, research.md §2) — нативный FS-watcher, только
  `NativeBackend`.
- File System Access API через `wasm-bindgen`/`web-sys` — только `WebBackend`.
- `sqlx` (драйвер `sqlite`) — персистентный кэш на native.
- `wa-sqlite` (SQLite, скомпилированный в WASM, поверх OPFS) — персистентный кэш на
  web.
- `wasm-bindgen` + генератор типов (`tsify` или эквивалент) — граница ядро↔UI
  (принцип IV конституции).
- Хэш содержимого для сравнения/diff (например, BLAKE3) — платформонезависим,
  доступен на обоих таргетах без cfg-разделения.

**Storage**: Заметки — обычные файлы в выбранной пользователем папке (FR-033,
принцип VI конституции). Персистентный кэш — SQLite, производное и полностью
восстановимое состояние (research.md §6); не хранит содержимое заметок, только
метаданные и граф обратных ссылок.

**Testing**: `cargo test` (native) и `wasm-bindgen-test` (`wasm32-unknown-unknown`) —
обе обязательны для каждой единицы логики ядра (принцип I, принцип II NON-NEGOTIABLE).

**Target Platform**: `wasm32-unknown-unknown` (web-клиент, Next.js) и нативная сборка
(desktop/mobile через Tauri).

**Project Type**: Rust-workspace ядро (`packages/core`), потребляемое тремя UI-клиентами
с общим UI-кодом (web/desktop/mobile) — не отдельные проекты на клиент.

**Performance Goals** (из spec.md Success Criteria):
- Дерево пригодно для навигации ≤3с, полный обход ≤10с — vault до 10 000 заметок
  (SC-002; это же — заявленный верхний предел гарантий).
- Повторное открытие ранее открытого vault ≤1с (SC-003).
- Внешнее изменение отражается в приложении ≤2с (SC-004).
- Массовое внешнее изменение 1000 файлов улаживается ≤15с без потери отзывчивости
  интерфейса (SC-005).

**Constraints**:
- Полностью офлайн: ни одна операция фичи не зависит от сети (FR-032, принцип VII
  конституции).
- Обязателен паритет native/`wasm32-unknown-unknown` для каждой единицы логики ядра
  (принцип II, NON-NEGOTIABLE); платформенные различия — только за трейтом.
- UI не дублирует бизнес-логику (принцип I) — только вызывает `VaultBackend` и
  реагирует на `VaultEvent`.
- Символические ссылки не раскрываются при обходе (spec.md Assumptions).
- Скрытые файлы/папки и служебные каталоги систем контроля версий исключены из vault
  (FR-004).
- Персистентный кэш — производное состояние, не авторитетный источник (принцип VI,
  архитектурные ограничения конституции).

**Scale/Scope**: До 10 000 заметок включительно — жёсткий верхний предел, а не порог
деградации: vault большего размера система не открывает вовсе, прерывая обход и называя
причину (FR-038, кларификация 2026-08-09). Режима best-effort выше предела не
существует. Отдельная заметка — до 10 МБ включительно; выше порога заметка остаётся
видимой в дереве, но открытие отклоняется с сообщением о причине (spec Assumptions).
Массовые внешние изменения — до 1000 файлов под гарантией SC-005.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Принцип | Статус | Обоснование |
|---|---|---|
| I. Бизнес-логика живёт в ядре | PASS | Вся vault-логика — в `packages/core` за трейтом `VaultBackend`; UI только вызывает трейт и рендерит `VaultEvent` (contracts/frontend-events.md). Каждая операция трейта покрывается `cargo test` + `wasm-bindgen-test`. |
| II. Паритет двух таргетов (NON-NEGOTIABLE) | PASS | Единый трейт, `NativeBackend`/`WebBackend` — единственное место условной компиляции (research.md §1). Зависимости, недоступные на одном из таргетов (`notify`, `sqlx` ↔ FS Access API, `wa-sqlite`), заключены строго внутри соответствующей реализации через `cfg`, не просачиваются в публичный API. |
| III. Спецификация описывает только поведение | N/A для плана | Проверено на этапе `/speckit-specify`/`/speckit-clarify`; `spec.md` не содержит технических решений — все они здесь. |
| IV. TypeScript strict, без any | PASS | Все типы границы ядро↔UI порождаются из Rust (`tsify`/эквивалент), не дублируются вручную (research.md §9, data-model.md «Внешнее изменение»). |
| V. Мост к Neovim — зона повышенного риска | N/A | Фича не затрагивает msgpack-rpc мост; `/speckit-clarify` для этого гейта не требовался. |
| VI. Данные принадлежат пользователю (local-first) | PASS | Заметки — обычные файлы (FR-033). Персистентный SQLite-кэш — производный и восстановимый, не авторитетный источник (research.md §6); при повреждении кэша — откат на полный обход без потери пользовательских данных. |
| VII. Корректная деградация в офлайне | PASS | Фича не имеет сетевых зависимостей вообще (FR-032) — офлайн является нормальным, а не деградированным режимом. |

Нарушений, требующих `Complexity Tracking`, нет.

## Project Structure

### Documentation (this feature)

```text
specs/001-vault-manager/
├── plan.md              # этот файл
├── research.md          # Phase 0 — решения и обоснования
├── data-model.md         # Phase 1 — сущности
├── quickstart.md         # Phase 1 — сценарии проверки
├── contracts/
│   ├── vault-backend-trait.md   # внутренний контракт ядра
│   └── frontend-events.md       # контракт ядро → UI
└── tasks.md              # Phase 2 (создаётся /speckit-tasks, не этой командой)
```

### Source Code (repository root)

```text
packages/
└── core/                        # единственное место бизнес-логики (принцип I)
    ├── Cargo.toml                # workspace member; таргеты native + wasm32-unknown-unknown
    └── src/
        ├── lib.rs
        ├── vault/                # эта фича
        │   ├── mod.rs
        │   ├── backend.rs         # трейт VaultBackend (contracts/vault-backend-trait.md)
        │   ├── native.rs          # NativeBackend: std::fs/tokio::fs + notify, cfg(not(wasm32))
        │   ├── web.rs              # WebBackend: File System Access API, cfg(wasm32)
        │   ├── index.rs            # in-memory индекс путей/метаданных
        │   ├── cache.rs            # персистентный кэш: sqlx (native) / wa-sqlite (web)
        │   ├── conflict.rs         # обнаружение конфликтов (research.md §5)
        │   └── events.rs           # VaultEvent и разметка для WASM/JS границы
        └── graph/                  # смежный модуль (парсинг ссылок, построение графа);
                                     # vault-manager — потребитель через backlinks()

packages/shell/                    # команды-обёртки над NativeBackend, общие для
                                   # обеих нативных оболочек; отдаёт builder(),
                                   # без Context и плагинов (см. решение ниже)

apps/
├── web/                          # Next.js — тонкая обёртка над ui/workspace
├── desktop/                      # Tauri desktop shell (T112-T115)
│   ├── src/openDesktopVault.ts    # жест: нативный диалог выбора папки
│   └── src-tauri/                 # точка входа, свой tauri.conf.json, плагин диалога
└── mobile/                       # Tauri Android shell
    ├── src/openMobileVault.ts     # жест: путь из обзора папок ниже
    ├── src/FolderPicker.tsx       # обзор папок устройства вместо диалога ОС
    └── src-tauri/                 # точка входа, свой tauri.conf.json, gen/android

ui/                                # общий UI-код (React): дерево, диалог конфликта, diff-view
└── src/
    ├── vault-client.ts             # поверхность ядра для UI (одна на все таргеты)
    ├── vault-client-web.ts         # её реализация на WASM (только web)
    ├── vault-client-tauri.ts       # её реализация поверх команд Tauri (desktop + mobile)
    └── workspace/Workspace.tsx     # рабочая область, общая для всех трёх клиентов

# Тесты ядра — рядом с реализацией, по принципу II:
packages/core/tests/
├── vault_native.rs                # cargo test — только native
└── vault_wasm.rs                  # wasm-bindgen-test — только wasm32-unknown-unknown
```

**Structure Decision**: Rust-workspace с единственным ядром (`packages/core`) и
тонкими UI-оболочками поверх общего UI-кода — не «отдельный проект на клиент» и не
классическое разделение backend/frontend, поскольку вся логика этой фичи целиком
локальна и не имеет серверной части. `vault/` — самостоятельный подмодуль `core`,
отделённый от `graph/` по границе, зафиксированной в research.md §8.

**Про `apps/mobile` (решение T116, 2026-08-10; пересмотрено 2026-09-15)**: мобильная
оболочка была отложена и из структуры выше убрана. Причина была не в архитектуре, а в
проверяемости: Tauri mobile требует Android SDK/NDK либо Xcode, которых нет в `flake.nix`,
а добавлять в воспроизводимое окружение тулчейн, который здесь нельзя ни собрать, ни
запустить, значит поставить в план обещание, за которым нет прогона.

**2026-09-15 отсрочка снята для Android.** Обоснование снято ровно в той части, в которой
было верным: `androidenv` собирает SDK и NDK декларативно, из того же `flake.lock`, и
прогон есть — `nix develop .#android` плюс `pnpm --filter @typsidian/mobile build` дают
APK. Тулчейн лежит в **отдельном** devShell: SDK+NDK весят ~3 ГБ, и платить их каждому
`nix develop` ради web/desktop незачем. iOS остаётся отложенным по прежней причине: Xcode
в Nix не воспроизводится, и на Linux его не запустить.

Предсказание об абстракции подтвердилось: мобильная оболочка — это третья реализация
`ui/src/vault-client.ts` плюс свой жест выбора папки, и ничего переписывать не пришлось.
Что при этом переехало:

- общая реализация `VaultClient` поверх команд Tauri — из `apps/desktop/src/tauriVaultClient.ts`
  в `ui/src/vault-client-tauri.ts`, рядом с `vault-client-web.ts`. Она одна на обе
  нативные оболочки; в оболочках остались только жесты открытия
  (`openDesktopVault.ts`, `openMobileVault.ts`);
- команды Tauri — из `apps/desktop/src-tauri/src/lib.rs` в `packages/shell`. Крейт отдаёт
  `builder()`, а `Context`, плагины и точку входа добавляет оболочка: `generate_context!`
  читает `tauri.conf.json` своего крейта, а права плагина `tauri-build` ищет в
  зависимостях того крейта, где макрос раскрыт.

**Чего у мобильной оболочки нет и почему.** Системного диалога выбора папки: `pick_folder` у
`tauri-plugin-dialog` реализован только под desktop, а системный SAF отдаёт
`content://`-адреса, которых `NativeBackend` не понимает — он построен на `std::fs`. Поэтому
обзор папок рисует само окно (`FolderPicker.tsx`) поверх команд `browse_root`/`browse_dir`/
`browse_create_dir` из `packages/shell`: они читают обычную файловую систему устройства и
vault не касаются. Начинается обзор с корня общей памяти, а не с приватного каталога
приложения: vault обязан переживать удаление приложения и быть виден синхронизации
(принцип VI). Доступ к общей памяти даёт `MANAGE_EXTERNAL_STORAGE`, который пользователь
выдаёт вручную; без него обзор честно отвечает, что доступа нет.

**2026-09-23 выбор папки на Android появился.** До этой правки его не было вовсе: команда
`default_vault_dir` возвращала один фиксированный путь `Documents/Typsidian`, и папку с
заметками, лежащую где-то ещё (карта памяти, каталог синхронизации), открыть было нельзя.
Команды не стало вместе с появлением обзора — первый запуск на чистом устройстве закрывает
«создать папку», ради которого она заводила каталог сама.

Корзины ОС на Android тоже нет — `native::move_to_trash` там возвращает `false`, и удаление
сразу безвозвратное. Это не заглушка: тот же путь срабатывает на рабочем столе, когда
корзина недоступна.

В `specs/002-drawing-boards/tasks.md` мобильный таргет больше не блокирует приёмку
T067-T068: сборка, на которой их можно прогнать, существует.

## Post-Design Constitution Check

*Повторная проверка после Phase 1 (data-model.md, contracts/, quickstart.md).*

| Принцип | Статус | Примечание после дизайна |
|---|---|---|
| I | PASS | `backlinks()`-оркестрация подтверждения удаления/переименования явно закреплена за UI как вызов существующего примитива ядра (contracts/vault-backend-trait.md), а не новая логика — граница не размылась. |
| II | PASS | `data-model.md` не вводит типов, которые существуют только на одном таргете; `VaultEvent` — общий для обоих backend. |
| IV | PASS | `contracts/frontend-events.md` фиксирует исчерпывающую обработку варианта события в TS `strict` (`never` в `default`) — не оставляет места для `any`. |
| VI | PASS | `data-model.md` явно помечает `EditConflict`/`local_buffer` как in-memory-only, не персистентный — согласовано с FR-035 и с тем, что кэш не хранит содержимое заметок. |
| VII | PASS | Дизайн не добавил сетевых зависимостей. |

Gate пройден повторно без новых нарушений.

## Complexity Tracking

> Пусто — нарушений Constitution Check, требующих обоснования, нет.
