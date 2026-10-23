import type { Translate } from "../i18n/index.js";

// Время снимков и удалений (фича 029, макет S1, S2): «Сегодня», «Вчера» или дата,
// время — отдельно. Общее для окна истории и «Недавно удалённых».

export interface SnapshotClock {
  /** Время без даты: «13:40». */
  timeOf(ms: number): string;
  /** День: «Сегодня», «Вчера» или дата без времени. */
  dayOf(ms: number): string;
  /** День со временем в строку: «сегодня 13:40». */
  whenOf(ms: number): string;
}

export function snapshotClock(t: Translate, lang: string): SnapshotClock {
  const timeOf = (ms: number): string =>
    new Date(ms).toLocaleTimeString(lang, { hour: "2-digit", minute: "2-digit" });
  const dayOf = (ms: number): string => {
    const day = new Date(ms);
    const today = new Date();
    const yesterday = new Date();
    yesterday.setDate(today.getDate() - 1);
    if (day.toDateString() === today.toDateString()) return t("snapshots.today");
    if (day.toDateString() === yesterday.toDateString()) return t("snapshots.yesterday");
    // Только дата: `formatDate` добавил бы время, а оно здесь своё.
    return day.toLocaleDateString(lang, { dateStyle: "long" });
  };
  const whenOf = (ms: number): string => `${dayOf(ms).toLowerCase()} ${timeOf(ms)}`;
  return { timeOf, dayOf, whenOf };
}
