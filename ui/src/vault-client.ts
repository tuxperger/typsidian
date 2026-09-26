import type {
  AccessState,
  Board,
  BoardEvent,
  BoardWriteProfile,
  CommandId,
  DiffLine,
  EditConflict,
  Embed,
  EmbedPlan,
  EmbedRef,
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
  NameCheck,
  Note,
  PreviewState,
  ResolvedBinding,
  SearchQuery,
  SearchResults,
  Shortcut,
  Template,
  TemplateEvent,
  TemplateLibraryView,
  VaultEvent,
  VaultOverrides,
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
  backlinks(path: string): Promise<Note[]>;
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
   * Чем является файл по пути (фича 002, FR-004).
   *
   * Для файлов, уже попавших в дерево, спрашивать не нужно: вид лежит в
   * `Note.kind`. Этот вызов — для пути, которого в дереве ещё нет, например
   * имени создаваемого файла.
   */
  fileKind(path: string): Promise<FileKind>;

  // ── Несохранённые правки и конфликты (001 US4) ──────────────────────

  beginEdit(path: string, buffer: string): Promise<void>;
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

  /** Схема элемента каталога; `undefined` — вставляется сразу (FR-003). */
  elementSchema(id: string): Promise<ElementSpec | undefined>;
  /** Элемент, непосредственно содержащий каретку (FR-006, FR-016). */
  elementAt(text: string, offset: number): Promise<ElementAtResult>;
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
}
