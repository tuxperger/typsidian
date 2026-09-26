"use client";

import { useEffect, useRef } from "react";

// Закрытие всплывающего по Escape — одно на все наложения приложения.
//
// Почему общий механизм, а не `onKeyDown` в каждом окне: обработчик на самом
// окне срабатывает, только пока фокус внутри него. Стоит нажать мимо — фокус
// уходит на `body`, и Escape перестаёт что-либо закрывать. Слушатель на
// `window` от этого свободен.
//
// Почему со стеком, а не просто слушателем в каждом компоненте: наложения
// вкладываются (галерея заготовок → диалог сохранения, меню вставки → параметры
// блока). Независимые слушатели закрыли бы по одному нажатию оба, и пользователь
// потерял бы не то окно, которое собирался. Закрывается ровно верхнее —
// смонтированное последним.

/** Стек открытых наложений: закрывается верхнее. */
const stack: (() => void)[] = [];
let listening = false;

function onKeyDown(event: KeyboardEvent): void {
  if (event.key !== "Escape") return;
  const top = stack[stack.length - 1];
  if (top === undefined) return;
  // Останавливаем до того, как Escape доберётся до редактора: там он снимает
  // поля сниппета, и закрытие окна не должно попутно бросать вставку.
  event.preventDefault();
  event.stopPropagation();
  top();
}

/**
 * Закрывает наложение по Escape, пока оно смонтировано.
 *
 * `null` — наложение закрыто или закрывать его нечем: в стек оно не встаёт, и
 * Escape достаётся тому, кто под ним. Именно так ведут себя диалоги решения
 * (конфликт правок): у них нет «просто закрыть», и молча исчезать им нельзя.
 */
export function useDismissOnEscape(onDismiss: (() => void) | null): void {
  // Колбэк в ref: новая ссылка на него не должна переставлять запись в стеке —
  // иначе окно всплывало бы наверх стека на каждый свой рендер.
  const dismissRef = useRef(onDismiss);
  dismissRef.current = onDismiss;

  const active = onDismiss !== null;
  useEffect(() => {
    if (!active) return undefined;

    const entry = (): void => dismissRef.current?.();
    stack.push(entry);
    if (!listening) {
      window.addEventListener("keydown", onKeyDown, true);
      listening = true;
    }

    return () => {
      const at = stack.lastIndexOf(entry);
      if (at !== -1) stack.splice(at, 1);
    };
  }, [active]);
}
