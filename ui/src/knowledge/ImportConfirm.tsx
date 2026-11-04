"use client";

import { useId, useState } from "react";
import type { ImportEstimate } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import type { FailureKey } from "./knowledgeText.js";

// Подтверждение импорта до отправки (фича 055, макет B, FR-002): сколько файлов уйдёт модели,
// объём, примерная стоимость и модель, что пропущено и почему. Оценку считает ядро
// (`knowledge.importEstimate`); здесь она только показывается и передаётся запуску как есть.

/** Из скольких единиц объёма показывается «тыс.». */
const THOUSAND = 1000;

export interface ImportConfirmProps {
  /** Выбранные пути: для заголовка. */
  paths: readonly string[];
  estimate: ImportEstimate;
  /** Отказ запуска — например, оценка устарела. */
  error: FailureKey | null;
  /** Оценка устарела: «Добавить» заменяется на «Оценить заново». */
  stale: boolean;
  onStart: () => void;
  onReestimate: () => void;
  onCancel: () => void;
}

export function ImportConfirm({
  paths,
  estimate,
  error,
  stale,
  onStart,
  onReestimate,
  onCancel,
}: ImportConfirmProps): React.JSX.Element {
  const { t, formatNumber } = useI18n();
  const id = useId();
  const [showSkipped, setShowSkipped] = useState(false);
  useDismissOnEscape(onCancel);

  const notes = estimate.files.filter((file) => file.kind === "Note").length;
  const texts = estimate.files.filter((file) => file.kind === "Text").length;
  const pdfs = estimate.files.filter((file) => file.kind === "Pdf").length;
  const kinds = [
    notes > 0 ? t("knowledge.import.kindNotes", { count: notes }) : "",
    texts > 0 ? t("knowledge.import.kindText", { count: texts }) : "",
    pdfs > 0 ? t("knowledge.import.kindPdf", { count: pdfs }) : "",
  ]
    .filter((part) => part !== "")
    .join(", ");
  const skippedTotal = estimate.skipped.length + estimate.skipped_more;
  const covered = estimate.files.reduce((sum, file) => sum + (file.parts - file.parts_new), 0);
  const sendable = estimate.files.length > 0;

  const volume =
    estimate.units >= THOUSAND
      ? t("knowledge.import.volumeK", { value: formatNumber(Math.round(estimate.units / THOUSAND)) })
      : t("knowledge.import.volumeUnits", { value: formatNumber(estimate.units) });
  const cost =
    typeof estimate.cost_usd !== "number"
      ? t("knowledge.import.costUnknown")
      : estimate.cost_usd < 0.01
        ? t("knowledge.import.costTiny")
        : t("knowledge.import.costValue", { value: formatNumber(Math.round(estimate.cost_usd * 100) / 100) });
  const costLine =
    estimate.model === undefined || estimate.model === ""
      ? cost
      : t("knowledge.import.costWithModel", { cost, model: estimate.model });

  return (
    <div className="ts-overlay ts-overlay--center" role="presentation">
      <section
        className="ts-lesson-dialog ts-import"
        role="dialog"
        aria-modal="true"
        aria-labelledby={`${id}-title`}
      >
        <h2 className="ts-lesson-dialog__title" id={`${id}-title`}>
          {paths.length === 1
            ? t("knowledge.import.confirm.title", { name: paths[0] ?? "" })
            : t("knowledge.import.confirm.titleMany", { count: paths.length })}
        </h2>

        <dl className="ts-import__kv">
          <div>
            <dt>{t("knowledge.import.files")}</dt>
            <dd>
              {sendable
                ? t("knowledge.import.filesValue", { total: formatNumber(estimate.files.length), kinds })
                : "0"}
            </dd>
          </div>
          <div>
            <dt>{t("knowledge.import.volume")}</dt>
            <dd>{volume}</dd>
          </div>
          <div>
            <dt>{t("knowledge.import.cost")}</dt>
            <dd>{costLine}</dd>
          </div>
          {skippedTotal > 0 && (
            <div>
              <dt>{t("knowledge.import.skipped")}</dt>
              <dd>
                {t("knowledge.import.skippedValue", { count: skippedTotal })}{" "}
                <button
                  type="button"
                  className="ts-btn ts-btn--sm"
                  aria-expanded={showSkipped}
                  aria-controls={`${id}-skipped`}
                  onClick={() => setShowSkipped((open) => !open)}
                >
                  {showSkipped ? t("knowledge.import.skippedHide") : t("knowledge.import.skippedShow")}
                </button>
              </dd>
            </div>
          )}
        </dl>

        {showSkipped && (
          <ul className="ts-import__skipped" id={`${id}-skipped`}>
            {estimate.skipped.map((item) => (
              <li key={item.path}>
                <span className="ts-import__name">{item.path}</span>
                <span className="ts-lesson__hint">{t(`knowledge.import.reason.${item.reason}`)}</span>
              </li>
            ))}
            {estimate.skipped_more > 0 && (
              <li className="ts-lesson__hint">{t("knowledge.import.skippedMore", { count: estimate.skipped_more })}</li>
            )}
          </ul>
        )}

        {covered > 0 && <p className="ts-lesson__hint">{t("knowledge.import.covered", { count: covered })}</p>}
        {(!sendable || estimate.parts_new === 0) && (
          <p className="ts-lesson__hint" role="status">
            {t("knowledge.import.nothingNew")}
          </p>
        )}
        <p className="ts-lesson__hint">{t("knowledge.import.hint")}</p>
        {error !== null && (
          <p className="ts-lesson__bad" role="alert">
            {t(error)}
          </p>
        )}

        <div className="ts-lesson__acts">
          <button type="button" className="ts-btn" onClick={onCancel}>
            {t("knowledge.import.cancel")}
          </button>
          {stale ? (
            // eslint-disable-next-line jsx-a11y/no-autofocus -- окно открыто ради этого решения
            <button type="button" className="ts-btn ts-btn--accent" autoFocus onClick={onReestimate}>
              {t("knowledge.import.reestimate")}
            </button>
          ) : (
            // eslint-disable-next-line jsx-a11y/no-autofocus -- окно открыто ради этого решения
            <button type="button" className="ts-btn ts-btn--accent" autoFocus disabled={!sendable} onClick={onStart}>
              {t("knowledge.import.start")}
            </button>
          )}
        </div>
      </section>
    </div>
  );
}
