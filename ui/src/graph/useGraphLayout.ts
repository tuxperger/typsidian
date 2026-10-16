import { useCallback, useEffect, useRef } from "react";
import type { VaultGraph } from "../wasm-core/typsidian_core.js";
import type { FromWorker, LayoutForces, ToWorker } from "./layout-protocol.js";

// Мост к воркеру раскладки (фича 025, T012, research.md §6, §8).
//
// Воркер создаётся так же, как воркер сборки предпросмотра (005): `new URL(…,
// import.meta.url)` резолвится сборщиком оболочки — Turbopack в Next и Vite в
// Tauri-оболочках видят `layout.worker.js` рядом с этим файлом в `ui/dist`.

export interface GraphLayout {
  /** Последние положения `x0, y0, x1, y1, …` в порядке `graph.nodes`. */
  positions: React.RefObject<Float32Array>;
  /** Подписка на новые положения (вызывается не чаще кадра); возвращает отписку. */
  onFrame: (listener: () => void) => () => void;
  /** Раскладка ещё движется. */
  moving: React.RefObject<boolean>;
  drag: (index: number, x: number, y: number) => void;
  release: () => void;
  reheat: (alpha: number) => void;
}

export function useGraphLayout(
  graph: VaultGraph | null,
  forces: LayoutForces,
  renames: [string, string][],
): GraphLayout {
  const positions = useRef<Float32Array>(new Float32Array(0));
  const workerRef = useRef<Worker | null>(null);
  const listeners = useRef(new Set<() => void>());
  const genRef = useRef(0);
  const forcesRef = useRef(forces);
  forcesRef.current = forces;
  const renamesRef = useRef(renames);
  renamesRef.current = renames;
  const moving = useRef(false);

  const send = useCallback((message: ToWorker, transfer: Transferable[] = []) => {
    workerRef.current?.postMessage(message, transfer);
  }, []);

  useEffect(() => {
    const worker = new Worker(new URL("./layout.worker.js", import.meta.url), {
      type: "module",
    });
    workerRef.current = worker;
    worker.onmessage = (event: MessageEvent<FromWorker>) => {
      const data = event.data;
      if (data.gen !== genRef.current) return;
      if (data.type === "tick") {
        positions.current = data.positions;
        for (const listener of listeners.current) listener();
      } else {
        moving.current = false;
        // Холст перерисуется и отметит покой.
        for (const listener of listeners.current) listener();
      }
    };
    worker.postMessage({ type: "setForces", forces: forcesRef.current } satisfies ToWorker);
    return () => {
      worker.postMessage({ type: "stop" } satisfies ToWorker);
      worker.terminate();
      workerRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (graph === null) return;
    genRef.current += 1;
    const edges = new Uint32Array(graph.edges.length * 2);
    graph.edges.forEach((edge, i) => {
      edges[i * 2] = edge.from;
      edges[i * 2 + 1] = edge.to;
    });
    positions.current = new Float32Array(graph.nodes.length * 2);
    moving.current = true;
    send(
      {
        type: "setGraph",
        gen: genRef.current,
        ids: graph.nodes.map((node) => node.id),
        edges,
        renames: renamesRef.current,
        center: graph.center ?? null,
      },
      [edges.buffer],
    );
  }, [graph, send]);

  const sentForces = useRef(forces);
  useEffect(() => {
    // Объект сил пересоздаётся каждый рендер: сравниваем числа, а первый
    // показ воркер получил при создании и подогревать раскладку не должен.
    const prev = sentForces.current;
    if (
      prev.center === forces.center &&
      prev.repel === forces.repel &&
      prev.link === forces.link &&
      prev.distance === forces.distance
    ) {
      return;
    }
    sentForces.current = forces;
    send({ type: "setForces", forces });
  }, [forces, send]);

  const onFrame = useCallback((listener: () => void) => {
    listeners.current.add(listener);
    return () => {
      listeners.current.delete(listener);
    };
  }, []);

  return {
    positions,
    onFrame,
    moving,
    drag: (index, x, y) => send({ type: "drag", index, x, y }),
    release: () => send({ type: "release" }),
    reheat: (alpha) => send({ type: "reheat", alpha }),
  };
}
