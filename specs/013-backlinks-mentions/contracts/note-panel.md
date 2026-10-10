# Contract: колонка заметки и режим «Связи» в общем UI

Макет: Claude Design «Typsidian: Typst Editor Design» → `Typsidian Связи.dc.html`, экраны
1a (колонка), 1b (состояния), 1c (телефон). Расхождения — research.md §16.

## VaultClient

Расширение `ui/src/vault-client.ts`; обе реализации обязательны (`vault-client-web.ts`,
`vault-client-tauri.ts`). Типы — из `wasm-core` через `import type`.

```ts
links: {
  backlinksPage(target: string, openTexts: OpenText[], offset: number, limit: number): Promise<BacklinksPage>;
  mentionsAvailability(target: string): Promise<MentionsAvailability>;
  mentionsStart(target: string): Promise<MentionsStart>;            // Started | Unavailable
  mentionsStep(state: MentionScan, openTexts: OpenText[]): Promise<MentionStep>;
  mentionsCancel(state: MentionScan): Promise<MentionScan>;
  mentionsIn(target: string, paths: string[], openTexts: OpenText[]): Promise<SourceGroup[]>;
  linkMention(request: LinkRequest, editorText: string | null): Promise<LinkOutcome>;
};
```

## Оболочка колонки — `ui/src/note-panel/`

Заводится этой фичей; 015 и 017 добавляют режимы, не меняя оболочку.

```ts
// modes.ts
export interface NotePanelModeSpec {
  id: NotePanelMode;                 // тип из ядра
  title: PhraseKey;                  // «Связи»
  badge: string | null;              // «5 · 2»
  render: () => React.ReactNode;     // тело режима
}

// NotePanel.tsx
<NotePanel modes={NotePanelModeSpec[]} active={NotePanelMode} onSelect presentation="column" | "sheet" onClose />
```

- `column`: `.ts-notepanel`, 248px (`--ts-notepanel-w`), между деревом и редактором (или
  доской), фон и рамка как у `.side` макета. Скрытая колонка не рендерится вовсе — как
  скрытое дерево.
- `sheet`: `.ts-notepanel--sheet` снизу на скриме, ручка-«grab»; закрывается по Escape
  (`useDismissOnEscape`), тапу на скрим и после перехода по строке.
- Шапка 26px (34px в шторке): названия режимов (активный — `.ts-eyebrow` оранжевым), справа
  тег-счётчик активного режима. Один режим — название без переключения.
- `useNotePanelState(device)` (`useNotePanelState.ts`): читает `Settings.note_panel` из
  снимка устройства один раз, отдаёт `{ shown, mode, mentionsExpanded, toggle, setMode,
  setMentionsExpanded }`, пишет `SettingsChange.note_panel` с паузой 400 мс. Без `device` —
  состояние в памяти.
- `useNarrowViewport()` (`useNarrowViewport.ts`): `matchMedia("(max-width: 720px)")`; на
  сервере и в первом кадре — `false`.
- Все правила CSS — с префиксом `.ts-notepanel*` / `.ts-links*` в `ui/src/styles/app.css`.

## Workspace

- Новый необязательный проп `device?: DeviceSettingsClient`; оболочки `apps/web`,
  `apps/desktop`, `apps/mobile` передают уже созданный клиент.
- Кнопка в шапке «Панель заметки» (`.ts-btn`, `.ts-btn--on` при показанной колонке); на
  узком окне подписана названием активного режима и открывает шторку.
- Сочетание `toggle-note-panel` через `useShortcuts`, команда в палитре, строка в
  `ShortcutsSection.tsx`.
- `linksRevision` — счётчик-триггер обновления (research.md §14): события vault,
  `NoteEditor.onSaved(path)`, итог «Связать».
- Переход по строке: `revealPlace({ path, line, column })` — существующий `revealDiagnostic`,
  обобщённый до этих трёх полей. После 014 его оборачивает история переходов (014).
- Сегмент статус-бара «N ссылок · M упоминаний» — проп `linksSummary` у `NoteEditor`; есть
  только при показанной колонке.

## useNoteLinks (`ui/src/note-panel/links/useNoteLinks.ts`)

Правил здесь нет — только ритм запросов. Вход: `client`, `target` (путь открытой заметки
или доски; `null` — ничего не открыто), `openTexts`, `enabled`, `revision`, `bulkChanging`.

```text
смена target            → сбросить всё, requestId++, backlinksPage(target, 0, 50)
revision изменился      → пауза 150 мс → backlinksPage(target, 0, 50)
                          раскрытые упоминания: mentionsIn([путь]) | убрать группу | проход заново
bulkChanging            → состояние «идёт сбор», запросы после конца
ответ с чужим target/requestId → отбросить                                   (FR-009)
«показать ещё»          → backlinksPage(target, next_offset, 50), дописать
раскрыть упоминания     → mentionsStart; пока Running: step → дописать → уступить циклу событий
свернуть / смена target / размонтирование → mentionsCancel, цикл останавливается   (FR-012)
```

`openTexts` в 013 — пустой список: единственная открытая заметка и есть цель. 014 передаёт
тексты всех открытых вкладок.

## Режим «Связи» (`ui/src/note-panel/links/LinksMode.tsx`)

| Состояние | Что показано (макет) |
|---|---|
| ничего не открыто | пояснение «откройте заметку» |
| первый запрос в пути / массовое изменение | hazard-полоса + «Собираю сведения о ссылках vault… Список появится сам.» (1b) |
| ссылок нет | пояснение с примером `[[название]]` (1b) |
| ссылки есть | раздел «▾ Обратные ссылки» со счётчиком; группы: название, папка (если другая), число; строки-фрагменты с `<mark class="ts-links__mark">` (1a) |
| источник с `problem` | строка «Заметка больше предела размера — фрагмент не показан.» / «не читается» (1a) |
| показана часть | «показано N из M заметок, ближайшие по папке — первыми» + «Показать ещё 50» (1b) |
| упоминания свёрнуты | «▸ Упоминания» + «раскройте, чтобы найти» (1b) |
| название короче трёх знаков | пояснение с названием (1b) |
| проход идёт | hazard-полоса + «Ищу упоминания: N из M заметок» + «Свернуть» (1b) |
| упоминания найдены | группы и строки, у строки кнопка «Связать» (1a) |
| упоминаний нет | пояснение |
| vault только на чтение | «Связать» недоступна + причина (1b) |
| исход `Stale` | тег «устарело» у группы, жёлтая строка; список этой заметки перезапрашивается (1b) |
| исходы `UnsavedElsewhere`, `Unexpressible`, `Failed`, порт `NoEditor` | строка с причиной у упоминания, файл не тронут |

- Клик или Enter по строке-фрагменту → `revealPlace`; в шторке — ещё и закрыть её.
- «Связать»: `editorText = editors.textOf(source)` → `linkMention` → `Written`: revision++;
  `EditorEdit`: `editors.applyEdit(source, edit)` (contracts/editor-edit-port.md), revision++
  после автосохранения.
- Строки — кнопки с `aria-label` «открыть <заметка>, строка N»; разделы — раскрывающиеся
  заголовки с `aria-expanded`.

## Тексты

Новые файлы `ui/src/i18n/locales/en/links.ts` и `ui/src/i18n/locales/ru/links.ts`,
подключённые в `en.ts` и `ru.ts`; ключи `notePanel.*` (оболочка) и `links.*` (режим), формы
числа — `ruN`. Хардкода в `ui/src/note-panel/` нет (`scripts/check-i18n.sh`).
