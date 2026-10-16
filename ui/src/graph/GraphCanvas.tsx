"use client";

import { forwardRef, useCallback, useEffect, useImperativeHandle, useMemo, useRef } from "react";
import type {
  GraphGroup,
  GraphNode,
  GroupColor,
  VaultGraph,
} from "../wasm-core/typsidian_core.js";
import type { GraphLayout } from "./useGraphLayout.js";

// Холст графа (фича 025, T013, T023, research.md §7): Canvas 2D с пакетной
// заливкой. Связи — один Path2D на состояние (обычные / выделенные /
// приглушённые), узлы — пакетами по цвету и форме. Положения приходят из воркера
// раскладки; здесь они только рисуются и ищутся указателем по сетке-индексу.

export interface GraphCanvasHandle {
  zoomBy: (factor: number) => void;
  fit: () => void;
}

export interface GraphCanvasProps {
  graph: VaultGraph;
  layout: GraphLayout;
  groups: GraphGroup[];
  arrows: boolean;
  /** Масштаб, с которого видны подписи (0..1 из настроек). */
  labelZoom: number;
  /** Множитель размера узлов (0..1 из настроек). */
  nodeSize: number;
  /** `id` открытой заметки: её узел получает кольцо. */
  currentPath: string | null;
  /** Подписи всегда — режим колонки заметки. */
  alwaysLabels?: boolean;
  /** Смена значения заново вписывает граф в экран. */
  fitKey: string;
  /** Узел с клавиатурной фокусировкой из списка для чтения с экрана. */
  focusedIndex: number | null;
  ariaLabel: string;
  onOpen: (node: GraphNode) => void;
}

const TOKEN_OF_GROUP: Record<GroupColor, string> = {
  orange: "--orange-500",
  blue: "--blue-600",
  green: "--green-600",
  amber: "--amber-600",
  red: "--red-600",
  hivis: "--hivis-500",
  steel: "--steel-300",
};

const CELL = 32;
const TINY_NODE = 2.6;
const EDGE_BUDGET = 2500;
const DIM_ALPHA = 0.2;
const DRAG_THRESHOLD = 4;
const LONG_PRESS_MS = 380;
const MAX_ZOOM_LABELS = 500;
const MANY_VISIBLE = 2000;

interface Palette {
  group: Record<GroupColor, string>;
  plain: string;
  edge: string;
  edgeHot: string;
  edgeMuted: string;
  ring: string;
  hover: string;
  label: string;
  labelCurrent: string;
  font: string;
  fontMono: string;
}

function readPalette(element: Element): Palette {
  const style = getComputedStyle(element);
  const token = (name: string): string => style.getPropertyValue(name).trim() || "currentColor";
  const group = {} as Record<GroupColor, string>;
  for (const name of Object.keys(TOKEN_OF_GROUP) as GroupColor[]) {
    group[name] = token(TOKEN_OF_GROUP[name]);
  }
  return {
    group,
    plain: token("--steel-400"),
    edge: token("--steel-700"),
    edgeHot: token("--orange-500"),
    edgeMuted: token("--steel-800"),
    ring: token("--orange-400"),
    hover: token("--steel-100"),
    label: token("--steel-300"),
    labelCurrent: token("--steel-100"),
    font: token("--font-sans"),
    fontMono: token("--font-mono"),
  };
}

type Shape = "circle" | "square" | "diamond";

function shapeOf(node: GraphNode): Shape {
  if (node.kind === "Tag") return "square";
  if (node.kind === "Attachment") return "diamond";
  return "circle";
}

interface View {
  tx: number;
  ty: number;
  k: number;
}

interface Pointer {
  x: number;
  y: number;
}

export const GraphCanvas = forwardRef<GraphCanvasHandle, GraphCanvasProps>(function GraphCanvas(
  props,
  ref,
) {
  const { graph, layout, groups, arrows, labelZoom, nodeSize, currentPath } = props;
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const propsRef = useRef(props);
  propsRef.current = props;

  // ── Производные данные графа ─────────────────────────────────────────────

  /** Соседи в формате CSR: `offsets[i]..offsets[i + 1]` в `targets`. */
  const adjacency = useMemo(() => {
    const n = graph.nodes.length;
    const offsets = new Uint32Array(n + 1);
    for (const edge of graph.edges) {
      offsets[edge.from + 1]! += 1;
      offsets[edge.to + 1]! += 1;
    }
    for (let i = 0; i < n; i += 1) offsets[i + 1]! += offsets[i]!;
    const fill = offsets.slice(0, n);
    const targets = new Uint32Array(offsets[n]!);
    for (const edge of graph.edges) {
      targets[fill[edge.from]!++] = edge.to;
      targets[fill[edge.to]!++] = edge.from;
    }
    return { offsets, targets };
  }, [graph]);

  const shapes = useMemo(() => graph.nodes.map(shapeOf), [graph]);
  const baseRadius = useMemo(() => {
    const scale = 0.55 + nodeSize * 0.9;
    return Float32Array.from(graph.nodes, (node) => (2.6 + 1.7 * Math.sqrt(node.degree)) * scale);
  }, [graph, nodeSize]);
  const currentIndex = useMemo(
    () => (currentPath === null ? -1 : graph.nodes.findIndex((node) => node.id === currentPath)),
    [graph, currentPath],
  );

  // Всё, что рисование читает, лежит в одном ref: холст не пересоздаётся
  // от каждого рендера, а перерисовка не зависит от замыканий.
  const data = useRef({ adjacency, shapes, baseRadius, currentIndex, groups });
  data.current = { adjacency, shapes, baseRadius, currentIndex, groups };

  // ── Состояние рисования ──────────────────────────────────────────────────

  const state = useRef({
    view: { tx: 0, ty: 0, k: 1 } as View,
    target: null as View | null,
    autoFit: true,
    width: 0,
    height: 0,
    dpr: 1,
    hover: -1,
    raf: 0,
    palette: null as Palette | null,
    sx: new Float32Array(0),
    sy: new Float32Array(0),
    head: new Int32Array(0),
    next: new Int32Array(0),
    cols: 0,
    rows: 0,
    drag: null as { index: number; x: number; y: number; moved: boolean } | null,
    dragSend: false,
    pan: null as { x: number; y: number; moved: boolean } | null,
    pointers: new Map<number, Pointer>(),
    pinch: null as { dist: number; k: number } | null,
    pressTimer: 0,
    pressHit: -1,
    pointerStart: { x: 0, y: 0 },
    pointerType: "mouse",
    hoverEventAt: 0,
  });

  // ── Рисование ────────────────────────────────────────────────────────────

  const hitTest = useCallback((px: number, py: number): number => {
    const s = state.current;
    if (s.cols === 0) return -1;
    const cx = Math.floor(px / CELL);
    const cy = Math.floor(py / CELL);
    let best = -1;
    let bestDist = Infinity;
    for (let gy = Math.max(0, cy - 1); gy <= Math.min(s.rows - 1, cy + 1); gy += 1) {
      for (let gx = Math.max(0, cx - 1); gx <= Math.min(s.cols - 1, cx + 1); gx += 1) {
        for (let i = s.head[gy * s.cols + gx]!; i !== -1; i = s.next[i]!) {
          const dx = s.sx[i]! - px;
          const dy = s.sy[i]! - py;
          const r = Math.max(screenRadius(i, s.view.k), 5) + 3;
          const d = dx * dx + dy * dy;
          if (d <= r * r && d < bestDist) {
            best = i;
            bestDist = d;
          }
        }
      }
    }
    return best;
  }, []);

  function screenRadius(index: number, k: number): number {
    const base = data.current.baseRadius[index] ?? 3;
    return Math.max(1.6, base * Math.min(Math.max(k, 0.45), 2.4));
  }

  const draw = useCallback(() => {
    const s = state.current;
    s.raf = 0;
    const canvas = canvasRef.current;
    if (canvas === null || s.width === 0) return;
    const ctx = canvas.getContext("2d");
    if (ctx === null) return;
    const p = propsRef.current;
    const palette = s.palette ?? (s.palette = readPalette(canvas));
    const pos = p.layout.positions.current;
    const n = p.graph.nodes.length;
    const { adjacency: adj, shapes: shapeList, currentIndex: current } = data.current;
    ctx.setTransform(s.dpr, 0, 0, s.dpr, 0, 0);
    ctx.clearRect(0, 0, s.width, s.height);
    if (n === 0 || pos.length < n * 2) return;

    // Перенос перетаскиваемого узла в воркер — раз за кадр.
    if (s.drag !== null && s.dragSend) {
      s.dragSend = false;
      const wx = (s.drag.x - s.view.tx) / s.view.k;
      const wy = (s.drag.y - s.view.ty) / s.view.k;
      p.layout.drag(s.drag.index, wx, wy);
    }

    // Вписывание: пока пользователь не двигал вид, камера следует за графом.
    let animating = false;
    if (s.autoFit) {
      s.target = fitView(pos, n, s.width, s.height);
      const t = s.target;
      const v = s.view;
      v.tx += (t.tx - v.tx) * 0.22;
      v.ty += (t.ty - v.ty) * 0.22;
      v.k += (t.k - v.k) * 0.22;
      animating = Math.abs(t.k - v.k) > t.k * 0.002 || Math.abs(t.tx - v.tx) > 0.5;
      if (!animating) {
        v.tx = t.tx;
        v.ty = t.ty;
        v.k = t.k;
      }
    }
    const { tx, ty, k } = s.view;

    // Экранные координаты и сетка-индекс для попадания указателя.
    if (s.sx.length !== n) {
      s.sx = new Float32Array(n);
      s.sy = new Float32Array(n);
      s.next = new Int32Array(n);
    }
    const cols = Math.max(1, Math.ceil(s.width / CELL));
    const rows = Math.max(1, Math.ceil(s.height / CELL));
    if (s.head.length !== cols * rows) s.head = new Int32Array(cols * rows);
    s.head.fill(-1);
    s.cols = cols;
    s.rows = rows;
    let visible = 0;
    for (let i = 0; i < n; i += 1) {
      const x = pos[i * 2]! * k + tx;
      const y = pos[i * 2 + 1]! * k + ty;
      s.sx[i] = x;
      s.sy[i] = y;
      if (x >= 0 && y >= 0 && x < s.width && y < s.height) {
        const cell = Math.floor(y / CELL) * cols + Math.floor(x / CELL);
        s.next[i] = s.head[cell]!;
        s.head[cell] = i;
        visible += 1;
      }
    }

    // Выделение: наведённый (или выбранный с клавиатуры) узел и его соседи.
    const focus = s.hover >= 0 ? s.hover : (p.focusedIndex ?? -1);
    const hot = focus >= 0 && focus < n ? new Uint8Array(n) : null;
    if (hot !== null) {
      hot[focus] = 1;
      for (let j = adj.offsets[focus]!; j < adj.offsets[focus + 1]!; j += 1) {
        hot[adj.targets[j]!] = 1;
      }
    }

    // Связи.
    const plain = new Path2D();
    const hotPath = new Path2D();
    const muted = new Path2D();
    const edges = p.graph.edges;
    const sx = s.sx;
    const sy = s.sy;
    // Пока граф движется или его тянут, на большом графе рисуется каждая n-я связь:
    // растр 15 000 обводок на CPU не укладывается в кадр. В покое — все.
    const busy = p.layout.moving.current || s.drag !== null || s.pinch !== null || (s.pan?.moved ?? false);
    const stride = busy && edges.length > EDGE_BUDGET ? Math.ceil(edges.length / EDGE_BUDGET) : 1;
    for (let e = 0; e < edges.length; e += 1) {
      const edge = edges[e]!;
      if (stride > 1 && e % stride !== 0 && edge.from !== focus && edge.to !== focus) continue;
      const ax = sx[edge.from]!;
      const ay = sy[edge.from]!;
      const bx = sx[edge.to]!;
      const by = sy[edge.to]!;
      if (
        (ax < 0 && bx < 0) ||
        (ay < 0 && by < 0) ||
        (ax > s.width && bx > s.width) ||
        (ay > s.height && by > s.height)
      ) {
        continue;
      }
      const target =
        hot === null ? plain : edge.from === focus || edge.to === focus ? hotPath : muted;
      target.moveTo(ax, ay);
      target.lineTo(bx, by);
    }
    // Толщина чуть меньше пикселя: Skia берёт быстрый путь «волосяной» линии,
    // а 15 000 обычных обводок с антиалиасингом на программном растре — десятки мс.
    ctx.lineWidth = 0.9;
    ctx.strokeStyle = palette.edge;
    ctx.stroke(plain);
    if (hot !== null) {
      ctx.strokeStyle = palette.edgeMuted;
      ctx.stroke(muted);
      ctx.strokeStyle = palette.edgeHot;
      ctx.lineWidth = 1.5;
      ctx.stroke(hotPath);
    }

    // Стрелки направления; при `both_ways` — две.
    if (p.arrows && (k > 0.25 || edges.length < 3000)) {
      const head = Math.min(9, 4 + k * 3);
      const arrowPath = new Path2D();
      const arrowHot = new Path2D();
      const addArrow = (path: Path2D, from: number, to: number): void => {
        const dx = sx[to]! - sx[from]!;
        const dy = sy[to]! - sy[from]!;
        const length = Math.hypot(dx, dy);
        if (length < 1) return;
        const ux = dx / length;
        const uy = dy / length;
        const gap = screenRadius(to, k) + 1;
        if (length < gap + head) return;
        const tipX = sx[to]! - ux * gap;
        const tipY = sy[to]! - uy * gap;
        path.moveTo(tipX, tipY);
        path.lineTo(tipX - ux * head - uy * head * 0.45, tipY - uy * head + ux * head * 0.45);
        path.lineTo(tipX - ux * head + uy * head * 0.45, tipY - uy * head - ux * head * 0.45);
        path.closePath();
      };
      for (const edge of edges) {
        const x1 = sx[edge.from]!;
        const y1 = sy[edge.from]!;
        const x2 = sx[edge.to]!;
        const y2 = sy[edge.to]!;
        if (Math.max(x1, x2) < 0 || Math.min(x1, x2) > s.width) continue;
        if (Math.max(y1, y2) < 0 || Math.min(y1, y2) > s.height) continue;
        const isHot = hot !== null && (edge.from === focus || edge.to === focus);
        const path = isHot ? arrowHot : arrowPath;
        addArrow(path, edge.from, edge.to);
        if (edge.both_ways) addArrow(path, edge.to, edge.from);
      }
      ctx.fillStyle = hot === null ? palette.edge : palette.edgeMuted;
      if (hot === null) {
        ctx.fillStyle = palette.label;
        ctx.globalAlpha = 0.55;
      }
      ctx.fill(arrowPath);
      ctx.globalAlpha = 1;
      ctx.fillStyle = palette.edgeHot;
      ctx.fill(arrowHot);
    }

    // Узлы пакетами по цвету, форме и приглушению.
    const batches = new Map<string, { color: string; shape: Shape; dim: boolean; list: number[] }>();
    const nodes = p.graph.nodes;
    for (let i = 0; i < n; i += 1) {
      const x = sx[i]!;
      const y = sy[i]!;
      if (x < -20 || y < -20 || x > s.width + 20 || y > s.height + 20) continue;
      const node = nodes[i]!;
      const group = node.group;
      const entry = group === undefined || group === null ? undefined : data.current.groups[group];
      const color = entry === undefined ? palette.plain : palette.group[entry.color];
      const dim = hot !== null && hot[i] === 0;
      const shape = shapeList[i]!;
      const key = `${color}|${shape}|${dim ? 1 : 0}`;
      let batch = batches.get(key);
      if (batch === undefined) {
        batch = { color, shape, dim, list: [] };
        batches.set(key, batch);
      }
      batch.list.push(i);
    }
    for (const batch of batches.values()) {
      ctx.globalAlpha = batch.dim ? DIM_ALPHA : 1;
      ctx.fillStyle = batch.color;
      ctx.beginPath();
      for (const i of batch.list) {
        const x = sx[i]!;
        const y = sy[i]!;
        const r = screenRadius(i, k);
        if (batch.shape === "circle" && r < TINY_NODE) {
          // Узел в пару пикселей: квадрат неотличим от круга, а `fillRect` не строит
          // общий контур из тысяч подпутей, который на CPU-растре заливается долго.
          ctx.fillRect(x - r, y - r, r * 2, r * 2);
        } else if (batch.shape === "circle") {
          ctx.moveTo(x + r, y);
          ctx.arc(x, y, r, 0, Math.PI * 2);
        } else if (batch.shape === "square") {
          ctx.rect(x - r, y - r, r * 2, r * 2);
        } else {
          const d = r * 1.25;
          ctx.moveTo(x, y - d);
          ctx.lineTo(x + d, y);
          ctx.lineTo(x, y + d);
          ctx.lineTo(x - d, y);
          ctx.closePath();
        }
      }
      ctx.fill();
    }
    ctx.globalAlpha = 1;

    // Кольцо открытой заметки и наведённого узла.
    ctx.lineWidth = 2;
    if (current >= 0 && current < n) {
      ctx.strokeStyle = palette.ring;
      ctx.beginPath();
      const r = screenRadius(current, k) + 3;
      ctx.arc(sx[current]!, sy[current]!, r, 0, Math.PI * 2);
      ctx.stroke();
    }
    if (focus >= 0 && focus < n) {
      ctx.strokeStyle = palette.hover;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(sx[focus]!, sy[focus]!, screenRadius(focus, k) + 2, 0, Math.PI * 2);
      ctx.stroke();
    }

    // Подписи: по масштабу, у выделенных и у открытой заметки.
    const threshold = 0.5 + 1.75 * p.labelZoom;
    const zoomLabels =
      p.alwaysLabels === true || (k >= threshold && (visible <= MANY_VISIBLE || k >= threshold * 2));
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    let drawn = 0;
    const label = (i: number, bright: boolean): void => {
      const node = nodes[i]!;
      const x = sx[i]!;
      const y = sy[i]! + screenRadius(i, k) + 3;
      if (x < -60 || y < -20 || x > s.width + 60 || y > s.height + 20) return;
      ctx.font = `${bright ? 12 : 11}px ${i === current ? palette.fontMono : palette.font}`;
      ctx.fillStyle = bright ? palette.labelCurrent : palette.label;
      ctx.fillText(node.label, x, y);
    };
    if (hot !== null) {
      for (let i = 0; i < n; i += 1) if (hot[i] === 1) label(i, true);
    } else if (zoomLabels) {
      for (let i = 0; i < n && drawn < MAX_ZOOM_LABELS; i += 1) {
        const x = sx[i]!;
        const y = sy[i]!;
        if (x < 0 || y < 0 || x > s.width || y > s.height) continue;
        label(i, i === current);
        drawn += 1;
      }
    }
    if (hot === null && current >= 0 && !zoomLabels) label(current, true);

    canvas.dataset["moving"] = String(p.layout.moving.current);
    canvas.dataset["draws"] = String(Number(canvas.dataset["draws"] ?? "0") + 1);
    if (s.hoverEventAt > 0) {
      canvas.dataset["hoverMs"] = Math.max(0, performance.now() - s.hoverEventAt).toFixed(1);
      s.hoverEventAt = 0;
    }
    if (animating || s.drag !== null) requestDraw();
  }, []);

  const requestDraw = useCallback(() => {
    const s = state.current;
    if (s.raf === 0) s.raf = requestAnimationFrame(draw);
  }, [draw]);

  // ── Размер и подписка на положения ───────────────────────────────────────

  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) return;
    const s = state.current;
    const resize = (): void => {
      const rect = canvas.getBoundingClientRect();
      s.dpr = window.devicePixelRatio || 1;
      s.width = rect.width;
      s.height = rect.height;
      canvas.width = Math.max(1, Math.round(rect.width * s.dpr));
      canvas.height = Math.max(1, Math.round(rect.height * s.dpr));
      if (s.view.tx === 0 && s.view.ty === 0) {
        s.view = { tx: rect.width / 2, ty: rect.height / 2, k: s.view.k };
      }
      requestDraw();
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);
    const unsubscribe = layout.onFrame(requestDraw);
    return () => {
      observer.disconnect();
      unsubscribe();
      cancelAnimationFrame(s.raf);
      s.raf = 0;
    };
  }, [layout, requestDraw]);

  // Любое изменение входных данных — перерисовка.
  useEffect(() => {
    requestDraw();
  }, [graph, groups, arrows, labelZoom, nodeSize, currentPath, props.focusedIndex, requestDraw]);

  useEffect(() => {
    state.current.autoFit = true;
    requestDraw();
  }, [props.fitKey, requestDraw]);

  // ── Масштаб и вписывание ─────────────────────────────────────────────────

  const zoomAt = useCallback(
    (factor: number, px: number, py: number) => {
      const s = state.current;
      s.autoFit = false;
      const k = Math.min(Math.max(s.view.k * factor, 0.02), 8);
      const real = k / s.view.k;
      s.view = { k, tx: px - (px - s.view.tx) * real, ty: py - (py - s.view.ty) * real };
      requestDraw();
    },
    [requestDraw],
  );

  useImperativeHandle(
    ref,
    () => ({
      zoomBy: (factor) => zoomAt(factor, state.current.width / 2, state.current.height / 2),
      fit: () => {
        state.current.autoFit = true;
        requestDraw();
      },
    }),
    [zoomAt, requestDraw],
  );

  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) return;
    const onWheel = (event: WheelEvent): void => {
      event.preventDefault();
      const rect = canvas.getBoundingClientRect();
      zoomAt(Math.exp(-event.deltaY * (event.deltaMode === 1 ? 0.05 : 0.0016)), event.clientX - rect.left, event.clientY - rect.top);
    };
    canvas.addEventListener("wheel", onWheel, { passive: false });
    return () => canvas.removeEventListener("wheel", onWheel);
  }, [zoomAt]);

  // ── Указатель ────────────────────────────────────────────────────────────

  const local = (event: React.PointerEvent<HTMLCanvasElement>): Pointer => {
    const rect = event.currentTarget.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  };

  const setCursor = (canvas: HTMLCanvasElement, mode: "pan" | "node" | "drag"): void => {
    canvas.dataset["cursor"] = mode;
  };

  const startDrag = (index: number, at: Pointer): void => {
    const s = state.current;
    window.clearTimeout(s.pressTimer);
    s.drag = { index, x: at.x, y: at.y, moved: true };
    s.dragSend = true;
    s.hover = index;
    requestDraw();
  };

  const onPointerDown = (event: React.PointerEvent<HTMLCanvasElement>): void => {
    const s = state.current;
    const canvas = event.currentTarget;
    const at = local(event);
    canvas.setPointerCapture(event.pointerId);
    s.pointers.set(event.pointerId, at);
    s.pointerType = event.pointerType;
    s.pointerStart = at;
    if (s.pointers.size === 2) {
      // Второй палец: щипок отменяет перетаскивание и сдвиг.
      window.clearTimeout(s.pressTimer);
      if (s.drag !== null) layout.release();
      s.drag = null;
      s.pan = null;
      const [a, b] = [...s.pointers.values()] as [Pointer, Pointer];
      s.pinch = { dist: Math.hypot(a.x - b.x, a.y - b.y), k: s.view.k };
      s.autoFit = false;
      return;
    }
    const hit = hitTest(at.x, at.y);
    s.pressHit = hit;
    if (hit >= 0) {
      if (event.pointerType === "mouse") {
        // Мышью узел тянется сразу; щелчок без движения откроет его.
        s.drag = { index: hit, x: at.x, y: at.y, moved: false };
        setCursor(canvas, "drag");
      } else {
        // Сенсор: долгое нажатие берёт узел, быстрое касание открывает его.
        s.pan = { x: at.x, y: at.y, moved: false };
        s.pressTimer = window.setTimeout(() => {
          if (s.pan !== null && !s.pan.moved) {
            s.pan = null;
            startDrag(hit, s.pointerStart);
            navigator.vibrate?.(8);
          }
        }, LONG_PRESS_MS);
      }
    } else {
      s.pan = { x: at.x, y: at.y, moved: false };
    }
  };

  const onPointerMove = (event: React.PointerEvent<HTMLCanvasElement>): void => {
    const s = state.current;
    const canvas = event.currentTarget;
    const at = local(event);
    if (s.pointers.has(event.pointerId)) s.pointers.set(event.pointerId, at);

    if (s.pinch !== null && s.pointers.size >= 2) {
      const [a, b] = [...s.pointers.values()] as [Pointer, Pointer];
      const dist = Math.hypot(a.x - b.x, a.y - b.y);
      const k = Math.min(Math.max(s.pinch.k * (dist / Math.max(s.pinch.dist, 1)), 0.02), 8);
      zoomAt(k / s.view.k, (a.x + b.x) / 2, (a.y + b.y) / 2);
      return;
    }

    if (s.drag !== null) {
      if (!s.drag.moved && Math.hypot(at.x - s.pointerStart.x, at.y - s.pointerStart.y) > DRAG_THRESHOLD) {
        s.drag.moved = true;
      }
      if (s.drag.moved) {
        s.drag.x = at.x;
        s.drag.y = at.y;
        s.dragSend = true;
        s.hover = s.drag.index;
        requestDraw();
      }
      return;
    }

    if (s.pan !== null) {
      const dx = at.x - s.pan.x;
      const dy = at.y - s.pan.y;
      if (!s.pan.moved && Math.hypot(at.x - s.pointerStart.x, at.y - s.pointerStart.y) > DRAG_THRESHOLD) {
        s.pan.moved = true;
        window.clearTimeout(s.pressTimer);
      }
      if (s.pan.moved) {
        s.autoFit = false;
        s.view = { ...s.view, tx: s.view.tx + dx, ty: s.view.ty + dy };
        s.pan = { ...s.pan, x: at.x, y: at.y };
        setCursor(canvas, "pan");
        requestDraw();
      }
      return;
    }

    // Наведение: подсветка соседей за один кадр.
    if (event.pointerType === "mouse") {
      const hit = hitTest(at.x, at.y);
      setCursor(canvas, hit >= 0 ? "node" : "pan");
      if (hit !== s.hover) {
        s.hover = hit;
        s.hoverEventAt = event.timeStamp;
        requestDraw();
      }
    }
  };

  const finish = (event: React.PointerEvent<HTMLCanvasElement>, cancelled: boolean): void => {
    const s = state.current;
    const canvas = event.currentTarget;
    const at = local(event);
    window.clearTimeout(s.pressTimer);
    s.pointers.delete(event.pointerId);
    if (s.pinch !== null) {
      if (s.pointers.size < 2) s.pinch = null;
      s.pan = null;
      return;
    }
    const drag = s.drag;
    s.drag = null;
    if (drag !== null) {
      layout.release();
      setCursor(canvas, "node");
      if (!drag.moved && !cancelled) {
        const node = propsRef.current.graph.nodes[drag.index];
        if (node !== undefined) propsRef.current.onOpen(node);
      }
      if (event.pointerType !== "mouse") s.hover = -1;
      requestDraw();
      return;
    }
    const pan = s.pan;
    s.pan = null;
    // Касание узла без движения — открыть.
    if (pan !== null && !pan.moved && !cancelled && s.pressHit >= 0) {
      const hit = hitTest(at.x, at.y);
      const node = propsRef.current.graph.nodes[hit];
      if (hit === s.pressHit && node !== undefined) propsRef.current.onOpen(node);
    }
    setCursor(canvas, "pan");
  };

  const onPointerLeave = (): void => {
    const s = state.current;
    if (s.drag === null && s.hover !== -1) {
      s.hover = -1;
      requestDraw();
    }
  };

  // Зонд для сценариев проверки (scripts/dev/scenarios/graph.mjs): экранное
  // положение узла и число перерисовок. Продукт на него не опирается.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) return;
    const s = state.current;
    (canvas as HTMLCanvasElement & { tsProbe?: unknown }).tsProbe = {
      nodeAt: (index: number): [number, number] => [s.sx[index] ?? 0, s.sy[index] ?? 0],
      indexOf: (id: string): number => propsRef.current.graph.nodes.findIndex((node) => node.id === id),
      zoom: (): number => s.view.k,
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="ts-graph__surface"
      data-cursor="pan"
      role="img"
      aria-label={props.ariaLabel}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={(event) => finish(event, false)}
      onPointerCancel={(event) => finish(event, true)}
      onPointerLeave={onPointerLeave}
    />
  );
});

/** Вид, в который целиком помещается облако положений. */
function fitView(pos: Float32Array, n: number, width: number, height: number): View {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (let i = 0; i < n; i += 1) {
    const x = pos[i * 2]!;
    const y = pos[i * 2 + 1]!;
    if (x < minX) minX = x;
    if (x > maxX) maxX = x;
    if (y < minY) minY = y;
    if (y > maxY) maxY = y;
  }
  const margin = 48;
  const w = Math.max(maxX - minX, 40);
  const h = Math.max(maxY - minY, 40);
  const k = Math.min(
    Math.max((width - margin * 2) / w, 0.01),
    Math.max((height - margin * 2) / h, 0.01),
    2.2,
  );
  return {
    k,
    tx: width / 2 - ((minX + maxX) / 2) * k,
    ty: height / 2 - ((minY + maxY) / 2) * k,
  };
}
