"use client";

import { useI18n } from "../i18n/index.js";
import { ImportConfirm } from "./ImportConfirm.js";
import { ImportJob } from "./ImportJob.js";
import type { KnowledgeImport } from "./useKnowledgeImport.js";

// Все стадии импорта (фича 055) в одном месте рабочей области: оценка, подтверждение (B),
// ход (C) и итог (D). Подтверждение модальное, ход и итог — нет: правка заметок продолжается.

export interface ImportHostProps {
  flow: KnowledgeImport;
  /** «Открыть граф» в итоге. */
  onOpenGraph: () => void;
}

export function ImportHost({ flow, onOpenGraph }: ImportHostProps): React.JSX.Element | null {
  const { t } = useI18n();
  const { phase } = flow;

  switch (phase.kind) {
    case "idle":
      return null;
    case "estimating":
      return (
        <div className="ts-import-card" role="status">
          <p className="ts-lesson-dialog__title">{t("knowledge.import.estimating")}</p>
        </div>
      );
    case "refused":
      return (
        <div className="ts-import-card" role="alert">
          <p className="ts-lesson__bad">{t(phase.error)}</p>
          <div className="ts-lesson__acts">
            <button type="button" className="ts-btn" onClick={flow.dismiss}>
              {t("knowledge.import.done.close")}
            </button>
          </div>
        </div>
      );
    case "confirm":
      return (
        <ImportConfirm
          paths={phase.paths}
          estimate={phase.estimate}
          error={phase.error}
          stale={phase.stale}
          onStart={flow.start}
          onReestimate={() => flow.begin(phase.paths)}
          onCancel={flow.dismiss}
        />
      );
    case "running":
    case "done":
      return (
        <ImportJob
          estimate={phase.estimate}
          job={flow.job}
          running={phase.kind === "running"}
          onCancel={flow.cancel}
          onRetry={flow.retry}
          onOpenGraph={() => {
            flow.dismiss();
            onOpenGraph();
          }}
          onClose={flow.dismiss}
        />
      );
  }
}
