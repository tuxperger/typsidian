import type { Entries } from "../define.js";
import { enN } from "../define.js";

// Колонка заметки (`notePanel.*`) и её режим «Связи» (`links.*`) — фича 013.
export const links = {
  // ── Оболочка колонки ───────────────────────────────────────────────
  "notePanel.button": "Note panel",
  "notePanel.command": "Show or hide the note panel",
  "notePanel.outlineCommand": "Show or hide the note outline",
  "notePanel.label": "Note panel",
  "notePanel.modes": "Panel mode",
  "notePanel.close": "Close the note panel",
  "notePanel.openNote": "Open a note to see how it is connected.",

  // ── Режим «Связи» ──────────────────────────────────────────────────
  "links.mode": "Links",
  "links.backlinks.title": "Backlinks",
  "links.mentions.title": "Mentions",
  "links.backlinks.collecting": "Collecting vault link data… The list will appear on its own.",
  "links.backlinks.failed": "Could not load the links. The list will refresh on the next change.",
  "links.backlinks.empty": "No note links here yet.",
  "links.backlinks.emptyExample": "A link from another note looks like",
  "links.problem.tooLarge": "The note is over the size limit — the snippet is not shown.",
  "links.problem.unreadable": "The note cannot be read — the snippet is not shown.",
  "links.count": (p: { count: number }) => enN(p.count, "# link", "# links"),
  "links.notesCount": (p: { count: number }) => enN(p.count, "# note", "# notes"),
  "links.rowLabel": (p: { note: string; line: number }) => `open ${p.note}, line ${p.line}`,
  "links.partial": (p: { shown: number; total: number }) =>
    `showing ${p.shown} of ${p.total} notes, closest by folder first`,
  "links.more": (p: { count: number }) => `Show ${p.count} more`,

  // ── Упоминания ─────────────────────────────────────────────────────
  "links.mentions.collapsed": "expand to find",
  "links.mentions.scanning": (p: { scanned: number; total: number }) =>
    `Looking for mentions: ${p.scanned} of ${p.total} notes`,
  "links.mentions.collapse": "Collapse",
  "links.mentions.tooShort": (p: { title: string }) =>
    `The title “${p.title}” is shorter than three characters — mentions are not searched: there would be too many matches.`,
  "links.mentions.none": "This title is not mentioned anywhere without a link.",
  "links.mentions.failed": "Could not search for mentions.",
  "links.mentions.link": "Link",
  "links.mentions.linkLabel": (p: { note: string; line: number }) =>
    `link the mention in ${p.note}, line ${p.line}`,
  "links.mentions.readOnly": "The vault is open read-only — linking is not possible.",
  "links.mentions.stale": "outdated",
  "links.mentions.staleNote":
    "The mention is no longer at this place — the note has changed. The list was refreshed, the file is untouched.",
  "links.mentions.unsaved": "The note has unsaved changes — the link was not written.",
  "links.mentions.unexpressible": "A link to this note cannot be written.",
  "links.mentions.writeFailed": "Could not write the link.",
  "links.mentions.noEditor":
    "The editor of the open note cannot accept this change — the file is untouched.",

  // ── Строка состояния редактора ─────────────────────────────────────
  "links.status.links": (p: { count: number }) => enN(p.count, "# link", "# links"),
  "links.status.mentions": (p: { count: number }) => enN(p.count, "# mention", "# mentions"),
} as const satisfies Entries;
