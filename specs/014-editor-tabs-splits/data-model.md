# Data Model: Вкладки и разделение редактора

Все типы — `packages/core/src/workspace/`, с `Serialize`/`Deserialize` и `Tsify` под wasm32;
в UI приходят через `import type` (принцип IV).

## Состояние

### WorkspaceState

| Поле | Тип | Правило |
|---|---|---|
| `groups` | `Vec<Group>` | длина 1 или 2; вторая группа не бывает пустой |
| `active_group` | `GroupSlot` (`First` \| `Second`) | указывает на существующую группу |
| `ratio_permille` | `u16` | доля первой группы, 200…800; умолчание 500 |
| `next_tab` | `u32` | счётчик выдачи `TabId` |

### Group

| Поле | Тип | Правило |
|---|---|---|
| `tabs` | `Vec<Tab>` | порядок полосы; путь в группе уникален (FR-002) |
| `active` | `Option<TabId>` | `None` только у пустой единственной группы |
| `recent` | `Vec<TabId>` | порядок активности, свежая первой; те же идентификаторы, что в `tabs` (FR-006) |
| `history` | `History` | своя у группы (FR-010) |

### Tab

`id: TabId` (не меняется при переименовании и переносе между группами), `path: String` (путь
в vault), `kind: FileKind` (заметка или доска — из индекса). Путь доски уникален на всё
состояние (research.md §8).

### History

`entries: Vec<HistoryEntry { path: String, place: Option<TextPlace> }>`, `cursor: usize`.
Не длиннее 100; соседние записи одного пути и места не повторяются; после добавления записи
всё правее указателя отброшено (FR-012).

### TextPlace

`line: u32`, `column: u32` — с единицы, те же единицы, что у `DiagnosticLocation` и перехода
`reveal`.

## Вход

### WorkspaceAction (`#[serde(tag = "kind")]`)

| Действие | Поля | Что делает |
|---|---|---|
| `Open` | `path`, `target: Option<TextPlace>`, `leaving: Option<TextPlace>` | вкладка в активной группе либо активация существующей (FR-001, FR-002); запись в историю |
| `Activate` | `tab`, `leaving` | активная вкладка и её группа; запись в историю |
| `Cycle` | `direction: Next \| Previous`, `leaving` | соседняя по полосе, по кругу (FR-003) |
| `Close` | `tab` | закрыть с проверкой (FR-005); активной становится предыдущая по `recent` (FR-006) |
| `CloseOthers` | `tab` | закрыть прочие вкладки группы, каждую с проверкой |
| `Move` | `tab`, `group`, `index` | порядок в полосе и перенос между группами (FR-003, FR-016) |
| `MoveToOtherGroup` | `tab` | перенос в другую группу (FR-016) |
| `Split` | `leaving` | вторая группа с активной вкладкой (заметка дублируется, доска переносится), фокус в ней (FR-015) |
| `Unsplit` | — | вкладки второй группы дописываются в первую без дубликатов (FR-015) |
| `FocusGroup` | `group`, `leaving` | каретка в другой группе (US3 AS4) |
| `SetRatio` | `permille` | доля первой группы, с подрезкой (FR-018) |
| `Back` / `Forward` | `leaving` | движение по истории активной группы (FR-010…FR-014) |
| `Renamed` | `from`, `to` | путь во вкладках и истории |
| `Deleted` | `path` | вкладки пути закрываются (кроме случая конфликта), записи истории вычищаются |

### Facts

`notes: Vec<NoteFact { path, kind: FileKind, unsaved: bool, conflict: bool }>` — только пути,
названные состоянием и действием; отсутствие записи означает «заметки нет».
`width_px: u32` — ширина окна.

## Выход

### Transition

| Поле | Тип | Смысл |
|---|---|---|
| `state` | `WorkspaceState` | новое состояние (при отказе — прежнее) |
| `focus` | `Option<Focus { group, tab, path, kind, place: Option<TextPlace> }>` | куда поставить каретку; `place` задан при `Open` с `target` и при `Back`/`Forward` |
| `refused` | `Vec<CloseRefusal { tab, path, reason: Unsaved \| Conflict }>` | вкладки, которые не закрылись |
| `rejected` | `Option<Rejection>` | `NotFound`, `TooNarrow`, `NothingToSplit`, `SingleGroup`, `NoHistory`, `UnknownTab` |
| `closed` | `Vec<TabId>` | закрытые вкладки — UI освобождает их снимки редактора |

### WorkspaceView

| Поле | Тип | Смысл |
|---|---|---|
| `presentation` | `Split` \| `Single` \| `List` | research.md §10 |
| `groups` | `Vec<GroupView>` | две при `Split`, иначе одна (слитая) |
| `active_group` | `GroupSlot` | группа с кареткой |
| `ratio_permille` | `u16` | |
| `active` | `Option<ActiveNote { tab, path, name, kind }>` | за ней следуют предпросмотр и панели (FR-009) |
| `can_split`, `can_unsplit`, `can_move_to_other` | `bool` | доступность действий (FR-020) |
| `tab_count` | `u32` | вкладок во всех группах |

`GroupView`: `slot`, `tabs: Vec<TabView>`, `active: Option<TabId>`, `can_back`,
`can_forward` (FR-014).

`TabView`: `id`, `group` (настоящая группа вкладки — в слитой полосе они разные), `path`,
`title` (имя без расширения), `hint: Option<String>` (кратчайший отличающий хвост пути
папки, если `title` повторяется среди открытых вкладок, FR-007), `kind`, `unsaved`,
`conflict`, `active`.

## Сохранённый набор

`SavedWorkspace { groups: Vec<SavedGroup { tabs: Vec<String>, active: Option<u32> }>,
active_group: GroupSlot, ratio_permille: u16 }` — значение в
`Settings.workspaces: Vec<VaultWorkspaceEntry { vault: VaultId, value: SavedWorkspace }>`
(`packages/core/src/settings/mod.rs`). Изменение — `SettingsChange.workspace:
Option<VaultWorkspaceEntry>`, заменяет запись своего vault целиком.

`restore(saved, notes)`: пути без заметки в индексе пропускаются; повтор пути в группе
снимается; пустая группа убирается; групп больше двух — лишние отбрасываются; `active` вне
диапазона — первая вкладка; доля подрезается; история пуста, `recent` — активная, затем
порядок полосы. Негодная запись в файле настроек читается как отсутствующая (FR-023).

## Переходы, проверяемые тестами

- `Open` нового пути → вкладка в конце активной группы, активна, прежняя на месте.
- `Open` открытого в активной группе пути → та же вкладка, число вкладок прежнее.
- `Open` несуществующего пути → `rejected: NotFound`, состояние прежнее.
- `Close` активной → активна предыдущая по `recent`; последней во второй группе → одна
  группа; последней в первой при непустой второй → вторая становится первой (FR-019).
- `Close` при `unsaved` → `refused: Unsaved`; при `conflict` → `refused: Conflict` и фокус
  на вкладке; при том же пути в другой группе → закрывается.
- `Back` после `A→B→C` дважды → `A` с местом ухода; `Forward` → `B`; `Open D` после `Back` →
  `can_forward = false`.
- `Back` на закрытую → вкладка открыта заново; на удалённую → пропуск; некуда →
  `rejected: NoHistory`.
- `Split` при `width_px < 900` → `rejected: TooNarrow`; единственная вкладка-доска →
  `NothingToSplit`.
- `view` при двух группах и `width_px < 900` → `Single`, вкладки обеих групп, состояние не
  изменено; при `< 720` → `List`.
- `snapshot` → `restore` с теми же заметками даёт те же вкладки, порядок, группы, активные и
  долю (SC-003).

## Состояние UI (не модель)

| Что | Где | Ключ |
|---|---|---|
| текст, автосохранение, чтение, ошибка чтения | `NoteDocument` (`ui/src/editor/noteDocument.ts`) | путь |
| `EditorState`, прокрутка | `ui/src/editor/tabEditorStates.ts` | `TabId` |
| значение `WorkspaceState`, последний `WorkspaceView` | `ui/src/workspace/tabs/useWorkspaceTabs.ts` | окно |
