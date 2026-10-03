# Quickstart: проверка PDF-экспорта

Все команды — из корня репозитория внутри `nix develop` (cargo-команды строго по одной).

## Ядро

```sh
nix develop --command cargo test -p typsidian-core --lib export
nix develop --command cargo test -p typsidian-core --test export_native --test placement_native
nix develop --command cargo build -p typsidian-core --target wasm32-unknown-unknown
nix develop --command bash -c "cd packages/core && wasm-pack test --headless --chrome -- --test export_wasm --test placement_wasm"
```

Что доказывают тесты:
- `export_cases` (оба таргета): заметка с изображением и импортом даёт PDF, который начинается с `%PDF-`, а
  число страниц совпадает с предпросмотром (FR-001). Заметка с ошибкой не создаёт и не меняет
  файл (FR-008). Повторный экспорт заменяет файл (FR-030). Путь, занятый папкой, даёт
  `TargetIsFolder` (FR-031). Без `source_text` заметка читается с диска, удалённая даёт
  `NoteMissing`.
- `export_cases`, пакет: переходы из data-model.md, запрещённые состояния, «остановить при ошибке»,
  отмена во время `Exporting`, оценка времени.
- `folder` (модульные) и `vault_settings_cases`: все правила `validate_folder`; `write_vault_settings` отклоняет негодную папку и
  не трогает файл.
- `export_native`, `write_bytes`: после имитированного сбоя между записью и `rename` на месте цели
  прежний файл, а `.typsidian/tmp/` очищается при открытии.

## UI

```sh
nix develop --command bash -c 'cd ui && pnpm typecheck'
nix develop --command pnpm build
```

## Ручная проверка (web, `pnpm dev`, и desktop)

1. Открыть vault с `Диссертация/Введение.typ`, нажать `Ctrl+P`. Появится уведомление
   «export/Диссертация/Введение.pdf · N мс», а файл откроется в стороннем просмотрщике со
   встроенными шрифтами (`pdffonts` показывает `emb yes`).
2. Внести ошибку и нажать `Ctrl+P`: будет «не удалось», переход к ошибке, прежний PDF не
   тронут.
3. `Ctrl+Shift+P` → «все» → «Экспортировать». Статусы и прогресс меняются, во время пакета
   можно печатать. На третьей заметке нажать «Отменить»: в `export/` только целые PDF,
   `find export -size 0` пуст, во `.typsidian/tmp/` пусто.
4. Закрыть Центр во время пакета: пилюля «экспорт N/M» в шапке останется, клик по ней
   вернёт Центр.
5. Настройки → Vault → «Папка экспорта» = `../x`: будет отказ с объяснением. Затем
   `Выгрузка/2026`: следующий экспорт пойдёт туда.
