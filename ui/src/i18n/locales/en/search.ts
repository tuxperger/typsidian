import type { Entries } from "../define.js";
import { enN } from "../define.js";

export const search = {
  "search.title": "Search the vault",
  "search.placeholder": "search notes…",
  "search.inputLabel": "Search query",
  "search.regexHint": "Search with a regular expression",
  "search.caseHint": "Match case",
  "search.close": "Close search",
  "search.filters": "filters",
  "search.linksToDisabled": "Open a note to find the notes linking to it",
  "search.linksToHint": "Only notes that link to the open one",
  "search.linksTo": (p: { name: string }) => `links to ${p.name}`,
  "search.openNote": "the open note",
  "search.modeLabel": "AND / OR",
  "search.modeGroup": "How conditions combine",
  "search.and": "AND",
  "search.or": "OR",
  "search.queryLabel": "query:",
  "search.nothingFound": "nothing found",
  "search.emptyHint": "Type a query or turn on a filter — search covers every note in the vault.",
  "search.vaultRoot": "vault root",
  "search.matches": (p: { count: number }) => enN(p.count, "# match", "# matches"),
  "search.notes": (p: { count: number }) => enN(p.count, "# note", "# notes"),
  "search.line": (p: { line: number }) => `line ${p.line} · `,
  "search.truncated": (p: { count: number }) =>
    `Showing the first ${p.count} notes — refine the query.`,
  "search.open": "open",
  "search.navigate": "↑↓ select",
  "search.escClose": "Esc to close",
  "search.busy": "searching…",
  "search.echoCase": "case-sensitive",
  "search.echoNone": "no conditions",
  "search.badPattern": (p: { detail: string }) => `Invalid regular expression: ${p.detail}`,
  // Замена по vault (фича 027).
  "replace.command": "Replace in vault",
  "replace.toggle": "Replace",
  "replace.toggleHint": "Replace across the vault",
  "replace.inputLabel": "Replace with",
  "replace.placeholder": "replace with…",
  "replace.groupsHint": "groups: $1, ${name}",
  "replace.echo": (p: { from: string; to: string }) => `replace ${p.from} → «${p.to}»`,
  "replace.noteMeta": (p: { selected: number; total: number }) => `${p.selected} of ${p.total}`,
  "replace.inEditor": "unsaved edits — the replacement goes to the editor",
  "replace.selectNote": (p: { name: string }) => `Select all matches in ${p.name}`,
  "replace.selectMatch": (p: { line: number }) => `Replace on line ${p.line}`,
  "replace.selectedCount": (p: { selected: number; total: number; notes: number }) =>
    `selected ${p.selected} of ${p.total} · ${enN(p.notes, "# note", "# notes")}`,
  "replace.apply": (p: { count: number }) => `Replace selected (${p.count})`,
  "replace.applyShort": (p: { count: number }) => `Replace (${p.count})`,
  "replace.hintToggle": "Space select",
  "replace.hintNote": "⇧Space whole note",
  "replace.hintApply": "Mod+Enter replace",
  "replace.running": "Replacing…",
  "replace.progress": (p: { done: number; total: number }) =>
    `written ${p.done} of ${enN(p.total, "# note", "# notes")} · the editor stays available`,
  "replace.stop": "Stop",
  "replace.stopped": (p: { done: number; total: number }) =>
    `Stopped: ${p.done} of ${enN(p.total, "# note", "# notes")} changed`,
  "replace.done": (p: { matches: number; notes: number }) =>
    `Replaced ${enN(p.matches, "# match", "# matches")} in ${enN(p.notes, "# note", "# notes")}`,
  "replace.undo": "Undo replace",
  "replace.skippedStale": (p: { names: string }) =>
    `Skipped — changed on disk after the preview: ${p.names}`,
  "replace.skippedUnsaved": (p: { names: string }) =>
    `Skipped — unsaved edits outside the editor: ${p.names}`,
  "replace.skippedFailed": (p: { names: string }) => `Skipped — could not write: ${p.names}`,
  "replace.readOnly": "The vault is read-only — nothing was replaced.",
  "replace.refresh": "refresh preview",
  "replace.undone": (p: { notes: number }) =>
    `Returned ${enN(p.notes, "# note", "# notes")} to the text before the replacement`,
  "replace.undoPartial": "Undo did not apply everywhere",
  "replace.undoChanged": (p: { names: string }) =>
    `Changed after the replacement and left untouched: ${p.names}. Restore them by hand or from history.`,
  "replace.tooMany": "Too many to review",
  "replace.tooManyDetail": (p: { count: number }) =>
    `${enN(p.count, "# match", "# matches")}; replace shows up to 10,000. Narrow the query or filter by folder and tag.`,
  "replace.badPatternTitle": "Pattern not understood",
  "replace.nothing": "No matches — nothing to replace.",
} as const satisfies Entries;
