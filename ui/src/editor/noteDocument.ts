import { ChangeSet, Text } from "@codemirror/state";
import type { ReadError, SnapshotReason } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import type { Phrase } from "../i18n/index.js";

// Документ на путь (фича 014, research.md §3, contracts/frontend-workspace.md).
//
// Раньше текст заметки, чтение с диска и автосохранение жили внутри компонента
// `NoteEditor`: смонтированный редактор — один таймер, один буфер. С вкладками
// это ломается дважды: заметка в двух группах давала бы два таймера и две записи,
// а заметка в неактивной вкладке — ни одной (SC-004). Поэтому всё это принадлежит
// документу: на путь один документ, один поток `beginEdit`, одно автосохранение,
// один `registerFlusher`. Виды (CodeMirror в группах) только показывают текст и
// отдают правки.
//
// Принцип I: правила не изменились и остались за ядром. Документ не решает, когда
// запись допустима, — признак «есть неразрешённый конфликт» он спрашивает у ядра
// (`pendingConflict`), запись отдаёт тому же `write`. Своё здесь только ритм:
// пауза после последней правки.

/**
 * Пауза от последнего нажатия клавиши до записи файла.
 *
 * Тот же порядок, что у автосохранения досок (`BOARD_AUTOSAVE_DEBOUNCE_MS`), но
 * короче: заметка — это текст, его сериализовать не нужно, а «сохранено» в
 * статус-баре должно появляться в темпе набора, а не через секунду после него.
 * Писать по каждому нажатию нельзя — это файл на диск на каждую букву.
 */
export const NOTE_AUTOSAVE_DEBOUNCE_MS = 600;

/**
 * Пауза, через которую ядро узнаёт новую версию несохранённого буфера (005
 * T064). Первая правка объявляется сразу — с этого мига внешнее изменение
 * не применяется молча (FR-035); следующие нажатия сводятся в один вызов:
 * буфер целиком на каждый символ — лишние мегабайты через очередь вызовов
 * wasm или IPC на длинной заметке.
 */
export const EDIT_ANNOUNCE_DEBOUNCE_MS = 150;

/**
 * Что показывает статус-бар редактора.
 *
 * Форма — как у `BoardSaveState` из ядра, но тип свой: у заметки есть
 * состояние, которого у доски нет. `Held` — «ядро держит конфликт по этому
 * файлу, и до решения пользователя автосохранение молчит»; у досок тот же гейт
 * стоит внутри ядра (`boards::store::write_board`) и наружу приходит отказом.
 */
export type NoteSaveState =
  | { state: "Clean" }
  | { state: "Dirty" }
  | { state: "Saving" }
  | { state: "Held" }
  | { state: "Failed"; reason: Phrase };

export type DocumentReadState = "loading" | "ready" | ReadError;

/** Вид документа — один `EditorView` в группе. Документу он нужен для рассылки чужих правок. */
export interface DocumentView {
  /**
   * Правка пришла от соседнего вида того же документа (или заменён весь текст).
   * Вид применяет её, не записывая в свою историю отмены. `whole` — текст заменён
   * целиком: каретка не должна уехать за конец нового текста.
   */
  receive(changes: ChangeSet, whole: boolean): void;
  /** Текст вида целиком — для сверки, если вид и документ разошлись. */
  text(): string;
}

/** Снимок для React: меняется ссылка, когда меняется текст или состояние. */
export interface DocumentSnapshot {
  readonly text: string;
  readonly read: DocumentReadState;
  readonly save: NoteSaveState;
  /** Растёт с каждым изменением: удобно как ключ эффектов. */
  readonly revision: number;
}

export interface NoteDocument {
  readonly path: string;
  text(): string;
  status(): { read: DocumentReadState; save: NoteSaveState };
  snapshot(): DocumentSnapshot;
  subscribe(listener: () => void): () => void;
  /** Вид объявляет себя, чтобы получать чужие правки. Возвращает отмену. */
  attach(view: DocumentView): () => void;
  /** Правка из вида: `beginEdit`, автосохранение, рассылка остальным видам. */
  edit(from: DocumentView, changes: ChangeSet): void;
  /**
   * Принятая внешняя версия, вставка доски, возврат версии. `saved = true` —
   * текст уже на диске (его оттуда и прочитали), писать его второй раз нельзя;
   * `false` — обычная правка, она уйдёт в файл автосохранением.
   */
  replace(text: string, saved: boolean): void;
  /** Перечитать файл. Документ с несохранёнными правками не трогается. */
  reload(): Promise<void>;
  /** Записать немедленно, не дожидаясь паузы. */
  flush(): Promise<void>;
  /** Есть ли что терять: несохранённое, идущая запись, конфликт. */
  isDirty(): boolean;
}

/** Куда документ сообщает о важном для вкладок и колонки заметки. */
export interface NoteDocumentHooks {
  /** Запись удалась: заметка на диске изменилась (013 — колонка заметки обновляется). */
  onSaved?: (path: string) => void;
  /** Документ перешёл «чисто ↔ несохранённое» — вкладки перечитывают признаки. */
  onDirtyChange?: (path: string, dirty: boolean) => void;
  /** Состояние поменялось — реестр решает, не выгрузить ли документ. */
  onSettled?: (document: NoteDocument) => void;
  /** Повод для снимка заметки (фича 029): решает ли ядро его делать — его дело. */
  onSnapshot?: (path: string, text: string, reason: SnapshotReason) => void;
}

const LINE_BREAK = /\r\n?|\n/;

function toText(text: string): Text {
  return Text.of(text.split(LINE_BREAK));
}

/** Тот же перевод строк, что делает CodeMirror: сверка с диском идёт по нему. */
function normalize(text: string): string {
  return text.replace(/\r\n?/g, "\n");
}

/** Причина отказа записи словами ядра (001 FR-009). */
function describeWriteFailure(cause: unknown): Phrase {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    switch ((cause as { kind: unknown }).kind) {
      case "Conflict":
        return (t) => t("editor.save.conflict");
      case "AccessDenied":
        return (t) => t("editor.save.accessDenied");
      case "Io":
        return (t) => t("editor.save.io");
      default:
        break;
    }
  }
  return (t) => t("editor.save.failed");
}

/** Сужение `unknown` до `ReadError` без `any` (принцип IV). */
function asReadError(cause: unknown): ReadError {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    const { kind } = cause as { kind: unknown };
    if (kind === "NotFound" || kind === "TooLarge" || kind === "Io") {
      return { kind } as ReadError;
    }
  }
  return { kind: "Io" };
}

/** Реализация документа. Создаётся реестром, снаружи — только интерфейс. */
export class NoteDocumentImpl implements NoteDocument {
  path: string;
  /** Сколько держателей (виды и потребители текста) сейчас используют документ. */
  holders = 0;

  private readonly client: VaultClient;
  private readonly hooks: NoteDocumentHooks;
  private readonly views = new Set<DocumentView>();
  private readonly listeners = new Set<() => void>();
  private doc: Text = Text.of([""]);
  private current: DocumentSnapshot = {
    text: "",
    read: "loading",
    save: { state: "Clean" },
    revision: 0,
  };
  /** Что уже лежит на диске: по нему видно, есть ли что писать. */
  private saved: string | null = null;
  /** Последнее содержимое буфера — его читают таймер и `flush`. */
  private buffer: string | null = null;
  /** Ядру объявлена правка, которую ещё не сняла запись или `discardEdit`. */
  private announced = false;
  private timer: number | null = null;
  /** Объявление новой версии буфера ядру, отложенное до паузы набора. */
  private announceTimer: number | null = null;
  private disposed = false;
  private loading: Promise<void> | null = null;
  private readonly unregisterFlusher: () => void;

  constructor(client: VaultClient, path: string, hooks: NoteDocumentHooks) {
    this.client = client;
    this.path = path;
    this.hooks = hooks;
    // Снимок истории делается только с сохранённого (011 FR-006): клиент обязан
    // уметь дописать этот буфер по требованию, не дожидаясь паузы.
    this.unregisterFlusher = client.registerFlusher(() => this.flush());
  }

  // ── Чтение ────────────────────────────────────────────────────────────

  /** Первое чтение с диска. Повторный вызов ждёт то же чтение. */
  load(): Promise<void> {
    this.loading ??= this.readFromDisk(false);
    return this.loading;
  }

  private async readFromDisk(replaceViews: boolean): Promise<void> {
    const path = this.path;
    try {
      const text = normalize(await this.client.read(path));
      if (this.disposed) return;
      if (replaceViews) {
        // Перечитывание: если пока читали, пользователь успел начать печатать,
        // его правка важнее — устаревший диск поверх неё не кладётся.
        if (this.isDirty()) return;
        if (text === this.doc.toString()) {
          this.saved = text;
          this.buffer = text;
          return;
        }
        this.setText(text, true);
      } else {
        this.doc = toText(text);
      }
      // Прочитанное с диска — не правка: говорится, что именно этот текст сейчас
      // в файле, иначе первое же открытие заметки перезаписывало бы её тем же.
      this.saved = text;
      // Снимок (029 FR-001): исходное состояние при открытии, новое — после
      // внешнего изменения файла.
      this.hooks.onSnapshot?.(path, text, replaceViews ? "External" : "Open");
      this.buffer = text;
      this.publish({ text: this.doc.toString(), read: "ready", save: { state: "Clean" } });
    } catch (cause) {
      if (this.disposed) return;
      if (!replaceViews) this.publish({ read: asReadError(cause) });
    }
  }

  async reload(): Promise<void> {
    if (this.current.read !== "ready" || this.isDirty()) return;
    await this.readFromDisk(true);
  }

  // ── Чтение состояния ──────────────────────────────────────────────────

  text(): string {
    return this.current.text;
  }

  status(): { read: DocumentReadState; save: NoteSaveState } {
    return { read: this.current.read, save: this.current.save };
  }

  snapshot(): DocumentSnapshot {
    return this.current;
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  isDirty(): boolean {
    return this.current.save.state !== "Clean";
  }

  get viewCount(): number {
    return this.views.size;
  }

  private publish(patch: Partial<Omit<DocumentSnapshot, "revision">>): void {
    const before = this.isDirty();
    this.current = { ...this.current, ...patch, revision: this.current.revision + 1 };
    for (const listener of [...this.listeners]) listener();
    const after = this.isDirty();
    if (before !== after) this.hooks.onDirtyChange?.(this.path, after);
    this.hooks.onSettled?.(this);
  }

  private setSave(next: NoteSaveState): void {
    this.publish({ save: next });
  }

  // ── Виды ──────────────────────────────────────────────────────────────

  attach(view: DocumentView): () => void {
    this.views.add(view);
    return () => {
      this.views.delete(view);
      this.hooks.onSettled?.(this);
    };
  }

  edit(from: DocumentView, changes: ChangeSet): void {
    if (changes.empty) return;
    if (changes.length !== this.doc.length) {
      // Вид и документ разошлись (не должно случаться): правда — у вида, который
      // только что принял правку пользователя; остальным уходит весь текст.
      this.setText(from.text(), true, from);
      this.changed(this.doc.toString());
      return;
    }
    this.doc = changes.apply(this.doc);
    for (const view of this.views) {
      if (view !== from) view.receive(changes, false);
    }
    this.changed(this.doc.toString());
  }

  replace(text: string, saved: boolean): void {
    this.setText(text, true);
    if (saved) {
      this.cancelTimer();
      this.saved = this.doc.toString();
      this.buffer = this.saved;
      this.announced = false;
      this.publish({ text: this.doc.toString(), save: { state: "Clean" } });
    } else {
      this.changed(this.doc.toString());
    }
  }

  /** Заменяет текст документа целиком и рассылает замену видам (кроме `except`). */
  private setText(text: string, whole: boolean, except?: DocumentView): void {
    const old = this.doc;
    this.doc = toText(text);
    const changes = ChangeSet.of({ from: 0, to: old.length, insert: this.doc }, old.length);
    for (const view of this.views) {
      if (view !== except) view.receive(changes, whole);
    }
  }

  // ── Автосохранение ────────────────────────────────────────────────────

  /** Объявить новую версию буфера — она уйдёт в файл после паузы. */
  private changed(next: string): void {
    this.buffer = next;
    if (next === this.saved) {
      // Буфер вернулся к сохранённому тексту: ядру больше нечего считать
      // несохранённым, иначе вкладку нельзя было бы закрыть (research.md §6).
      this.cancelTimer();
      this.cancelAnnounce();
      if (this.announced) {
        this.announced = false;
        void this.client.discardEdit(this.path).catch(() => undefined);
      }
      this.publish({ text: next, save: { state: "Clean" } });
      return;
    }
    // FR-035: ядро должно знать о несохранённых правках и держать сам буфер —
    // иначе внешнее изменение будет применено молча.
    if (!this.announced) {
      this.announced = true;
      this.cancelAnnounce();
      void this.client.beginEdit(this.path, next).catch(() => undefined);
    } else if (this.announceTimer === null) {
      this.announceTimer = window.setTimeout(() => {
        this.announceTimer = null;
        // К этому мигу буфер мог уйти вперёд — объявляется последний.
        const latest = this.buffer;
        if (this.disposed || latest === null || latest === this.saved) return;
        void this.client.beginEdit(this.path, latest).catch(() => undefined);
      }, EDIT_ANNOUNCE_DEBOUNCE_MS);
    }
    this.cancelTimer();
    this.timer = window.setTimeout(() => {
      this.timer = null;
      void this.save();
    }, NOTE_AUTOSAVE_DEBOUNCE_MS);
    const save = this.current.save;
    this.publish({ text: next, save: save.state === "Dirty" ? save : { state: "Dirty" } });
  }

  private cancelTimer(): void {
    if (this.timer !== null) {
      window.clearTimeout(this.timer);
      this.timer = null;
    }
  }

  private cancelAnnounce(): void {
    if (this.announceTimer !== null) {
      window.clearTimeout(this.announceTimer);
      this.announceTimer = null;
    }
  }

  private async save(): Promise<void> {
    const buffer = this.buffer;
    const path = this.path;
    if (this.disposed || buffer === null || buffer === this.saved) return;

    // Пока ядро держит неразрешённый конфликт по этой заметке, запись молчит:
    // выбор версии принадлежит пользователю (001 FR-021), и автосохранение «на
    // всякий случай» стёрло бы внешнюю правку прежде, чем он её увидел.
    try {
      if ((await this.client.pendingConflict(path)) !== undefined) {
        this.setSave({ state: "Held" });
        return;
      }
    } catch {
      // Спросить не удалось — считаем, что конфликт возможен, и не пишем:
      // ошибиться в эту сторону значит отложить запись, в другую — потерять
      // чужой текст.
      this.setSave({ state: "Held" });
      return;
    }

    // Запись покрывает отложенное объявление: файл и так станет этим буфером.
    this.cancelAnnounce();
    this.setSave({ state: "Saving" });
    try {
      await this.client.write(path, buffer, undefined);
      this.saved = buffer;
      this.announced = false;
      this.hooks.onSaved?.(path);
      this.hooks.onSnapshot?.(path, buffer, "Save");
      // Пока шла запись, могли успеть набрать ещё: тогда остаётся «есть
      // несохранённое», а не «сохранено».
      this.setSave(this.buffer === buffer ? { state: "Clean" } : { state: "Dirty" });
    } catch (cause) {
      this.setSave({ state: "Failed", reason: describeWriteFailure(cause) });
    }
  }

  async flush(): Promise<void> {
    this.cancelTimer();
    await this.save();
  }

  /** Выгрузка: таймер и регистрация. Вызывается реестром, когда документ никому не нужен. */
  dispose(): void {
    // Снимок при закрытии (029 FR-001): то, что лежит на диске.
    if (this.current.read === "ready" && this.saved !== null) {
      this.hooks.onSnapshot?.(this.path, this.saved, "Close");
    }
    this.disposed = true;
    this.cancelTimer();
    this.cancelAnnounce();
    this.unregisterFlusher();
    this.listeners.clear();
    this.views.clear();
  }
}
