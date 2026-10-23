import type {
  AccessState,
  AttachmentBatchState,
  AttachmentBatchStep,
  AttachmentBatchView,
  BacklinksPage,
  BibliographyOffer,
  CitationPlan,
  CitationPlanError,
  CitationScanView,
  CitingNote,
  CreateSourceFileError,
  LibrarySummary,
  MissingKey,
  NoteCheck,
  RefreshStep,
  SourceHit,
  SourceHits,
  Board,
  BoardEvent,
  BoardWriteProfile,
  Capture,
  CaptureOutcome,
  CaptureSettings,
  DailyDate,
  DailyNeighbours,
  CommandId,
  DiffLine,
  EditConflict,
  Embed,
  EmbedPlan,
  EmbedRef,
  HighlightEdit,
  HighlightWindow,
  ImportJob,
  ImportState,
  ImportSurvey,
  FileKind,
  FileStamp,
  InsertionEdit,
  EditOutcome,
  EditRequest,
  ElementAtResult,
  ElementSpec,
  RenderOutcome,
  RenderRequest,
  InsertionPoint,
  IncomingItem,
  InsertMoment,
  LinkOutcome,
  LinkHealth,
  NoteHistory,
  DeletedNote,
  SpellConfig,
  EmbedInsertion,
  AddOutcome,
  BookmarkPlace,
  BookmarkTarget,
  BookmarksView,
  RecoverOutcome,
  RestoreOutcome,
  SnapshotReason,
  SnapshotUsage,
  TakeOutcome,
  LinkFixRequest,
  LinkRequest,
  MentionScan,
  MentionStep,
  MentionsAvailability,
  MentionsStart,
  MoveImpact,
  MoveOutcome,
  MoveRequest,
  MoveTarget,
  NoteProperties,
  OpenDaily,
  Outline,
  OutlineRequest,
  OutlineView,
  ParsedConditions,
  PropertyEdit,
  PropertyKeyInfo,
  ReindexOutcome,
  LibraryStatus,
  SelectBlock,
  Selection,
  SelectionResult,
  SelectLibrary,
  TextEdit,
  NameCheck,
  Note,
  OpenText,
  PreviewState,
  ResolvedBinding,
  CompletionList,
  CompletionRequest,
  DefinitionOutcome,
  DefinitionRequest,
  GraphRequest,
  MarksRequest,
  ReferenceMark,
  VaultGraph,
  SearchQuery,
  SearchResults,
  ReplaceQuery,
  ReplacePreview,
  ReplaceNoteRequest,
  ReplaceNoteOutcome,
  UndoRecord,
  UndoOutcome,
  Shortcut,
  SavedWorkspace,
  ShowAction,
  ShowKeymap,
  ShowState,
  SlideLabel,
  SlidePlace,
  PageSlideInfo,
  SourceGroup,
  Template,
  TemplateEvent,
  TemplateLibraryView,
  Transition,
  VaultEvent,
  VaultOverrides,
  WorkspaceAction,
  WorkspaceState,
  WorkspaceView,
} from "./wasm-core/typsidian_core.js";

// Поверхность ядра, которой пользуется общий UI (001 T113, T114).
//
// Зачем интерфейс, если есть сгенерированный `WebBackend`: на web ядро живёт в
// том же процессе (WASM), на desktop — за границей процесса, за командами Tauri
// поверх `NativeBackend`. Формы вызовов у них совпадают, а способ доставки —
// нет, и `ui/` не имеет права знать, какой из них используется: иначе в общем
// коде появилось бы ветвление по таргету, которое принцип II запрещает.
//
// **Типы при этом остаются сгенерированными из Rust** (принцип IV): они
// импортируются из `wasm-core` как типы, то есть на этапе компиляции. Ни одна
// оболочка не описывает форму `Note` или `VaultEvent` руками, и desktop-сборка
// при этом не тянет WASM в рантайм — `import type` стирается.
//
// Всё, что здесь async, async и в ядре либо становится таким на границе
// процесса. Единственное исключение — `hasUnresolvedEdits`, см. ниже.

/** Ход и отмена массового переноса (фича 023, принцип X). */
export interface MoveControl {
  onProgress?: (done: number, total: number) => void;
  signal?: AbortSignal;
}

/**
 * Разобранный текст одного редактора в ядре (фича 024, contracts/core-highlight-api.md).
 */
export interface HighlightSession {
  /**
   * Правки редактора (UTF-16, каждая — в тексте после предыдущих) и раскраска
   * окон одним вызовом: тройки `[from, to, role | flags << 8]`, роли —
   * `editor/highlight/roles.ts`. `undefined` — сессия разошлась с редактором
   * (`length` — длина текста после правок), её надо открыть заново.
   */
  update(
    edits: HighlightEdit[],
    length: number,
    windows: HighlightWindow[],
  ): Promise<Uint32Array | undefined>;
  /**
   * То же без ожидания — там, где ядро в том же потоке, что и редактор (web).
   * Редактор зовёт его прямо в обновлении вида, и раскраска попадает в ту же
   * транзакцию, что и набранный символ (SC-002). Нет — только `update`.
   */
  updateNow?: (
    edits: HighlightEdit[],
    length: number,
    windows: HighlightWindow[],
  ) => Uint32Array | undefined;
  /**
   * Слова с ошибками в окнах (фича 030): пары `[from, to)` в UTF-16 текста,
   * который сессия получила последним `update`. `undefined` — ответа нет
   * (сессия закрыта или занята), редактор спросит снова.
   */
  spelling(windows: HighlightWindow[]): Promise<Uint32Array | undefined>;
  close(): void;
}

export interface VaultClient {
  // ── Дерево и файлы (001 FR-003…FR-014) ──────────────────────────────

  /**
   * Имя папки, открытой как vault — первая часть хлебных крошек в шапке
   * (шаблон § Header bar).
   *
   * Свойство, а не метод: корень не меняется, пока vault открыт, и спрашивать
   * его на каждую отрисовку заголовка незачем. Приходит из ядра
   * (`WebBackend::rootPath`, команда `root_path` на рабочем столе), а не
   * запоминается интерфейсом при выборе папки: тогда это было бы второе место,
   * знающее, какой vault открыт.
   */
  readonly rootName: string;

  list(): Promise<Note[]>;
  read(path: string): Promise<string>;
  write(path: string, content: string, expected?: FileStamp | null): Promise<void>;
  create(path: string): Promise<void>;
  rename(from: string, to: string): Promise<void>;
  delete(path: string): Promise<void>;

  // ── Перенос между папками (фича 023) ────────────────────────────────

  /**
   * Папки, которые могут принять элемент, в порядке показа; `query` — фильтр
   * окна «Переместить в…», пустой — все. Допустимость и совпадение решает ядро:
   * жест хранит ответ как множество, окно рисует строки как пришли.
   */
  moveTargets(from: string, query: string): Promise<MoveTarget[]>;
  /**
   * Заметки, чьи ссылки перенос сделает битыми (FR-016). Отказ переноса
   * (`MoveRefusal`) приходит отклонённым промисом.
   */
  moveImpact(request: MoveRequest): Promise<MoveImpact>;
  /**
   * Перенос заметки, доски или папки. На web папка копируется пофайлово —
   * отсюда ход и отмена; на рабочем столе перенос атомарен, и `control` не
   * используется.
   */
  moveEntry(request: MoveRequest, control?: MoveControl): Promise<MoveOutcome>;

  backlinks(path: string): Promise<Note[]>;
  /** Граф связей vault одним вызовом ядра (фича 025). */
  graph(request: GraphRequest): Promise<VaultGraph>;
  /** Список меток и заметок у каретки (фича 018). Текст — буфер редактора. */
  completions(request: CompletionRequest): Promise<CompletionList>;
  /** Куда ведёт ссылка под кареткой (фича 018, FR-005). */
  definitionAt(request: DefinitionRequest): Promise<DefinitionOutcome>;
  /** Отметки ссылок в никуда и повторных меток (фича 018, FR-006). */
  referenceMarks(request: MarksRequest): Promise<ReferenceMark[]>;
  brokenLinks(path: string): Promise<string[]>;
  accessState(): Promise<AccessState>;

  /**
   * Расширенный поиск по vault (макет «Утилиты» § Поиск).
   *
   * Возвращает готовые группы и строки, уже разрезанные на куски с пометкой
   * «подсветить»: где именно совпало, решает ядро — иначе regex и регистр
   * пришлось бы разбирать второй раз в интерфейсе.
   */
  search(query: SearchQuery): Promise<SearchResults>;

  /**
   * Замена по vault (фича 027): предпросмотр, замена в одной заметке, отмена.
   * Правила — в ядре (`search::replace`); тексты открытых заметок передаются,
   * чтобы места и отпечатки считались по редактору, а не по диску.
   */
  replacePreview(query: ReplaceQuery, openTexts: OpenText[]): Promise<ReplacePreview>;
  replaceInNote(
    request: ReplaceNoteRequest,
    editorText: string | null,
  ): Promise<ReplaceNoteOutcome>;
  replaceUndo(record: UndoRecord, editorText: string | null): Promise<UndoOutcome>;

  /**
   * Чем является файл по пути (фича 002, FR-004).
   *
   * Для файлов, уже попавших в дерево, спрашивать не нужно: вид лежит в
   * `Note.kind`. Этот вызов — для пути, которого в дереве ещё нет, например
   * имени создаваемого файла.
   */
  fileKind(path: string): Promise<FileKind>;

  // ── Несохранённые правки и конфликты (001 US4) ──────────────────────

  beginEdit(path: string, buffer: string): Promise<void>;
  /**
   * Снять объявленные правки: буфер вернулся к сохранённому тексту (фича 014,
   * research.md §6). Без этого такая заметка осталась бы в ядре «несохранённой»,
   * а вкладку с ней нельзя было бы закрыть.
   */
  discardEdit(path: string): Promise<void>;
  pendingConflict(path: string): Promise<EditConflict | undefined>;
  conflictDiff(path: string): Promise<DiffLine[]>;
  resolveConflictKeepLocal(path: string): Promise<void>;
  resolveConflictAcceptExternal(path: string): Promise<string | undefined>;

  /** Что будет потеряно при закрытии (FR-037). Пустой список — закрывать безопасно. */
  unresolvedEditPaths(): Promise<string[]>;

  /**
   * Есть ли что терять — **синхронно**.
   *
   * Существует ровно из-за `beforeunload`: браузер не ждёт промисов и требует
   * решения в момент обработки события. Реализация обязана ошибаться в сторону
   * «есть несохранённое»: лишний вопрос пользователю раздражает, пропущенный —
   * теряет его текст. На web это прямой вызов ядра в том же процессе, на
   * desktop — снимок, который клиент поддерживает сам.
   */
  hasUnresolvedEdits(): boolean;

  /**
   * Дописывает на диск всё, что ещё лежит в буферах редакторов, и отвечает,
   * удалось ли (фича 011, FR-006): снимок истории делается только с того, что
   * пользователь видит. `false` — что-то осталось несохранённым (отказ записи,
   * неразрешённый конфликт), и снимок делать нельзя.
   *
   * Буферы держат редакторы, а не клиент, поэтому они объявляют здесь свой
   * способ дописать — см. `registerFlusher`.
   */
  flushAll(): Promise<boolean>;

  /** Редактор объявляет, как дописать свой буфер. Возвращает отмену. */
  registerFlusher(flush: () => Promise<void>): () => void;

  // ── Поток внешних изменений (001 FR-016) ────────────────────────────

  /**
   * Подписка на `VaultEvent`. Возвращает функцию отписки.
   *
   * Способ доставки — забота клиента, и он различается принципиально: у File
   * System Access API нет подписки на изменения, поэтому web сверяется по
   * таймеру (research.md §3), а native получает push от `notify`. Общий UI
   * видит один поток событий и ни о чём из этого не знает (T114).
   */
  subscribe(onEvents: (events: VaultEvent[]) => void): () => void;

  // ── Доски и вставки (фича 002) ──────────────────────────────────────

  /**
   * Может ли этот клиент писать доски (FR-035).
   *
   * Свойство, а не метод, и `readonly`: профиль выбирает оболочка при сборке,
   * общий UI его только **читает** — чтобы объявить ограничение пользователю
   * (FR-034) и не показывать инструменты рисования. Гейт записи от этого
   * значения не зависит: он в ядре и сработает, даже если интерфейс
   * ошибётся (research.md §8).
   *
   * Синхронное: у оболочки оно известно до открытия vault, а рендер доски
   * не должен ждать ответа, чтобы не мигнуть режимом правки на клиенте,
   * которому правка недоступна.
   */
  readonly boardWriteProfile: BoardWriteProfile;

  createBoard(path: string): Promise<Board>;
  readBoard(path: string): Promise<string>;
  writeBoard(
    path: string,
    content: string,
    expected?: FileStamp | null,
  ): Promise<void>;
  writeSnapshot(
    snapshotPath: string,
    svg: string,
    boardPath: string,
    boardContent: string,
  ): Promise<void>;
  embedsOf(notePath: string, noteText: string): Promise<Embed[]>;
  embedsInText(notePath: string, noteText: string): Promise<EmbedRef[]>;
  embedsChanged(
    notePath: string,
    noteText: string,
    changed: string,
  ): Promise<BoardEvent | undefined>;
  /**
   * Куда и под каким именем писать снимок новой вставки (фича 002 FR-018,
   * фича 006 FR-004).
   *
   * `overrides` передаётся целиком, а не разобранным на политику и список
   * разделов: расчёт пути принадлежит ядру (принцип I), и разбор настроек в
   * TS был бы вторым местом, знающим это правило.
   */
  planEmbed(
    notePath: string,
    noteText: string,
    boardPath: string,
    overrides: VaultOverrides,
  ): Promise<EmbedPlan>;
  embedMarkup(plan: EmbedPlan): Promise<string>;

  // ── Шаблоны и сниппеты (фича 003) ───────────────────────────────────

  /**
   * Состав библиотеки заготовок и состояние папки `templates/`
   * (FR-004, FR-007, FR-009).
   *
   * Библиотека — проекция того же индекса заметок, что питает дерево, а не
   * отдельный реестр: заметка попадает в галерею независимо от того, как она
   * там оказалась, и продолжает участвовать в поиске и графе ссылок.
   */
  templateLibrary(): Promise<TemplateLibraryView>;

  /** Содержимое заготовки для применения — создания заметки или вставки. */
  readTemplate(path: string): Promise<string>;

  /**
   * Превью одной заготовки (FR-011, FR-012, FR-014).
   *
   * Кэш живёт в ядре и ключуется хэшем содержимого: повторный вопрос про ту же
   * заготовку файл не перечитывает, а правка снаружи пересчитывает превью
   * сама. Момент вызова выбирает галерея — она одна знает, какие карточки
   * видны (research.md §4).
   */
  templatePreview(template: Template): Promise<PreviewState>;

  /** Проверка имени будущей заготовки до любых изменений на диске (FR-029). */
  checkTemplateName(name: string): Promise<NameCheck>;

  /**
   * Сохраняет содержимое как заготовку (FR-002, FR-028, FR-030).
   *
   * `confirmOverwrite` передаётся ядру, а не проверяется здесь: правило
   * коллизии живёт в ядре, иначе между проверкой и записью появилась бы гонка.
   */
  saveAsTemplate(
    name: string,
    content: string,
    confirmOverwrite: boolean,
  ): Promise<Template>;

  /**
   * Какую правку совершить над текстом, чтобы вставить заготовку
   * (FR-031…FR-035).
   *
   * Правку, а не готовый текст: одна отмена обязана возвращать вставленное
   * целиком, не обнуляя историю предыдущих правок, и это возможно только если
   * редактор выполнит ровно одну точечную транзакцию.
   */
  planInsertion(
    textLength: number,
    point: InsertionPoint,
    content: string,
  ): Promise<InsertionEdit>;

  // ── Параметры элементов Typst (фича 008) ─────────────────────────────
  //
  // Чистые функции ядра: на web — статические методы wasm, на desktop и
  // Android — команды Tauri. Формы запроса и ответа одни (FR-022).

  /**
   * Схема элемента по ключу из ядра (`call:figure`) — для правки найденного
   * элемента. Параметры ядро читает из самого Typst.
   */
  elementSchema(id: string): Promise<ElementSpec | undefined>;
  /**
   * Схема элемента, который вставляет шаблон каталога (текст без полей
   * сниппета); `undefined` — не элемент с параметрами, вставляется сразу
   * (FR-003).
   */
  elementSchemaFor(template: string): Promise<ElementSpec | undefined>;
  /** Элемент, непосредственно содержащий каретку (FR-006, FR-016). */
  elementAt(text: string, offset: number): Promise<ElementAtResult>;
  /**
   * Оглавление заметки (фича 015, contracts/frontend-outline.md). Что считать
   * заголовком, какие строки видны, какой раздел текущий и как свёрнутость
   * переживает правку, решает ядро (`structure`); клиент только передаёт
   * вопрос. `Outline` держит окно и возвращает при каждом вопросе.
   */
  outline(text: string): Promise<Outline>;
  outlineView(outline: Outline, request: OutlineRequest): Promise<OutlineView>;
  /** Ключ текущего раздела; `null` — каретка выше первого заголовка или скрыта. */
  outlineActive(outline: Outline, request: OutlineRequest): Promise<string | null>;
  outlineCarry(prev: Outline, next: Outline, collapsed: string[]): Promise<string[]>;
  /**
   * Сессия подсветки Typst для одного редактора (фича 024): ядро держит
   * разобранный текст и обновляет его правками. Закрывает сессию редактор.
   */
  openHighlight(text: string): HighlightSession;
  /** «Будет вставлено» (FR-004, FR-005). */
  renderElement(request: RenderRequest): Promise<RenderOutcome>;
  /** Точечные замены для правки на месте (FR-008, FR-009). */
  planElementEdit(request: EditRequest): Promise<EditOutcome>;

  /**
   * Затрагивает ли событие файла библиотеку заготовок (FR-003).
   *
   * Спрашивается у ядра, а не выводится сверкой пути с «templates/» здесь:
   * правило «что считается заготовкой» обязано жить в одном месте.
   */
  templateEvent(event: VaultEvent): Promise<TemplateEvent | undefined>;

  // ── Настройки уровня vault и разделы (фича 006) ─────────────────────

  /**
   * Настройки, принадлежащие открытому vault: политика расположения файлов
   * вставок, помеченные разделы и перекрытия значений устройства
   * (FR-002, FR-015, FR-026).
   *
   * Поле со значением `undefined` означает «спросить устройство», а не
   * «значение по умолчанию» (FR-030).
   */
  vaultSettings(): Promise<VaultOverrides>;

  /** Записывает настройки уровня vault (FR-029). */
  setVaultSettings(overrides: VaultOverrides): Promise<void>;

  /**
   * Помечает папку как корень раздела и снимает пометку (FR-013).
   *
   * Возвращает обновлённый набор настроек: дереву нужно новое состояние, и
   * второй запрос за тем, что ядро уже посчитало, был бы лишним кругом через
   * границу.
   */
  markSection(folder: string): Promise<VaultOverrides>;
  unmarkSection(folder: string): Promise<VaultOverrides>;

  // ── Сочетания клавиш ────────────────────────────────────────────────

  /**
   * Действующие сочетания всех команд: умолчание ядра, если пользователь не
   * задал своего.
   *
   * Чистая функция от уже прочитанных настроек, но через клиента: на desktop и
   * Android wasm не загружается вовсе, и «разобрать сочетание» там умеет только
   * нативная сторона. Разбирать их вторично в TypeScript нельзя — это второй
   * источник истины о том, что такое сочетание (принцип I).
   *
   * `mac` решает только подпись (⌘ против Ctrl); само сочетание от платформы не
   * зависит, иначе файл настроек стал бы непереносимым между машинами.
   */
  keybindings(overrides: VaultOverrides, mac: boolean): Promise<ResolvedBinding[]>;

  /**
   * Назначает команде сочетание, либо снимает его при `null`.
   *
   * Снятое сочетание — не то же, что умолчание: команда остаётся в палитре, но
   * клавишами не вызывается. Для возврата к умолчанию есть `resetKeybinding`.
   */
  setKeybinding(command: CommandId, shortcut: Shortcut | null): Promise<VaultOverrides>;

  /** Возвращает команду к умолчанию ядра. */
  resetKeybinding(command: CommandId): Promise<VaultOverrides>;

  // ── Вкладки и разделение редактора (фича 014) ───────────────────────
  //
  // Модель вкладок — в ядре (`packages/core/src/workspace`): окно держит
  // значение состояния и отдаёт его сюда вместе с действием. Правил здесь нет.
  // Ширина окна идёт в ядро, потому что подачу (две группы, одна полоса, список)
  // выбирает оно, а не интерфейс.

  workspaceEmpty(): Promise<WorkspaceState>;
  workspaceApply(
    state: WorkspaceState,
    action: WorkspaceAction,
    widthPx: number,
  ): Promise<Transition>;
  workspaceView(state: WorkspaceState, widthPx: number): Promise<WorkspaceView>;
  workspaceSnapshot(state: WorkspaceState): Promise<SavedWorkspace>;
  workspaceRestore(saved: SavedWorkspace): Promise<WorkspaceState>;

  // ── Связи заметки (фича 013) ────────────────────────────────────────

  /** Обратные ссылки и упоминания: одна поверхность на всех оболочках (FR-020). */
  readonly links: LinksClient;

  // ── Закладки (фича 032) ──────────────────────────────────────────

  /** Закладки vault: порядок, группы и доступность целей — в ядре. */
  readonly bookmarks: BookmarksClient;

  // ── Встраивание заметок (фича 031) ───────────────────────────────

  /** «Встроить заметку»: библиотека в vault, `#import` и `#embed("")` у каретки. */
  readonly embeds: EmbedsClient;

  // ── Проверка орфографии (фича 030) ──────────────────────────────────

  /** Настройки проверки, словарь vault и подсказки: правила — в ядре. */
  readonly spelling: SpellingClient;

  // ── Снимки заметок (фича 029) ───────────────────────────────────────

  /** Снимки, история, возврат и удалённые заметки: правила — в ядре. */
  readonly snapshots: SnapshotsClient;

  // ── Показ слайдов (фича 019) ────────────────────────────────────────

  /** Колода показа: чистые функции ядра над сведениями о страницах сборки. */
  readonly slides: SlidesClient;

  // ── Свойства заметок и выборки (фича 020) ───────────────────────────

  /** Свойства: разбор и правка записи, отбор, блоки выборки, пересчёт индекса. */
  readonly properties: PropertiesClient;

  // ── Быстрый захват и ежедневные заметки (фича 022) ──────────────────

  /** Захват, «Сегодня» и соседние дни: правила — в `packages/core/src/capture`. */
  readonly capture: CaptureClient;

  // ── Импорт из Obsidian (фича 021) ───────────────────────────────────

  /** Пакетный импорт папки Markdown: источник только для чтения, шаги по одной заметке. */
  readonly import: ImportClient;

  // ── Вложения (фича 016) ─────────────────────────────────────────────

  /** Пачка вложений: правила — в `packages/core/src/attachments`. */
  readonly attachments: AttachmentOps;

  // ── Источники (фича 017) ─────────────────────────────────────────

  /** Библиотека источников и индекс цитат: правила — в `packages/core/src/sources`. */
  readonly sources: SourcesOps;
}

/**
 * Вложения из буфера и перетаскиванием (фича 016,
 * contracts/frontend-attachments.md). Значение пачки держит UI и передаёт его
 * туда и обратно; вызовы с vault на web идут через очередь вызовов wasm.
 */
export interface AttachmentOps {
  start(
    overrides: VaultOverrides,
    notePath: string,
    items: IncomingItem[],
    moment: InsertMoment,
  ): Promise<AttachmentBatchState>;
  next(state: AttachmentBatchState): Promise<AttachmentBatchStep>;
  confirm(
    state: AttachmentBatchState,
    index: number,
    accept: boolean,
  ): Promise<AttachmentBatchState>;
  store(
    state: AttachmentBatchState,
    index: number,
    bytes: Uint8Array,
  ): Promise<AttachmentBatchState>;
  sourceFailed(state: AttachmentBatchState, index: number): Promise<AttachmentBatchState>;
  cancel(state: AttachmentBatchState): Promise<AttachmentBatchState>;
  view(state: AttachmentBatchState): Promise<AttachmentBatchView>;
  planMarkup(
    textLength: number,
    point: InsertionPoint,
    before: string,
    after: string,
    paths: string[],
    block: boolean,
  ): Promise<InsertionEdit | undefined>;
  /** Нормальная форма названия папки; отказ — `AttachmentsFolderProblem`. */
  validateFolder(raw: string): Promise<string>;
}

/**
 * Куда указывает выбранная исходная папка импорта: хендл браузера (web) либо
 * путь (рабочий стол, Android). Оболочка выбирает папку своим жестом, а клиент
 * vault открывает её тем способом, который у него есть.
 */
export type ImportSourceTarget =
  | { kind: "handle"; handle: FileSystemDirectoryHandle }
  | { kind: "path"; path: string };

/**
 * Импорт из Obsidian (фича 021, contracts/core-import-api.md).
 *
 * Перевод, имена, ссылки, журнал и отчёт — в ядре (`packages/core/src/import`).
 * Клиент держит открытый источник и передаёт шаги через свою границу: wasm в том
 * же процессе или команды Tauri. Ход пакета ведёт окно (`useImport`): звать
 * `step`, пока `next < total`, уступая UI-потоку; отмена — перестать звать и
 * вызвать `finish`. Отказ — значение `ImportError` из ядра (`{ kind }`).
 */
export interface ImportClient {
  /** Открыть исходную папку только для чтения; прежняя закрывается. */
  open(target: ImportSourceTarget, name: string): Promise<void>;
  /** Закрыть источник (в конце пакета или при отказе от диалога). */
  close(): Promise<void>;
  /** C1, C4: что в источнике и можно ли продолжить прежний импорт. */
  survey(job: ImportJob): Promise<ImportSurvey>;
  /** План пакета; продолжение по журналу ядро находит само. */
  begin(job: ImportJob): Promise<ImportState>;
  /** Один файл. `SessionLost` — план потерян, нужен новый `begin`. */
  step(state: ImportState): Promise<ImportState>;
  /** Журнал и заметка отчёта; путь заметки. Зовётся и после отмены. */
  finish(state: ImportState): Promise<string>;
}

/** Ход и отмена пересчёта индекса свойств (принцип X, фича 020). */
export interface ReindexControl {
  onProgress?: (done: number, total: number) => void;
  signal?: AbortSignal;
}

/**
 * Свойства заметок и выборки (фича 020, contracts/core-properties-api.md).
 *
 * Разбор записи, её правка, отбор заметок и блоки `#select` — функции ядра
 * (`packages/core/src/properties`). Клиент только передаёт вопрос через свою
 * границу: wasm в том же процессе или команда Tauri.
 */
export interface PropertiesClient {
  /** Свойства заметки по тексту (буфер редактора, а не файл). */
  read(text: string): Promise<NoteProperties>;
  /** План правки записи; отказ неоднозначной записи — `Refused` с `Properties`. */
  edit(text: string, change: PropertyEdit): Promise<EditOutcome>;
  /** Блоки `#select(…)` в тексте. */
  selectBlocks(text: string): Promise<SelectBlock[]>;
  /** Правка со строкой `#import` библиотеки выборки или пустой список. */
  selectImportEdits(text: string): Promise<TextEdit[]>;
  /** Эталонный файл библиотеки выборки. */
  selectLibrary(): Promise<SelectLibrary>;
  /** Условия из строки запроса (`тег:проект`) и остаток текста. */
  parseConditions(text: string, knownKeys: string[]): Promise<ParsedConditions>;
  /** Отбор заметок по выборке. */
  select(selection: Selection): Promise<SelectionResult>;
  /** Ключи свойств vault и виды их значений. */
  keys(): Promise<PropertyKeyInfo[]>;
  /** Построен ли индекс свойств по текущему формату. */
  current(): Promise<boolean>;
  /** Пакетный пересчёт индекса; отмена оставляет кэш согласованным. */
  reindex(control?: ReindexControl): Promise<ReindexOutcome>;
  /** Кладёт `.typsidian/lib/select.typ` в vault, если его нет. */
  ensureSelectLibrary(): Promise<LibraryStatus>;
  /** Отпечаток входов блоков заметки; `undefined` — блоков нет. */
  selectDigest(text: string): Promise<string | undefined>;
}

/**
 * Колода показа слайдов (фича 019, contracts/core-slides-api.md).
 *
 * Состояние показа держит окно редактора и передаёт его сюда вместе с
 * действием: что значит «дальше», где граница колоды и какой слайд стоит под
 * кареткой, решает ядро (`packages/core/src/slides`). Окно показа ядра не
 * вызывает. Пустая колода — отказ `SlidesError` (`{ kind: "EmptyDeck" }`).
 */
export interface SlidesClient {
  /** Начало показа: страница под кареткой (UTF-16) либо первая. */
  start(
    slides: PageSlideInfo[],
    caret: number | undefined,
    nowMs: number,
  ): Promise<ShowState>;
  navigate(state: ShowState, slides: PageSlideInfo[], action: ShowAction): Promise<ShowState>;
  /** Перенос номера через пересборку (FR-007). */
  carry(state: ShowState, slides: PageSlideInfo[]): Promise<ShowState>;
  /** Место в заметке для страницы: куда поставить каретку на выходе. */
  caret(slides: PageSlideInfo[], page: number): Promise<SlidePlace | undefined>;
  label(slides: PageSlideInfo[], page: number): Promise<SlideLabel>;
  keymap(): Promise<ShowKeymap>;
}

/**
 * Закладки (фича 032). Каждая правка отдаёт новый вид; отказ — исключение
 * со значением `BookmarkError`.
 */
export interface BookmarksClient {
  view(): Promise<BookmarksView>;
  add(target: BookmarkTarget, name: string | null): Promise<AddOutcome>;
  remove(id: string): Promise<BookmarksView>;
  rename(id: string, name: string): Promise<BookmarksView>;
  move(id: string, place: BookmarkPlace): Promise<BookmarksView>;
  groupCreate(name: string): Promise<BookmarksView>;
  groupRename(id: string, name: string): Promise<BookmarksView>;
  groupToggle(id: string): Promise<BookmarksView>;
  groupDelete(id: string): Promise<BookmarksView>;
}

/**
 * Встраивание заметок (фича 031). Правки — в координатах исходного текста,
 * одной транзакцией редактора; отказ записи библиотеки — исключение.
 */
export interface EmbedsClient {
  insert(text: string, offset: number): Promise<EmbedInsertion>;
}

/**
 * Проверка орфографии (фича 030, contracts/core-spellcheck-api.md). Отказ
 * правки словаря — исключение со значением `SpellError`.
 */
export interface SpellingClient {
  /** Перечитать настройки vault и словарь vault; на web — и загрузить словари. */
  configure(): Promise<SpellConfig>;
  addWord(word: string): Promise<SpellConfig>;
  removeWord(word: string): Promise<SpellConfig>;
  /** Исправления слова, до пяти. */
  suggest(word: string): Promise<string[]>;
}

/**
 * Локальные снимки заметок (фича 029). Пора ли снимать, срок хранения,
 * различия, возврат и восстановление решает ядро (`snapshots`); клиент зовёт
 * его в нужные моменты. Время берёт сама граница — часы устройства.
 */
export interface SnapshotsClient {
  /** Снимок заметки, если пора; пропуск и отказ — штатные ответы. */
  take(path: string, text: string, reason: SnapshotReason): Promise<TakeOutcome>;
  /** История заметки, новые первыми. */
  history(path: string): Promise<NoteHistory>;
  /** Различия снимка с текстом; пусто — снимка нет. */
  diff(id: string, against: string): Promise<DiffLine[]>;
  text(id: string): Promise<string | null>;
  /** Вернуть снимок; `editorText` — текст открытой заметки, тогда ответ — правка редактора. */
  restore(path: string, id: string, editorText: string | null): Promise<RestoreOutcome>;
  deleted(): Promise<DeletedNote[]>;
  recover(path: string, asPath: string | null): Promise<RecoverOutcome>;
  usage(): Promise<SnapshotUsage>;
  /** Удалить все снимки vault; ответ — сколько удалено. */
  clear(): Promise<number>;
}

/**
 * Связи заметки: кто на неё ссылается и где стоит её название без ссылки
 * (фича 013, contracts/note-panel.md).
 *
 * Правил здесь нет: что считается ссылкой и упоминанием, как строятся фрагменты
 * и порядок источников, решает ядро (`packages/core/src/links`). Клиент только
 * передаёт вопрос через свою границу — wasm в том же процессе или команда Tauri.
 *
 * `openTexts` — тексты заметок, открытых в редакторе: места и фрагменты для них
 * считаются по буферу, а не по диску (research.md §10).
 */
export interface LinksClient {
  /** Порция обратных ссылок; источники ближайшие по папке — первыми. */
  backlinksPage(
    target: string,
    openTexts: OpenText[],
    offset: number,
    limit: number,
  ): Promise<BacklinksPage>;
  /** Можно ли искать упоминания названия цели (FR-011). */
  mentionsAvailability(target: string): Promise<MentionsAvailability>;
  /** Начало прохода по vault; отказ — штатный ответ `Unavailable`, а не ошибка. */
  mentionsStart(target: string): Promise<MentionsStart>;
  /** Одна порция прохода: новое состояние и найденные группы. */
  mentionsStep(state: MentionScan, openTexts: OpenText[]): Promise<MentionStep>;
  /** Отмена прохода: после неё `mentionsStep` ничего не читает (FR-012). */
  mentionsCancel(state: MentionScan): Promise<MentionScan>;
  /** Упоминания только в названных заметках — точечное обновление (research.md §8). */
  mentionsIn(target: string, paths: string[], openTexts: OpenText[]): Promise<SourceGroup[]>;
  /**
   * «Связать»: превратить упоминание в ссылку. `editorText` — текст заметки в
   * редакторе, если она в нём открыта; тогда ядро вернёт правку для редактора,
   * а на диск ничего не запишет (FR-016).
   */
  linkMention(request: LinkRequest, editorText: string | null): Promise<LinkOutcome>;
  /** Битые ссылки по целям и заметки без входящих одним вызовом (фича 028). */
  health(openTexts: OpenText[]): Promise<LinkHealth>;
  /**
   * Починка ссылок одной заметки (фича 028): исходы — как у замены 027,
   * отмена — `replaceUndo`.
   */
  fix(request: LinkFixRequest, editorText: string | null): Promise<ReplaceNoteOutcome>;
}

/**
 * Общая часть `flushAll`/`registerFlusher` для обеих реализаций клиента:
 * список редакторов и порядок «дописать всё → спросить ядро, осталось ли
 * несохранённое». Вопрос к ядру у реализаций свой, поэтому приходит аргументом.
 */
export function createFlushRegistry(
  unresolvedEditPaths: () => Promise<string[]>,
): Pick<VaultClient, "flushAll" | "registerFlusher"> {
  const flushers = new Set<() => Promise<void>>();
  return {
    registerFlusher(flush) {
      flushers.add(flush);
      return () => {
        flushers.delete(flush);
      };
    },
    async flushAll() {
      // Отказ одного редактора не мешает дописать остальные: итог всё равно
      // определяет ядро — по тому, что осталось несохранённым.
      await Promise.allSettled([...flushers].map((flush) => flush()));
      return (await unresolvedEditPaths()).length === 0;
    },
  };
}

/**
 * Быстрый захват и ежедневные заметки (фича 022, contracts/core-capture-api.md).
 *
 * Вид записи, путь по дате, соседние дни и решение «правка буфера или запись
 * файла» — функции ядра (`packages/core/src/capture`). Клиент только передаёт
 * вопрос через свою границу. Настройки `open_daily`, `daily_neighbours` и
 * `write` ядро читает из `vault.json` само: окно держит копию, которая может
 * устареть. Чистые функции получают `CaptureSettings` из `settings`.
 */
export interface CaptureClient {
  /** Действующие настройки: умолчания применены к перекрытиям vault. */
  settings(overrides: VaultOverrides): Promise<CaptureSettings>;
  /** Запись захвата словами Typst — образец в настройках и сама запись. */
  entry(settings: CaptureSettings, capture: Capture, imagePath?: string): Promise<string>;
  /** Заметка назначения захвата; `undefined` — путь не годится. */
  targetPath(settings: CaptureSettings, capture: Capture): Promise<string | undefined>;
  dailyPath(settings: CaptureSettings, date: DailyDate): Promise<string | undefined>;
  /** Дата ежедневной заметки по пути; `undefined` — это не ежедневная заметка. */
  dailyDateOf(settings: CaptureSettings, path: string): Promise<DailyDate | undefined>;
  /**
   * Записать захват. Заметка открыта в редакторе — передать её полный текст в
   * `openBuffer`, ответ будет правкой буфера (`EditBuffer`); иначе ядро пишет
   * файл (`Written`). Отказ — значение `Failed`, а не исключение.
   */
  write(capture: Capture, openBuffer?: string, image?: Uint8Array): Promise<CaptureOutcome>;
  /** «Сегодня»: создаёт заметку по шаблону при отсутствии. Отказ — `DailyError`. */
  openDaily(date: DailyDate): Promise<OpenDaily>;
  dailyNeighbours(date: DailyDate): Promise<DailyNeighbours>;
}

/**
 * Источники vault (фича 017, contracts/frontend-sources.md). Состояние —
 * в ядре по обе стороны границы; здесь только вызовы.
 */
export interface SourcesOps {
  refresh(registered: string[]): Promise<RefreshStep>;
  summary(): Promise<LibrarySummary>;
  search(query: string, limit: number, uncitedOnly: boolean): Promise<SourceHits>;
  /** Для дополнения по `@` (018). */
  forCompletion(prefix: string, limit: number): Promise<SourceHit[]>;
  scanStart(): Promise<CitationScanView>;
  scanStep(maxNotes: number): Promise<CitationScanView>;
  scanCancel(): Promise<CitationScanView>;
  noteChanged(path: string): Promise<void>;
  noteRemoved(path: string): Promise<void>;
  noteRenamed(from: string, to: string): Promise<void>;
  citing(key: string): Promise<CitingNote[]>;
  missingKeys(): Promise<MissingKey[]>;
  checkNote(text: string, knownLabels: string[]): Promise<NoteCheck>;
  /** Отказ — `CitationPlanError` значением, а не исключением. */
  planCitation(
    text: string,
    caret: number,
    file: string,
    key: string,
  ): Promise<{ ok: true; plan: CitationPlan } | { ok: false; error: CitationPlanError }>;
  bibliographyOffer(text: string, notePath: string, file: string): Promise<BibliographyOffer>;
  createFile(): Promise<{ ok: true; path: string } | { ok: false; error: CreateSourceFileError }>;
}
