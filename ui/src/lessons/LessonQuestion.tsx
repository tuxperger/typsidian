"use client";

import { useId } from "react";
import type { Confidence, LessonQuestion as QuestionData } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import { modEnterLabel } from "./lessonEnv.js";

// Вопрос учителя и форма ответа (макет A, C, J): варианты или свободный ответ,
// уверенность, «думаю вслух», Mod+Enter. Черновик хранит вкладка урока: пока идёт
// ход, форма скрыта, а при отказе «урок изменился» ответ не должен пропасть.

export interface AnswerDraft {
  /** Индекс выбранного варианта; `null` — не выбран. */
  choice: number | null;
  /** Свободный ответ (для вопроса без вариантов). */
  text: string;
  confidence: Confidence | null;
  thinking: string;
}

export const EMPTY_DRAFT: AnswerDraft = { choice: null, text: "", confidence: null, thinking: "" };

const LEVELS: readonly Confidence[] = ["Guess", "Likely", "Sure"];

/** Можно ли отправить: выбран вариант либо непустой свободный ответ. */
export function canAnswer(question: QuestionData | undefined, draft: AnswerDraft): boolean {
  return question?.choices !== undefined && question.choices.length > 0
    ? draft.choice !== null
    : draft.text.trim() !== "";
}

export interface LessonQuestionProps {
  question: QuestionData | undefined;
  draft: AnswerDraft;
  onDraft(draft: AnswerDraft): void;
  onSubmit(): void;
  /** «Не знаю» — только у вопросов замера (052): ответ-сведение без текста. */
  onDontKnow?: () => void;
}

export function LessonQuestion({ question, draft, onDraft, onSubmit, onDontKnow }: LessonQuestionProps): React.JSX.Element {
  const { t } = useI18n();
  const id = useId();
  const choices = question?.choices ?? [];
  const ready = canAnswer(question, draft);

  const submitOnModEnter = (event: React.KeyboardEvent): void => {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey) && ready) {
      event.preventDefault();
      onSubmit();
    }
  };

  return (
    <form
      className="ts-lesson__q"
      aria-label={question?.question ?? t("lessons.q.freeAnswer")}
      onKeyDown={submitOnModEnter}
      onSubmit={(event) => {
        event.preventDefault();
        if (ready) onSubmit();
      }}
    >
      {question?.part != null && <PartKicker part={question.part} />}
      {question !== undefined && <p className="ts-lesson__qt">{question.question}</p>}

      {choices.length > 0 ? (
        <div
          className="ts-lesson__opts"
          role="radiogroup"
          aria-label={t("lessons.q.choices")}
          onKeyDown={(event) => {
            // Цифра выбирает вариант с этим номером, как в макете.
            const index = Number(event.key) - 1;
            if (!event.ctrlKey && !event.metaKey && Number.isInteger(index) && index >= 0 && index < choices.length) {
              onDraft({ ...draft, choice: index });
            }
          }}
        >
          {choices.map((choice, index) => (
            <label key={`${index}:${choice}`} className="ts-lesson__opt" data-selected={draft.choice === index}>
              <input
                type="radio"
                name={`${id}-choice`}
                checked={draft.choice === index}
                onChange={() => onDraft({ ...draft, choice: index })}
              />
              <span className="ts-lesson__optkey">{index + 1}</span>
              <span>{choice}</span>
            </label>
          ))}
        </div>
      ) : (
        <textarea
          className="ts-lesson__ta"
          aria-label={t("lessons.q.freeAnswer")}
          placeholder={t("lessons.q.answerPlaceholder")}
          value={draft.text}
          rows={3}
          onChange={(event) => onDraft({ ...draft, text: event.target.value })}
        />
      )}

      <div className="ts-lesson__lbl" id={`${id}-conf`}>
        {t("lessons.q.confidence")}
      </div>
      <span className="ts-lesson__seg" role="group" aria-labelledby={`${id}-conf`}>
        {LEVELS.map((level) => (
          <button
            key={level}
            type="button"
            aria-pressed={draft.confidence === level}
            onClick={() => onDraft({ ...draft, confidence: draft.confidence === level ? null : level })}
          >
            {t(`lessons.q.confidence.${level}`)}
          </button>
        ))}
      </span>

      <label className="ts-lesson__lbl" htmlFor={`${id}-think`}>
        {t("lessons.q.thinking")} <span className="ts-lesson__hint">{t("lessons.q.optional")}</span>
      </label>
      <textarea
        id={`${id}-think`}
        className="ts-lesson__ta"
        placeholder={t("lessons.q.thinkingPlaceholder")}
        value={draft.thinking}
        rows={2}
        onChange={(event) => onDraft({ ...draft, thinking: event.target.value })}
      />

      <div className="ts-lesson__acts">
        <span className="ts-lesson__hint ts-lesson__acts-hint">
          {onDontKnow !== undefined
            ? t("lessons.q.dontKnowHint")
            : choices.length === 0 && draft.confidence === null && !ready
              ? t("lessons.q.confidenceOptional")
              : t("lessons.q.sendHint", { keys: modEnterLabel() })}
        </span>
        {onDontKnow !== undefined && (
          <button type="button" className="ts-btn" onClick={onDontKnow}>
            {t("lessons.q.dontKnow")}
          </button>
        )}
        <button type="submit" className="ts-btn ts-btn--accent" disabled={!ready}>
          {t("lessons.q.send")}
        </button>
      </div>
    </form>
  );
}

/** Метка части цели над вопросом замера (макет 052, B). */
function PartKicker({ part }: { part: string }): React.JSX.Element {
  const { t } = useI18n();
  return <div className="ts-lesson__kicker">{t("lessons.q.part", { part })}</div>;
}

/** Вопрос из истории: только чтение. */
export function PastQuestion({ question }: { question: QuestionData }): React.JSX.Element {
  const choices = question.choices ?? [];
  return (
    <div className="ts-lesson__q ts-lesson__q--past">
      {question.part != null && <PartKicker part={question.part} />}
      <p className="ts-lesson__qt">{question.question}</p>
      {choices.length > 0 && (
        <ol className="ts-lesson__plist">
          {choices.map((choice, index) => (
            <li key={`${index}:${choice}`}>{choice}</li>
          ))}
        </ol>
      )}
    </div>
  );
}
