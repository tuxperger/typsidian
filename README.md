# Typsidian

Локальный **local-first** редактор заметок на языке [Typst](https://typst.app/) — по духу
Obsidian, но с Typst вместо Markdown.

Vault — это обычная папка с файлами на диске. Никакого сервера, аккаунта и проприетарного
формата: заметки читаются и правятся любым сторонним редактором, а Typsidian просто самый
удобный способ с ними работать.

> **Статус: ранняя разработка.** Работают менеджер vault, доски для рисования и панель
> настроек. Компиляции Typst и preview, моста к Neovim, полнотекстового поиска и мобильного
> клиента пока нет — см. [Что готово](#что-готово).

---

## Идея

Одно Rust-ядро (`packages/core`) — один поведенческий контракт для всех клиентов. Оно
собирается в WASM для веба и нативно для desktop/mobile; UI на React общий, клиент-специфичен
только тонкий связующий слой.

```
                    ┌──────────────────────────┐
                    │   packages/core (Rust)   │
                    │  вся бизнес-логика:      │
                    │  vault, доски, настройки │
                    └────────────┬─────────────┘
                    wasm32       │       native
              ┌─────────────────┴──────────────────┐
              ▼                                    ▼
     ui/ (React, общий)                   ui/ (React, общий)
              │                                    │
      apps/web (Next.js)              apps/desktop (Tauri) · apps/mobile (планируется)
```

Платформенные различия — файловая система, вотчеры, персистентность — скрыты за трейтами
внутри ядра. Вызывающий код никогда не ветвится по платформе.

## Что готово

| Фича | Спека | Состояние |
| --- | --- | --- |
| **Vault Manager** — открытие папки как vault, дерево заметок, создание/переименование/удаление, обнаружение внешних изменений, диалог конфликта с diff, предупреждение о битых ссылках | [`specs/001-vault-manager`](specs/001-vault-manager/spec.md) | реализовано; открыты ручные прогоны `quickstart.md` |
| **Drawing Boards** — файлы-доски на Excalidraw в том же workspace, автосохранение, вставка снимка доски в заметку, индикатор устаревшей вставки | [`specs/002-drawing-boards`](specs/002-drawing-boards/spec.md) | реализовано для web; открыты мобильный просмотр и прогоны quickstart |
| **Application Settings** — разделы Vault / Внешний вид / Кейбиндинги / Дополнительно, применение без кнопки «сохранить», формат-сохраняющая запись TOML | [`specs/004-app-settings`](specs/004-app-settings/spec.md) | в работе: разделы Vault, Внешний вид и Дополнительно готовы, раздел Кейбиндинги (режим редактирования) — нет |
| **Templates & Snippets** — библиотека заготовок в обычной папке vault | [`specs/003-templates-snippets`](specs/003-templates-snippets/spec.md) | спланировано, не начато |

Ещё не спроектировано: компиляция Typst и preview, мост к Neovim через msgpack-rpc, граф
ссылок сверх узкого парсера `[[ссылка]]`, полнотекстовый поиск, мобильный клиент,
синхронизация между устройствами.

## Структура репозитория

```
packages/core/        Rust-ядро: vault/, boards/, settings/, graph/. Единственное
                      место бизнес-логики. Тесты — и cargo, и wasm-bindgen-test.
ui/                   Общий React-UI (@typsidian/ui): workspace, дерево vault,
                      редактор, полотно доски, панель настроек, CSS-токены.
apps/web/             Web-клиент на Next.js 16.
apps/desktop/         Desktop-оболочка на Tauri 2 + Vite (src-tauri/ — команды-обёртки).
specs/                Артефакты Spec Kit по фичам: spec / plan / tasks / research /
                      data-model / contracts / quickstart.
.specify/             Шаблоны, скрипты и конституция проекта.
flake.nix             Воспроизводимое dev-окружение (Nix).
```

## Окружение разработки

### Через Nix (рекомендуется)

```bash
nix develop
```

Шелл даёт всё сразу: Rust stable с таргетом `wasm32-unknown-unknown`, `wasm-pack`,
`wasm-bindgen-cli`, `binaryen`, Node 22, pnpm, `cargo-tauri` с системными WebKitGTK/GTK для
линковки Tauri, sqlite, neovim и chromium+chromedriver из nixpkgs — последние два нужны, чтобы
`wasm-bindgen-test` реально гонялся в браузере (свой chromedriver от `wasm-pack` на NixOS не
стартует).

### Вручную

- Rust stable + таргет `wasm32-unknown-unknown`
- `wasm-pack`, `wasm-bindgen-cli`
- Node 22, pnpm 10+
- Для desktop: `cargo-tauri`, pkg-config и системные WebKitGTK 4.1 / GTK3 / libsoup3 / dbus
- Для wasm-тестов: Chromium и chromedriver

## Сборка и запуск

Порядок важен: ядро → wasm-биндинги → общий UI → клиент.

```bash
pnpm install

# 1. Собрать ядро в WASM и разложить биндинги в ui/src/wasm-core
pnpm build:wasm

# 2. Собрать общий UI-пакет (tsc + копирование стилей и wasm-core в ui/dist)
pnpm --filter @typsidian/ui build
```

**Web:**

```bash
pnpm dev            # next dev
pnpm build          # next build
```

Веб-клиент опирается на File System Access API и OPFS — нужен браузер на Chromium.

**Desktop:**

```bash
pnpm --filter @typsidian/desktop dev    # cargo tauri dev
pnpm --filter @typsidian/desktop build  # cargo tauri build
```

## Тесты и проверки

```bash
cargo test -p typsidian-core                          # ядро, нативный таргет
wasm-pack test --headless --chrome packages/core      # ядро, wasm32
cargo build -p typsidian-core --target wasm32-unknown-unknown
pnpm typecheck                                        # tsc --noEmit по всем пакетам
```

Бенчмарки ядра (`harness = false`, меряют сквозные сценарии, а не микрооперации):

```bash
cargo bench -p typsidian-core --bench vault_scale
cargo bench -p typsidian-core --bench vault_bulk_change
cargo bench -p typsidian-core --bench board_digest
```

Корневой `cargo build`/`cargo test` намеренно ограничен ядром через `default-members`: иначе
проверка wasm-таргета в корне тянула бы Tauri, GTK и WebKit под wasm32 и падала.

## Как ведётся работа

Проект живёт по [конституции](.specify/memory/constitution.md) и процессу
[Spec Kit](https://github.com/github/spec-kit). Ключевое, что стоит знать до первого PR:

1. **Бизнес-логика — только в `packages/core`.** UI её не дублирует и не «подстраховывает».
2. **Паритет двух таргетов (non-negotiable).** Изменение ядра обязано собираться и проходить
   тесты и под `wasm32-unknown-unknown`, и нативно. Иначе фича не готова.
3. **`spec.md` описывает только поведение** — без API, крейтов и путей к файлам. Технические
   решения живут в `plan.md`.
4. **TypeScript strict, без `any`** и без `@ts-ignore` в роли глушителя ошибок типов.
5. **Мост к Neovim и встраиваемые движки — зона повышенного риска:** отдельный
   `/speckit-clarify` до планирования, синхронизация с файлом vault вместо общего состояния,
   никакой стилизации чужого DOM.
6. **Индексы и поиск строятся локально.** Содержимое заметок не покидает устройство — включая
   эмбеддинги, сводки и любые производные представления.
7. **Массовые операции — отменяемые и неблокирующие**, с предсказуемым состоянием vault после
   отмены.

Последовательность работы над фичей: `/speckit-specify` → `/speckit-checklist` →
`/speckit-clarify` → `/speckit-plan` → `/speckit-tasks` → `/speckit-analyze` →
`/speckit-implement`.

## Лицензия

Не определена.
