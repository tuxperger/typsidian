/**
 * Какой язык действует сейчас (фича 010, FR-008, FR-009, FR-018).
 *
 * Здесь, а не в ядре: вход функции — состав каталогов, которые существуют
 * только в интерфейсе (plan.md → Complexity Tracking). Сам выбор пользователя
 * хранит и проверяет ядро; эта функция лишь решает, какой из поставляемых
 * каталогов ему соответствует.
 */

/** Язык отката: эталонный каталог, полный всегда (FR-009, FR-027). */
export const FALLBACK_LANGUAGE = "en";

/** Основной подтег: `en-GB` → `en`, `RU` → `ru`. */
export function primarySubtag(tag: string): string {
  return tag.split(/[-_]/)[0]?.toLowerCase() ?? "";
}

/**
 * Действующий язык.
 *
 * - явный выбор, если такой язык поставляется;
 * - иначе — первый из языков системы (в порядке их приоритета), чей основной
 *   подтег поставляется;
 * - иначе — английский.
 *
 * Нераспознанный выбор (язык из более новой версии) ведёт себя как «как в
 * системе», но сам не затирается — это забота хранилища (FR-018).
 */
export function resolveLanguage(
  choice: string | undefined,
  system: readonly string[],
  available: readonly string[],
): string {
  if (choice !== undefined) {
    const chosen = primarySubtag(choice);
    if (available.includes(chosen)) return chosen;
  }
  for (const tag of system) {
    const candidate = primarySubtag(tag);
    if (available.includes(candidate)) return candidate;
  }
  return FALLBACK_LANGUAGE;
}
