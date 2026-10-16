"use client";

import { useState } from "react";
import type { ReportEntry } from "../wasm-core/typsidian_core.js";
import { Icon } from "../icons/Icons.js";
import { useI18n, type Translate } from "../i18n/index.js";
import { lossText, skipText } from "./describe.js";
import type { ImportRun } from "./useImport.js";

// Ход и итог импорта из Obsidian (фича 021; макет — артборд C2, предложение
// снимка — подпись к C3). Карточка в углу, как у пакетного экспорта 009: работа
// продолжается, пока она открыта; свернуть можно в строку шапки.

/** Сколько проблемных заметок показывает итог; остальное — в заметке отчёта. */
const PROBLEMS_SHOWN = 5;

export interface ImportProgressProps {
  run: ImportRun;
  onCancel: () => void;
  onToggleMinimized: () => void;
  onDismiss: () => void;
  onOpenReport: (path: string) => void;
  /**
   * Снимок истории 011 после импорта (FR-012): есть, только когда история
   * включена. Возвращает, удался ли снимок.
   */
  takeSnapshot: ((message: string) => Promise<boolean>) | null;
}

export function ImportProgress({
  run,
  onCancel,
  onToggleMinimized,
  onDismiss,
  onOpenReport,
  takeSnapshot,
}: ImportProgressProps): React.JSX.Element | null {
  const { t } = useI18n();
  const { state } = run;
  const total = state?.total ?? 0;
  const done = state?.next ?? 0;
  const over = run.phase === "done" || run.phase === "failed";
  const percent = total === 0 ? 0 : Math.min(100, (done / total) * 100);

  // Свёрнутую карточку заменяет кнопка в шапке окна; итог сворачивать нечего.
  if (run.minimized && !over) return null;

  const title = !over
    ? t("import.title")
    : run.cancelled
      ? t("import.done.cancelled")
      : run.phase === "failed"
        ? t("import.title")
        : t("import.done.title");

  return (
    <section
      className="ts-import-card"
      aria-label={t("import.title")}
      aria-live={over ? "polite" : "off"}
    >
      <div className="ts-import-card__head">
        <b>{title}</b>
        <span className="ts-import-card__count">
          {t("import.progress.counter", { done, total })}
        </span>
      </div>

      <div
        className="ts-import-card__bar"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={total}
        aria-valuenow={done}
        aria-label={t("import.progress.label", { done, total })}
      >
        <span style={{ width: `${percent}%` }} />
      </div>

      {!over && state?.current !== undefined && (
        <div className="ts-import-card__current" title={state.current}>
          {state.current}
        </div>
      )}

      {state !== null && (
        <div className="ts-import-card__tally">
          <span>
            <i className="ts-import-card__dot ts-import-card__dot--clean" aria-hidden="true" />
            {t("import.progress.clean", { count: state.clean })}
          </span>
          <span>
            <i className="ts-import-card__dot ts-import-card__dot--lossy" aria-hidden="true" />
            {t("import.progress.lossy", { count: state.lossy })}
          </span>
          <span>
            <i className="ts-import-card__dot ts-import-card__dot--skipped" aria-hidden="true" />
            {t("import.progress.skipped", { count: state.skipped })}
          </span>
          {state.resumed > 0 && (
            <span>
              <i className="ts-import-card__dot ts-import-card__dot--resumed" aria-hidden="true" />
              {t("import.progress.resumed", { count: state.resumed })}
            </span>
          )}
        </div>
      )}

      {over && state !== null && total === 0 && (
        <p className="ts-dialog__hint">{t("import.done.empty")}</p>
      )}

      {over && run.cancelled && (
        <p className="ts-dialog__hint">{t("import.done.resumeHint")}</p>
      )}

      {over && state !== null && state.recent.length > 0 && (
        <Problems t={t} entries={state.recent.slice(-PROBLEMS_SHOWN)} />
      )}

      {run.error !== null && (
        <p className="ts-dialog__error" role="alert">
          {run.error}
        </p>
      )}
      {over && run.phase === "done" && run.reportPath === null && run.error === null && (
        <p className="ts-dialog__error" role="alert">
          {t("import.done.reportFailed")}
        </p>
      )}

      {over && state !== null && state.clean + state.lossy > 0 && takeSnapshot !== null && (
        <SnapshotOffer take={() => takeSnapshot(t("import.done.snapshotMessage"))} />
      )}

      <div className="ts-import-card__actions">
        {!over ? (
          <>
            <button
              type="button"
              className="ts-btn ts-btn--ghost"
              aria-label={t("import.progress.minimize")}
              title={t("import.progress.minimize")}
              onClick={onToggleMinimized}
            >
              <Icon name="chevron" size={12} />
            </button>
            <span className="ts-spacer" />
            <button
              type="button"
              className="ts-btn ts-btn--outline"
              disabled={run.cancelling || run.phase === "finishing"}
              onClick={onCancel}
            >
              {run.cancelling ? t("import.progress.cancelling") : t("import.progress.cancel")}
            </button>
          </>
        ) : (
          <>
            <span className="ts-spacer" />
            <button type="button" className="ts-btn" onClick={onDismiss}>
              {t("import.done.close")}
            </button>
            {run.reportPath !== null && (
              <button
                type="button"
                className="ts-btn ts-btn--accent"
                onClick={() => {
                  if (run.reportPath !== null) onOpenReport(run.reportPath);
                }}
              >
                {t("import.done.openReport")}
              </button>
            )}
          </>
        )}
      </div>
    </section>
  );
}

function Problems({ t, entries }: { t: Translate; entries: ReportEntry[] }): React.JSX.Element {
  return (
    <div className="ts-import-card__problems">
      <span className="ts-field__label">{t("import.done.problems")}</span>
      <ul>
        {entries.map((entry, index) => {
          const reason =
            entry.outcome === "Skipped" && entry.skip_reason !== undefined
              ? skipText(t, entry.skip_reason)
              : entry.losses[0] !== undefined
                ? lossText(t, entry.losses[0].kind)
                : "";
          return (
            <li key={`${index}:${entry.source}`} title={reason}>
              <span className="ts-import-card__source">{entry.source}</span>
              <span className="ts-import-card__reason">{reason}</span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

type SnapshotPhase = "idle" | "busy" | "taken" | "failed";

/** Предложение снимка истории: импортированное откатывается одним шагом. */
function SnapshotOffer({ take }: { take: () => Promise<boolean> }): React.JSX.Element {
  const { t } = useI18n();
  const [phase, setPhase] = useState<SnapshotPhase>("idle");
  if (phase === "taken") {
    return (
      <p className="ts-dialog__hint" role="status">
        {t("import.done.snapshotTaken")}
      </p>
    );
  }
  return (
    <div className="ts-import-card__offer">
      <p className="ts-dialog__hint">{t("import.done.snapshotOffer")}</p>
      {phase === "failed" && (
        <p className="ts-dialog__error" role="alert">
          {t("import.done.snapshotFailed")}
        </p>
      )}
      <button
        type="button"
        className="ts-btn ts-btn--outline"
        disabled={phase === "busy"}
        onClick={() => {
          setPhase("busy");
          take().then(
            (ok) => setPhase(ok ? "taken" : "failed"),
            () => setPhase("failed"),
          );
        }}
      >
        {t("import.done.snapshot")}
      </button>
    </div>
  );
}
