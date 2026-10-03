/**
 * Язык-зависимое оформление (фича 010, FR-021, FR-022): формы слов при числах,
 * числа, даты, размеры файлов.
 *
 * Всё через стандартный `Intl`: правило выбора формы принадлежит языку, и
 * переписывать его руками в каждом каталоге — верный способ получить три
 * слегка разных правила. Данные CLDR уже есть в каждом рантайме, где живёт
 * интерфейс, поэтому в бандл они не едут.
 */

/** Формы слова при числе. `other` обязательна: она есть в каждом языке. */
export type PluralForms = Partial<Record<Intl.LDMLPluralRule, string>> & { other: string };

const pluralRules = new Map<string, Intl.PluralRules>();
const numberFormats = new Map<string, Intl.NumberFormat>();

function rulesFor(lang: string): Intl.PluralRules {
  let rules = pluralRules.get(lang);
  if (rules === undefined) {
    rules = new Intl.PluralRules(lang);
    pluralRules.set(lang, rules);
  }
  return rules;
}

function numbersFor(lang: string): Intl.NumberFormat {
  let format = numberFormats.get(lang);
  if (format === undefined) {
    format = new Intl.NumberFormat(lang);
    numberFormats.set(lang, format);
  }
  return format;
}

/**
 * Форма слова при числе: «1 заметка», «3 заметки», «9 заметок» / «1 note».
 *
 * `#` в форме заменяется числом, оформленным по правилам языка. Форма без `#`
 * возвращается как есть — так пишется «одна заметка» словами, если язык этого
 * просит.
 */
export function plural(lang: string, count: number, forms: PluralForms): string {
  const form = forms[rulesFor(lang).select(count)] ?? forms.other;
  return form.replace("#", numbersFor(lang).format(count));
}

export function formatNumber(lang: string, value: number): string {
  return numbersFor(lang).format(value);
}

export function formatDate(lang: string, date: Date, style: "short" | "long" = "short"): string {
  return new Intl.DateTimeFormat(lang, {
    dateStyle: style === "long" ? "long" : "medium",
    timeStyle: "short",
  }).format(date);
}

/**
 * Размер файла в двоичных единицах. Единица — через `Intl` (`unit: "kilobyte"`
 * и т. п.): «12 КБ» и «12 kB» — это правило языка, а не каталога.
 */
export function formatBytes(lang: string, bytes: number): string {
  const units = ["byte", "kilobyte", "megabyte", "gigabyte"] as const;
  let value = bytes;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  return new Intl.NumberFormat(lang, {
    style: "unit",
    unit: units[index] ?? "byte",
    unitDisplay: "short",
    maximumFractionDigits: index === 0 ? 0 : 1,
  }).format(value);
}

/**
 * «5 минут назад» / «5 minutes ago» — через `Intl.RelativeTimeFormat`: и форма
 * слова при числе, и порядок слов принадлежат языку.
 */
export function formatRelative(
  lang: string,
  value: number,
  unit: "minute" | "hour" | "day",
): string {
  return new Intl.RelativeTimeFormat(lang, { numeric: "always" }).format(value, unit);
}

/** «340 ms» / «1.2 s» — длительность операции; единица и её запись — правило языка. */
export function formatDuration(lang: string, ms: number): string {
  const seconds = ms >= 1000;
  return new Intl.NumberFormat(lang, {
    style: "unit",
    unit: seconds ? "second" : "millisecond",
    unitDisplay: "short",
    maximumFractionDigits: seconds ? 1 : 0,
  }).format(seconds ? ms / 1000 : Math.round(ms));
}
