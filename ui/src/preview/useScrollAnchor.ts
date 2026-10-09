"use client";

import { useCallback, useMemo, type RefObject } from "react";
import type { ResolvedAnchor, ScrollAnchor } from "../wasm-core/typsidian_core.js";

// Удержание места при пересборке (005 FR-004, FR-034, T068).
//
// Здесь только две вещи, которые знает один интерфейс: что сейчас видно на
// экране и как прокрутить к названному прогону. Какое место нового документа
// считать «тем же», решает ядро (`preview/anchor.rs`) — поиска совпадений
// здесь нет и быть не должно (принцип I).

/** Якорь вместе с тем, где он стоял на экране. */
export interface CapturedAnchor {
  anchor: ScrollAnchor;
  /** Расстояние от верха области прокрутки до верха прогона, px. */
  offset: number;
}

export interface ScrollAnchorControl {
  /** Снять якорь с первого видимого прогона. `null` — удерживать нечего. */
  capture: () => CapturedAnchor | null;
  /** Прокрутить так, чтобы названный ядром прогон встал на прежнее место. */
  restore: (resolved: ResolvedAnchor, offset: number) => void;
}

const PAGE = ".ts-preview__page";
const RUN = ".ts-textlayer text";

export function useScrollAnchor(container: RefObject<HTMLElement | null>): ScrollAnchorControl {
  const capture = useCallback((): CapturedAnchor | null => {
    const node = container.current;
    // В самом начале документа удерживать нечего: там он и должен остаться.
    if (node === null || node.scrollTop <= 0) return null;
    const top = node.getBoundingClientRect().top;
    const pages = node.querySelectorAll<HTMLElement>(PAGE);
    for (let pageIndex = 0; pageIndex < pages.length; pageIndex += 1) {
      const page = pages[pageIndex];
      if (page === undefined || page.getBoundingClientRect().bottom <= top) continue;
      const runs = page.querySelectorAll<SVGTextElement>(RUN);
      for (let runIndex = 0; runIndex < runs.length; runIndex += 1) {
        const run = runs[runIndex];
        if (run === undefined) continue;
        const box = run.getBoundingClientRect();
        if (box.bottom <= top) continue;
        return {
          anchor: {
            page_index: pageIndex,
            run_text: run.textContent ?? "",
            run_index: runIndex,
          },
          offset: box.top - top,
        };
      }
    }
    return null;
  }, [container]);

  const restore = useCallback(
    (resolved: ResolvedAnchor, offset: number): void => {
      const node = container.current;
      if (node === null) return;
      const page = node.querySelectorAll<HTMLElement>(PAGE)[resolved.page_index];
      const run = page?.querySelectorAll<SVGTextElement>(RUN)[resolved.run_index];
      if (run === undefined) return;
      const top = node.getBoundingClientRect().top;
      node.scrollTop += run.getBoundingClientRect().top - top - offset;
    },
    [container],
  );

  return useMemo(() => ({ capture, restore }), [capture, restore]);
}
