"use client";

import { useId } from "react";
import type { DeniedLesson, LessonEvent, LessonView, ProblemOutcome } from "../wasm-core/typsidian_core.js";
import { Icon } from "../icons/Icons.js";
import { useI18n } from "../i18n/index.js";
import { modEnterLabel } from "./lessonEnv.js";

// Лента практики (фича 053, макет B–D): задача без названия знания, замок на решении,
// подсказки, итог с разбором и решением, «Следующая задача». Какое решение показывать и
// когда, решает ядро: `LessonView` отдаёт его только у задачи с итогом, здесь решение лишь
// выводится. Ничего из хода практики интерфейс не вычисляет, кроме номера подсказки внутри
// задачи и признака «последний итог ждёт кнопку».

const GLYPH: Record<ProblemOutcome, string> = {
  Correct: "✓",
  Partial: "◐",
  Wrong: "✕",
  Unsolved: "—",
};

/** Номер подсказки внутри своей задачи (с единицы) по индексу события `Hint`. */
export function hintNumbers(events: readonly LessonEvent[]): Map<number, number> {
  const numbers = new Map<number, number>();
  let count = 0;
  events.forEach((event, index) => {
    if (event.kind === "Problem") count = 0;
    if (event.kind === "Hint") {
      count += 1;
      numbers.set(index, count);
    }
  });
  return numbers;
}

/** Последний итог ещё не сменила ни новая задача, ни «Следующая задача» (как в ядре). */
export function awaitingNext(events: readonly LessonEvent[]): boolean {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    const kind = events[i]?.kind;
    if (kind === "Verdict") return true;
    if (kind === "NextProblem" || kind === "Problem") return false;
  }
  return false;
}

export interface PracticeRowProps {
  event: LessonEvent;
  view: LessonView;
  /** Номер подсказки внутри задачи; задан только для `Hint`. */
  hintNumber: number | undefined;
  /** Кнопка «Следующая задача» под этим итогом; не задана у прошлых итогов. */
  onNext: (() => void) | undefined;
}

/** Строка ленты для событий практики; остальные события рисует вкладка урока. */
export function PracticeRow({ event, view, hintNumber, onNext }: PracticeRowProps): React.JSX.Element | null {
  switch (event.kind) {
    case "Material":
      return (
        <MaterialRow
          practice={view.header.kind === "Practice"}
          lessons={event.lessons.length}
          description={event.description}
          denied={event.denied ?? []}
          knowledge={event.knowledge?.statements.length ?? 0}
        />
      );
    case "Problem": {
      const info = view.practice?.problems.find((problem) => problem.n === event.n);
      return <ProblemCard n={event.n} statement={event.statement} locked={info === undefined || info.outcome == null} />;
    }
    case "Hint":
      return <HintCard n={hintNumber ?? 1} text={event.text} />;
    case "Verdict": {
      const info = view.practice?.problems.find((problem) => problem.n === event.n);
      return (
        <VerdictCard
          outcome={event.outcome}
          review={event.review}
          solution={info?.solution ?? undefined}
          onNext={onNext}
        />
      );
    }
    case "NextProblem":
      return <NextSeparator />;
    default:
      return null;
  }
}

function MaterialRow({
  practice,
  lessons,
  description,
  denied,
  knowledge,
}: {
  /** Материал практики; у урока (054) событие `Material` несёт только выбранные утверждения графа. */
  practice: boolean;
  lessons: number;
  description: string | undefined;
  denied: readonly DeniedLesson[];
  /** Утверждений графа знаний, выбранных пользователем (054). */
  knowledge: number;
}): React.JSX.Element {
  const { t } = useI18n();
  const hasText = description !== undefined && description !== "";
  // Практика только с опорой на граф не «по описанию темы»: заголовок материала не нужен.
  const header = practice && (lessons > 0 || hasText || knowledge === 0);
  return (
    <>
      {header && (
        <div className="ts-lesson__sep">
          {lessons > 0 ? t("practice.sep.material", { count: lessons }) : t("practice.sep.materialTopic")}
        </div>
      )}
      {knowledge > 0 && <div className="ts-lesson__sep">{t("knowledge.material", { count: knowledge })}</div>}
      {description !== undefined && description !== "" && (
        <div className="ts-lesson__body ts-lesson__body--soft">{description}</div>
      )}
      {denied.map((item) => (
        <div key={item.path} className="ts-practice__denied" role="note">
          <Icon name="lock" size={14} />
          <span>
            {t("practice.denied", { path: item.path })}
            <small>{t(`practice.denied.${item.reason}`)}</small>
          </span>
        </div>
      ))}
    </>
  );
}

function NextSeparator(): React.JSX.Element {
  const { t } = useI18n();
  return <div className="ts-lesson__sep">{t("practice.sep.next")}</div>;
}

function ProblemCard({ n, statement, locked }: { n: number; statement: string; locked: boolean }): React.JSX.Element {
  const { t } = useI18n();
  return (
    <section className="ts-practice__prob" aria-label={t("practice.problem", { n })}>
      <div className="ts-practice__pn">{t("practice.problem", { n })}</div>
      <div className="ts-lesson__body">{statement}</div>
      {locked && (
        <div className="ts-practice__lock" role="note">
          <Icon name="lock" size={14} />
          <span>
            {t("practice.lock")}
            <small>{t("practice.lockFile")}</small>
          </span>
        </div>
      )}
    </section>
  );
}

function HintCard({ n, text }: { n: number; text: string }): React.JSX.Element {
  const { t } = useI18n();
  return (
    <div className="ts-practice__hint">
      <b>{t("practice.hint", { n })}</b>
      <div className="ts-lesson__body">{text}</div>
    </div>
  );
}

function VerdictCard({
  outcome,
  review,
  solution,
  onNext,
}: {
  outcome: ProblemOutcome;
  review: string;
  solution: string | undefined;
  onNext: (() => void) | undefined;
}): React.JSX.Element {
  const { t } = useI18n();
  return (
    <section className="ts-practice__prob" aria-label={t(`practice.outcome.${outcome}`)}>
      <div className={`ts-practice__verdict ts-practice__verdict--${outcome}`}>
        <span aria-hidden="true">{GLYPH[outcome]}</span> {t(`practice.outcome.${outcome}`)}
      </div>
      {review !== "" && <div className="ts-practice__review ts-lesson__body">{review}</div>}
      {solution !== undefined && solution !== "" ? (
        <div className="ts-practice__sol">
          <div className="ts-practice__soltitle">{t("practice.solution")}</div>
          <div className="ts-lesson__body">{solution}</div>
        </div>
      ) : (
        <div className="ts-lesson__box ts-lesson__box--warn ts-practice__nosol" role="status">
          {t("practice.noSolution")}
        </div>
      )}
      {onNext !== undefined && (
        <div className="ts-lesson__acts">
          <span className="ts-lesson__hint ts-lesson__acts-hint">{t("practice.nextHint")}</span>
          <button type="button" className="ts-btn ts-btn--accent" onClick={onNext}>
            {t("practice.next")}
          </button>
        </div>
      )}
    </section>
  );
}

export interface PracticeFormProps {
  text: string;
  onText(text: string): void;
  onSend(): void;
  onHint(): void;
  onGiveUp(): void;
}

/** Поле ответа и рассуждения открытой задачи с «Сдаюсь», «Подсказка», «Ответить» (макет B). */
export function PracticeForm({ text, onText, onSend, onHint, onGiveUp }: PracticeFormProps): React.JSX.Element {
  const { t } = useI18n();
  const id = useId();
  const ready = text.trim() !== "";
  return (
    <form
      className="ts-practice__form"
      aria-label={t("practice.answer")}
      onSubmit={(event) => {
        event.preventDefault();
        if (ready) onSend();
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter" && (event.ctrlKey || event.metaKey) && ready) {
          event.preventDefault();
          onSend();
        }
      }}
    >
      <label className="ts-lesson__lbl" htmlFor={`${id}-answer`}>
        {t("practice.answer")}
      </label>
      <textarea
        id={`${id}-answer`}
        className="ts-lesson__ta"
        rows={3}
        placeholder={t("practice.answerPlaceholder")}
        value={text}
        onChange={(event) => onText(event.target.value)}
      />
      <div className="ts-lesson__acts">
        <span className="ts-lesson__hint ts-lesson__acts-hint">{t("practice.sendHint", { keys: modEnterLabel() })}</span>
        <button type="button" className="ts-btn" onClick={onGiveUp}>
          {t("practice.giveUp")}
        </button>
        <button type="button" className="ts-btn" onClick={onHint}>
          {t("practice.hintButton")}
        </button>
        <button type="submit" className="ts-btn ts-btn--accent" disabled={!ready}>
          {t("practice.send")}
        </button>
      </div>
    </form>
  );
}
