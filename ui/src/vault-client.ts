import type {
  AccessState,
  Board,
  BoardEvent,
  DiffLine,
  EditConflict,
  Embed,
  EmbedPlan,
  EmbedRef,
  FileKind,
  FileStamp,
  Note,
  VaultEvent,
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
  planEmbed(
    notePath: string,
    noteText: string,
    boardPath: string,
  ): Promise<EmbedPlan>;
  embedMarkup(plan: EmbedPlan): Promise<string>;
}
