"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyEditorFontSize,
  DEVICE_FONT_SIZE_VARIABLE,
} from "./applyToDocument.js";
import { FONT_SIZE_HINT } from "./EditorFontSizeField.js";

/**
 * Размер шрифта редактора, меняемый прямо на месте — колесом с Ctrl/⌘ и
 * ступенями в статус-баре (004 FR-016, FR-017).
 *
 * Зачем поверх `useVaultSettings`: значение живёт в настройках открытой папки, а
 * запись туда — это файл на диске. Писать его на каждый щелчок колеса нельзя, а
 * ждать записи, чтобы увидеть новый кегль, — тем более: размер обязан меняться в
 * темпе жеста. Поэтому шаг применяется сразу переменной на корне документа, а в
 * настройки уезжает после паузы. Два слоя размера (папка и устройство) при этом
 * не путаются: пишется и снимается ровно слой папки, как и раньше, — см.
 * `applyToDocument`.
 *
 * Границы — те же, что у поля в панели настроек: ядро отбрасывает значение вне
 * диапазона при чтении файла, и записать сюда то, что оно отвергнет, значило бы
 * соврать пользователю о применённом размере.
 */
export interface EditorFontSizeControl {
  /** Число, которое показывается пользователю и от которого считается шаг. */
  size: number;
  /** Задан ли размер явно для этой папки. `false` — действует слой ниже. */
  isOverridden: boolean;
  /** Сдвинуть на один пиксель с упором в границы диапазона. */
  step: (direction: 1 | -1) => void;
  /** Снять перекрытие: размер вернётся к значению устройства или к макету. */
  reset: () => void;
}

/** Пауза перед записью в настройки папки: колесо шлёт события каждый кадр. */
const PERSIST_DEBOUNCE_MS = 400;

export function useEditorFontSizeControl(
  /** Размер из настроек папки; `null`/`undefined` — перекрытия нет. */
  stored: number | null | undefined,
  /** Записать перекрытие в настройки папки; `null` — снять. */
  persist: (size: number | null) => void | Promise<void>,
): EditorFontSizeControl {
  // Размер, заданный жестом в этом окне. `null` — жеста не было, действует то,
  // что пришло из настроек.
  const [live, setLive] = useState<number | null>(null);
  // Что показывать в статус-баре. В состоянии, а не вычислением при отрисовке:
  // запасное значение спрашивается у документа, а его на сервере нет.
  const [shown, setShown] = useState<number>(FONT_SIZE_HINT.fallback);

  const storedSize = stored ?? null;
  // Настройки могли измениться помимо этого жеста — правкой файла руками или
  // полем в панели. Тогда источник истины они, а не прошлый жест.
  const seenStoredRef = useRef(storedSize);
  if (seenStoredRef.current !== storedSize) {
    seenStoredRef.current = storedSize;
    if (live !== storedSize) setLive(null);
  }

  const effective = live ?? storedSize;

  // Перекрытие применяется отсюда — единственным местом, как и раньше: слой
  // папки старше слоя устройства, и порядок старшинства выражен в самом правиле
  // CSS, а не в том, кто записал последним.
  useEffect(() => {
    applyEditorFontSize(effective);
  }, [effective]);

  useEffect(() => {
    setShown(effective ?? deviceSize() ?? FONT_SIZE_HINT.fallback);
  }, [effective]);

  // Пауза перед записью и сама запись — в ref: шаг обязан оставаться стабильным
  // колбэком, иначе слушатель колеса переставлялся бы на каждый щелчок.
  const timerRef = useRef<number | null>(null);
  const persistRef = useRef(persist);
  persistRef.current = persist;

  const schedulePersist = useCallback((size: number | null) => {
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(() => {
      timerRef.current = null;
      void persistRef.current(size);
    }, PERSIST_DEBOUNCE_MS);
  }, []);

  useEffect(
    () => () => {
      if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    },
    [],
  );

  const step = useCallback(
    (direction: 1 | -1) => {
      setLive((current) => {
        const from = current ?? seenStoredRef.current ?? deviceSize() ?? FONT_SIZE_HINT.fallback;
        const next = Math.min(
          FONT_SIZE_HINT.max,
          Math.max(FONT_SIZE_HINT.min, Math.round(from) + direction),
        );
        if (next === current) return current;
        schedulePersist(next);
        return next;
      });
    },
    [schedulePersist],
  );

  const reset = useCallback(() => {
    setLive(null);
    schedulePersist(null);
  }, [schedulePersist]);

  return { size: shown, isOverridden: effective !== null, step, reset };
}

/**
 * Размер, заданный для устройства, — тот слой, что лежит под перекрытием папки.
 *
 * Читается из инлайн-стиля `<html>`, куда его ставит
 * `applyAppearanceToDocument`, а не из настроек: хранилище устройства есть
 * только в web-сборке, и общий редактор о нём знать не должен. Ничего нет —
 * значит слоя нет, и первый шаг считается от значения по умолчанию.
 */
function deviceSize(): number | null {
  if (typeof document === "undefined") return null;
  const raw = document.documentElement.style.getPropertyValue(
    DEVICE_FONT_SIZE_VARIABLE,
  );
  const value = Number.parseFloat(raw);
  return Number.isFinite(value) ? value : null;
}
