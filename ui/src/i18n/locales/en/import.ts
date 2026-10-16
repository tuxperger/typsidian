import type { Entries } from "../define.js";
import { enN } from "../define.js";
import { formatNumber } from "../../format.js";

const num = (value: number): string => formatNumber("en", value);

// Импорт из Obsidian (фича 021): диалог C1/C4/C5, карточка хода C2, предложение
// снимка и тексты заметки отчёта C3 (уходят в ядро через `ImportJob.texts`).
export const importing = {
  "import.command": "Import from Obsidian…",
  "import.pickTitle": "Choose the folder with Obsidian notes",
  "import.title": "Import from Obsidian",

  // ── C1: настройка ──────────────────────────────────────────────────
  "import.source.label": "From — folder with Markdown notes",
  "import.source.none": "No folder chosen",
  "import.source.choose": "Choose…",
  "import.source.again": "Choose another…",
  "import.survey.reading": "Counting files…",
  "import.survey.notes": (p: { count: number }) => enN(p.count, "note", "notes"),
  "import.survey.images": (p: { count: number }) => enN(p.count, "image", "images"),
  "import.survey.other": (p: { count: number }) => enN(p.count, "other file", "other files"),
  "import.survey.excluded": "without .obsidian/ and .trash/",
  "import.dest.label": "To — folder in the vault",
  "import.dest.root": "vault root",
  "import.dest.default": "from-obsidian",
  "import.dest.change": "Change",
  "import.dest.done": "Done",
  "import.dest.input": "Destination folder in the vault",
  "import.dest.hint":
    "Images go in by the vault attachment rules (016). The source folder isn’t changed.",
  "import.conflict.label": "If a file with the same name already exists",
  "import.conflict.skip": "Skip it",
  "import.conflict.skipHint":
    "The existing file stays; the note is listed in the report as skipped",
  "import.conflict.beside": "Put it beside",
  "import.conflict.besideHint": "Under the name “name (import).typ”",
  "import.other.copy": "Copy other files as they are",
  "import.other.copyHint": (p: { count: number }) =>
    `PDF, audio, archives — ${num(p.count)}. Otherwise they are listed in the report as skipped`,
  "import.start": (p: { count: number }) => `Import ${enN(p.count, "# note", "# notes")}`,

  // ── C4: продолжение ────────────────────────────────────────────────
  "import.resume.intro": (p: { when: string; done: number; total: number }) =>
    `This folder was already imported here ${p.when}; the import stopped at ${num(p.done)} / ${num(p.total)}.`,
  "import.resume.hint": (p: { done: number; left: number }) =>
    `What was moved earlier won’t be repeated: ${num(p.done)} already in place, ${num(p.left)} left to move. Links between old and new notes will match up.`,
  "import.resume.start": (p: { count: number }) => `Continue the import — ${num(p.count)}`,
  "import.resume.fresh": "Choose another folder…",

  // ── C5: нет доступа к папкам ───────────────────────────────────────
  "import.unavailable":
    "This browser can’t pick a folder on disk, so the import isn’t available. Open Typsidian in Chrome or Edge, or in the desktop app.",
  "import.noVault": "Open a vault first: the import puts notes into it.",

  // ── Отказы ─────────────────────────────────────────────────────────
  "import.error.open": "The folder couldn’t be opened.",
  "import.error.NoSource": "The source folder isn’t open any more. Choose it again.",
  "import.error.SessionLost": "The import session was lost.",
  "import.error.AccessDenied": "No access to the folder or the vault.",
  "import.error.Io": "A disk error interrupted the import.",
  "import.error.generic": (p: { detail: string }) => `The import failed: ${p.detail}`,

  // ── C2: ход ────────────────────────────────────────────────────────
  "import.progress.counter": (p: { done: number; total: number }) => `${num(p.done)} / ${num(p.total)}`,
  "import.progress.cancel": "Cancel",
  "import.progress.cancelling": "Cancelling…",
  "import.progress.minimize": "Collapse to the status line",
  "import.progress.expand": "Show the import progress",
  "import.progress.label": (p: { done: number; total: number }) =>
    `Import progress: ${num(p.done)} of ${num(p.total)}`,
  "import.progress.clean": (p: { count: number }) => `${num(p.count)} clean`,
  "import.progress.lossy": (p: { count: number }) => `${num(p.count)} with losses`,
  "import.progress.skipped": (p: { count: number }) => `${num(p.count)} skipped`,
  "import.progress.resumed": (p: { count: number }) => `${num(p.count)} moved earlier`,
  "import.pill": (p: { done: number; total: number }) => `import ${num(p.done)} / ${num(p.total)}`,

  // ── Итог ───────────────────────────────────────────────────────────
  "import.done.title": "Import finished",
  "import.done.cancelled": "Import cancelled",
  "import.done.empty": "There was nothing to import.",
  "import.done.openReport": "Open the report",
  "import.done.reportFailed": "The report note wasn’t written.",
  "import.done.resumeHint": "Run the import again with the same folders to continue.",
  "import.done.problems": "Needs a look",
  "import.done.snapshotOffer":
    "Take a history snapshot so the imported notes can be rolled back as one step?",
  "import.done.snapshot": "Take a snapshot",
  "import.done.snapshotTaken": "Snapshot taken.",
  "import.done.snapshotFailed": "The snapshot wasn’t taken.",
  "import.done.snapshotMessage": "Import from Obsidian",
  "import.done.close": "Close",

  // ── Тексты заметки отчёта (C3) ─────────────────────────────────────
  "import.report.fileName": (p: { stamp: string }) => `import-${p.stamp}`,
  "import.report.title": (p: { when: string }) => `Import from Obsidian — ${p.when}`,
  "import.report.source": "Source",
  "import.report.destination": "Destination",
  "import.report.summary": "Summary",
  "import.report.clean": "Imported cleanly",
  "import.report.lossy": "Imported with losses",
  "import.report.skipped": "Skipped",
  "import.report.resumed": "Imported earlier",
  "import.report.cancelled": "The import was cancelled before it finished.",
  "import.report.lossyHeading": "Notes with losses",
  "import.report.skippedHeading": "Skipped files",
  "import.report.renamedHeading": "Renamed",
  "import.report.line": "line {n}",
  "import.report.lossHtml": "HTML left as source text",
  "import.report.lossEmbed": "Embed replaced by a link or left as source text",
  "import.report.lossPluginBlock": "Plugin block (Dataview, Tasks…) left as source text",
  "import.report.lossDiagram": "Diagram (mermaid…) left as source text",
  "import.report.lossMath": "Formula left as source text",
  "import.report.lossSectionLink": "Link to a section leads to the whole note",
  "import.report.lossBlockLink": "Link to a block leads to the whole note",
  "import.report.lossMissingTarget": "Link target not found",
  "import.report.lossAmbiguousTarget": "Several notes share the name; the nearest was used",
  "import.report.lossProperties": "Properties header left as a comment",
  "import.report.lossSyntax": "Note left as source text",
  "import.report.skipTooLarge": "file is larger than 5 MB",
  "import.report.skipNotUtf8": "text isn’t UTF-8",
  "import.report.skipOtherFile": "not Markdown or an image; copying other files is off",
  "import.report.skipExistsSkipped": "a file with this name already exists",
  "import.report.skipUnreadable": "the file couldn’t be read",
  "import.report.skipWriteFailed": "the note couldn’t be written",
} as const satisfies Entries;
