/**
 * Язык интерфейса (фича 010): реестр языков, контекст и `t`.
 *
 * # Как добавить язык (FR-026)
 *
 * 1. Создать `locales/<код>.ts` с объектом типа `Catalog` (полный) или
 *    `Partial<Catalog>` (частичный — недостающее покажется по-английски).
 * 2. Добавить запись в [`LANGUAGES`] ниже: код, название на самом языке,
 *    полнота, язык редактора досок, если тот его поддерживает.
 *
 * Больше ничего не меняется: экраны берут тексты только отсюда.
 *
 * # Почему смена языка ничего не пересоздаёт
 *
 * Язык — значение контекста. Потребители перерисовываются, но язык не входит
 * ни в один `key`, поэтому редактор, доска и предпросмотр остаются теми же
 * экземплярами — с правками, курсором и состоянием (FR-013).
 */

import {
  createContext,
  createElement,
  useContext,
  useEffect,
  useMemo,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import {
  formatBytes,
  formatDate,
  formatDuration,
  formatNumber,
  formatRelative,
} from "./format.js";
import { en, type Catalog, type MessageKey, type MessageParams } from "./locales/en.js";
import { ru } from "./locales/ru.js";
import { FALLBACK_LANGUAGE, resolveLanguage } from "./resolve.js";

export type { Catalog, MessageKey, MessageParams } from "./locales/en.js";
export { plural } from "./format.js";

export interface LanguageInfo {
  code: string;
  /** Название на самом языке (FR-011). */
  nativeName: string;
  /** Полный ли каталог. Частичный допустим только для языка в работе (FR-027). */
  complete: boolean;
  /** Код языка редактора досок, если тот этот язык поддерживает (FR-025). */
  engineLang?: string;
}

interface LanguageEntry extends LanguageInfo {
  catalog: Partial<Catalog>;
}

const REGISTRY: readonly LanguageEntry[] = [
  { code: "en", nativeName: "English", complete: true, engineLang: "en", catalog: en },
  // Название языка — на самом языке, в любом интерфейсе (FR-011). i18n-ignore
  { code: "ru", nativeName: "Русский", complete: true, engineLang: "ru-RU", catalog: ru }, // i18n-ignore
];

/** Поставляемые языки в порядке списка выбора. */
export const LANGUAGES: readonly LanguageInfo[] = REGISTRY.map(
  ({ catalog: _catalog, ...info }) => info,
);

const AVAILABLE = REGISTRY.map((entry) => entry.code);

type Entry = string | ((params: never) => string);

function render(entry: Entry, params: readonly unknown[]): string {
  if (typeof entry === "string") return entry;
  // Сигнатура ключа уже сверена в `t` через `MessageParams`; здесь её только
  // исполняют, поэтому сужение до `unknown` безопасно.
  return (entry as unknown as (params: unknown) => string)(params[0]);
}

export type Translate = <K extends MessageKey>(key: K, ...params: MessageParams<K>) => string;

/**
 * Текст, который собирается при показе, а не при возникновении (FR-014).
 *
 * Отказ, сохранённый в состоянии готовой строкой, после смены языка остался бы
 * на прежнем; фраза же перерисовывается вместе с экраном на новом.
 */
export type Phrase = (t: Translate) => string;

function translatorFor(catalog: Partial<Catalog>): Translate {
  return (key, ...params) => {
    // Нет перевода — английский, а не пустое место или ключ (FR-027).
    const entry: Entry = catalog[key] ?? en[key];
    return render(entry, params);
  };
}

export interface I18n {
  lang: string;
  t: Translate;
  /** Английский вариант — для поиска в палитре команд (FR-024). */
  tEn: Translate;
  /** Язык редактора досок: его собственный код, если он этот язык знает, иначе `en` (FR-025). */
  engineLang: string;
  formatNumber(value: number): string;
  formatDate(date: Date, style?: "short" | "long"): string;
  formatBytes(bytes: number): string;
  /** Длительность операции из миллисекунд: «340 ms», «1.2 s». */
  formatDuration(ms: number): string;
  /** Отрицательное значение — в прошлом: `formatRelative(-5, "minute")`. */
  formatRelative(value: number, unit: "minute" | "hour" | "day"): string;
}

function makeI18n(lang: string): I18n {
  const entry = REGISTRY.find((known) => known.code === lang) ?? REGISTRY[0];
  const catalog = entry?.catalog ?? en;
  return {
    lang,
    t: translatorFor(catalog),
    tEn: translatorFor(en),
    engineLang: entry?.engineLang ?? "en",
    formatNumber: (value) => formatNumber(lang, value),
    formatDate: (date, style) => formatDate(lang, date, style),
    formatBytes: (bytes) => formatBytes(lang, bytes),
    formatDuration: (ms) => formatDuration(lang, ms),
    formatRelative: (value, unit) => formatRelative(lang, value, unit),
  };
}

let current: I18n = makeI18n(FALLBACK_LANGUAGE);

/**
 * Текущий язык вне React — для кода, который вызывается из обработчиков, а не
 * при рендере (заголовок нативного диалога, текст для буфера обмена). Всё, что
 * рисуется, берёт язык через [`useI18n`], иначе не перерисуется при смене.
 */
export function currentI18n(): I18n {
  return current;
}

const I18nContext = createContext<I18n>(current);

// Языки системы. Снимок сервера пустой: при гидрации React возьмёт его, а после
// — клиентский, и расхождения разметки не будет.
const NO_LANGUAGES: readonly string[] = [];
let languagesSnapshot: readonly string[] | null = null;

function subscribeLanguages(onChange: () => void): () => void {
  const handler = (): void => {
    languagesSnapshot = null;
    onChange();
  };
  window.addEventListener("languagechange", handler);
  return () => window.removeEventListener("languagechange", handler);
}

function clientLanguages(): readonly string[] {
  if (languagesSnapshot === null) {
    languagesSnapshot =
      navigator.languages.length > 0 ? [...navigator.languages] : [navigator.language];
  }
  return languagesSnapshot;
}

function serverLanguages(): readonly string[] {
  return NO_LANGUAGES;
}

export function I18nProvider(props: {
  /** Выбор из настроек устройства; `undefined` — «как в системе». */
  choice: string | undefined;
  children: ReactNode;
}): React.JSX.Element {
  const system = useSyncExternalStore(subscribeLanguages, clientLanguages, serverLanguages);
  const lang = resolveLanguage(props.choice, system, AVAILABLE);
  const value = useMemo(() => makeI18n(lang), [lang]);
  current = value;

  useEffect(() => {
    // Язык документа — для средств чтения с экрана (FR-006).
    document.documentElement.lang = lang;
  }, [lang]);

  return createElement(I18nContext.Provider, { value }, props.children);
}

export function useI18n(): I18n {
  return useContext(I18nContext);
}
