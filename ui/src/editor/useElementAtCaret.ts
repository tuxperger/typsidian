import { useEffect, useState } from "react";
import type { VaultClient } from "../vault-client.js";
import type { ElementAtResult, UnavailableReason } from "../wasm-core/typsidian_core.js";
import type { Translate } from "../i18n/index.js";

/**
 * Пауза после движения каретки, после которой спрашивается элемент под ней
 * (research.md §3). Разбор заметки в 10 000 строк занимает единицы
 * миллисекунд, но делать его на каждое нажатие незачем: кнопка «Параметры»
 * нужна, когда каретка остановилась, а не пока идёт набор (FR-021).
 */
export const ELEMENT_AT_CARET_DELAY_MS = 150;

/**
 * Элемент, непосредственно содержащий каретку (FR-006, FR-016), — для кнопки
 * на панели вставки. `null` — ещё не спрошено.
 *
 * Решение «что под кареткой» принимает ядро; хук только выбирает момент
 * вопроса.
 */
export function useElementAtCaret(
  client: VaultClient,
  text: string,
  offset: number,
): ElementAtResult | null {
  const [result, setResult] = useState<ElementAtResult | null>(null);

  useEffect(() => {
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void client.elementAt(text, offset).then((next) => {
        if (!cancelled) setResult(next);
      });
    }, ELEMENT_AT_CARET_DELAY_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [client, text, offset]);

  return result;
}

/**
 * Почему «Параметры» недоступны — словами (FR-011). Одна формулировка на все
 * места вызова: панель, контекстное меню, палитру и сочетание (T042).
 */
export function describeUnavailable(t: Translate, reason: UnavailableReason): string {
  switch (reason.kind) {
    case "Unclosed":
      return t("editor.params.unclosed");
    case "DuplicateParam":
      return t("editor.params.duplicate", { name: reason.name });
    case "SpreadArgs":
      return t("editor.params.spread");
    case "UnknownPositional":
      return t("editor.params.unknownPositional");
    case "MathRows":
      return t("editor.params.mathRows");
  }
}

