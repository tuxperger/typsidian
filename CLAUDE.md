# Typsidian

Заметки на Typst в духе Obsidian: vault из обычных файлов на диске, живой
предпросмотр, доски, шаблоны, экспорт. Ядро на Rust (`packages/core`) собирается под
нативный таргет и `wasm32`; один UI (`ui/`) на три оболочки — web (`apps/web`,
Next.js), рабочий стол и Android (`apps/desktop`, `apps/mobile`, Tauri). Принципы
проекта — `.specify/memory/constitution.md` (I–X); этот файл им не противоречит и
их не пересказывает.

## Как здесь работают

- **Любая фича — через Spec Kit.** Порядок и правила — скилл `flow-typsidian`.
  Команды `/speckit-*` вызываются самостоятельно через Skill. Код в `packages/`,
  `ui/`, `apps/`, `nix/`, `scripts/`, `flake.nix` правится только под открытую
  задачу `tasks.md` активной фичи; иначе хук `spec-gate` запросит подтверждение.
- **Идеи и предложения** фиксируются спекой стадии `spec` (скилл
  `roadmap-typsidian`), а не TODO в коде. Состояние всех фич — `specs/ROADMAP.md`
  (генерируется). `.specify/feature.json` — след последней `/speckit-*` команды,
  а не приоритет.
- **Тулчейна нет в PATH.** `cargo`, `pnpm`, `wasm-pack` — только через
  `nix develop -c …` или `.specify/scripts/typsidian/gates.sh` (скилл
  `quality-gates-typsidian`). Cargo-команды — строго по одной, без таймаутов.
- **Вся бизнес-логика — в `packages/core`** (принцип I), с тестами на обоих путях:
  `cargo test` и `wasm-bindgen-test` (принцип II). UI её не дублирует.
- **Тексты интерфейса** — только из `ui/src/i18n/locales/`; хардкод ловит хук.
- **Коммиты** — по скиллу `git-typsidian`: в `master`, фичей целиком, по-русски.
  Дерево обычно грязное на сотню файлов — чужое не откатывать и не коммитить.
- Язык артефактов Spec Kit, комментариев в коде, сообщений коммитов и общения —
  русский; идентификаторы — английские.

## Скиллы проекта

| Скилл | Когда |
| --- | --- |
| `flow-typsidian` | начало/продолжение фичи, выбор следующей `/speckit-*` команды |
| `roadmap-typsidian` | записать идею, посмотреть стадии, переключить активную фичу |
| `quality-gates-typsidian` | запуск cargo/pnpm, проверки перед «готово» и перед коммитом |
| `git-typsidian` | любой коммит, ребейз, пуш |

## Агенты

| Агент | Зона |
| --- | --- |
| `rust-core-agent` | `packages/core/**/*.rs` |
| `frontend-agent` | `ui/`, `apps/web`, вёрстка по макету |
| `tauri-shell-agent` | `apps/desktop`, `apps/mobile`, `src-tauri/` |
| `nvim-bridge-agent` | мост к headless Neovim |
| `nix-env-agent` | `flake.nix`, `nix/`, новые нативные зависимости |
| `speckit-agent` | артефакты `specs/**` |
| `security-review-agent` | read-only ревью vault-manager, моста, сети |
| `test-runner-agent` | прогон тестов со сводкой |

## Хуки

`.claude/settings.json` → `.claude/hooks/`:

| Хук | Событие | Что делает |
| --- | --- | --- |
| `session-context.sh` | SessionStart | активная фича, стадии всех фич, следующий шаг |
| `spec-gate.sh` | PreToolUse Edit/Write | правка кода продукта без открытых задач → запрос подтверждения (`TYPSIDIAN_SPEC_GATE=off` отключает) |
| `git-guard.sh` | PreToolUse Bash | разрушающие git-команды → запрос подтверждения |
| `check-i18n-edit.sh` | PostToolUse Edit/Write | хардкод интерфейса в изменённом файле |
| `post-edit.sh` | PostToolUse Edit/Write | `rustfmt --check` файла, линт `spec.md`/`tasks.md`, перегенерация роадмапа |
| `stop-gate.sh` | Stop | `cargo fmt --check` при изменённых `.rs`, актуальность роадмапа |

Хуки этапов Spec Kit — `.specify/extensions.yml`: после `/speckit-specify` —
`/speckit-clarify`, после `/speckit-tasks` — `/speckit-analyze`, после
`/speckit-implement` — `quality-gates-typsidian` и `/speckit-converge`.

## Скрипты

```bash
.specify/scripts/typsidian/feature.sh [list|next|use NNN]   # активная фича
.specify/scripts/typsidian/roadmap.sh [--check]             # specs/ROADMAP.md
.specify/scripts/typsidian/gates.sh [fast|core|ui|all]
scripts/check-i18n.sh [FILE...]                             # хардкод интерфейса
```

## Проверка в работающем приложении

Типы и тесты ядра не ловят ошибок времени выполнения в интерфейсе. Правку `ui/` стоит
открыть в браузере до того, как называть её готовой:

```bash
nix develop -c pnpm dev                                     # web на :3000, в фоне
nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
    [сценарий.mjs] --opfs-vault --shot /путь/снимок.png     # консоль, исключения, снимок
```

`scripts/dev/browser.mjs` — headless chromium из dev-оболочки без зависимостей; описание
методов сценария — в начале файла, образец — `scripts/dev/scenarios/rules-editor.mjs`.
`--opfs-vault` подменяет системный выбор папки хранилищем браузера. Оболочку рабочего
стола так открыть нельзя (`tauri-driver` в nixpkgs проекта нет). Отладчик ядра —
`rust-lldb` (пакетно: `rust-lldb -b -o '…' -- бинарь`).
