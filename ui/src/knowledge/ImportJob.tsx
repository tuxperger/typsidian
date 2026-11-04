"use client";

import type { ImportEstimate, ImportFileJob, KnowledgeJob } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import { failureKeyOfKind } from "./knowledgeText.js";

// Ход и итог импорта заметок (фича 055, макеты C и D): прогресс по файлам с «Отменить», затем
// итог по каждому файлу — готово, частично, пропущен с причиной, сбой — и «Повторить» у
// частичных и сбойных. Карточка не модальная: заметки в это время правятся (SC-003).

const MIN_FILL = 6;

export interface ImportJobProps {
  estimate: ImportEstimate;
  /** Задание из реестра ядра; `null` — оно ещё не появилось в опросе. */
  job: KnowledgeJob | null;
  /** Идёт ли импорт (иначе показан итог). */
  running: boolean;
  onCancel: () => void;
  onRetry: (path: string) => void;
  onOpenGraph: () => void;
  onClose: () => void;
}

const FINISHED = new Set<ImportFileJob["state"]>(["Done", "Partial", "Failed", "Skipped"]);

/** Строки до первого опроса: файлы оценки ждут, пропущенные уже пропущены. */
function placeholderRows(estimate: ImportEstimate): ImportFileJob[] {
  return [
    ...estimate.files.map(
      (file): ImportFileJob => ({
        path: file.path,
        state: "Pending",
        added_propositions: 0,
        parts_done: 0,
        parts: file.parts,
      }),
    ),
    ...estimate.skipped.map(
      (item): ImportFileJob => ({
        path: item.path,
        state: "Skipped",
        reason: item.reason,
        added_propositions: 0,
        parts_done: 0,
        parts: 0,
      }),
    ),
  ];
}

const GLYPH = {
  Done: "✓",
  Partial: "◐",
  Failed: "✕",
  Skipped: "—",
  Running: "◌",
  Pending: "◌",
} as const satisfies Record<ImportFileJob["state"], string>;

export function ImportJob({
  estimate,
  job,
  running,
  onCancel,
  onRetry,
  onOpenGraph,
  onClose,
}: ImportJobProps): React.JSX.Element {
  const { t } = useI18n();
  const rows = job?.files ?? placeholderRows(estimate);
  const sent = new Set(estimate.files.map((file) => file.path));
  const total = estimate.files.length;
  const done = rows.filter((row) => sent.has(row.path) && FINISHED.has(row.state)).length;
  const percent = total === 0 ? 0 : Math.min(100, Math.round((done / total) * 100));

  const describe = (row: ImportFileJob): string => {
    switch (row.state) {
      case "Done":
        return row.added_propositions > 0 && running
          ? t("knowledge.import.file.Added", { count: row.added_propositions })
          : row.added_propositions > 0
            ? `${t("knowledge.import.file.Done")} · ${t("knowledge.import.file.Added", { count: row.added_propositions })}`
            : t("knowledge.import.file.Done");
      case "Running":
        return t("knowledge.import.file.Running", {
          done: Math.min(row.parts_done + 1, Math.max(row.parts, 1)),
          total: Math.max(row.parts, 1),
        });
      case "Pending":
        return running ? t("knowledge.import.file.Pending") : t("knowledge.import.file.NotSent");
      case "Partial":
        return t("knowledge.import.file.Partial", { count: Math.max(row.parts - row.parts_done, 1) });
      case "Failed":
        return row.failure === undefined
          ? t("knowledge.import.file.FailedPlain")
          : t("knowledge.import.file.Failed", { why: t(failureKeyOfKind(row.failure.kind)) });
      case "Skipped":
        return t("knowledge.import.file.Skipped", {
          why: row.reason === undefined ? "" : t(`knowledge.import.reason.${row.reason}`),
        });
    }
  };

  const title = running
    ? total === 0
      ? t("knowledge.import.job.starting")
      : t("knowledge.import.job.title", { done, total })
    : job?.state === "Cancelled"
      ? t("knowledge.import.done.cancelled")
      : job?.state === "Failed"
        ? t("knowledge.import.done.stopped")
        : t("knowledge.import.done.title");

  const added = job?.propositions_added ?? 0;

  return (
    <section className="ts-import-card" aria-label={t("knowledge.import.job.label")}>
      <p className="ts-lesson-dialog__title" role="status">
        {title}
      </p>
      {running && (
        <span
          className="ts-kg__prog ts-import-card__prog"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent}
          aria-label={t("knowledge.import.job.label")}
        >
          <i style={{ width: `${Math.max(percent, MIN_FILL)}%` }} />
        </span>
      )}
      {!running && job !== null && (
        <p className="ts-lesson__hint">
          {job.state === "Failed" && job.failure !== undefined && !rows.some((row) => row.state === "Failed") &&
            `${t(failureKeyOfKind(job.failure.kind))} `}
          {added > 0
            ? t("knowledge.import.done.added", {
                statements: t("knowledge.statements", { count: added }),
              })
            : t("knowledge.import.done.addedNone")}{" "}
          {job.state !== "Done" && added > 0 && t("knowledge.import.done.kept")}
        </p>
      )}
      <ul className="ts-import__files">
        {rows.map((row) => (
          <li key={row.path} className="ts-import__file" data-state={row.state}>
            <span className="ts-import__icon" aria-hidden="true">
              {row.state === "Skipped" && row.reason === "Denied" ? "⛔" : GLYPH[row.state]}
            </span>
            <span className="ts-import__name">{row.path}</span>
            <span className="ts-lesson__hint">{describe(row)}</span>
            {!running && (row.state === "Partial" || row.state === "Failed") && (
              <button
                type="button"
                className="ts-btn ts-btn--sm"
                aria-label={t("knowledge.import.file.retryAria", { path: row.path })}
                onClick={() => onRetry(row.path)}
              >
                {t("knowledge.import.file.retry")}
              </button>
            )}
          </li>
        ))}
      </ul>
      {estimate.skipped_more > 0 && (
        <p className="ts-lesson__hint">
          {t("knowledge.import.skippedMore", { count: estimate.skipped_more })}
        </p>
      )}
      <div className="ts-lesson__acts">
        {running ? (
          <button type="button" className="ts-btn" onClick={onCancel}>
            {t("knowledge.import.job.cancel")}
          </button>
        ) : (
          <>
            <button type="button" className="ts-btn" onClick={onClose}>
              {t("knowledge.import.done.close")}
            </button>
            <button type="button" className="ts-btn ts-btn--accent" onClick={onOpenGraph}>
              {t("knowledge.import.done.openGraph")}
            </button>
          </>
        )}
      </div>
    </section>
  );
}
