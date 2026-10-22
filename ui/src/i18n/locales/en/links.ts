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

  // ── Состояние ссылок (фича 028) ────────────────────────────────────
  "linkHealth.title": "Link health",
  "linkHealth.command": "Link health: broken links and orphan notes",
  "linkHealth.filterBroken": "filter by target or note…",
  "linkHealth.filterOrphans": "filter by name or folder…",
  "linkHealth.filterLabel": "Filter",
  "linkHealth.hideEntries": "hide entries",
  "linkHealth.hideEntriesHint": "Inbox and daily notes have no links by nature",
  "linkHealth.loading": "collecting…",
  "linkHealth.brokenCount": (p: { links: number; targets: number }) =>
    `${enN(p.links, "# broken link", "# broken links")} · ${enN(p.targets, "# target", "# targets")}`,
  "linkHealth.close": "Close link health",
  "linkHealth.tabBroken": "Broken links",
  "linkHealth.tabOrphans": "No incoming",
  "linkHealth.noBroken": "All links lead to existing notes.",
  "linkHealth.noOrphans": "Every note has incoming links.",
  "linkHealth.entry": "entry",
  "linkHealth.open": "Open",
  "linkHealth.intentional": "Intentional",
  "linkHealth.intentionalHint": "Stop showing this note here in this vault",
  "linkHealth.hintBroken": "Click a line to open the note at the link · Esc closes",
  "linkHealth.hintOrphans": "“Intentional” — no longer shown in this vault",
  "linkHealth.hiddenIntentional": (p: { count: number }) => `hidden as intentional: ${p.count}`,
  "linkHealth.kind.note": "note",
  "linkHealth.kind.board": "board",
  "linkHealth.kind.attachment": "attachment",
  "linkHealth.embeds": (p: { count: number }) => enN(p.count, "# embed", "# embeds"),
  "linkHealth.createNote": "Create note",
  "linkHealth.createBoard": "Create board",
  "linkHealth.redirect": "Redirect to…",
  "linkHealth.unlinkAll": "Remove links",
  "linkHealth.removeEmbeds": "Remove embeds",
  "linkHealth.line": (p: { line: number }) => `line ${p.line}`,
  "linkHealth.unlinkOne": "Remove this link, keep its text",
  "linkHealth.moreSites": (p: { count: number }) => `and ${p.count} more`,
  "linkHealth.pickPlaceholder": "note name…",
  "linkHealth.pickLabel": "Note to redirect to",
  "linkHealth.pickNothing": "No such note.",
  "linkHealth.readOnly": "The vault is read-only — nothing changed.",
  "linkHealth.created": (p: { name: string }) => `Created “${p.name}”`,
  "linkHealth.fixedNotes": (p: { count: number }) =>
    `Fixed in ${enN(p.count, "# note", "# notes")}, some skipped`,
  "linkHealth.redirected": (p: { count: number; name: string }) =>
    `Redirected ${enN(p.count, "# link", "# links")} to “${p.name}”`,
  "linkHealth.unlinked": (p: { count: number }) => `Removed ${enN(p.count, "# link", "# links")}`,
  "linkHealth.undo": "Undo",
  "linkHealth.dismiss": "Dismiss",
  "linkHealth.skipped": (p: { names: string }) =>
    `Skipped — changed since or has unsaved edits: ${p.names}`,
  "linkHealth.footer": (p: { count: number }) => `${p.count} broken`,
  "linkHealth.footerTitle": "Open link health",
} as const satisfies Entries;
