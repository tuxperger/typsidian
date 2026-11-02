"use client";

import type { ProbePart, ProbeState } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";

// Итог замера понимания (фича 052, макет C): по каждой части цели — понятна ли она — и
// стартовая точка обучения. Кнопки есть только у итога, после которого урок ждёт
// пользователя; в прошлом итоге остаётся лишь запись.

export interface ProbeResultProps {
  parts: readonly ProbePart[];
  /** С чего начать обучение (текст учителя). */
  start: string;
  /** Итог последний и урок ждёт «Начать обучение». */
  actions?: {
    onStart(): void;
    /** Не задан, пока у урока нет плана. */
    onShowPlan?: () => void;
  };
}

const STATE_CLASS: Record<ProbeState, string> = {
  Known: "ts-dot--ok",
  Partial: "ts-dot--half",
  Unknown: "ts-dot--no",
};

export function ProbeResultCard({ parts, start, actions }: ProbeResultProps): React.JSX.Element {
  const { t } = useI18n();
  return (
    <section className="ts-lesson__probe" aria-label={t("lessons.probe.title")}>
      <h3 className="ts-lesson__probe-title">{t("lessons.probe.title")}</h3>
      <ul className="ts-lesson__parts">
        {parts.map((part, index) => (
          <li key={`${index}:${part.name}`} className="ts-lesson__part">
            <span className={`ts-dot ${STATE_CLASS[part.state]}`} aria-hidden="true" />
            <span className="ts-lesson__part-name">
              {part.name}
              {part.note !== "" && <small className="ts-lesson__part-note">{part.note}</small>}
            </span>
            <span className="ts-lesson__part-state">{t(`lessons.probe.state.${part.state}`)}</span>
          </li>
        ))}
      </ul>
      {start !== "" && (
        <p className="ts-lesson__box ts-lesson__probe-start">
          {t("lessons.probe.startWith")} <b>{start}</b>
        </p>
      )}
      {actions !== undefined && (
        <div className="ts-lesson__acts">
          {actions.onShowPlan !== undefined && (
            <button type="button" className="ts-btn" onClick={actions.onShowPlan}>
              {t("lessons.probe.showPlan")}
            </button>
          )}
          <button type="button" className="ts-btn ts-btn--accent" onClick={actions.onStart}>
            {t("lessons.probe.begin")}
          </button>
        </div>
      )}
    </section>
  );
}
