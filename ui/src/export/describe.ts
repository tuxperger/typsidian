import type {
  Diagnostic,
  ExportReport,
  JobStatus,
} from "../wasm-core/typsidian_core.js";
import type { I18n } from "../i18n/index.js";
import { diagnosticText } from "../i18n/messages.js";

// Подписи экспорта словами (фича 009). Только оформление: исход, состояние
// строки и причину решило ядро, здесь они превращаются в текст на языке
// интерфейса (фича 010). Вызываются при рендере — после смены языка подпись
// перерисовывается на новом (FR-014).

/** Размер записанного файла, если он записан. */
export function writtenBytes(report: ExportReport): number | null {
  return report.outcome.kind === "Written" ? report.outcome.bytes : null;
}

/** Первая ошибка с местом в тексте — к ней ведёт «перейти к ошибке». */
export function firstErrorWithPlace(report: ExportReport): Diagnostic | null {
  return (
    report.diagnostics.find(
      (d) => d.severity === "Error" && d.location !== undefined,
    ) ?? null
  );
}

/** Причина отказа одной строкой (FR-008, FR-016, FR-031). */
export function failureReason({ t }: I18n, report: ExportReport): string {
  switch (report.outcome.kind) {
    case "Written":
      return "";
    case "NoteMissing":
      return t("export.failure.noteMissing");
    case "NoteUnreadable":
      return t("export.failure.noteUnreadable");
    case "WriteFailed":
      switch (report.outcome.reason.kind) {
        case "AccessDenied":
          return t("export.failure.accessDenied");
        case "TargetIsFolder":
          return t("export.failure.targetIsFolder", { path: report.pdf_path });
        case "Io":
          return t("export.failure.writeFailed");
      }
      return t("export.failure.writeFailed");
    case "CompileFailed": {
      const error = report.diagnostics.find((d) => d.severity === "Error");
      if (error === undefined) return t("export.failure.compileFailed");
      const message = diagnosticText(t, error);
      const place = error.location;
      return place === undefined
        ? message
        : t("export.failure.at", { message, path: place.path, line: place.line });
    }
  }
}

/** Подпись состояния строки пакета (FR-015, FR-016). */
export function jobLabel(
  i18n: I18n,
  status: JobStatus,
  report: ExportReport | undefined,
  elapsedMs: number | undefined,
): string {
  const { t } = i18n;
  switch (status) {
    case "Queued":
      return t("export.job.queued");
    case "Exporting":
      return t("export.job.exporting");
    case "Done":
    case "DoneWithWarnings": {
      const parts = [
        status === "Done" ? t("export.job.done") : t("export.job.doneWithWarnings"),
      ];
      if (elapsedMs !== undefined) parts.push(i18n.formatDuration(elapsedMs));
      const bytes = report === undefined ? null : writtenBytes(report);
      if (bytes !== null) parts.push(i18n.formatBytes(bytes));
      return parts.join(" · ");
    }
    case "Failed":
      return report === undefined
        ? t("export.job.failed")
        : t("export.job.failedWith", { reason: failureReason(i18n, report) });
    case "Skipped":
      return t("export.job.skipped");
    case "Cancelled":
      return t("export.job.cancelled");
  }
}
