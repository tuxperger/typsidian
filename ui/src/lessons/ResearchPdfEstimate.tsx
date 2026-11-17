"use client";

import type { ResearchEstimate } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";

// Оценка чтения PDF до отправки (фича 056, макет C): файл, объём, страницы, стоимость и
// пометка «прочитано не целиком». Считает ядро (`research.estimate`); здесь только показ.

const THOUSAND = 1000;

export function ResearchPdfEstimate({ estimate }: { estimate: ResearchEstimate }): React.JSX.Element {
  const { t, formatNumber } = useI18n();
  const volume =
    estimate.units >= THOUSAND
      ? t("research.pdf.volumeK", { value: formatNumber(Math.round(estimate.units / THOUSAND)) })
      : t("research.pdf.volumeUnits", { value: formatNumber(estimate.units) });
  const cost =
    typeof estimate.cost_usd !== "number"
      ? t("research.pdf.costUnknown")
      : estimate.cost_usd < 0.01
        ? t("research.pdf.costTiny")
        : t("research.pdf.costValue", { value: formatNumber(Math.round(estimate.cost_usd * 100) / 100) });
  const costLine =
    estimate.model === undefined || estimate.model === "" ? cost : `${cost} · ${estimate.model}`;
  return (
    <>
      <dl className="ts-import__kv">
        <div>
          <dt>{t("research.pdf.file")}</dt>
          <dd className="ts-research__path">{estimate.path}</dd>
        </div>
        <div>
          <dt>{t("research.pdf.volume")}</dt>
          <dd>
            {t("research.pdf.volumeValue", {
              volume,
              pages: t("research.pdf.pages", { count: estimate.pages }),
            })}
          </dd>
        </div>
        <div>
          <dt>{t("research.pdf.cost")}</dt>
          <dd>{costLine}</dd>
        </div>
      </dl>
      {estimate.truncated && (
        <p className="ts-lesson__hint" role="status">
          {t("research.pdf.truncated", {
            read: formatNumber(estimate.pages_read),
            total: formatNumber(estimate.pages),
          })}
        </p>
      )}
    </>
  );
}
