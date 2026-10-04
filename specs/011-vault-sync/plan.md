# Implementation Plan: Синхронизация vault — история версий и доставка между устройствами

**Branch**: `011-vault-sync` | **Date**: 2026-10-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/011-vault-sync/spec.md`

## Summary

История версий (git) в папке vault, обмен с удалённым репозиторием и совместная работа с
Syncthing по модели «хранитель истории». Git — внешний движок за трейтом ядра `GitEngine`
(native — libgit2 со своими транспортами поверх `reqwest`/системного `ssh`; web —
isomorphic-git поверх папки пользователя). Вся политика — правила исключения, `.stignore`,
хранитель, автоснимки, трёхстороннее слияние, откат, разбор конфликтных копий, чтение
состояния Syncthing — в новом модуле ядра `sync`. UI — панель, индикатор, мастер,
редактор слияния. Подробности — [research.md](research.md).

## Technical Context

**Language/Version**: Rust 2021 (ядро, оболочка Tauri), TypeScript strict (UI)

**Primary Dependencies**: новые — `git2` (`vendored-libgit2`, без `https`/`ssh`; только
native), `similar` (diff3, оба таргета), `keyring` 3 (desktop: `sync-secret-service`,
`crypto-rust`, `apple-native`, `windows-native`), `quick-xml` (native, конфиг Syncthing),
`getrandom` (идентификатор устройства; `js` под wasm32), `isomorphic-git` (npm, только web).
Уже есть: `reqwest` + `rustls`, `tokio`, `notify`, `serde_json`, `tsify-next`.

**Storage**: `.git` в папке vault (хранитель/без Syncthing); `.typsidian/vault.json` →
`sync`; настройки устройства → `device`, `vault_sync[vault_id]`; секреты — `SecretStore`;
зеркало не-хранителя — каталог данных приложения / OPFS.

**Testing**: `cargo test` (модули `sync::*` + интеграционные `tests/sync_*.rs` на временных
репозиториях и `file://`/loopback удалённом), `wasm-bindgen-test` (чистые модули политики
+ `WebSyncSession` с JS-заглушкой движка), бенчмарк `sync_status`, `tsc --noEmit`,
`scripts/check-i18n.sh`.

**Target Platform**: web (wasm32, Chromium с File System Access), desktop (Linux/macOS/
Windows, Tauri), Android (Tauri)

**Project Type**: local-first приложение: Rust-ядро + общий UI + три оболочки

**Performance Goals**: обзор и список изменений ≤ 2 с на 5 000 заметок (SC-002); ошибка сети
≤ 30 с (SC-004); UI не блокируется ни одной операцией обмена (принцип X)

**Constraints**: офлайн-первично (VII); секреты вне vault (FR-014); никакого своего сервера
(VI); Syncthing — только чтение + `.stignore` (FR-023); без системного OpenSSL (research §2)

**Scale/Scope**: до 5 000 заметок (001 `SUPPORTED_NOTE_COUNT`), история до десятков тысяч
снимков (постраничный `log`)

## Constitution Check

| Принцип | Как соблюдается |
| --- | --- |
| I. Логика в ядре | Политика целиком в `packages/core/src/sync`: правила исключения, хранитель, автоснимки (`autosnapshot::decide`), diff3, план `pull` и откат, разбор конфликтных копий, Syncthing-клиент. UI и JS-адаптер isomorphic-git — связующий слой без решений (research §1, §3, §12). |
| II. Паритет таргетов | Один `SyncSession<E: GitEngine>`; `cfg` только внутри `engine/native.rs` и `engine/web.rs`; чистые модули (`ignore`, `conflict_copy`, `merge`, `autosnapshot`, `keeper`) тестируются на обоих путях. `git2` — только под native, что допустимо: он за трейтом. |
| III. Спека — поведение | spec.md не называет библиотек; все выборы — здесь и в research.md. |
| IV. Strict TS | Типы `SyncOverview`, `Conflict`, `SyncError`… — из Rust через `tsify`; JS-адаптер движка типизирован теми же типами. |
| V. Мост к Neovim | Не затрагивается. Смена файла после `pull`/отката идёт тем же путём внешнего изменения (001), что и для любого редактора. |
| VI. Local-first | Синхронизация опциональна (FR-038); история — стандартный `.git` в папке; сервера и аккаунта нет; посредник в web — только указанный пользователем. |
| VII. Офлайн | Все локальные операции без сети (FR-008); тайм-ауты и отмена (research §9); офлайн-поведение покрыто тестами (loopback на закрытый порт, отмена посреди fetch). |
| VIII. Внешние движки | git — чёрный ящик за `GitEngine`; ядро владеет файлами (пишет через `VaultBackend`). Syncthing — внешний процесс, только чтение по его API; единственный файл, который пишем от его имени, — `.stignore`. |
| IX. Локальность индексов | Индексы не меняются по форме; содержимое заметок уходит только в удалённый репозиторий, явно указанный пользователем, — это и есть пользовательское действие, а не производное представление. |
| X. Массовые операции | Первый снимок, `pull`, `clone`, откат `pull` — с прогрессом и `CancelToken` (механизм 009); `pull` атомарен через журнал исходных версий (research §9). |

**Внешние процессы**: системный `ssh` (транспорт git на desktop) и read-only API Syncthing
санкционированы поправкой конституции 1.2.0 (2026-10-03, «Архитектурные ограничения →
Внешние процессы»); их рамки — явные аргументы `ssh`, только чтение Syncthing, ключ в
хранилище устройства, `.stignore` с подтверждения — соблюдены research §2, §5, §8.

## Project Structure

### Documentation (this feature)

```text
specs/011-vault-sync/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── core-sync-api.md
│   └── frontend-sync.md
└── tasks.md
```

### Source Code (repository root)

```text
packages/core/src/
├── sync/
│   ├── mod.rs              # SyncSession<E>, SyncOverview, SyncError, события
│   ├── engine/
│   │   ├── mod.rs          # трейт GitEngine, RepoProbe, Auth, RemoteSpec
│   │   ├── native.rs       # git2 + регистрация транспортов
│   │   ├── transport_http.rs   # smart HTTP поверх reqwest (native)
│   │   ├── transport_ssh.rs    # системный ssh (desktop)
│   │   └── web.rs          # JsGitEngine (wasm_bindgen extern)
│   ├── ignore.rs           # .gitignore/.stignore: блоки с маркерами, отчёты
│   ├── conflict_copy.rs    # разбор имён sync-conflict
│   ├── merge.rs            # diff3 поверх similar
│   ├── pull.rs             # план слияния, журнал, откат
│   ├── keeper.rs           # хранитель, спор
│   ├── autosnapshot.rs     # decide()
│   ├── syncthing.rs        # REST-клиент, discover config.xml (native)
│   ├── secrets.rs          # трейт SecretStore + реализации
│   ├── device.rs           # DeviceIdentity
│   └── web.rs              # WebSyncSession (#[wasm_bindgen])
├── vault/backend.rs        # is_index_excluded (общая), list_conflict_copies
├── vault/service_dir.rs    # derived_paths()
├── settings/               # device.id/name, vault_sync[vault_id]
└── vault/vault_settings.rs # ключ sync
packages/core/tests/sync_*.rs
packages/core/benches/sync_status.rs
packages/shell/src/lib.rs   # команды sync_*, таймер tick, SecretStore desktop/android
ui/src/sync/
├── sync-client.ts, sync-client-web.ts, sync-client-tauri.ts
├── git-engine-web.ts       # адаптер isomorphic-git (fs над FileSystemDirectoryHandle)
├── SyncIndicator.tsx, SyncPanel.tsx, EnableHistoryWizard.tsx,
├── FileHistory.tsx, MergeEditor.tsx, RemoteDialog.tsx, SyncSettingsSection.tsx
└── useSync.ts
ui/src/i18n/locales/{ru,en}/sync.*
apps/web/package.json       # isomorphic-git (через ui)
flake.nix                   # при необходимости: cmake/pkg-config для libgit2-sys
```

**Structure Decision**: новый модуль ядра `sync` по образцу `export` (сессия + web-обёртка),
команды — в общей оболочке `packages/shell`, UI — каталог `ui/src/sync`.

## Complexity Tracking

| Отклонение | Зачем | Почему проще не подходит |
| --- | --- | --- |
| Два новых внешних процесса/сервиса (системный `ssh`; локальный API Syncthing) при ограничении «Neovim — единственная интеграция с внешним процессом» | FR-014a (ключи на desktop) и FR-022 (состояние Syncthing) — решения пользователя в Clarifications | Встроенный SSH (`libssh2`) требует OpenSSL и всё равно не использует `~/.ssh/config`/агент так, как ожидает пользователь; без API Syncthing нельзя показать прогресс и отложить автоснимок. Оба интегрированы по образцу принципа VIII: только явно заданный канал (stdio протокола git / read-only REST), тайм-ауты, без общего состояния. **Принято**: поправка конституции 1.2.0 (2026-10-03). |
| C-зависимость `libgit2` (вендорная) | push/fetch/status на native при отсутствии push в чистом Rust (research §1) | `gix` без push; системный `git` недоступен на Android |
