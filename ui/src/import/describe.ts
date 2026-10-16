import type {
  ImportError,
  LossKind,
  ReportTexts,
  SkipReason,
} from "../wasm-core/typsidian_core.js";
import type { I18n, MessageKey, Translate } from "../i18n/index.js";

// Слова импорта для интерфейса (фича 021). Какие потери и пропуски бывают, знает
// ядро (`LossKind`, `SkipReason`); здесь только подбор перевода.

const LOSS_KEYS: Record<LossKind, MessageKey> = {
  Html: "import.report.lossHtml",
  Embed: "import.report.lossEmbed",
  PluginBlock: "import.report.lossPluginBlock",
  Diagram: "import.report.lossDiagram",
  Math: "import.report.lossMath",
  SectionLink: "import.report.lossSectionLink",
  BlockLink: "import.report.lossBlockLink",
  MissingTarget: "import.report.lossMissingTarget",
  AmbiguousTarget: "import.report.lossAmbiguousTarget",
  Properties: "import.report.lossProperties",
  Syntax: "import.report.lossSyntax",
};

const SKIP_KEYS: Record<SkipReason, MessageKey> = {
  TooLarge: "import.report.skipTooLarge",
  NotUtf8: "import.report.skipNotUtf8",
  OtherFile: "import.report.skipOtherFile",
  ExistsSkipped: "import.report.skipExistsSkipped",
  Unreadable: "import.report.skipUnreadable",
  WriteFailed: "import.report.skipWriteFailed",
};

export const lossText = (t: Translate, kind: LossKind): string => t(LOSS_KEYS[kind]);
export const skipText = (t: Translate, reason: SkipReason): string => t(SKIP_KEYS[reason]);

const pad = (value: number): string => String(value).padStart(2, "0");

/**
 * Тексты заметки отчёта на языке интерфейса (`ImportJob.texts`). Имя файла
 * несёт время запуска, чтобы отчёты разных запусков не затирали друг друга.
 */
export function reportTexts(i18n: I18n, now: Date): ReportTexts {
  const { t } = i18n;
  const stamp = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}`;
  return {
    file_name: t("import.report.fileName", { stamp }),
    title: t("import.report.title", { when: i18n.formatDate(now) }),
    source: t("import.report.source"),
    destination: t("import.report.destination"),
    summary: t("import.report.summary"),
    clean: t("import.report.clean"),
    lossy: t("import.report.lossy"),
    skipped: t("import.report.skipped"),
    resumed: t("import.report.resumed"),
    cancelled: t("import.report.cancelled"),
    lossy_heading: t("import.report.lossyHeading"),
    skipped_heading: t("import.report.skippedHeading"),
    renamed_heading: t("import.report.renamedHeading"),
    line: t("import.report.line"),
    loss_html: t(LOSS_KEYS.Html),
    loss_embed: t(LOSS_KEYS.Embed),
    loss_plugin_block: t(LOSS_KEYS.PluginBlock),
    loss_diagram: t(LOSS_KEYS.Diagram),
    loss_math: t(LOSS_KEYS.Math),
    loss_section_link: t(LOSS_KEYS.SectionLink),
    loss_block_link: t(LOSS_KEYS.BlockLink),
    loss_missing_target: t(LOSS_KEYS.MissingTarget),
    loss_ambiguous_target: t(LOSS_KEYS.AmbiguousTarget),
    loss_properties: t(LOSS_KEYS.Properties),
    loss_syntax: t(LOSS_KEYS.Syntax),
    skip_too_large: t(SKIP_KEYS.TooLarge),
    skip_not_utf8: t(SKIP_KEYS.NotUtf8),
    skip_other_file: t(SKIP_KEYS.OtherFile),
    skip_exists_skipped: t(SKIP_KEYS.ExistsSkipped),
    skip_unreadable: t(SKIP_KEYS.Unreadable),
    skip_write_failed: t(SKIP_KEYS.WriteFailed),
  };
}

const IMPORT_ERRORS: readonly ImportError["kind"][] = [
  "NoSource",
  "SessionLost",
  "AccessDenied",
  "Io",
];

/** Отказ ядра или оболочки как `ImportError`, если он им является. */
export function asImportError(cause: unknown): ImportError | null {
  if (typeof cause !== "object" || cause === null || !("kind" in cause)) return null;
  const kind = (cause as { kind: unknown }).kind;
  return IMPORT_ERRORS.find((known) => known === kind) !== undefined
    ? (cause as ImportError)
    : null;
}

/** Причина отказа словами. `openingSource` — отказ открыть саму папку. */
export function describeImportFailure(t: Translate, cause: unknown, openingSource: boolean): string {
  const known = asImportError(cause);
  if (known !== null) return t(`import.error.${known.kind}`);
  if (openingSource) return t("import.error.open");
  const detail =
    typeof cause === "object" && cause !== null && "message" in cause
      ? String((cause as { message: unknown }).message)
      : String(cause);
  return t("import.error.generic", { detail });
}
