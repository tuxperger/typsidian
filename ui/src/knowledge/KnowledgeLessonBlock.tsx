"use client";

import type { KnowledgeJob } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import type { KnowledgeExtract } from "./useKnowledgeExtract.js";

// Граф знаний в ленте урока (фича 054, макет C): строка итога последнего пополнения этого
// урока и «Пополнить граф». Кнопка нужна, пока автопополнение выключено, и когда в графе
// ещё есть непокрытые объяснения: ручное пополнение доступно всегда (FR-003).

export interface KnowledgeLessonBlockProps {
  path: string;
  /** Символов объяснений учителя, ещё не попавших в граф (`LessonView`). */
  pendingChars: number;
  jobs: readonly KnowledgeJob[];
  extract: KnowledgeExtract;
  onOpenKnowledge: () => void;
}

export function KnowledgeLessonBlock({
  path,
  pendingChars,

  jobs,
  extract,
  onOpenKnowledge,
}: KnowledgeLessonBlockProps): React.JSX.Element | null {
  const { t, formatNumber } = useI18n();
  const mine = jobs.filter((job) => job.source === path);
  const running = mine.some((job) => job.state === "Running");
  const last = mine
    .filter((job) => job.state === "Done")
    .reduce<KnowledgeJob | undefined>(
      (best, job) => (best === undefined || (job.finished_at ?? 0) > (best.finished_at ?? 0) ? job : best),
      undefined,
    );
  // Непокрытые объяснения — повод для кнопки. При включённом автопополнении оно идёт само,
  // и кнопка остаётся на случай отказа или короткого урока, при выключенном — это единственный путь.
  const canFill = pendingChars > 0 || extract.busy;
  if (last === undefined && !canFill && extract.error === null) return null;

  const summary =
    last === undefined
      ? null
      : last.propositions_added === 0 && last.chains_added === 0
        ? t("knowledge.lesson.summaryNone")
        : t("knowledge.lesson.summary", {
            statements: t("knowledge.statements", { count: last.propositions_added }),
            chains: t("knowledge.chainsCount", { count: last.chains_added }),
          });

  return (
    <div className="ts-kg__lesson">
      {summary !== null && last !== undefined && (
        <div className="ts-lesson__read">
          <span>
            ◇ {summary}
            {last.propositions_merged > 0 && last.propositions_added > 0
              ? ` ${t("knowledge.lesson.summaryMerged", { count: last.propositions_merged })}`
              : ""}
          </span>
          <button type="button" className="ts-lesson__link" onClick={onOpenKnowledge}>
            {t("knowledge.lesson.open")}
          </button>
        </div>
      )}
      {canFill && (
        <div className="ts-kg__lesson-row">
          <button
            type="button"
            className="ts-btn"
            disabled={extract.busy || running}
            onClick={extract.start}
          >
            {t("knowledge.lesson.fill")}
          </button>
          <span className="ts-lesson__hint">
            {t("knowledge.lesson.fillHint", { count: formatNumber(pendingChars) })}
          </span>
        </div>
      )}
      {extract.error !== null && (
        <p className="ts-lesson__bad" role="alert">
          {t(extract.error)}
        </p>
      )}
    </div>
  );
}
