import type { Translate } from "../i18n/index.js";
import type {
  AttachmentRefusalReason,
  BatchItem,
  ItemStatus,
  StoreProblem,
} from "../wasm-core/typsidian_core.js";

// Исходы пачки вложений — словами (фича 016, макет 1b). Причины называет ядро;
// здесь только перевод значений в текст, без собственных правил.

/** Имя файла для отчёта: своё у перетащенного, «из буфера» — у безымянного. */
export function itemName(t: Translate, item: BatchItem): string {
  const saved = savedPath(item.status);
  if (saved !== null) return basename(saved);
  return item.incoming.name ?? t("attachments.unnamed");
}

/** Путь записанного или найденного файла. */
export function savedPath(status: ItemStatus): string | null {
  return status.kind === "Stored" || status.kind === "Reused" ? status.path : null;
}

export function refusalText(t: Translate, reason: AttachmentRefusalReason): string {
  return reason === "ReadOnly"
    ? t("attachments.refused.readOnly")
    : t("attachments.refused.notANote");
}

export function problemText(t: Translate, problem: StoreProblem): string {
  switch (problem.kind) {
    case "AccessDenied":
      return t("attachments.problem.accessDenied");
    case "Io":
      return t("attachments.problem.io");
    case "NameBusy":
      return t("attachments.problem.nameBusy");
    case "SourceUnreadable":
      return t("attachments.problem.sourceUnreadable");
  }
}

/** Почему единственный файл не вставлен; `null` — вставлен. */
export function itemFailure(t: Translate, status: ItemStatus): string | null {
  switch (status.kind) {
    case "Failed":
      return problemText(t, status.problem);
    case "Skipped":
      return t("attachments.problem.skipped");
    default:
      return null;
  }
}

function basename(path: string): string {
  const at = path.lastIndexOf("/");
  return at === -1 ? path : path.slice(at + 1);
}
