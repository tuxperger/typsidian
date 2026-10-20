"use client";

import { useLayoutEffect, useState } from "react";

// Положение всплывающего у каретки: меню вставки (003), палитра цитат (017),
// список дополнения (018) — одно правило на всех.

/** Координаты каретки в окне. */
export interface CaretAnchor {
  left: number;
  top: number;
  bottom: number;
}

/** Отступ от края окна, если меню не помещается под кареткой. */
const VIEWPORT_MARGIN = 8;
/** Зазор между кареткой и меню. */
const CARET_GAP = 6;

/**
 * Меню под кареткой, но целиком в окне: снизу места может не хватить, и тогда
 * оно встаёт над строкой. Размер берётся замером, а не константой, — иначе
 * правка отступов в CSS молча ломала бы раскладку.
 */
export function useMenuPosition(
  menuRef: React.RefObject<HTMLDivElement | null>,
  anchor: CaretAnchor | null,
): { left: number; top: number } | null {
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);

  useLayoutEffect(() => {
    const node = menuRef.current;
    if (!node) return;
    const box = node.getBoundingClientRect();

    const maxLeft = window.innerWidth - box.width - VIEWPORT_MARGIN;
    const wanted = anchor?.left ?? (window.innerWidth - box.width) / 2;
    const left = Math.max(VIEWPORT_MARGIN, Math.min(wanted, Math.max(VIEWPORT_MARGIN, maxLeft)));

    const below = (anchor?.bottom ?? window.innerHeight * 0.12) + CARET_GAP;
    const fits = below + box.height <= window.innerHeight - VIEWPORT_MARGIN;
    const above = (anchor?.top ?? 0) - box.height - CARET_GAP;
    const top = fits ? below : Math.max(VIEWPORT_MARGIN, above);

    setPosition({ left, top });
  }, [menuRef, anchor]);

  return position;
}
