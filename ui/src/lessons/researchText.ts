import type { ResearchView } from "../wasm-core/typsidian_core.js";

// Показ исследования (фича 056): подписи страниц и проверка ссылок. Правила — в ядре; здесь
// только форма строки.

/** Исследование ещё идёт: опрашивать вид, показывать «Отменить». */
export function isActive(item: ResearchView): boolean {
  return item.status === "Queued" || item.status === "Running";
}

/** Номера страниц как диапазоны: [23, 25, 26] → ["23", "25–26"]. */
export function pageRanges(pages: readonly number[]): string[] {
  const sorted = [...new Set(pages)].sort((a, b) => a - b);
  const out: string[] = [];
  let from = 0;
  while (from < sorted.length) {
    let to = from;
    while (to + 1 < sorted.length && (sorted[to + 1] ?? 0) === (sorted[to] ?? 0) + 1) to += 1;
    const first = sorted[from] ?? 0;
    const last = sorted[to] ?? 0;
    out.push(first === last ? String(first) : `${first}–${last}`);
    from = to + 1;
  }
  return out;
}

/** Внешняя ссылка источника: только http(s), иначе показывается текстом. */
export function safeUrl(url: string | undefined): string | undefined {
  if (url === undefined) return undefined;
  try {
    const parsed = new URL(url);
    return parsed.protocol === "https:" || parsed.protocol === "http:" ? parsed.href : undefined;
  } catch {
    return undefined;
  }
}

/** Хост ссылки для подписи: «en.wikipedia.org». */
export function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

/** Путь PDF — по расширению: выбор файла в списке vault. */
export function isPdfPath(path: string): boolean {
  return path.toLowerCase().endsWith(".pdf");
}

