"use client";

import type { PracticeView } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";

// Сводка практики (фича 053, макет E): задачи, итоги, число подсказок и переход к
// разбору. Список строит ядро (`LessonView.practice`); «трудности» ядро не считает —
// агент пишет их в разбор последней задачи, он и показан внизу (research §5). Оценок и
// расписаний нет (FR-009).

export interface PracticeSummaryProps {
  practice: PracticeView;
  /** Лист узкого экрана: поверх ленты, с кнопкой закрытия. */
  sheet: boolean;
  onClose(): void;
  /** Прокрутить ленту к событию с этим номером. */
  onJump(anchor: number): void;
}

export function PracticeSummary({ practice, sheet, onClose, onJump }: PracticeSummaryProps): React.JSX.Element {
  const { t } = useI18n();
  return (
    <aside
      className={sheet ? "ts-plan ts-plan--sheet" : "ts-plan"}
      aria-label={t("practice.summary.title")}
      {...(sheet ? { role: "dialog", "aria-modal": true } : {})}
    >
      <div className="ts-plan__head">
        <b>{t("practice.summary.title")}</b>
        {sheet && (
          <button type="button" className="ts-btn ts-btn--sm" autoFocus onClick={onClose}>
            {t("practice.summary.close")}
          </button>
        )}
      </div>
      <div className="ts-plan__body ts-practice__sum">
        {practice.problems.length === 0 ? (
          <p className="ts-lesson__hint">{t("practice.summary.empty")}</p>
        ) : (
          <ul className="ts-practice__rows" aria-label={t("practice.summary.list")}>
            {practice.problems.map((problem) => (
              <li key={problem.n}>
                <button
                  type="button"
                  className="ts-practice__row"
                  aria-label={t("practice.summary.jump", { n: problem.n, title: problem.title })}
                  onClick={() => onJump(problem.anchor)}
                >
                  <span className="ts-practice__rown">{problem.n}</span>
                  <span className="ts-practice__rowtitle">{problem.title}</span>
                  <span
                    className={
                      problem.outcome == null
                        ? "ts-practice__rowout"
                        : `ts-practice__rowout ts-practice__verdict--${problem.outcome}`
                    }
                  >
                    {problem.outcome == null ? t("practice.summary.open") : t(`practice.outcome.${problem.outcome}`).toLowerCase()}
                    {problem.hints > 0 && ` · ${t("practice.summary.hints", { count: problem.hints })}`}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
        {practice.last_review != null && practice.last_review !== "" && (
          <>
            <div className="ts-lesson__lbl">{t("practice.summary.review")}</div>
            <p className="ts-practice__sumreview">{practice.last_review}</p>
          </>
        )}
        <p className="ts-lesson__hint">{t("practice.summary.hint")}</p>
      </div>
    </aside>
  );
}
