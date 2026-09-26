# Quickstart: проверка фичи Template Download

**Фича**: `007-template-download` | **Дата**: 2026-09-25

Как убедиться, что фича работает. Не инструкция по реализации — сценарии проверки со ссылками
на [spec.md](./spec.md), [data-model.md](./data-model.md) и
[contracts/template-install.md](./contracts/template-install.md).

## Предусловия

Весь инструментарий существует только внутри dev-shell:

```bash
nix develop
```

Тестовый vault — любая папка с заметками; ниже `~/media/notes`. Перед прогоном убрать
установленное прошлыми попытками, чтобы сценарии начинались с «шаблона нет»:

```bash
rm -rf ~/media/notes/.typsidian/packages/preview/example
```

## Автоматические проверки

Гейт принципа II — оба таргета. **Сеть в тестах не участвует**: источник в тестах пути сборки —
адрес-ловушка, который обрывает попытку до первого байта, но оставляет диагностику с адресом.

```bash
cargo test -p typsidian-core --test install_native
wasm-pack test --headless --chrome packages/core --test install_wasm
cargo test -p typsidian-core --test placement_native        # годность своих адресов
wasm-pack test --headless --chrome packages/core --test placement_wasm
```

| Проверка | Что доказывает |
|----------|----------------|
| `an_installed_template_is_not_fetched_again` | SC-002: повторное открытие в сеть не ходит |
| `a_note_without_imports_never_reaches_for_the_network` | FR-008, SC-009 |
| `the_fetch_path_sends_nothing_about_the_note` | FR-021, принцип IX |
| `disabled_auto_download_makes_no_attempt` / `enabled_…` | FR-009, SC-004 |
| `an_explicit_install_ignores_the_switch` | FR-010 |
| `a_user_url_is_never_replaced_by_the_registry` | FR-004, US4 AS2 |
| `a_local_namespace_is_only_reported_missing` | FR-005, US1 AS4 |
| `compiling_reports_a_tampered_template_without_reinstalling` | FR-023 |
| `a_path_traversal_rejects_the_whole_archive` | FR-019, SC-007 |
| `a_read_only_vault_refuses_installation` | FR-018 |
| `a_bad_template_source_is_refused_before_writing` | FR-004: негодный адрес не пишется |

> Если `wasm-pack test` падает паникой `wasm-bindgen` (walrus, `decode.rs`) или тайм-аутом до
> первого теста — это инструмент, а не тесты: повторить прогон. Если падает ICE `rustc` или
> `rust-lld: invalid opcode` — удалить `target/wasm32-unknown-unknown/debug`: артефакты от
> прежнего компилятора после обновления dev-shell несовместимы.

Гейт принципа IV — после правки ядра обе пересборки:

```bash
pnpm build:wasm && pnpm -C ui build && pnpm typecheck
```

## Ручные сценарии

Заметка для всех сценариев — `~/media/notes/Шаблон.typ`:

```typst
#import "@preview/example:0.1.0": *
Текст заметки.
```

### 1. Загрузка сама (US1, US2)

1. Открыть vault, открыть `Шаблон.typ`.
2. **Ожидание**: в предпросмотре предупреждение «шаблон @preview/example:0.1.0 загружен с
   https://packages.typst.org/preview/example-0.1.0.tar.gz (N файлов, отпечаток …)»;
   документ собран.
3. На диске: `.typsidian/packages/preview/example/0.1.0/typst.toml` и
   `.typsidian-fingerprint`.
4. Закрыть и открыть заметку снова. **Ожидание**: сообщения о загрузке нет (SC-002).

### 2. Соседняя версия (US1 AS3)

Поменять версию на `0.1.1` (или любую существующую). **Ожидание**: загружена именно она,
папка `0.1.0` на месте.

### 3. `@local` в сеть не ходит (US1 AS4)

`#import "@local/мой:0.1.0": *` — **ожидание**: только «шаблон … не установлен», ни слова о
загрузке и источнике.

### 4. Выключатель (US3)

1. Настройки → web: раздел «Vault» → «Внешние шаблоны Typst»; desktop/Android: «Шаблоны
   Typst». Снять «Загружать недостающие шаблоны», **закрыть** настройки.
2. Удалить папку шаблона, открыть заметку. **Ожидание**: «шаблон … не установлен, а
   автозагрузка выключена», рядом кнопка «Загрузить шаблон»; в сеть не ходили (FR-011:
   без перезапуска).
3. Нажать «Загрузить шаблон». **Ожидание**: строка «Шаблон … загружен с …», документ
   пересобран (FR-010).

### 5. Свой адрес (US4)

1. В том же разделе «Свои адреса»: `@preview/example:0.1.0` → `https://127.0.0.1:9/x.tar.gz`,
   «Добавить».
2. Удалить папку шаблона, включить автозагрузку, открыть заметку. **Ожидание**: отказ назван
   с адресом `https://127.0.0.1:9/x.tar.gz`, реестр не подставлен (US4 AS2).
3. Ввести `example` без `@` и версии → «Добавить». **Ожидание**: причина «Адрес для шаблона
   «example» не сохранён…», поля не очищены.

### 6. Отпечаток (US5)

1. Установить шаблон (сценарий 1). В разделе шаблонов — строка с полным отпечатком.
2. Дописать пробел в любой `.typ` шаблона, перезапустить приложение, открыть заметку.
   **Ожидание**: предупреждение «содержимое шаблона … разошлось с записанным отпечатком …;
   приложение его не переустанавливает». Файл с правкой на месте (FR-023).
3. На втором устройстве поставить ту же версию: отпечатки совпадают (FR-024).

### 7. Офлайн (принцип VII, FR-012…FR-014)

1. Удалить папку шаблона, отключить сеть (desktop: `nmcli networking off`; web: DevTools →
   Network → Offline).
2. Открыть заметку. **Ожидание**:
   - desktop: «сети нет: https://packages.typst.org/… недостижим»;
   - web: «обращение к … запрещено средой этого клиента; шаблон можно положить вручную…» —
     браузер не отличает отсутствие сети от запрета (research §2), и формулировка называет обе
     возможности (FR-015).
3. Редактор отзывчив, набранный текст на месте, остальные заметки собираются (SC-005).
4. Ни одна попытка не висит дольше 30 с (SC-006).

### 8. Vault только для чтения (FR-018)

```bash
chmod -R a-w ~/media/notes/.typsidian
```

Удалить нельзя — поэтому начинать с vault, где шаблона нет, либо снять права до установки.
**Ожидание**: «папка заметок недоступна на запись», ни одного нового файла. После проверки
вернуть права: `chmod -R u+w ~/media/notes/.typsidian`.

## Что отметить

Результаты ручных сценариев 1–8 для web и desktop — в
[checklists/requirements.md](./checklists/requirements.md), раздел «Прогон quickstart».
