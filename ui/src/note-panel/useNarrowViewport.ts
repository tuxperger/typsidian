"use client";

import { useEffect, useState } from "react";

/**
 * Порог узкого окна — тот же, что у остальных `@media (max-width: 720px)` в
 * `app.css`. Подачу колонки заметки («колонка или шторка») решает ширина окна,
 * а не таргет (принцип II): ветвления по платформе в общем UI нет.
 */
export const NARROW_VIEWPORT_QUERY = "(max-width: 720px)";

/**
 * Узкое ли окно. На сервере и в первом кадре — `false`: `Workspace`
 * рендерится и в Next.js, где `window` нет, и значение, прочитанное здесь,
 * разошлось бы с серверной разметкой. Настоящее приезжает эффектом.
 */
export function useNarrowViewport(): boolean {
  const [narrow, setNarrow] = useState(false);

  useEffect(() => {
    const query = window.matchMedia(NARROW_VIEWPORT_QUERY);
    setNarrow(query.matches);
    const onChange = (event: MediaQueryListEvent): void => setNarrow(event.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  return narrow;
}
