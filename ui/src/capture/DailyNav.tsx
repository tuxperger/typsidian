"use client";

import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { hintWith } from "../workspace/useShortcuts.js";
import type { DailyNote } from "./useDailyNote.js";

/**
 * «Сегодня» и ‹ › у ежедневной заметки в шапке (фича 022, макет D4).
 *
 * Дату и соседей называет ядро через `useDailyNote`; здесь только кнопки.
 * ‹ › появляются у ежедневной заметки; нет заметки в эту сторону — кнопка
 * неактивна, а не спрятана, чтобы ряд не прыгал.
 */
export interface DailyNavProps {
  daily: DailyNote;
  /** Подпись сочетания «Сегодня»; пустая — снято. */
  todayShortcut: string;
}

export function DailyNav({ daily, todayShortcut }: DailyNavProps): React.JSX.Element {
  const { t, lang } = useI18n();
  const position = daily.position;
  return (
    <>
      {position !== null && (
        <div className="ts-daily" role="group" aria-label={t("capture.daily.nav")}>
          <button
            type="button"
            className="ts-btn ts-btn--icon ts-daily__step"
            disabled={position.previous === null}
            title={position.previous === null ? t("capture.daily.noPrevious") : t("capture.daily.previous")}
            aria-label={t("capture.daily.previous")}
            onClick={() => void daily.openPrevious()}
          >
            <Icon name="chevron" size={14} className="ts-icon--flip" />
          </button>
          <time
            className="ts-daily__date"
            dateTime={isoDate(position.date)}
          >
            {dateLabel(lang, position.date)}
          </time>
          <button
            type="button"
            className="ts-btn ts-btn--icon ts-daily__step"
            disabled={position.next === null}
            title={position.next === null ? t("capture.daily.noNext") : t("capture.daily.next")}
            aria-label={t("capture.daily.next")}
            onClick={() => void daily.openNext()}
          >
            <Icon name="chevron" size={14} />
          </button>
        </div>
      )}
      <button
        type="button"
        className="ts-btn ts-daily__today"
        title={hintWith(t("capture.daily.todayHint"), todayShortcut)}
        onClick={() => void daily.openToday()}
      >
        {t("capture.daily.today")}
      </button>
    </>
  );
}

function isoDate(date: { year: number; month: number; day: number }): string {
  const pad = (value: number): string => String(value).padStart(2, "0");
  return `${date.year}-${pad(date.month)}-${pad(date.day)}`;
}

/** «Ср, 15 октября» по языку интерфейса. */
function dateLabel(lang: string, date: { year: number; month: number; day: number }): string {
  return new Intl.DateTimeFormat(lang, { weekday: "short", day: "numeric", month: "long" }).format(
    new Date(date.year, date.month - 1, date.day),
  );
}
