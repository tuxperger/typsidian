import { useCallback, useRef, useState } from "react";

/**
 * Обзор папок как жест: открыть и дождаться ответа.
 *
 * Нужен затем, что `Workspace` ждёт от оболочки **функцию** «выбери папку»
 * (`VaultOpener`), а на рабочем столе и в браузере такой функцией оказывается
 * системный диалог: он сам рисует себя и сам возвращает путь. На Android
 * диалога нет, обзор рисует само окно — и промежуток между «показали обзор» и
 * «пользователь выбрал» приходится сшивать здесь. Ответ ждёт промис, который
 * разрешает уже нажатие в `FolderPicker`.
 */
export interface FolderPicker {
  /** Показывает обзор и ждёт ответа. `null` — пользователь его закрыл. */
  pick: () => Promise<string | null>;
  /** Показан ли обзор сейчас: по нему окно решает, рисовать ли его. */
  isOpen: boolean;
  /** Выбрана папка. */
  choose: (path: string) => void;
  /** Обзор закрыт без выбора. */
  cancel: () => void;
}

export function useFolderPicker(): FolderPicker {
  const [isOpen, setOpen] = useState(false);
  // Разрешение промиса живёт в ref, а не в состоянии: его вызывает обработчик
  // нажатия, и перерисовка ему не нужна.
  const pending = useRef<((path: string | null) => void) | null>(null);

  const pick = useCallback(() => {
    setOpen(true);
    return new Promise<string | null>((resolve) => {
      // Второго обзора одновременно быть не может: кнопка «Открыть vault»
      // одна и живёт на экране приветствия, который обзор собой закрывает.
      pending.current = resolve;
    });
  }, []);

  const settle = useCallback((path: string | null) => {
    setOpen(false);
    const resolve = pending.current;
    pending.current = null;
    // Промис разрешается ровно один раз: повторное нажатие после ответа
    // (например, по закрывающемуся обзору) не должно вернуть второй путь.
    resolve?.(path);
  }, []);

  const choose = useCallback((path: string) => settle(path), [settle]);
  const cancel = useCallback(() => settle(null), [settle]);

  return { pick, isOpen, choose, cancel };
}
