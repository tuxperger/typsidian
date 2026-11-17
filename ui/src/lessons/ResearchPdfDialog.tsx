"use client";

import { useId, useRef, useState } from "react";
import type { AiFailure, ResearchEstimate } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { failureKind } from "./lessonEnv.js";
import { ResearchPdfEstimate } from "./ResearchPdfEstimate.js";
import { isPdfPath } from "./researchText.js";

// «В PDF…» (фича 056, макет C): путь PDF в vault и вопрос (список файлов, кроме заметок, ядро
// не отдаёт, поэтому путь вводится; проверку и отказ делает оценка ядра), затем оценка до отправки и
// «Изучить». Закрытый для ИИ файл ядро отклоняет оценкой (`Forbidden`) — это и есть отказ.

type Step =
  | { kind: "pick" }
  | { kind: "estimating"; path: string }
  | { kind: "ready"; estimate: ResearchEstimate }
  | { kind: "denied"; path: string; failure: AiFailure["kind"] | null };

export interface ResearchPdfDialogProps {
  client: VaultClient;
  /** Вопрос, уже написанный в поле «Исследовать…». */
  question: string;
  onStart: (path: string, question: string) => void;
  onCancel: () => void;
}

export function ResearchPdfDialog({ client, question, onStart, onCancel }: ResearchPdfDialogProps): React.JSX.Element {
  const { t } = useI18n();
  const id = useId();
    const [asked, setAsked] = useState(question);
  const [path, setPath] = useState("");
  const [step, setStep] = useState<Step>({ kind: "pick" });
  const attempt = useRef(0);
  useDismissOnEscape(onCancel);

  const estimate = (path: string): void => {
    const mine = ++attempt.current;
    setStep({ kind: "estimating", path });
    client.research
      .estimate(path)
      .then((found) => {
        if (mine === attempt.current) setStep({ kind: "ready", estimate: found });
      })
      .catch((error: unknown) => {
        if (mine !== attempt.current) return;
        // Закрытая папка, не-PDF, нет файла: отправлять нечего.
        setStep({ kind: "denied", path, failure: failureKind(error) });
      });
  };

  const canNext = isPdfPath(path.trim()) && asked.trim() !== "";

  return (
    <div className="ts-overlay ts-overlay--center" role="presentation">
      <section
        className="ts-lesson-dialog ts-import"
        role="dialog"
        aria-modal="true"
        aria-labelledby={`${id}-title`}
      >
        <h2 className="ts-lesson-dialog__title" id={`${id}-title`}>
          {t("research.pdf.title")}
        </h2>

        {step.kind === "pick" && (
          <>
            <label className="ts-lesson__lbl" htmlFor={`${id}-p`}>
              {t("research.pdf.path")}
            </label>
            {/* eslint-disable-next-line jsx-a11y/no-autofocus -- окно открыто ради выбора */}
            <input
              id={`${id}-p`}
              className="ts-field__input"
              autoFocus
              spellCheck={false}
              placeholder={t("research.pdf.pathPlaceholder")}
              value={path}
              onChange={(event) => setPath(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && canNext) estimate(path.trim());
              }}
            />
            <label className="ts-lesson__lbl" htmlFor={`${id}-q`}>
              {t("research.pdf.question")}
            </label>
            <input
              id={`${id}-q`}
              className="ts-field__input"
              value={asked}
              onChange={(event) => setAsked(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && canNext) estimate(path.trim());
              }}
            />
            <p className="ts-lesson__hint">{t("research.pdf.hint")}</p>
            <div className="ts-lesson__acts">
              <button type="button" className="ts-btn" onClick={onCancel}>
                {t("research.pdf.cancel")}
              </button>
              <button
                type="button"
                className="ts-btn ts-btn--accent"
                disabled={!canNext}
                onClick={() => estimate(path.trim())}
              >
                {t("research.pdf.next")}
              </button>
            </div>
          </>
        )}

        {step.kind === "estimating" && (
          <p className="ts-lesson__hint" role="status">
            {t("research.pdf.estimating")}
          </p>
        )}

        {step.kind === "denied" && (
          <>
            <p className="ts-lesson__bad" role="alert">
              {step.failure === null || step.failure === "Forbidden" ? t("research.box.denied", { path: step.path }) : t(`ai.failure.${step.failure}`)}
            </p>
            <div className="ts-lesson__acts">
              <button type="button" className="ts-btn" onClick={onCancel}>
                {t("research.pdf.cancel")}
              </button>
              <button type="button" className="ts-btn" onClick={() => setStep({ kind: "pick" })}>
                {t("research.pdf.title")}
              </button>
            </div>
          </>
        )}

        {step.kind === "ready" && (
          <>
            <ResearchPdfEstimate estimate={step.estimate} />
            {step.estimate.problem !== undefined && (
              <p className="ts-lesson__bad" role="alert">
                {t(`knowledge.import.reason.${step.estimate.problem}`)}
              </p>
            )}
            <div className="ts-lesson__acts">
              <button type="button" className="ts-btn" onClick={onCancel}>
                {t("research.pdf.cancel")}
              </button>
              {/* eslint-disable-next-line jsx-a11y/no-autofocus -- окно открыто ради этого решения */}
              <button
                type="button"
                className="ts-btn ts-btn--accent"
                autoFocus
                disabled={step.estimate.problem !== undefined}
                onClick={() => onStart(step.estimate.path, asked.trim())}
              >
                {t("research.pdf.study")}
              </button>
            </div>
          </>
        )}
      </section>
    </div>
  );
}
