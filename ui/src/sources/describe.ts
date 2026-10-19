import type { Translate } from "../i18n/index.js";
import type {
  CitationMark,
  CitationPlanError,
  CreateSourceFileError,
  Source,
  SourceReadProblemKind,
} from "../wasm-core/typsidian_core.js";
import type { EditorMark } from "../editor/marks.js";

// Значения ядра словами (фича 017). Правил здесь нет — только перевод.

export function problemText(t: Translate, kind: SourceReadProblemKind): string {
  switch (kind.kind) {
    case "Syntax":
      return t("sources.problem.syntax");
    case "Field":
      return t("sources.problem.field");
    case "DuplicateKey":
      return t("sources.problem.duplicate");
    case "NoEntryType":
      return t("sources.problem.noType");
    case "FileUnreadable":
      return t("sources.problem.unreadable");
    case "FileSyntax":
      return t("sources.problem.fileSyntax");
    case "TooMany":
      return t("sources.problem.tooMany", { hidden: kind.hidden });
  }
}

/** Технический текст крейта — подробностью под причиной. */
export function problemDetail(kind: SourceReadProblemKind): string | null {
  switch (kind.kind) {
    case "Syntax":
    case "Field":
    case "FileSyntax":
      return kind.detail;
    case "FileUnreadable":
      return kind.reason;
    default:
      return null;
  }
}

export function planErrorText(t: Translate, error: CitationPlanError): string {
  if (error.kind === "UnknownKey") return t("sources.plan.unknownKey");
  return error.mode === "Comment" ? t("sources.plan.comment") : t("sources.plan.string");
}

export function createErrorText(t: Translate, error: CreateSourceFileError): string {
  switch (error.kind) {
    case "AlreadyExists":
      return t("sources.create.alreadyExists");
    case "AccessDenied":
      return t("sources.create.accessDenied");
    case "Io":
      return t("sources.create.io");
  }
}

/** «Кнут Д. Э., Ершов А. · 1984» — строка палитры и панели. */
export function authorsLine(source: Source): string {
  const authors = source.authors.length > 3
    ? `${source.authors.slice(0, 3).join(", ")} …`
    : source.authors.join(", ");
  return [authors, source.year === undefined ? "" : String(source.year)]
    .filter((part) => part !== "")
    .join(" · ");
}

/** Отметки ядра — входом общего механизма с переведённой причиной. */
export function citationMarks(t: Translate, marks: CitationMark[]): EditorMark[] {
  return marks.map((mark) => {
    const { key, ambiguous } = mark.kind;
    return {
      from: mark.place.from,
      to: mark.place.to,
      tone: "error",
      message: ambiguous ? t("sources.mark.ambiguous", { key }) : t("sources.mark.missing", { key }),
    };
  });
}
