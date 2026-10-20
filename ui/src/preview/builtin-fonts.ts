// Встроенный набор шрифтов предпросмотра для web (фича 005, T029,
// research.md §6). Файлы лежат рядом с wasm-модулем, а не внутри него:
// `scripts/builtin-fonts.mjs` кладёт их в статику вместе с `index.json`.
// Загружаются один раз, при первой сборке, и остаются в кэше устройства
// (Cache API) — второе открытие и работа без сети их уже не скачивают.

/** Где лежит набор в статике web-приложения. */
const FONTS_BASE = "/fonts/typst/";

interface FontsIndex {
  version: string;
  files: string[];
}

let loading: Promise<Uint8Array[]> | null = null;

/** Байты файлов встроенного набора; повторный вызов отдаёт тот же ответ. */
export function loadBuiltinFonts(): Promise<Uint8Array[]> {
  loading ??= load().catch((cause: unknown) => {
    // Отказ не запоминается: следующая сборка попробует снова.
    loading = null;
    throw cause;
  });
  return loading;
}

async function load(): Promise<Uint8Array[]> {
  const index = (await (await fetch(`${FONTS_BASE}index.json`)).json()) as FontsIndex;
  // Версия набора — в имени кэша: новый набор не смешивается со старым.
  const cache = await openCache(`typsidian-fonts-${index.version}`);
  return Promise.all(index.files.map((name) => fetchFont(cache, `${FONTS_BASE}${name}`)));
}

async function openCache(name: string): Promise<Cache | null> {
  try {
    return typeof caches === "undefined" ? null : await caches.open(name);
  } catch {
    // Кэш недоступен (частный режим) — шрифты просто скачиваются.
    return null;
  }
}

async function fetchFont(cache: Cache | null, url: string): Promise<Uint8Array> {
  const cached = await cache?.match(url).catch(() => undefined);
  if (cached !== undefined) return new Uint8Array(await cached.arrayBuffer());
  const response = await fetch(url);
  if (!response.ok) throw new Error(`font ${url}: ${response.status}`);
  await cache?.put(url, response.clone()).catch(() => undefined);
  return new Uint8Array(await response.arrayBuffer());
}
