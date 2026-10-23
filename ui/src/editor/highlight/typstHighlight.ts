import { ChangeSet, RangeSetBuilder, StateEffect, type Extension, type Text } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";
import type { HighlightEdit, HighlightWindow } from "../../wasm-core/typsidian_core.js";
import type { HighlightSession, VaultClient } from "../../vault-client.js";
import { classesOf } from "./roles.js";

// Подсветка Typst (фича 024, contracts/frontend-highlight.md).
//
// Роли считает ядро — тем же разбором, которым собирает заметку (принцип I).
// Ядро держит сессию: разобранный текст редактора, который обновляется
// правками, а не разбирается заново на каждое нажатие (research.md §2). Здесь
// только расписание запросов, правки и перевод ответа в декорации. Ввод ядра
// не ждёт: пока ответ в пути, прежняя раскраска сдвигается вместе с правкой, а
// пришедший ответ доводится до текущего текста теми же правками (FR-008).

type Client = Pick<VaultClient, "openHighlight">;

/** Сколько строк вокруг видимой части красить заранее (research.md §2). */
const MARGIN_LINES = 50;

/** Пустой эффект: ответ пришёл, виду пора перерисоваться. */
const answered = StateEffect.define<null>();

/** Ошибка вызова пишется один раз за сеанс: на каждое нажатие — шум. */
let reported = false;

/** Классы фрагмента дорогие только в первый раз: значений немного. */
const marks = new Map<number, Decoration>();

function markOf(packed: number): Decoration {
  let mark = marks.get(packed);
  if (mark === undefined) {
    mark = Decoration.mark({ class: classesOf(packed) });
    marks.set(packed, mark);
  }
  return mark;
}

/** Видимые диапазоны, расширенные до строк и ещё на `MARGIN_LINES` в стороны. */
function windowsOf(view: EditorView): HighlightWindow[] {
  const doc = view.state.doc;
  // До первого измерения видимых диапазонов нет, а пустой список окон ядро
  // читает как «весь текст» — для заметки на тысячи строк это лишняя работа.
  const ranges = view.visibleRanges.length > 0 ? view.visibleRanges : [view.viewport];
  return ranges.map(({ from, to }) => {
    const first = Math.max(1, doc.lineAt(from).number - MARGIN_LINES);
    const last = Math.min(doc.lines, doc.lineAt(to).number + MARGIN_LINES);
    return { from: doc.line(first).from, to: doc.line(last).to };
  });
}

/**
 * Правки для ядра: каждая — в тексте после предыдущих. `iterChanges` даёт их
 * слева направо в координатах исходного текста; справа налево эти координаты
 * остаются верными после каждой применённой правки.
 */
function editsOf(changes: ChangeSet): HighlightEdit[] {
  const edits: HighlightEdit[] = [];
  changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
    edits.push({ from: fromA, to: toA, insert: inserted.toString() });
  });
  return edits.reverse();
}

function decorationsOf(packed: Uint32Array, doc: Text): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const length = doc.length;
  for (let at = 0; at + 2 < packed.length; at += 3) {
    const from = packed[at] ?? 0;
    const to = Math.min(packed[at + 1] ?? 0, length);
    if (from >= to) continue;
    builder.add(from, to, markOf(packed[at + 2] ?? 0));
  }
  return builder.finish();
}


// ── Орфография (фича 030) ───────────────────────────────────────────────────
//
// Проверка идёт по той же сессии ядра, что и раскраска: ядро знает границу
// текста и разметки (research.md §2). Здесь только расписание — через паузу
// после правки или прокрутки, когда сессия совпадает с текстом, — и
// подчёркивания. Ввод проверки не ждёт (SC-002).

/** Пауза после правки или прокрутки до проверки (research.md §6). */
const SPELL_DELAY_MS = 400;

const misspelledMark = Decoration.mark({ class: "ts-misspelled" });

/** Подчёркнутые слова вида — меню редактора ищет в них слово под щелчком. */
const spellMarks = new WeakMap<EditorView, DecorationSet>();

/** Виды с проверкой — для `refreshSpelling`. */
const spellViews = new Set<{ respell(): void }>();

/**
 * Перепроверить все открытые редакторы: настройки проверки или словарь vault
 * поменялись (US2, US3).
 */
export function refreshSpelling(): void {
  for (const view of spellViews) view.respell();
}

/** Подчёркнутое слово в позиции `pos`; `null` — там нет ошибки. */
export function misspelledAt(view: EditorView, pos: number): { from: number; to: number } | null {
  let found: { from: number; to: number } | null = null;
  spellMarks.get(view)?.between(pos, pos, (from, to) => {
    found = { from, to };
    return false;
  });
  return found;
}

function spellDecorationsOf(pairs: Uint32Array, doc: Text): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  for (let at = 0; at + 1 < pairs.length; at += 2) {
    const from = pairs[at] ?? 0;
    const to = Math.min(pairs[at + 1] ?? 0, doc.length);
    if (from < to) builder.add(from, to, misspelledMark);
  }
  return builder.finish();
}
export function typstHighlight(client: Client): Extension {
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet = Decoration.none;
      private session: HighlightSession;
      /** Правки, которых сессия ещё не видела, — от её текста к текущему. */
      private unsent: ChangeSet;
      /** Правки с начала запроса в пути; `null` — запроса нет. */
      private since: ChangeSet | null = null;
      /** Текст менялся или вид сдвинулся, пока запрос был в пути. */
      private stale = false;
      /** Запрос назначен на ближайшую микрозадачу, но ещё не ушёл. */
      private queued = false;
      private destroyed = false;
      /** Подчёркивания орфографии (030). */
      spelling: DecorationSet = Decoration.none;
      /** Правки с начала запроса проверки в пути; `null` — запроса нет. */
      private spellSince: ChangeSet | null = null;
      private spellTimer: ReturnType<typeof setTimeout> | null = null;
      private readonly spellHandle = { respell: () => this.armSpelling(0) };

      constructor(readonly view: EditorView) {
        const { doc } = view.state;
        this.session = client.openHighlight(doc.toString());
        this.unsent = ChangeSet.empty(doc.length);
        // Синхронная сессия красит заметку до первой отрисовки (SC-003).
        if (this.session.updateNow !== undefined) this.applyNow();
        else this.schedule();
        spellViews.add(this.spellHandle);
        this.armSpelling(0);
      }

      update(update: ViewUpdate): void {
        if (update.docChanged) {
          this.decorations = this.decorations.map(update.changes);
          this.unsent = this.unsent.compose(update.changes);
          if (this.since !== null) this.since = this.since.compose(update.changes);
          this.spelling = this.spelling.map(update.changes);
          spellMarks.set(this.view, this.spelling);
          if (this.spellSince !== null) this.spellSince = this.spellSince.compose(update.changes);
        }
        if (update.docChanged || update.viewportChanged) this.armSpelling(SPELL_DELAY_MS);
        if (!update.docChanged && !update.viewportChanged) return;
        if (this.session.updateNow !== undefined) this.applyNow();
        else this.schedule();
      }

      /**
       * Раскраска в самом обновлении вида — для ядра в том же потоке (web).
       * Ждать микрозадачу там значит пропускать вперёд перерисовку React,
       * которую будит та же правка: на заметке в 5 000 строк это несколько
       * миллисекунд до раскраски (quickstart §4). Поток это не нагружает —
       * wasm и так работает в нём.
       */
      private applyNow(): void {
        const now = this.session.updateNow;
        if (now === undefined) return;
        const { doc } = this.view.state;
        const windows = windowsOf(this.view);
        let packed: Uint32Array | undefined;
        try {
          packed = now(editsOf(this.unsent), doc.length, windows);
          this.unsent = ChangeSet.empty(doc.length);
          if (packed === undefined) {
            // Сессия разошлась с редактором: новая с текущим текстом.
            this.session.close();
            this.session = client.openHighlight(doc.toString());
            packed = this.session.updateNow?.([], doc.length, windows);
          }
        } catch (error) {
          if (!reported) {
            reported = true;
            console.warn("typsidian: syntax highlighting is unavailable", error);
          }
        }
        if (packed !== undefined) this.decorations = decorationsOf(packed, doc);
      }

      /**
       * Не больше одного запроса в пути; правки за время запроса — один
       * следующий. Ждать кадра незачем: ответ wasm приходит в той же задаче, и
       * раскраска успевает к той же отрисовке, что и набранный символ (SC-002).
       * Микрозадача — чтобы несколько правок одной транзакции дали один запрос
       * и чтобы `dispatch` ответа не попал внутрь текущего обновления вида.
       */
      private schedule(): void {
        if (this.since !== null) {
          this.stale = true;
          return;
        }
        if (this.queued) return;
        this.queued = true;
        queueMicrotask(() => {
          this.queued = false;
          if (!this.destroyed) void this.request();
        });
      }

      private async request(): Promise<void> {
        const { doc } = this.view.state;
        const windows = windowsOf(this.view);
        const edits = editsOf(this.unsent);
        this.unsent = ChangeSet.empty(doc.length);
        this.since = ChangeSet.empty(doc.length);
        this.stale = false;
        let packed: Uint32Array | undefined;
        let failed = false;
        try {
          packed = await this.session.update(edits, doc.length, windows);
        } catch (error) {
          failed = true;
          if (!reported) {
            reported = true;
            console.warn("typsidian: syntax highlighting is unavailable", error);
          }
        }
        if (this.destroyed) return;
        const since = this.since;
        this.since = null;
        if (packed === undefined) {
          // Сессия разошлась с редактором или вызов упал и правки могли не
          // дойти: новая сессия с текущим текстом, раскраска — следующим
          // запросом. После сбоя повтор — только по новой правке, без цикла.
          this.session.close();
          this.session = client.openHighlight(this.view.state.doc.toString());
          this.unsent = ChangeSet.empty(this.view.state.doc.length);
          if (!failed || this.stale) this.schedule();
          return;
        }
        // Ответ на текст, который с тех пор правили, всё равно лучше прежней
        // раскраски: правки, пришедшие за время запроса, сдвигают его так же,
        // как сдвигали старые декорации.
        this.decorations = decorationsOf(packed, doc).map(since);
        this.view.dispatch({ effects: answered.of(null) });
        if (this.stale) this.schedule();
      }

      /**
       * Проверка через `delay` мс. Пока сессия не догнала редактор (правки не
       * отправлены или раскраска в пути), ждём ещё — ответ проверки должен
       * относиться к тексту сессии.
       */
      private armSpelling(delay: number): void {
        if (this.spellTimer !== null) clearTimeout(this.spellTimer);
        this.spellTimer = setTimeout(() => {
          this.spellTimer = null;
          if (this.destroyed) return;
          if (!this.unsent.empty || this.since !== null || this.spellSince !== null) {
            this.armSpelling(SPELL_DELAY_MS / 4);
            return;
          }
          void this.requestSpelling();
        }, delay);
      }

      private async requestSpelling(): Promise<void> {
        const { doc } = this.view.state;
        this.spellSince = ChangeSet.empty(doc.length);
        let pairs: Uint32Array | undefined;
        try {
          pairs = await this.session.spelling(windowsOf(this.view));
        } catch {
          pairs = undefined;
        }
        if (this.destroyed) return;
        const since = this.spellSince;
        this.spellSince = null;
        if (pairs === undefined) return;
        this.spelling = spellDecorationsOf(pairs, doc).map(since);
        spellMarks.set(this.view, this.spelling);
        this.view.dispatch({ effects: answered.of(null) });
      }

      destroy(): void {
        this.destroyed = true;
        spellViews.delete(this.spellHandle);
        if (this.spellTimer !== null) clearTimeout(this.spellTimer);
        spellMarks.delete(this.view);
        this.session.close();
      }
    },
    {
      decorations: (plugin) => plugin.decorations,
      provide: (plugin) =>
        EditorView.decorations.of((view) => view.plugin(plugin)?.spelling ?? Decoration.none),
    },
  );
}

const perClient = new WeakMap<Client, Extension>();

/**
 * Одно расширение на клиента: редакторов много (вкладки, половины разделения),
 * а новая ссылка на расширение пересобирает его в каждом виде.
 */
export function typstHighlightFor(client: Client): Extension {
  let extension = perClient.get(client);
  if (extension === undefined) {
    extension = typstHighlight(client);
    perClient.set(client, extension);
  }
  return extension;
}
