import { SpellChecker, type SpellLanguage } from "../wasm-core/typsidian_core.js";

// Словари орфографии для web (фича 030, research.md §1). Как шрифты
// предпросмотра (005 T029): файлы лежат рядом с wasm-модулем, а не внутри него
// — их кладёт `scripts/spell-dictionaries.mjs` вместе с `index.json`.
// Загружаются при первой настройке проверки и остаются в кэше устройства
// (Cache API): второе открытие и работа без сети их не скачивают. Нативные
// оболочки словарей не грузят — там они вшиты в ядро.

const BASE = "/dictionaries/";

interface DictionariesIndex {
  version: string;
  languages: Record<SpellLanguage, { aff: string; dic: string }>;
}

const loading = new Map<SpellLanguage, Promise<void>>();
let index: Promise<DictionariesIndex> | null = null;

/** Загружает словари языков в ядро; повторный вызов не скачивает снова. */
export async function ensureDictionaries(languages: readonly SpellLanguage[]): Promise<void> {
  await Promise.all(languages.map((language) => ensure(language)));
}

function ensure(language: SpellLanguage): Promise<void> {
  if (SpellChecker.dictionaryReady(language)) return Promise.resolve();
  let pending = loading.get(language);
  if (pending === undefined) {
    pending = load(language).catch((cause: unknown) => {
      // Отказ не запоминается: следующая настройка попробует снова.
      loading.delete(language);
      throw cause;
    });
    loading.set(language, pending);
  }
  return pending;
}

async function load(language: SpellLanguage): Promise<void> {
  index ??= fetch(`${BASE}index.json`).then(async (response) => {
    if (!response.ok) throw new Error(`dictionaries: ${response.status}`);
    return (await response.json()) as DictionariesIndex;
  });
  const found = await index.catch((cause: unknown) => {
    index = null;
    throw cause;
  });
  const files = found.languages[language];
  const cache = await openCache(`typsidian-dictionaries-${found.version}`);
  const [aff, dic] = await Promise.all([
    fetchText(cache, `${BASE}${files.aff}`),
    fetchText(cache, `${BASE}${files.dic}`),
  ]);
  SpellChecker.loadDictionary(language, aff, dic);
}

async function openCache(name: string): Promise<Cache | null> {
  try {
    return typeof caches === "undefined" ? null : await caches.open(name);
  } catch {
    // Кэш недоступен (частный режим) — словари просто скачиваются.
    return null;
  }
}

async function fetchText(cache: Cache | null, url: string): Promise<string> {
  const cached = await cache?.match(url).catch(() => undefined);
  if (cached !== undefined) return cached.text();
  const response = await fetch(url);
  if (!response.ok) throw new Error(`dictionaries: ${url} ${response.status}`);
  await cache?.put(url, response.clone()).catch(() => undefined);
  return response.text();
}
