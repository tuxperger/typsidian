"use client";

import { useEffect, useState } from "react";

/**
 * Ширина окна для ядра: подачу вкладок (две группы, одна полоса, список) выбирает
 * оно по ширине, а не интерфейс. В первом кадре — ширина настольного окна: на
 * сервере (Next.js) окна нет, а показ вкладок до открытия vault всё равно пуст.
 */
const FALLBACK_WIDTH_PX = 1280;

export function useWindowWidth(): number {
  const [width, setWidth] = useState(() =>
    typeof window === "undefined" ? FALLBACK_WIDTH_PX : window.innerWidth,
  );
  useEffect(() => {
    const update = (): void => setWidth(window.innerWidth);
    update();
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);
  return width;
}
