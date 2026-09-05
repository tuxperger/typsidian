import { useEffect } from "react";
import type { VaultClient } from "../vault-client.js";

/**
 * Предупреждение перед закрытием окна с неразрешённым конфликтом или просто
 * несохранёнными правками (T099, FR-037, FR-035).
 *
 * Что именно будет потеряно, знает ядро: `unresolvedEditPaths()` возвращает
 * пути с неразрешённым конфликтом, внешним удалением при живых правках и
 * несохранённым буфером. UI не ведёт собственный учёт «грязных» заметок —
 * иначе появился бы второй источник истины (принцип I), способный разойтись
 * с ядром ровно в тот момент, когда цена расхождения — потерянный текст.
 *
 * Показать собственный текст в этом диалоге браузеры не дают: они выводят
 * свою формулировку, а от страницы требуется только `preventDefault()`.
 * Поэтому обязательство FR-037 «предупредить до закрытия» выполняется самим
 * фактом запроса подтверждения, а «потеря только после явного подтверждения» —
 * тем, что решение остаётся за пользователем в этом диалоге.
 */
export function useWindowCloseGuard(client: VaultClient | null): void {
  useEffect(() => {
    if (!client) return;

    const onBeforeUnload = (event: BeforeUnloadEvent): void => {
      // Синхронно: браузер не ждёт промисов в этом обработчике. Поэтому вопрос
      // задаётся через `hasUnresolvedEdits`, которому позволено ошибаться в
      // сторону «есть несохранённое» — см. `VaultClient`.
      if (!client.hasUnresolvedEdits()) return;
      event.preventDefault();
      // Устаревший, но всё ещё требуемый частью браузеров способ пометить
      // событие как «требует подтверждения» в дополнение к preventDefault().
      event.returnValue = "";
    };

    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, [client]);
}

/**
 * Тот же вопрос для оболочки рабочего стола (Tauri `onCloseRequested`), где
 * закрытие можно перехватить и показать собственный диалог. Возвращает пути,
 * которые будут потеряны, — пустой массив означает «закрывать безопасно».
 */
export async function unresolvedEditPaths(
  client: VaultClient | null,
): Promise<string[]> {
  return client ? client.unresolvedEditPaths() : [];
}
