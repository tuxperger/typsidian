import type { EditorState, StateEffect } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import type { TabId, TextPlace } from "../wasm-core/typsidian_core.js";

// Состояние редактора вкладки (фича 014, FR-004, research.md §4).
//
// `EditorView` существует только у видимой вкладки группы. Уходя с вкладки,
// редактор отдаёт сюда `EditorState` (каретка, выделение, своя история отмены) и
// прокрутку; вернувшись, поднимается из снимка. Каретка и прокрутка принадлежат
// редактору, а не модели вкладок в ядре, поэтому живут здесь.
//
// Полных снимков не больше `FULL_SNAPSHOTS`: у вытесненной вкладки остаются
// числа — смещения каретки и прокрутка, — документ же при необходимости
// перечитывается. Так память не растёт с числом вкладок.

/** Сколько вкладок держат полный `EditorState`. */
export const FULL_SNAPSHOTS = 20;

export interface ScrollPosition {
  /** Эффект восстановления прокрутки CodeMirror (`EditorView.scrollSnapshot`). */
  effect: StateEffect<unknown>;
  top: number;
}

export interface TabEditorEntry {
  state: EditorState | null;
  scroll: ScrollPosition | null;
  anchor: number;
  head: number;
  scrollTop: number;
  line: number;
  column: number;
}

function placeOf(state: EditorState): { line: number; column: number } {
  const line = state.doc.lineAt(state.selection.main.head);
  return { line: line.number, column: state.selection.main.head - line.from + 1 };
}

function entryOf(state: EditorState, scroll: ScrollPosition | null): TabEditorEntry {
  const { line, column } = placeOf(state);
  return {
    state,
    scroll,
    anchor: state.selection.main.anchor,
    head: state.selection.main.head,
    scrollTop: scroll?.top ?? 0,
    line,
    column,
  };
}

class TabEditorStates {
  /** Порядок вставки — порядок давности: свежая последней. */
  private readonly entries = new Map<TabId, TabEditorEntry>();
  /** Живые виды: каретка активной вкладки читается с них, а не из снимка. */
  private readonly live = new Map<TabId, () => EditorView | null>();

  /** Снимок вкладки, если он есть. Не удаляет его: вкладку могут открыть снова. */
  take(tab: TabId): TabEditorEntry | null {
    return this.entries.get(tab) ?? null;
  }

  /** Редактор вкладки уходит: запоминаем состояние и прокрутку. */
  store(tab: TabId, state: EditorState, scroll: ScrollPosition | null): void {
    this.entries.delete(tab);
    this.entries.set(tab, entryOf(state, scroll));
    this.evict();
  }

  /** Вид вкладки живёт: `caretOf` читает его, пока он не освободился. */
  bindLive(tab: TabId, get: () => EditorView | null): () => void {
    this.live.set(tab, get);
    return () => {
      if (this.live.get(tab) === get) this.live.delete(tab);
    };
  }

  /** Где каретка вкладки (строка и колонка с единицы) — место, с которого уходят. */
  caretOf(tab: TabId): TextPlace | null {
    const view = this.live.get(tab)?.() ?? null;
    if (view !== null) {
      const { line, column } = placeOf(view.state);
      return { line, column };
    }
    const entry = this.entries.get(tab);
    return entry === undefined ? null : { line: entry.line, column: entry.column };
  }

  /** Вкладка закрыта: снимок не нужен. */
  drop(tab: TabId): void {
    this.entries.delete(tab);
  }

  /** Разделение и перенос сохраняют каретку (US3 AS2): новая вкладка начинает с места старой. */
  copy(from: TabId, to: TabId): void {
    const view = this.live.get(from)?.() ?? null;
    if (view !== null) {
      this.entries.delete(to);
      this.entries.set(
        to,
        entryOf(view.state, { effect: view.scrollSnapshot(), top: view.scrollDOM.scrollTop }),
      );
      this.evict();
      return;
    }
    const entry = this.entries.get(from);
    if (entry === undefined) return;
    this.entries.delete(to);
    this.entries.set(to, { ...entry });
    this.evict();
  }

  /** Освободить всё: vault сменился. */
  clear(): void {
    this.entries.clear();
  }

  private evict(): void {
    let full = 0;
    for (const entry of this.entries.values()) if (entry.state !== null) full += 1;
    if (full <= FULL_SNAPSHOTS) return;
    // Самые давние теряют полный снимок, числа остаются.
    for (const entry of this.entries.values()) {
      if (full <= FULL_SNAPSHOTS) break;
      if (entry.state === null) continue;
      entry.state = null;
      entry.scroll = null;
      full -= 1;
    }
  }
}

/** Одно хранилище на окно: `TabId` уникален в пределах состояния окна. */
export const tabEditorStates = new TabEditorStates();
