# Contract: модуль `@typsidian/ui/i18n`

Единственная точка, через которую экраны и оболочки получают тексты.

## Экспорт

```ts
export type LanguageCode = string;                 // основной подтег: "en", "ru"
export interface LanguageInfo {
  code: LanguageCode;
  nativeName: string;                              // FR-011
  complete: boolean;                               // полный ли каталог (FR-028)
  engineLang?: string;                             // язык редактора досок (FR-025)
}

export const LANGUAGES: readonly LanguageInfo[];   // реестр; порядок = порядок в списке выбора

export function resolveLanguage(
  choice: string | undefined,                      // Settings.language; undefined = «как в системе»
  system: readonly string[],                       // navigator.languages
): LanguageCode;                                   // всегда поддерживаемый код; откат — "en"

export function I18nProvider(props: {
  choice: string | undefined;
  children: React.ReactNode;
}): JSX.Element;                                   // выставляет document.documentElement.lang

export interface I18n {
  lang: LanguageCode;
  t: Translate;                                    // t("tree.empty"), t("tree.count", { count })
  tEn: Translate;                                  // английский — для поиска в палитре (FR-024)
  message(m: AppMessage): string;                  // сообщение ядра на текущем языке (FR-014)
  diagnostic(d: Diagnostic): string;               // app ? message(app) : d.message (FR-004)
  error(e: KnownCoreError): string;                // отказы с `kind` (errors.*)
  formatNumber(n: number): string;                 // FR-022
  formatDate(d: Date, style?: "short" | "long"): string;
  formatBytes(n: number): string;
}
export function useI18n(): I18n;
```

## Поведение

- `t` для ключа, отсутствующего в каталоге текущего языка, возвращает английский вариант;
  служебный ключ на экран не попадает никогда (FR-027).
- `message` содержит исчерпывающий `switch` по `m.code` с веткой `never`: новый вариант
  `AppMessage` в ядре без перевода — ошибка `tsc`.
- Смена `choice` не пересоздаёт дерево: меняется только значение контекста (FR-013).
- Подписи сочетаний клавиш строятся из сочетания, а не из каталога; из каталога берутся
  только слова («Shift», «Ctrl» остаются общепринятыми обозначениями) (FR-023).

## Запреты

- Литералы с кириллицей в `ui/src`, `apps/*/src`, `apps/web/app` вне
  `ui/src/i18n/locales/` — провал `scripts/check-i18n.sh`.
- Показ `error.message` / `Diagnostic.message` при `app != null` в UI запрещён.
