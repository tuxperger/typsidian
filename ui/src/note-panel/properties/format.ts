import type { PropertyValue, ValueKind } from "../../wasm-core/typsidian_core.js";

// Показ значений свойств (фича 020). Что значение значит и как сравнивается —
// решает ядро; здесь только запись «год-месяц-день» и вид значка.

/** Значок вида значения слева от ключа (макет B1). Не текст интерфейса — условный знак. */
export const KIND_GLYPH: Record<ValueKind, string> = {
  Text: "Aa",
  Number: "12",
  Date: "▦",
  Flag: "✓",
  List: "#",
  Unknown: "ƒ",
};

/** `ГГГГ-ММ-ДД` для поля даты; пустая строка, если даты нет. */
export function isoDate(value: { year: number; month: number; day: number }): string {
  const pad = (n: number, width: number): string => String(n).padStart(width, "0");
  return `${pad(value.year, 4)}-${pad(value.month, 2)}-${pad(value.day, 2)}`;
}

/** Разбор `ГГГГ-ММ-ДД` из поля даты; `null` — не дата. */
export function parseIsoDate(
  text: string,
): { year: number; month: number; day: number } | null {
  const match = /^(\d{4,})-(\d{2})-(\d{2})$/.exec(text);
  if (match === null) return null;
  const [, year, month, day] = match;
  return { year: Number(year), month: Number(month), day: Number(day) };
}

/** Дата без времени на языке интерфейса. */
export function formatYmd(
  lang: string,
  value: { year: number; month: number; day: number },
): string {
  const date = new Date(Date.UTC(value.year, value.month - 1, value.day));
  return new Intl.DateTimeFormat(lang, { dateStyle: "medium", timeZone: "UTC" }).format(date);
}

/** Сегодняшняя дата — начальное значение нового свойства-даты. */
export function today(): { year: number; month: number; day: number } {
  const now = new Date();
  return { year: now.getFullYear(), month: now.getMonth() + 1, day: now.getDate() };
}

/** Начальное значение нового свойства выбранного вида. */
export function defaultValue(kind: Exclude<ValueKind, "Unknown">): PropertyValue {
  switch (kind) {
    case "Text":
      return { kind: "Text", value: "" };
    case "Number":
      return { kind: "Number", value: 0 };
    case "Date":
      return { kind: "Date", ...today() };
    case "Flag":
      return { kind: "Flag", value: false };
    case "List":
      return { kind: "List", values: [] };
  }
}

/** Ключ тегов в записи (data-model.md «Property»). */
export const TAGS_KEY = "tags";
