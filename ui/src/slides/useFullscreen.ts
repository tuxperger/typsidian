"use client";

import { useCallback, useEffect, useState } from "react";

/**
 * Развёрнуто ли окно на весь экран: полноэкранный режим документа, F11 в
 * браузере (окно равно экрану) и полный экран окна ОС на рабочем столе.
 */
export function isFullscreen(): boolean {
  if (document.fullscreenElement !== null) return true;
  return window.innerWidth >= screen.width && window.innerHeight >= screen.height;
}

/**
 * Состояние полноэкранного режима и просьба о нём. Просьба `requestFullscreen`
 * допустима только в обработчике жеста пользователя; отказ — штатный ответ
 * `false`, а не ошибка (A6).
 */
export function useFullscreen(): { fullscreen: boolean; request: () => Promise<boolean> } {
  const [fullscreen, setFullscreen] = useState(isFullscreen);

  useEffect(() => {
    const update = (): void => setFullscreen(isFullscreen());
    document.addEventListener("fullscreenchange", update);
    window.addEventListener("resize", update);
    return () => {
      document.removeEventListener("fullscreenchange", update);
      window.removeEventListener("resize", update);
    };
  }, []);

  const request = useCallback(async (): Promise<boolean> => {
    if (isFullscreen()) return true;
    try {
      await document.documentElement.requestFullscreen();
      return true;
    } catch {
      return false;
    }
  }, []);

  return { fullscreen, request };
}
