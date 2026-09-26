"use client";

import { useState } from "react";

/**
 * Перетаскиваемая граница между колонками рабочей области.
 *
 * В раскладке не занимает места: её ширина — ноль, а зона захвата и подсветка
 * нарисованы псевдоэлементами поверх рамки соседней колонки (`.ts-resizer` в
 * `app.css`). Иначе появление ручки сдвинуло бы все колонки на свою толщину, и
 * приложение разошлось бы с макетом на ровном месте.
 *
 * `role="separator"` с `tabIndex` — это window splitter из WAI-ARIA: границу
 * можно двигать и клавишами, а не только мышью. Поэтому же у неё есть
 * `aria-valuenow`: без него скринридер объявит разделитель, но не скажет, где
 * тот стоит.
 */
export interface PaneDividerProps {
  /** Что именно двигает эта граница. */
  label: string;
  /** Текущее положение и его пределы — в тех же единицах, что и шаг. */
  now: number;
  min: number;
  max: number;
  /** Новое положение указателя по горизонтали. */
  onDrag: (clientX: number) => void;
  /** Шаг стрелкой: -1 влево, 1 вправо. */
  onStep: (direction: -1 | 1) => void;
  /** Вернуть размер из макета — двойным щелчком. */
  onReset: () => void;
}

export function PaneDivider({
  label,
  now,
  min,
  max,
  onDrag,
  onStep,
  onReset,
}: PaneDividerProps): React.JSX.Element {
  const [dragging, setDragging] = useState(false);

  return (
    <div
      className="ts-resizer"
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={Math.round(now)}
      aria-valuemin={Math.round(min)}
      aria-valuemax={Math.round(max)}
      tabIndex={0}
      {...(dragging ? { "data-dragging": "true" } : {})}
      onPointerDown={(event) => {
        // Захват указателя вместо слушателей на окне: события продолжают
        // приходить сюда, даже когда курсор ушёл за пределы ручки — а он
        // уходит сразу, ручка шириной с рамку. `preventDefault` не даёт
        // начаться выделению текста в соседней колонке.
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        setDragging(true);
      }}
      onPointerMove={(event) => {
        if (dragging) onDrag(event.clientX);
      }}
      onPointerUp={(event) => {
        event.currentTarget.releasePointerCapture(event.pointerId);
        setDragging(false);
      }}
      onPointerCancel={() => setDragging(false)}
      onDoubleClick={onReset}
      onKeyDown={(event) => {
        if (event.key === "ArrowLeft") {
          event.preventDefault();
          onStep(-1);
        } else if (event.key === "ArrowRight") {
          event.preventDefault();
          onStep(1);
        } else if (event.key === "Enter") {
          // Возврат к макету с клавиатуры: двойной щелчок мышью делает то же.
          event.preventDefault();
          onReset();
        }
      }}
    />
  );
}
