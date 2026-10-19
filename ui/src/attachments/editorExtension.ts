import { StateEffect, StateField, type EditorState, type Extension } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  WidgetType,
  type DecorationSet,
} from "@codemirror/view";
import type { InsertionPoint } from "../wasm-core/typsidian_core.js";

// Вставка вложений в редактор заметки (фича 016, contracts/frontend-attachments.md).
//
// Здесь только маршрутизация событий DOM — кому достанется `paste` или `drop`
// (research.md §3) — и место вставки. Что из файлов изображение, куда его
// положить и какой текст вставить, решает ядро.

/** Где окажется вставка, когда копирование закончится. */
export interface TrackedPlace {
  /**
   * Точка вставки, перенесённая через все правки с момента броска, и текст
   * строки вокруг неё. `null` — редактора больше нет (заметку закрыли).
   */
  current(): PlaceNow | null;
  /** Редактор, в котором точка живёт; `null` — уничтожен. */
  view(): EditorView | null;
  /** Перестать следить. */
  release(): void;
}

export interface PlaceNow {
  point: InsertionPoint;
  /** Текст строки до точки и после неё — по нему ядро ставит переносы. */
  before: string;
  after: string;
  textLength: number;
}

/** Откуда пришли файлы и куда их вставлять. */
export interface AttachmentDrop {
  /** Камера — только из шторки телефона: имя снимку даёт ядро, как у буфера. */
  origin: "Clipboard" | "File" | "Camera";
  /** По записи на строку: перетаскивание и пачка из нескольких файлов. */
  block: boolean;
  place: TrackedPlace;
}

/** Порт к пачке вложений (`useAttachments`). */
export interface AttachmentPort {
  onFiles(files: File[], drop: AttachmentDrop): void;
  /** Брошена картинка ссылкой, без файла (research.md §15). */
  onLinkDropped(): void;
  /** Подпись индикатора: «N изображений — сюда, строка L». */
  indicatorLabel(count: number, line: number): string;
}

// ── Слежение за позицией ───────────────────────────────────────────────────

const addPlace = StateEffect.define<{ id: number; from: number; to: number }>();
const removePlace = StateEffect.define<number>();

/** Точки ожидающих вставок, переносимые через каждую правку. */
const placesField = StateField.define<ReadonlyMap<number, { from: number; to: number }>>({
  create: () => new Map(),
  update(places, transaction) {
    let next = places;
    if (transaction.docChanged && places.size > 0) {
      const mapped = new Map<number, { from: number; to: number }>();
      for (const [id, range] of places) {
        // Начало — к правке, конец — от неё: набранное на краю выделения
        // остаётся снаружи, как у полей сниппета.
        mapped.set(id, {
          from: transaction.changes.mapPos(range.from, 1),
          to: Math.max(
            transaction.changes.mapPos(range.from, 1),
            transaction.changes.mapPos(range.to, -1),
          ),
        });
      }
      next = mapped;
    }
    for (const effect of transaction.effects) {
      if (effect.is(addPlace)) {
        const copy = new Map(next);
        copy.set(effect.value.id, { from: effect.value.from, to: effect.value.to });
        next = copy;
      } else if (effect.is(removePlace) && next.has(effect.value)) {
        const copy = new Map(next);
        copy.delete(effect.value);
        next = copy;
      }
    }
    return next;
  },
});

/** Редакторы, которые уже уничтожены: их точкам вставлять некуда. */
const destroyed = new WeakSet<EditorView>();
let nextPlaceId = 1;

/** Следить за диапазоном `from…to` (выделение либо точка броска). */
export function trackRange(view: EditorView, from: number, to: number): TrackedPlace {
  const id = nextPlaceId++;
  view.dispatch({ effects: addPlace.of({ id, from, to }) });
  const alive = (): EditorView | null => (destroyed.has(view) ? null : view);
  return {
    view: alive,
    current() {
      const editor = alive();
      if (editor === null) return null;
      const range = editor.state.field(placesField, false)?.get(id);
      if (range === undefined) return null;
      return placeAt(editor.state, range.from, range.to);
    },
    release() {
      const editor = alive();
      if (editor !== null) editor.dispatch({ effects: removePlace.of(id) });
    },
  };
}

/** Следить за кареткой и выделением — вставка командой и из буфера. */
export function trackSelection(view: EditorView): TrackedPlace {
  const range = view.state.selection.main;
  return trackRange(view, range.from, range.to);
}

function placeAt(state: EditorState, from: number, to: number): PlaceNow {
  const startLine = state.doc.lineAt(from);
  const endLine = state.doc.lineAt(to);
  return {
    point: {
      offset: to,
      selection: from === to ? undefined : { start: from, end: to },
    },
    before: state.doc.sliceString(startLine.from, from),
    after: state.doc.sliceString(to, endLine.to),
    textLength: state.doc.length,
  };
}

// ── Индикатор места броска (макет 1a) ──────────────────────────────────────

interface DropMark {
  pos: number;
  label: string;
}

const setDropMark = StateEffect.define<DropMark | null>();

class DropWidget extends WidgetType {
  constructor(readonly label: string) {
    super();
  }

  override eq(other: DropWidget): boolean {
    return other.label === this.label;
  }

  toDOM(): HTMLElement {
    const mark = document.createElement("div");
    mark.className = "ts-attach-drop";
    mark.setAttribute("aria-hidden", "true");
    const tag = document.createElement("span");
    tag.className = "ts-attach-drop__tag";
    tag.textContent = this.label;
    mark.append(tag);
    return mark;
  }
}

const dropField = StateField.define<{ mark: DropMark | null; decorations: DecorationSet }>({
  create: () => ({ mark: null, decorations: Decoration.none }),
  update(value, transaction) {
    let mark = value.mark;
    if (mark !== null && transaction.docChanged) {
      mark = { ...mark, pos: transaction.changes.mapPos(mark.pos) };
    }
    for (const effect of transaction.effects) {
      if (effect.is(setDropMark)) mark = effect.value;
    }
    if (mark === value.mark) return value;
    return {
      mark,
      decorations:
        mark === null
          ? Decoration.none
          : Decoration.set([
              Decoration.widget({
                widget: new DropWidget(mark.label),
                side: -1,
                block: true,
              }).range(mark.pos),
            ]),
    };
  },
  provide: (field) => [
    EditorView.decorations.from(field, (value) => value.decorations),
    // Рамка редактора, пока над ним тащат файлы.
    EditorView.editorAttributes.from(field, (value) =>
      value.mark === null ? {} : { class: "ts-attach-dragging" },
    ),
  ],
});

// ── Обработчики событий ────────────────────────────────────────────────────

/** Начало строки под указателем; ниже последней строки — конец документа. */
function dropPosition(view: EditorView, event: DragEvent): { pos: number; line: number } {
  const at = view.posAtCoords({ x: event.clientX, y: event.clientY });
  if (at === null) {
    const last = view.state.doc.lineAt(view.state.doc.length);
    return { pos: view.state.doc.length, line: last.number };
  }
  const line = view.state.doc.lineAt(at);
  return { pos: line.from, line: line.number };
}

function carriesFiles(event: DragEvent): boolean {
  return Array.from(event.dataTransfer?.types ?? []).includes("Files");
}

/** Сколько файлов тащат: при перетаскивании виден только список элементов. */
function draggedCount(event: DragEvent): number {
  const items = Array.from(event.dataTransfer?.items ?? []);
  return Math.max(1, items.filter((item) => item.kind === "file").length);
}

function clearMark(view: EditorView): void {
  if (view.state.field(dropField).mark !== null) {
    view.dispatch({ effects: setDropMark.of(null) });
  }
}

/** Расширение вставки вложений. Порт берётся по ссылке: он живёт в рабочей области. */
export function attachmentExtension(port: () => AttachmentPort | null): Extension {
  return [
    placesField,
    dropField,
    EditorView.domEventHandlers({
      paste(event, view) {
        const data = event.clipboardData;
        if (data === null) return false;
        const files = Array.from(data.files);
        // Текст в буфере — обычная вставка (FR-002); `text/html` при пустом
        // тексте — копирование картинки со страницы, и это картинка.
        if (data.getData("text/plain") !== "" || files.length === 0) return false;
        const target = port();
        if (target === null) return false;
        event.preventDefault();
        target.onFiles(files, {
          origin: "Clipboard",
          block: files.length > 1,
          place: trackSelection(view),
        });
        return true;
      },
      dragenter(event) {
        if (!carriesFiles(event) || port() === null) return false;
        event.preventDefault();
        return true;
      },
      dragover(event, view) {
        const target = port();
        if (!carriesFiles(event) || target === null) return false;
        event.preventDefault();
        if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
        const { pos, line } = dropPosition(view, event);
        const label = target.indicatorLabel(draggedCount(event), line);
        const mark = view.state.field(dropField).mark;
        if (mark === null || mark.pos !== pos || mark.label !== label) {
          view.dispatch({ effects: setDropMark.of({ pos, label }) });
        }
        return true;
      },
      dragleave(event, view) {
        const into = event.relatedTarget;
        if (!(into instanceof Node) || !view.dom.contains(into)) clearMark(view);
        return false;
      },
      dragend(_event, view) {
        clearMark(view);
        return false;
      },
      drop(event, view) {
        clearMark(view);
        const target = port();
        if (target === null) return false;
        const files = Array.from(event.dataTransfer?.files ?? []);
        if (files.length > 0) {
          event.preventDefault();
          const { pos } = dropPosition(view, event);
          target.onFiles(files, {
            origin: "File",
            block: true,
            place: trackRange(view, pos, pos),
          });
          return true;
        }
        // Картинка, брошенная со страницы ссылкой: скачивать её приложение не
        // берётся ни при какой сети (research.md §15) — только объясняет.
        const html = event.dataTransfer?.getData("text/html") ?? "";
        if (/<img\b/i.test(html)) {
          event.preventDefault();
          target.onLinkDropped();
          return true;
        }
        return false;
      },
    }),
    destroyMarker,
  ];
}

/**
 * Помечает редактор уничтоженным: его точкам вставлять больше некуда, и пачка
 * отвечает «сохранены, но не вставлены» (research.md §10).
 */
const destroyMarker: Extension = ViewPlugin.fromClass(
  class {
    constructor(readonly view: EditorView) {}

    destroy(): void {
      destroyed.add(this.view);
    }
  },
);
