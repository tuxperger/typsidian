// Протокол между мостом `useGraphLayout` и воркером раскладки (фича 025, research.md §6).
// Один файл с типами, чтобы воркер и клиент не разошлись в форме сообщений.

/** Силы раскладки из настроек, доли 0..1. */
export interface LayoutForces {
  center: number;
  repel: number;
  link: number;
  distance: number;
}

export type ToWorker =
  | {
      type: "setGraph";
      /** Номер графа: ответы на прежний граф интерфейс отбрасывает. */
      gen: number;
      ids: string[];
      /** Пары индексов `[from, to, from, to, …]`. */
      edges: Uint32Array;
      /** Переименования `[старый, новый]`: положение старого id переходит новому. */
      renames: [string, string][];
      /** Индекс центра локального графа, если он есть. */
      center: number | null;
    }
  | { type: "setForces"; forces: LayoutForces }
  | { type: "drag"; index: number; x: number; y: number }
  | { type: "release" }
  | { type: "reheat"; alpha: number }
  | { type: "stop" };

export type FromWorker =
  | { type: "tick"; gen: number; positions: Float32Array; alpha: number }
  | { type: "settled"; gen: number };
