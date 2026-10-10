# Contract: вкладки в интерфейсе

UI ничего не решает о вкладках: он отправляет действие, получает `Transition`, исполняет
`focus`/`closed`/`refused` и рисует `WorkspaceView` (принцип I).

## `VaultClient` (`ui/src/vault-client.ts` и обе реализации)

```ts
workspaceEmpty(): Promise<WorkspaceState>;
workspaceApply(state: WorkspaceState, action: WorkspaceAction, widthPx: number): Promise<Transition>;
workspaceView(state: WorkspaceState, widthPx: number): Promise<WorkspaceView>;
workspaceSnapshot(state: WorkspaceState): Promise<SavedWorkspace>;
workspaceRestore(saved: SavedWorkspace): Promise<WorkspaceState>;
discardEdit(path: string): Promise<void>;
```

## `useWorkspaceTabs` (`ui/src/workspace/tabs/useWorkspaceTabs.ts`)

```ts
interface WorkspaceTabs {
  view: WorkspaceView | null;            // null — vault не открыт или набор ещё читается
  dispatch(action: WorkspaceActionInput): Promise<Transition | null>;
  open(path: string, target?: TextPlace): Promise<void>;   // дерево, поиск, палитра, граф, ссылка, reveal
  close(tab: TabId): Promise<void>;      // flush документа → Close → исполнение отказа
  refusal: CloseRefusal | null;          // для панели «Вкладка не закрыта»
  dismissRefusal(): void;
}
```

- Действия исполняются строго по очереди: следующее отправляется с состоянием, которое
  вернуло предыдущее.
- `leaving` подставляет хук: место каретки активной вкладки из `tabEditorStates`.
- `view` перечитывается после каждого перехода, при событиях vault о путях открытых вкладок,
  при переходе любого документа «чисто ↔ несохранённое» и при пересечении шириной окна
  порогов; на нажатия клавиш в тексте — нет.
- События vault: `NoteRenamed` → `Renamed` и смена ключа документа; `NoteDeleted` →
  `Deleted`; `NoteUpdated` → `NoteDocument.reload()` у документа без несохранённых правок.
- Смена клиента vault сбрасывает состояние и читает набор нового vault.

## `NoteDocument` (`ui/src/editor/noteDocument.ts`, реестр `noteDocuments.ts`)

```ts
interface NoteDocument {
  readonly path: string;
  text(): string;
  status(): { read: "loading" | "ready" | ReadError; save: NoteSaveState };
  subscribe(listener: () => void): () => void;
  attach(view: DocumentView): () => void;               // вид объявляет себя, получает чужие правки
  edit(from: DocumentView, changes: ChangeSet): void;   // правка из вида: beginEdit, автосохранение, рассылка
  replace(text: string, saved: boolean): void;          // принятая внешняя версия, вставка доски, возврат версии
  reload(): Promise<void>;
  flush(): Promise<void>;
}
acquire(client, path): NoteDocument;   release(path): void;   rename(from, to): void;
```

Обязательства:
- на путь — один документ, один поток `beginEdit`, одно автосохранение, один `registerFlusher`
  (FR-017);
- автосохранение не зависит от того, смонтирован ли хоть один вид (SC-004);
- правила автосохранения те же, что в `useNoteAutoSave.ts` сегодня: пауза 600 мс, молчание
  при неразрешённом конфликте (`pendingConflict`), состояния `Clean/Dirty/Saving/Held/Failed`;
- буфер, вернувшийся к сохранённому тексту, снимается `discardEdit`;
- документ без видов и без несохранённых правок выгружается.

## `tabEditorStates` (`ui/src/editor/tabEditorStates.ts`)

`take(tab)`, `store(tab, state, scroll)`, `caretOf(tab): TextPlace | null`, `drop(tab)`,
`copy(from, to)` (разделение и перенос сохраняют каретку, US3 AS2). Не больше 20 полных
снимков; вытесненные хранят `anchor`/`head`/`scrollTop`.

## `CodeEditor`

Новые пропы: `initialState?: EditorState`, `initialScroll?`, `onDispose(state, scroll)`,
`document: NoteDocument` вместо `value`/`onChange`. Пересоздаётся при смене `TabId`, а не
пути. Привязка `Mod-Alt-\` из `defaultKeymap` исключена.

## `NoteEditor`

Получает `document`, `tabId`, `active: boolean` и `place: TextPlace | null` (куда встать после
`focus`). Слушатели окна (`insert-menu`, `element-params`), `onEditorReady`, `paramsRequest`
работают только при `active` — иначе при двух группах команда срабатывала бы дважды.
Заголовок и «Закрыть» из тулбара убраны.

## Компоненты `ui/src/workspace/tabs/`

| Компонент | Что рисует | Макет |
|---|---|---|
| `EditorGroups.tsx` | одна или две группы и `PaneDivider` между ними; `focusin` → `FocusGroup` | 1a, 1b |
| `EditorGroup.tsx` | полоса и содержимое активной вкладки: `NoteEditor`, `BoardTab` либо пустое состояние | 1a, 1c |
| `TabStrip.tsx` | «‹ ›», вкладки (название, `hint`, точка, «конфликт», «доска», ×), «◫», контекстное меню, перетаскивание; активная прокручена в видимую область | 1a, 1b |
| `OpenNotesSheet.tsx` | шторка «Открытые заметки» при `presentation = List` | 1c |
| `CloseRefusedNotice.tsx` | «Вкладка не закрыта» с причиной | 1c |

Полоса — `role="tablist"`, вкладка — `role="tab"` с `aria-selected`; стрелки влево/вправо
переводят фокус по вкладкам, `Enter` активирует, `Delete` закрывает.

## `Workspace.tsx`

- `openNote`/`editorText` исчезают; все потребители берут заметку из `view.active`, текст — из
  её документа: `PreviewPane`, `useExport`, `Breadcrumb`, `Sidebar.openPath`, `GraphView`,
  `SearchOverlay.currentNote`, `FileHistory`, `insertLink`, `plainInputTarget`, колонка заметки
  `ui/src/note-panel/` (013).
- `openEntry`, `openPath`, `openBoard`, `revealDiagnostic` сводятся к `tabs.open(path, target?)`
  — сигнатуры для 013/015/018 не меняются.
- Диалог конфликта показывает конфликт активной вкладки.
- Новый проп `tabsStore?: WorkspaceTabsStore`:

```ts
interface WorkspaceTabsStore {
  load(): Promise<{ vault: VaultId; saved: SavedWorkspace | null } | null>;  // null — vault не опознан
  save(vault: VaultId, saved: SavedWorkspace): Promise<void>;
}
```

Реализации: `tabs-store-web.ts` (`deviceStore()` + `deviceStoreQueue`), `tabs-store-tauri.ts`
(`device_settings`, `apply_device_settings`). Не передан или отказал — вкладки работают, но
не переживают перезапуск; сообщений пользователю нет (FR-023).

## Тексты

Все строки — `workspace.tabs.*` в `ui/src/i18n/locales/{ru,en}/workspace.ts`; подписи команд
для панели сочетаний — в `ui/src/settings/ShortcutsSection.tsx`.
