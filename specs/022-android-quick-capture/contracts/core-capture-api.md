# Contract: API ядра захвата и плагина Android

## Ядро (wasm `capture/web.rs` и Tauri 1:1)

| Rust | wasm | Tauri | Что |
|---|---|---|---|
| `capture_settings(overrides)` | `captureSettings` | `capture_settings` | с умолчаниями |
| `entry(settings, capture) -> String` | `captureEntry` | `capture_entry` | образец записи (D6) и запись |
| `capture_write(…) -> CaptureOutcome` | `captureWrite` | `capture_write` | §6 |
| `daily_path(settings, date)` | `dailyPath` | `daily_path` | |
| `open_daily(settings, date) -> { path, created }` | `openDaily` | `open_daily` | создать по шаблону при отсутствии |
| `daily_neighbours(settings, date)` | `dailyNeighbours` | `daily_neighbours` | ‹ › |

Тесты (оба пути): все виды записи и приписки; экранирование; дописывание с переводом строки
и без; прежний текст неизменен; путь по трём видам названия; соседние дни через пропуски;
создание из шаблона и без; повторное «Сегодня» не создаёт вторую; конфликт отпечатка —
повтор на свежем тексте; предел размера.

## Плагин `capture` (Android)

| Команда / событие | Что |
|---|---|
| `pending() -> Vec<Capture>` | очередь из `filesDir/captures/pending/` |
| `resolve(id)` | удалить запись после успешной записи в vault |
| `returnToSource()` | `moveTaskToBack(true)`, если пришли из `ACTION_SEND` |
| событие `shared` | новая запись в очереди |
| событие `shortcut { action: "today" \| "capture" }` | ярлык |

В UI плагин приходит пропом `captureSource` из `apps/mobile/src/MobileWorkspace.tsx`.

## Фактическая поверхность (реализация ядра, research.md §12)

Чистые (wasm — свободные функции; Tauri — команды без vault):
`captureSettings(overrides: VaultOverrides) -> CaptureSettings`,
`captureEntry(settings, capture, imagePath?) -> string`,
`captureTargetPath(settings, capture) -> string | undefined`,
`dailyPath(settings, date) -> string | undefined`,
`dailyDateOf(settings, path) -> DailyDate | undefined`.

С vault (wasm — методы `WebBackend`; Tauri — команды с состоянием vault):
`captureWrite(capture, openBuffer?, image?) -> CaptureOutcome`,
`openDaily(date) -> OpenDaily` (отказ — `DailyError`),
`dailyNeighbours(date) -> DailyNeighbours`.

Tauri: `capture_settings`, `capture_entry`, `capture_target_path`, `daily_path`,
`daily_date_of`, `capture_write`, `open_daily`, `daily_neighbours`; аргументы camelCase
(`imagePath`, `openBuffer`).
