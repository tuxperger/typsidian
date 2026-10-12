import { useCallback, useEffect, useRef, useState } from "react";
import type { MoveTarget } from "../wasm-core/typsidian_core.js";

// Жест переноса в дереве (фича 023, research.md §8).
//
// Pointer Events, а не HTML5 Drag and Drop: тот не даёт долгого нажатия на
// сенсорном экране, не даёт своего ярлыка и в webview Tauri перехватывается
// системным приёмом файлов. Один механизм на три оболочки.
//
// Что допустимо, жест не решает: на захвате он один раз спрашивает ядро о
// папках, которые могут принять элемент, и дальше только проверяет
// принадлежность множеству (FR-012). Подсветка цели и положение ярлыка
// ставятся прямо в DOM, мимо React: дерево в тысячи строк не должно
// перерисовываться на каждое движение указателя (SC-006).

/** Параметры жеста — из макета; это не правила предметной области. */
const MOUSE_SLOP_PX = 4;
const TOUCH_SLOP_PX = 8;
const TOUCH_HOLD_MS = 400;
const EXPAND_DELAY_MS = 600;
const EDGE_PX = 28;
const MAX_SCROLL_PX_PER_FRAME = 14;

const DROP_FOLDER_CLASS = "ts-tree__folder--drop";
const DROP_ROOT_CLASS = "ts-tree--drop-root";
const SOURCE_CLASS = "ts-tree__row--dragging";
const GHOST_DENIED_CLASS = "ts-tree-drag--denied";

export type DragKind = "note" | "board" | "folder";

/** Что переносится — читается из `data-drag-*` строки дерева. */
export interface DragSource {
  path: string;
  name: string;
  kind: DragKind;
  /** Сколько файлов поедет вместе с папкой; для файла — `null`. */
  count: number | null;
  touch: boolean;
}

/** Над чем сейчас указатель — для объявления читалке экрана (FR-026). */
export type DragOver =
  | { kind: "none" }
  | { kind: "folder"; folder: string }
  | { kind: "root" }
  | { kind: "denied" };

export interface TreeDragOptions {
  /** Жест недоступен: идёт другая операция дерева или перенос. */
  disabled: boolean;
  /** Папки, которые могут принять элемент, — ответ ядра. */
  loadTargets: (from: string) => Promise<MoveTarget[]>;
  /** Vault недоступен на запись — жест не начинается, причина показывается (FR-006). */
  checkWritable: () => Promise<boolean>;
  onDenied: () => void;
  onDrop: (source: DragSource, toFolder: string) => void;
  /** Раскрыть свёрнутую папку под задержавшимся указателем (FR-013). */
  onExpand: (folder: string) => void;
}

interface Session {
  pointerId: number;
  touch: boolean;
  startX: number;
  startY: number;
  x: number;
  y: number;
  source: DragSource;
  sourceEl: HTMLElement;
  handleEl: HTMLElement;
  dragging: boolean;
  holdTimer: number | null;
  allowed: Set<string> | null;
  target: string | null;
  highlightEl: Element | null;
  expandTimer: number | null;
  expandFolder: string | null;
  frame: number | null;
}

function readSource(row: HTMLElement, touch: boolean): DragSource | null {
  const path = row.dataset["dragPath"];
  const name = row.dataset["dragName"];
  const kind = row.dataset["dragKind"];
  if (path === undefined || name === undefined) return null;
  if (kind !== "note" && kind !== "board" && kind !== "folder") return null;
  const count = row.dataset["dragCount"];
  return { path, name, kind, count: count === undefined ? null : Number(count), touch };
}

export function useTreeDrag({
  disabled,
  loadTargets,
  checkWritable,
  onDenied,
  onDrop,
  onExpand,
}: TreeDragOptions) {
  const containerRef = useRef<HTMLElement | null>(null);
  const ghostRef = useRef<HTMLDivElement | null>(null);
  const sessionRef = useRef<Session | null>(null);
  const [active, setActive] = useState<DragSource | null>(null);
  const [over, setOver] = useState<DragOver>({ kind: "none" });

  // Обработчики окна живут дольше одной отрисовки — свежие колбэки читаются
  // через ref.
  const optionsRef = useRef({ loadTargets, checkWritable, onDenied, onDrop, onExpand });
  optionsRef.current = { loadTargets, checkWritable, onDenied, onDrop, onExpand };

  const placeGhost = useCallback((session: Session) => {
    const ghost = ghostRef.current;
    if (ghost === null) return;
    ghost.style.transform = `translate(${session.x + 14}px, ${session.y + 10}px)`;
  }, []);

  const clearHighlight = useCallback((session: Session) => {
    session.highlightEl?.classList.remove(DROP_FOLDER_CLASS, DROP_ROOT_CLASS);
    session.highlightEl = null;
  }, []);

  const clearExpand = useCallback((session: Session) => {
    if (session.expandTimer !== null) window.clearTimeout(session.expandTimer);
    session.expandTimer = null;
    session.expandFolder = null;
  }, []);

  /** Цель под указателем: папка → она, заметка → её папка, пустое место → корень (FR-011). */
  const retarget = useCallback(
    (session: Session) => {
      const container = containerRef.current;
      if (container === null) return;
      const hit = document.elementFromPoint(session.x, session.y);
      const inside = hit !== null && container.contains(hit);
      const zone = inside ? hit.closest<HTMLElement>("[data-drop-folder]") : null;
      const folder = zone?.dataset["dropFolder"] ?? null;
      const allowed = folder !== null && session.allowed !== null && session.allowed.has(folder);
      const target = allowed ? folder : null;

      // Подсвечивается папка целиком: у строки заметки зона — её папка.
      let highlight: Element | null = null;
      if (target !== null && zone !== null) {
        highlight =
          target === ""
            ? container
            : zone.matches(".ts-tree__folder")
              ? zone
              : zone.closest(".ts-tree__folder");
      }
      if (highlight !== session.highlightEl) {
        clearHighlight(session);
        highlight?.classList.add(target === "" ? DROP_ROOT_CLASS : DROP_FOLDER_CLASS);
        session.highlightEl = highlight;
      }

      const denied = inside && target === null && session.allowed !== null;
      ghostRef.current?.classList.toggle(GHOST_DENIED_CLASS, denied);
      container.style.cursor = denied ? "not-allowed" : "grabbing";

      if (target !== session.target || denied) {
        session.target = target;
        setOver(
          target === null
            ? { kind: denied ? "denied" : "none" }
            : target === ""
              ? { kind: "root" }
              : { kind: "folder", folder: target },
        );
      }

      // Свёрнутая папка под задержавшимся указателем раскрывается (FR-013).
      const collapsed = inside ? hit.closest<HTMLElement>("[data-folder-collapsed]") : null;
      const collapsedPath = collapsed?.dataset["dropFolder"] ?? null;
      if (collapsedPath !== session.expandFolder) {
        clearExpand(session);
        if (collapsedPath !== null) {
          session.expandFolder = collapsedPath;
          session.expandTimer = window.setTimeout(() => {
            session.expandTimer = null;
            optionsRef.current.onExpand(collapsedPath);
          }, EXPAND_DELAY_MS);
        }
      }
    },
    [clearExpand, clearHighlight],
  );

  /** Прокрутка у краёв дерева, пока указатель в полосе (FR-013). */
  const autoScroll = useCallback(
    (session: Session) => {
      const container = containerRef.current;
      if (container === null || !session.dragging) return;
      const rect = container.getBoundingClientRect();
      let delta = 0;
      if (session.x >= rect.left && session.x <= rect.right) {
        const top = session.y - rect.top;
        const bottom = rect.bottom - session.y;
        if (top >= 0 && top < EDGE_PX) delta = -Math.ceil(((EDGE_PX - top) / EDGE_PX) * MAX_SCROLL_PX_PER_FRAME);
        else if (bottom >= 0 && bottom < EDGE_PX)
          delta = Math.ceil(((EDGE_PX - bottom) / EDGE_PX) * MAX_SCROLL_PX_PER_FRAME);
      }
      if (delta !== 0) {
        const before = container.scrollTop;
        container.scrollTop = before + delta;
        // Под неподвижным указателем после прокрутки оказалась другая строка.
        if (container.scrollTop !== before) retarget(session);
      }
      session.frame = window.requestAnimationFrame(() => autoScroll(session));
    },
    [retarget],
  );

  const finish = useCallback(
    (drop: boolean) => {
      const session = sessionRef.current;
      if (session === null) return;
      sessionRef.current = null;
      if (session.holdTimer !== null) window.clearTimeout(session.holdTimer);
      if (session.frame !== null) window.cancelAnimationFrame(session.frame);
      clearExpand(session);
      clearHighlight(session);
      session.sourceEl.classList.remove(SOURCE_CLASS);
      const container = containerRef.current;
      if (container !== null) container.style.cursor = "";
      if (!session.dragging) return;
      setActive(null);
      setOver({ kind: "none" });
      if (drop && session.target !== null) {
        optionsRef.current.onDrop(session.source, session.target);
      } else {
        // Отмена возвращает фокус туда, откуда элемент взяли (FR-014).
        session.handleEl.focus({ preventScroll: true });
      }
    },
    [clearExpand, clearHighlight],
  );

  const begin = useCallback(
    (session: Session) => {
      session.dragging = true;
      session.sourceEl.classList.add(SOURCE_CLASS);
      setActive(session.source);
      setOver({ kind: "none" });
      window.getSelection()?.removeAllRanges();
      session.frame = window.requestAnimationFrame(() => {
        placeGhost(session);
        autoScroll(session);
      });
      const { loadTargets: load, checkWritable: writable, onDenied: denied } = optionsRef.current;
      void writable().then((ok) => {
        if (sessionRef.current !== session) return;
        if (!ok) {
          finish(false);
          denied();
        }
      });
      void load(session.source.path).then(
        (targets) => {
          if (sessionRef.current !== session) return;
          session.allowed = new Set(targets.map((target) => target.path));
          retarget(session);
        },
        () => {
          if (sessionRef.current === session) finish(false);
        },
      );
    },
    [autoScroll, finish, placeGhost, retarget],
  );

  // Слушатели окна — пока идёт жест или его ожидание. Вешаются один раз:
  // состояние жеста живёт в ref.
  useEffect(() => {
    const onMove = (event: PointerEvent): void => {
      const session = sessionRef.current;
      if (session === null || event.pointerId !== session.pointerId) return;
      session.x = event.clientX;
      session.y = event.clientY;
      if (!session.dragging) {
        const moved = Math.hypot(session.x - session.startX, session.y - session.startY);
        if (session.touch) {
          // Палец поехал до порога удержания — это прокрутка, не перенос (SC-008).
          if (moved > TOUCH_SLOP_PX) finish(false);
          return;
        }
        if (moved < MOUSE_SLOP_PX) return;
        begin(session);
      }
      placeGhost(session);
      retarget(session);
    };
    const onUp = (event: PointerEvent): void => {
      const session = sessionRef.current;
      if (session === null || event.pointerId !== session.pointerId) return;
      const wasDragging = session.dragging;
      finish(true);
      if (!wasDragging) return;
      // Отпускание после переноса не должно стать кликом по строке под
      // указателем — иначе бросок открывал бы заметку.
      const swallow = (click: MouseEvent): void => {
        click.preventDefault();
        click.stopPropagation();
      };
      window.addEventListener("click", swallow, { capture: true, once: true });
      window.setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 0);
    };
    const onCancel = (event: PointerEvent): void => {
      if (sessionRef.current?.pointerId === event.pointerId) finish(false);
    };
    const onKey = (event: KeyboardEvent): void => {
      if (event.key !== "Escape" || sessionRef.current?.dragging !== true) return;
      event.preventDefault();
      event.stopPropagation();
      finish(false);
    };
    const onBlur = (): void => finish(false);
    // На сенсорном экране начавшийся жест не должен прокручивать страницу и
    // вызывать системное меню долгого нажатия.
    const onTouchMove = (event: TouchEvent): void => {
      if (sessionRef.current?.dragging === true && event.cancelable) event.preventDefault();
    };
    const onContextMenu = (event: MouseEvent): void => {
      if (sessionRef.current?.dragging !== true) return;
      event.preventDefault();
      event.stopPropagation();
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onCancel);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", onBlur);
    window.addEventListener("touchmove", onTouchMove, { passive: false });
    window.addEventListener("contextmenu", onContextMenu, true);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onCancel);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("touchmove", onTouchMove);
      window.removeEventListener("contextmenu", onContextMenu, true);
      finish(false);
    };
  }, [begin, finish, placeGhost, retarget]);

  /**
   * Вешается на контейнер дерева. Жест начинают только основные кнопки строк
   * (`data-drag-handle`): кнопка «⋯», поле переименования и меню — нет (FR-015).
   */
  const onPointerDown = useCallback(
    (event: React.PointerEvent<HTMLElement>) => {
      if (disabled || sessionRef.current !== null) return;
      if (event.pointerType === "mouse" && event.button !== 0) return;
      if (!(event.target instanceof Element)) return;
      const handle = event.target.closest<HTMLElement>("[data-drag-handle]");
      const row = handle?.closest<HTMLElement>("[data-drag-path]") ?? null;
      if (handle === null || handle === undefined || row === null) return;
      const touch = event.pointerType === "touch";
      const source = readSource(row, touch);
      if (source === null) return;
      const session: Session = {
        pointerId: event.pointerId,
        touch,
        startX: event.clientX,
        startY: event.clientY,
        x: event.clientX,
        y: event.clientY,
        source,
        sourceEl: row,
        handleEl: handle,
        dragging: false,
        holdTimer: null,
        allowed: null,
        target: null,
        highlightEl: null,
        expandTimer: null,
        expandFolder: null,
        frame: null,
      };
      sessionRef.current = session;
      if (touch) {
        session.holdTimer = window.setTimeout(() => {
          session.holdTimer = null;
          if (sessionRef.current !== session) return;
          begin(session);
          placeGhost(session);
          retarget(session);
        }, TOUCH_HOLD_MS);
      }
    },
    [begin, disabled, placeGhost, retarget],
  );

  return { containerRef, ghostRef, onPointerDown, active, over };
}

export type TreeDragController = ReturnType<typeof useTreeDrag>;
