"use client";

import { useEffect } from "react";
import type { DiagnosticLocation } from "../wasm-core/typsidian_core.js";
import { Icon } from "../icons/Icons.js";
import type { SingleExport } from "./useExport.js";
import { useI18n } from "../i18n/index.js";
import { failureReason, firstErrorWithPlace, writtenBytes } from "./describe.js";

/** Сколько висит сообщение об успехе. Отказ остаётся до закрытия. */
const SUCCESS_VISIBLE_MS = 6000;

export interface ExportToastProps {
  result: SingleExport;
  onDismiss: () => void;
  /** Перейти к месту ошибки в заметке (FR-008). */
  onReveal: (place: DiagnosticLocation) => void;
  /** Открыть Центр экспорта — там список недавних файлов (FR-007). */
  onOpenCenter: () => void;
  /** Скачать файл — vault на своём сервере (фича 037). */
  downloadUrl?: string;
}

/**
 * Итог экспорта одной заметки: куда записан файл и сколько заняло (FR-007),
 * либо почему не записан — с переходом к ошибке (FR-008).
 */
export function ExportToast({
  result,
  onDismiss,
  onReveal,
  onOpenCenter,
  downloadUrl,
}: ExportToastProps): React.JSX.Element {
  const i18n = useI18n();
  const { t } = i18n;
  const { report, elapsedMs } = result;
  const bytes = writtenBytes(report);
  const ok = bytes !== null;
  const error = ok ? null : firstErrorWithPlace(report);

  useEffect(() => {
    if (!ok) return;
    const timer = setTimeout(onDismiss, SUCCESS_VISIBLE_MS);
    return () => clearTimeout(timer);
  }, [ok, onDismiss, result]);

  return (
    <div
      className={ok ? "ts-export-toast" : "ts-export-toast ts-export-toast--failed"}
      role={ok ? "status" : "alert"}
    >
      <Icon name={ok ? "check" : "x"} size={12} />
      {ok ? (
        <span className="ts-export-toast__text">
          {t("export.toast.ready")} ·{" "}
          <span className="ts-export-toast__path">{report.pdf_path}</span> ·{" "}
          {i18n.formatDuration(elapsedMs)} · {i18n.formatBytes(bytes)}
        </span>
      ) : (
        <span className="ts-export-toast__text">
          {t("export.toast.failed", { reason: failureReason(i18n, report) })}
        </span>
      )}
      <span className="ts-spacer" />
      {error?.location && (
        <button
          type="button"
          className="ts-btn"
          onClick={() => {
            if (error.location) onReveal(error.location);
            onDismiss();
          }}
        >
          {t("export.toast.toError")}
        </button>
      )}
      {ok && downloadUrl !== undefined && (
        <a className="ts-btn ts-btn--outline" href={downloadUrl} download>
          {t("export.toast.download")}
        </a>
      )}
      {ok && (
        <button type="button" className="ts-btn ts-btn--quiet" onClick={onOpenCenter}>
          {t("export.toast.openCenter")}
        </button>
      )}
      <button
        type="button"
        className="ts-btn ts-btn--ghost"
        aria-label={t("export.toast.close")}
        onClick={onDismiss}
      >
        <Icon name="x" size={12} />
      </button>
    </div>
  );
}
