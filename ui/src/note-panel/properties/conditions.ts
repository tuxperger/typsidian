import type { Condition, PropertyValue, Ymd } from "../../wasm-core/typsidian_core.js";
import type { Translate } from "../../i18n/index.js";
import { formatYmd, isoDate } from "./format.js";

// Показ условий отбора словами (фича 020, макет B3/B4). Смысл условий и
// сравнение значений — в ядре; здесь только подписи для фишек и формы.

/** Значение условия одной строкой. */
export function valueText(lang: string, value: PropertyValue): string {
  switch (value.kind) {
    case "Text":
      return value.value;
    case "Number":
      return String(value.value);
    case "Date":
      return formatYmd(lang, value);
    case "Flag":
      return String(value.value);
    case "List":
      return value.values.join(", ");
    case "Unknown":
      return value.source;
  }
}

/** Значение для поля ввода: дата — `ГГГГ-ММ-ДД`, чтобы разбор был обратимым. */
export function valueInput(value: PropertyValue): string {
  switch (value.kind) {
    case "Date":
      return isoDate(value);
    case "Text":
      return value.value;
    case "Number":
      return String(value.value);
    case "Flag":
      return String(value.value);
    case "List":
      return value.values.join(", ");
    case "Unknown":
      return value.source;
  }
}

/** Что набрано в поле значения — число, дата, флажок или текст. */
export function valueFromInput(text: string): PropertyValue {
  const trimmed = text.trim();
  const date = /^(\d{4})-(\d{2})-(\d{2})$/.exec(trimmed);
  if (date !== null) {
    return { kind: "Date", year: Number(date[1]), month: Number(date[2]), day: Number(date[3]) };
  }
  if (trimmed === "true" || trimmed === "false") return { kind: "Flag", value: trimmed === "true" };
  if (trimmed !== "" && Number.isFinite(Number(trimmed))) {
    return { kind: "Number", value: Number(trimmed) };
  }
  return { kind: "Text", value: text };
}

function rangeText(t: Translate, lang: string, range: Extract<Condition, { range: unknown }>["range"]): string {
  if (range.kind === "LastDays") return t("properties.cond.lastDays", { count: range.days });
  const show = (ymd: Ymd): string => formatYmd(lang, ymd);
  return t("properties.cond.between", { from: show(range.from), to: show(range.to) });
}

/** Подпись фишки: имя жирным и значение (`тег` · `проект`). */
export function conditionParts(
  t: Translate,
  lang: string,
  condition: Condition,
): { name: string; text: string } {
  switch (condition.kind) {
    case "TagHas":
      return { name: t("properties.cond.tag"), text: condition.tag };
    case "Compare":
      return {
        name: condition.key,
        text: `${t(`properties.op.${condition.op}`)} ${valueText(lang, condition.value)}`,
      };
    case "Exists":
      return { name: condition.key, text: t("properties.cond.exists") };
    case "Folder":
      return { name: t("properties.cond.folder"), text: condition.folder };
    case "Modified":
      return { name: t("properties.cond.modified"), text: rangeText(t, lang, condition.range) };
    case "Created":
      return { name: t("properties.cond.created"), text: rangeText(t, lang, condition.range) };
  }
}
