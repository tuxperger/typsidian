"use client";

import { useState } from "react";
import type { KnowledgeJob } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";
import { failureKeyOfKind, sourceName } from "./knowledgeText.js";

// Полоса задания пополнения графа (фича 054, макет A): что пополняется, ход по пачкам и
// «Отменить». Урок и вид графа задание не ждут (FR-004). Отказ посреди задания остаётся
// строкой с причиной, пока её не скроют: готовое при этом уже в графе.

/** Отказавшее задание показывается, пока оно не старше этого. */
const FAILED_SHOWN_MS = 10 * 60 * 1000;
/** Минимум заполнения полосы, процентов: первая пачка идёт, и пустая полоса выглядела бы остановкой. */
const MIN_FILL = 6;

export interface KnowledgeJobsProps {
  client: VaultClient;
  vaultId: string;
  jobs: readonly KnowledgeJob[];
  /** Только задания этого урока; без значения — все. */
  source?: string;
}

export function KnowledgeJobs({ client, vaultId, jobs, source }: KnowledgeJobsProps): React.JSX.Element | null {
  const { t } = useI18n();
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(() => new Set());
  const mine = jobs.filter((job) => source === undefined || job.source === source);
  const running = mine.filter((job) => job.state === "Running");
  const failed = mine.filter(
    (job) =>
      job.state === "Failed" &&
      !dismissed.has(job.id) &&
      Date.now() - (job.finished_at ?? job.started_at) < FAILED_SHOWN_MS,
  );
  if (running.length === 0 && failed.length === 0) return null;

  return (
    <div className="ts-kg__jobs">
      {running.map((job) => {
        const total = Math.max(job.batches_total, 0);
        const percent = total === 0 ? 0 : Math.min(100, Math.round((job.batches_done / total) * 100));
        return (
          <div key={job.id} className="ts-kg__job" role="status" aria-label={t("knowledge.job.label")}>
            <span className="ts-kg__job-text">
              ◌ {t("knowledge.job.running", { name: sourceName(job.source) })} ·{" "}
              {total === 0
                ? t("knowledge.job.starting")
                : t("knowledge.job.batches", { done: Math.min(job.batches_done + 1, total), total })}
            </span>
            <span
              className="ts-kg__prog"
              role="progressbar"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={percent}
              aria-label={t("knowledge.job.label")}
            >
              <i style={{ width: `${Math.max(percent, MIN_FILL)}%` }} />
            </span>
            <button
              type="button"
              className="ts-btn ts-btn--sm"
              onClick={() => client.ai.cancel(vaultId, job.id)}
            >
              {t("knowledge.job.cancel")}
            </button>
          </div>
        );
      })}
      {failed.map((job) => (
        <div key={job.id} className="ts-kg__job ts-kg__job--err" role="alert">
          <span className="ts-kg__job-text">
            {t("knowledge.job.failed", { name: sourceName(job.source) })}{" "}
            {job.failure !== undefined && t(failureKeyOfKind(job.failure.kind))}{" "}
            {(job.propositions_added > 0 || job.chains_added > 0) && t("knowledge.job.failedKept")}
          </span>
          <button
            type="button"
            className="ts-btn ts-btn--sm"
            onClick={() => setDismissed((prev) => new Set([...prev, job.id]))}
          >
            {t("knowledge.job.dismiss")}
          </button>
        </div>
      ))}
    </div>
  );
}
