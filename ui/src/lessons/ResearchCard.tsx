"use client";

import { useEffect, useState } from "react";
import type { ResearchEstimate, ResearchView } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";
import { ResearchPdfEstimate } from "./ResearchPdfEstimate.js";
import { hostOf, isActive, pageRanges, safeUrl } from "./researchText.js";

// Карточка исследования в ленте урока (фича 056, макет A, B, D). Рисуется по
// `LessonView.research`, а не по событиям: состояние, источники и заметку считает ядро.

export interface ResearchCardProps {
  item: ResearchView;
  client: VaultClient;
  /** Пользователь отклонил запуск, запрошенный учителем (стоимость не подтверждена). */
  declined: boolean;
  onCancel: (id: string) => void;
  onRetry: (item: ResearchView) => void;
  onOpenNote: (path: string) => void;
  /** Запуск PDF учителя после подтверждения стоимости. */
  onConfirm: (item: ResearchView) => void;
  onDecline: (item: ResearchView) => void;
}

export function ResearchCard({
  item,
  client,
  declined,
  onCancel,
  onRetry,
  onOpenNote,
  onConfirm,
  onDecline,
}: ResearchCardProps): React.JSX.Element {
  const { t } = useI18n();
  const asking = item.launch_due && item.confirm_cost && !declined;
  const status = declined ? "Declined" : item.status;
  const kindLabel = t(`research.tag.${item.kind}`);

  const statusText =
    status === "Running" && item.found > 0
      ? t("research.status.RunningFound", { sources: t("research.sources", { count: item.found }) })
      : t(`research.status.${status}`);
  const tone =
    status === "Done"
      ? "ok"
      : status === "Failed"
        ? "bad"
        : status === "Interrupted" || status === "Declined"
          ? "warn"
          : status === "Cancelled"
            ? "mute"
            : "run";

  return (
    <article className="ts-research" data-status={status} aria-label={t("research.card.label", { kind: kindLabel, question: item.question })}>
      <div className="ts-research__head">
        <span className="ts-research__kind">{kindLabel}</span>
        <span className="ts-research__q">{item.question}</span>
        <span className={`ts-research__st ts-research__st--${tone}`} role="status">
          {asking ? "" : statusText}
        </span>
        {isActive(item) && !asking && (
          <button type="button" className="ts-btn ts-btn--sm" onClick={() => onCancel(item.id)}>
            {t("research.cancel")}
          </button>
        )}
        {(status === "Interrupted" || status === "Failed") && (
          <button type="button" className="ts-btn ts-btn--sm" onClick={() => onRetry(item)}>
            {t("research.retry")}
          </button>
        )}
      </div>

      {asking && item.path !== undefined && (
        <ConfirmPdf client={client} item={item} path={item.path} onConfirm={onConfirm} onDecline={onDecline} />
      )}

      {status === "Interrupted" && <p className="ts-research__hint">{t("research.interruptedHint")}</p>}
      {status === "Declined" && <p className="ts-research__hint">{t("research.declinedHint")}</p>}
      {status === "Failed" && item.failure !== undefined && (
        <p className="ts-lesson__bad ts-research__hint" role="alert">
          {t(`ai.failure.${item.failure.kind}`)}
        </p>
      )}

      {status === "Done" && <Result item={item} onOpenNote={onOpenNote} />}
    </article>
  );
}

function Result({
  item,
  onOpenNote,
}: {
  item: ResearchView;
  onOpenNote: (path: string) => void;
}): React.JSX.Element {
  const { t } = useI18n();
  const web = item.kind === "Web";
  return (
    <>
      {item.summary !== undefined && item.summary !== "" && <p className="ts-research__body">{item.summary}</p>}
      {item.sources.length === 0 ? (
        <p className="ts-research__hint">{t("research.noSources")}</p>
      ) : web ? (
        <ol className="ts-research__src">
          {item.sources.map((source, index) => {
            const url = safeUrl(source.url);
            const label = url === undefined ? source.title : `${source.title} — ${hostOf(url)}`;
            return (
              <li key={`${index}:${source.url ?? source.title}`}>
                {url === undefined ? (
                  label
                ) : (
                  <a
                    href={url}
                    target="_blank"
                    rel="noopener noreferrer"
                    aria-label={t("research.sourceOpen", { title: source.title })}
                  >
                    {label}
                  </a>
                )}
              </li>
            );
          })}
        </ol>
      ) : (
        <ul className="ts-research__src">
          {item.sources.map((source, index) => (
            <li key={`${index}:${source.title}`}>
              {(source.pages ?? []).length > 0 && (
                <>
                  {pageRanges(source.pages ?? [])
                    .map((range) => t("research.page", { n: range }))
                    .join(", ")}
                  {" · "}
                </>
              )}
              {item.path !== undefined ? (
                <button type="button" className="ts-lesson__link" onClick={() => onOpenNote(item.path ?? "")}>
                  {item.path}
                </button>
              ) : (
                source.title
              )}
            </li>
          ))}
        </ul>
      )}
      {web && <span className="ts-research__mark">{t("research.mark")}</span>}
      {item.note !== undefined && (
        <div className="ts-lesson__acts">
          <button type="button" className="ts-btn ts-btn--sm" onClick={() => onOpenNote(item.note ?? "")}>
            {t("research.openNote")}
          </button>
        </div>
      )}
    </>
  );
}

/** Подтверждение стоимости PDF, который просит учитель: та же оценка, что в макете C. */
function ConfirmPdf({
  client,
  item,
  path,
  onConfirm,
  onDecline,
}: {
  client: VaultClient;
  item: ResearchView;
  path: string;
  onConfirm: (item: ResearchView) => void;
  onDecline: (item: ResearchView) => void;
}): React.JSX.Element {
  const { t } = useI18n();
  const [estimate, setEstimate] = useState<ResearchEstimate | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let alive = true;
    client.research
      .estimate(path)
      .then((found) => alive && setEstimate(found))
      .catch(() => alive && setFailed(true));
    return () => {
      alive = false;
    };
  }, [client, path]);
  return (
    <div className="ts-research__ask">
      <p className="ts-research__hint">{t("research.confirm.hint")}</p>
      {estimate !== null && <ResearchPdfEstimate estimate={estimate} />}
      {failed && (
        <p className="ts-lesson__bad" role="alert">
          {t("research.box.denied", { path })}
        </p>
      )}
      <div className="ts-lesson__acts">
        <button type="button" className="ts-btn ts-btn--sm" onClick={() => onDecline(item)}>
          {t("research.pdf.cancel")}
        </button>
        <button
          type="button"
          className="ts-btn ts-btn--sm ts-btn--accent"
          disabled={estimate === null || estimate.problem !== undefined}
          onClick={() => onConfirm(item)}
        >
          {t("research.pdf.study")}
        </button>
      </div>
    </div>
  );
}
