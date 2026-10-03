# Contract: экспорт в общем UI

## PreviewClient

Расширение `ui/src/preview-client.ts`. Все методы обязательны в обеих реализациях:
`vault-client-web.ts` (wasm) и `vault-client-tauri.ts` (команды). Типы берутся из
`wasm-core` через `import type`, ручных дубликатов нет (принцип IV).

```ts
exportPdf(notePath: string, text: string | null, overrides: VaultOverrides): Promise<ExportReport>;
exportCandidates(overrides: VaultOverrides): Promise<ExportCandidate[]>;
exportBatch: {
  start(paths: string[], stopOnError: boolean, overrides: VaultOverrides): Promise<BatchState>;
  next(state: BatchState): Promise<BatchStep>;   // { state, next }
  record(state: BatchState, report: ExportReport, elapsedMs: number): Promise<BatchState>;
  skip(state: BatchState, notePath: string): Promise<BatchState>;
  cancel(state: BatchState): Promise<BatchState>;
  view(state: BatchState): Promise<BatchView>;
};
```

## useExport (ui/src/export/useExport.ts)

Живёт в `Workspace`, а не в Центре экспорта: закрытие Центра не прерывает пакет (FR-025).

Цикл, и никаких правил, кроме этих:

```text
while true:
  { state, next: path } = next(state); if path == null: break
  text = path == открытая заметка ? editorText : null        (FR-022)
  t0 = performance.now(); report = exportPdf(path, text, overrides)
  state = record(state, report, performance.now() - t0)
  await yieldToEventLoop()                                    (research.md §6)
```

- `cancel()` меняет состояние сразу; цикл увидит его на следующем `next`.
- Смена клиента vault или размонтирование `Workspace` → `cancel` (FR-026).
- Второй `start` при идущем пакете недоступен (FR-024).

## Экспорт текущей заметки

`Workspace` → `exportCurrent()`: `exportPdf(openNote, editorText, overrides)`. Итог
показывается уведомлением в шапке: путь и время либо «не удалось» с переходом к первой
ошибке (FR-007, FR-008). Команда скрыта, если открыта не заметка (FR-009).

Точки вызова: палитра ⌘K («Экспорт в PDF», «Центр экспорта»), `Mod+P` / `Mod+Shift+P`
через `useShortcuts`, кнопка «PDF» в тулбаре `PreviewPane`.

## Центр экспорта (ui/src/export/ExportCenter.tsx)

Макет: Claude Design «Typsidian: Typst Editor Design» → `Typsidian Утилиты.dc.html`, экран
Export. Модальное окно поверх рабочей области, как `SearchOverlay`.

- Шапка: иконка, «Центр экспорта», «выбрано N из M», закрыть.
- Слева: «все» · подсказка «shift+клик — диапазон» · «очередь N»; список по папкам с
  чекбоксами; справа в строке состояние задания: «в очереди», «компиляция» (hazard-полоса),
  «готово · 1.2 с · 214 КБ», «ошибка: …», «пропущено» или «не экспортируется». У строки
  в очереди есть кнопка «пропустить».
- Справа: формат «PDF» (единственный, без сегмента HTML), папка вывода только для чтения со
  ссылкой «изменить», которая открывает настройки vault; опция «Остановить при ошибке»;
  прогресс N / M, «осталось ~X с»; кнопки «Экспортировать» и «Отменить».
- Подвал: «EXPORT» · «уже готовые файлы сохраняются даже при отмене».
- Все правила CSS — с префиксом `.ts-export-*` (память проекта: селекторы без `.ts-*`
  залетают в Excalidraw).

## Настройки vault

`VaultSection` получает поле «Папка экспорта» (по умолчанию `export`). Отказ ядра
`InvalidExportFolder` показывается у поля, прежнее значение остаётся (FR-029).

## Индикатор в шапке

Пока пакет идёт, в шапке рядом со статусом компиляции видна пилюля «экспорт N/M»
(hazard-полоса). Клик открывает Центр экспорта (FR-025).
