"use client";

import { useEffect, useState } from "react";

/**
 * Порог телефона — тот же, что у `@media (max-width: 560px)` в `app.css`
 * (фича 010, T057, SC-006). Уже него три колонки рядом не помещаются: дерево,
 * текст и предпросмотр показываются по одной с переключателем внизу. Решает
 * ширина окна, а не таргет (принцип II).
 */
export const PHONE_VIEWPORT_QUERY = "(max-width: 560px)";

/** Окно уже телефонного порога. На сервере и в первом кадре — `false`. */
export function usePhoneViewport(): boolean {
  const [phone, setPhone] = useState(false);

  useEffect(() => {
    const query = window.matchMedia(PHONE_VIEWPORT_QUERY);
    setPhone(query.matches);
    const onChange = (event: MediaQueryListEvent): void => setPhone(event.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  return phone;
}
