---
name: quality-gates-typsidian
description: Гейты качества Typsidian по конституции — fmt, сборка ядра под wasm32 и нативный таргет, cargo test и wasm-bindgen-test, проверка типов фронтенда в strict, хардкод интерфейса. Читать перед тем, как назвать задачу или фичу готовой, после /speckit-implement, перед коммитом, а также когда нужно запустить cargo, pnpm или wasm-pack (их нет в PATH — только через nix develop).
argument-hint: "fast | core | ui | all"
user-invocable: true
---

# Гейты качества

Запрошенный уровень: `$ARGUMENTS` (пусто — подбери по таблице ниже).

## Как запускать тулчейн

`cargo`, `pnpm`, `wasm-pack`, `rustfmt` в PATH нет: всё живёт в dev-shell флейка.

```bash
nix develop -c cargo test -p typsidian-core settings::
nix develop -c pnpm typecheck
```

- Вход в dev-shell после смены `flake.lock` или сборки мусора качает зависимости
  минутами (один chromium — около двух), дальше — меньше секунды.
- **cargo — строго по одному.** Два cargo на один `target/` встают на
  `.cargo-lock`; оборванный таймаутом процесс сиротеет и держит лок дальше. Не
  запускать гейты параллельно с другой сборкой и не обрывать их таймаутом —
  давать запас по времени. Нативный cargo молчит без вывода → проверить лок
  (`flock -n 9 9<target/debug/.cargo-lock`), а не искать ошибку в правках.
- Корневые `cargo build`/`cargo test` ограничены ядром (`default-members`).
  Оболочки собираются явно: `-p typsidian-desktop`, `-p typsidian-mobile`,
  `cargo tauri build`.

## Уровни

```bash
.specify/scripts/typsidian/gates.sh fast   # fmt + i18n + cargo check ядра (native и wasm32)
.specify/scripts/typsidian/gates.sh core   # fast + cargo test + wasm-bindgen-test
.specify/scripts/typsidian/gates.sh ui     # build:wasm → build ui → typecheck
.specify/scripts/typsidian/gates.sh all    # core + ui
```

| Что изменилось | Минимум перед «готово» |
| --- | --- |
| Только `specs/`, `.claude/`, доки | ничего (роадмап проверит stop-хук) |
| `packages/core`, без изменения публичного API | `core` |
| `packages/core` с изменением API или типов на границе WASM/JS | `all` |
| Только `ui/src`, `apps/*/src`, `apps/web/app` | `ui` |
| `packages/shell`, `apps/*/src-tauri` | `fast` + сборка затронутой оболочки |
| `flake.nix`, `nix/` | `fast` + то, ради чего правка делалась |
| Закрытие фичи (`after_implement`) | `all` |

Уровень `core` — это Definition of done конституции для ядра: оба таргета
собираются (принцип II), `cargo test` и `wasm-bindgen-test` зелёные. Уровень `ui`
закрывает принцип IV (TypeScript `strict`, без `any`).

## Чего скрипт не проверяет

- **Оболочки Tauri.** `gates.sh` их не собирает — это долго и требует GUI-библиотек.
  Правка в `apps/desktop` или `apps/mobile` проверяется своей сборкой; обычный
  `cargo build` оболочки даёт dev-бинарь, который ходит на `localhost:1420`.
- **Поведение интерфейса.** JS-тестов в репозитории нет: зелёный `typecheck` не
  значит, что экран работает. Проверить руками или на стенде и сказать, как
  именно проверено.
- **Принцип I** (бизнес-логика только в ядре) и достижимость логики ядра из UI —
  это ревью и `/speckit-converge`, а не команда.

## Что считается пройденным

- Тест, который не запускался, — не пройден. Нет chromium/chromedriver, упал вход
  в dev-shell — в отчёте так и писать: «wasm-bindgen-test не запускался: …», а не
  «тесты зелёные».
- Уровень `ui` всегда идёт с пересборкой wasm и `ui/dist`: зелёный `ui` при
  красном `apps/desktop` — устаревший `ui/dist`, а не ошибка типов.
- Падение в файле, которого правка не касалась (ICE rustc, битый wasm-модуль), —
  сначала чистить артефакты сборки, потом искать ошибку в коде.
- `scripts/check-i18n.sh` красный → текст интерфейса уходит в
  `ui/src/i18n/locales/`; маркер `i18n-ignore` — только для строк, которые
  пользователю не показываются.

## Отчёт

После прогона — что запускалось, что прошло, что упало (с выводом), что не
запускалось и почему. Упавший гейт не объявляется «не связанным с изменением» без
проверки: дерево здесь обычно грязное на сотню файлов, поэтому `git stash` — только
с согласия пользователя, дешевле `git worktree add` на `HEAD`.

Длинный прогон уместно отдать `test-runner-agent`, чтобы логи не шли в диалог.
