import type { Entries } from "../define.js";
import { enN } from "../define.js";

// Библиография vault (фича 017). Эталон каталога.
export const sources = {
  "sources.command.insertCitation": "Insert citation…",
  "sources.command.toggle": "Sources",
  "sources.mode": "Sources",
  "sources.badge": (p: { count: number }) => enN(p.count, "# entry", "# entries"),
  "sources.problemsBadge": (p: { count: number }) => enN(p.count, "# error", "# errors"),

  "sources.search.placeholder": "search by author, year, key…",
  "sources.search.label": "Search sources",
  "sources.uncitedOnly": "Not cited",
  "sources.scan.progress": (p: { done: number; total: number }) =>
    `citations: ${p.done} of ${p.total}`,
  "sources.scan.cancel": "cancel",
  "sources.scan.stopped": "scan stopped",
  "sources.scan.resume": "continue",
  "sources.tag.uncited": "not cited",
  "sources.tag.broken": "broken",
  "sources.insertKey": (p: { key: string }) => `Insert @${p.key}`,
  "sources.keyLabel": (p: { key: string }) => `key: ${p.key}`,
  "sources.inText": (p: { count: number }) => enN(p.count, "in text # time", "in text # times"),
  "sources.citing.title": "Cited in",
  "sources.citing.none": "No note cites this source yet.",
  "sources.missing.title": "Not in source files",
  "sources.missing.notes": (p: { count: number }) => enN(p.count, "# note", "# notes"),
  "sources.problems.title": "Errors in files",
  "sources.problems.place": (p: { file: string; line: number }) =>
    p.line > 0 ? `${p.file} · line ${p.line}` : p.file,
  "sources.shown": (p: { shown: number; total: number }) =>
    `showing ${p.shown} of ${p.total} — refine the query`,
  "sources.nothingFound": "Nothing found.",

  "sources.empty.title": "The vault has no source files",
  "sources.empty.create": "Create references.bib",
  "sources.empty.pick": "Use an existing file…",
  "sources.empty.hint": "Put a .bib or .yml file into the notes folder — it is picked up by itself.",
  "sources.empty.readOnly": "The vault is open read-only — a file can’t be created.",
  "sources.empty.noEntries": (p: { path: string }) => `The source files have no entries: ${p.path}`,
  "sources.empty.created": "The file is empty — fill it in a text editor or a reference manager.",

  "sources.palette.label": "Insert citation",
  "sources.palette.placeholder": "author, title, year or key",
  "sources.palette.footer": "↑↓ select · ↵ insert · esc close",

  "sources.plan.string": "A citation can’t be inserted inside a string.",
  "sources.plan.comment": "A citation can’t be inserted inside a comment.",
  "sources.plan.unknownKey": "The source is gone from the file — refresh the list.",
  "sources.create.alreadyExists": "references.bib already exists.",
  "sources.create.accessDenied": "The vault refused the write.",
  "sources.create.io": "The file couldn’t be created.",

  "sources.offer.add": "The note has no bibliography. Add one?",
  "sources.offer.extend": (p: { file: string }) => `${p.file} isn’t in the bibliography. Add it?`,
  "sources.offer.accept": "Add",
  "sources.offer.later": "Not now",

  "sources.mark.missing": (p: { key: string }) => `No source with the key “${p.key}”.`,
  "sources.mark.ambiguous": (p: { key: string }) =>
    `No source with the key “${p.key}” and no label with that name.`,
  "sources.status": (p: { citations: number; broken: number }) =>
    p.broken > 0
      ? `${enN(p.citations, "# citation", "# citations")} · ${p.broken} broken`
      : enN(p.citations, "# citation", "# citations"),

  "sources.problem.syntax": "the entry can’t be read",
  "sources.problem.field": "a field has an invalid value",
  "sources.problem.duplicate": "the key is repeated",
  "sources.problem.noType": "the entry has no type",
  "sources.problem.unreadable": "the file can’t be read",
  "sources.problem.fileSyntax": "the YAML file is broken",
  "sources.problem.tooMany": (p: { hidden: number }) => `and ${p.hidden} more`,
} satisfies Entries;
