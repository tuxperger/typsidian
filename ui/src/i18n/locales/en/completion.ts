import type { Entries } from "../define.js";
import { enN } from "../define.js";

// Дополнение меток и ссылок (фича 018). Эталон каталога.
export const completion = {
  "completion.command.trigger": "Suggest labels and notes",
  "completion.command.goto": "Go to definition",
  "completion.label": "Suggestions",

  "completion.group.labels": "Labels",
  "completion.group.sources": "Sources",
  "completion.group.notes": "Notes",
  "completion.group.head": (p: { title: string; count: number }) => `${p.title} · ${p.count}`,
  "completion.group.more": (p: { shown: number; total: number }) =>
    `${p.shown} of ${p.total} — type more to narrow`,

  "completion.kind.heading": "heading",
  "completion.kind.figure": "figure",
  "completion.kind.table": "table",
  "completion.kind.equation": "equation",
  "completion.kind.other": "label",
  "completion.kind.source": "source",
  "completion.kind.note": "note",
  "completion.duplicate": "declared twice",
  "completion.thisFolder": "this folder",
  "completion.rootFolder": "vault root",

  "completion.footer": "↑↓ select · ↵ / ⇥ insert · esc close",
  "completion.willBe": (p: { text: string }) => `will be: ${p.text}`,
  "completion.status": (p: { count: number }) => enN(p.count, "# suggestion", "# suggestions"),

  "completion.setting.auto": "Suggest labels and notes while typing",
  "completion.setting.autoHint": (p: { shortcut: string }) =>
    p.shortcut === ""
      ? "When off, the list opens only from the command palette."
      : `When off, the list opens only with ${p.shortcut} or from the command palette.`,

  "completion.goto.noReference": "No reference at the caret.",
  "completion.goto.labelMissing": (p: { name: string }) =>
    `No label <${p.name}> in this note or its linked files.`,
  "completion.goto.labelFromBuild": (p: { name: string }) =>
    `<${p.name}> is created by code — it has no place in the text.`,
  "completion.goto.noteMissing": (p: { target: string }) => `There’s no note “${p.target}”.`,
  "completion.goto.outsideVault": (p: { target: string }) =>
    `“${p.target}” leads outside the vault folder.`,
  "completion.goto.source": (p: { key: string }) => `@${p.key} is a source — shown in Sources.`,

  "completion.mark.missingLabel": (p: { name: string }) => `No label <${p.name}>`,
  "completion.mark.missingLabelDetail":
    "It isn’t declared in this note, its linked files or the sources.",
  "completion.mark.missingNote": (p: { target: string }) => `No note “${p.target}”`,
  "completion.mark.missingNoteDetail": (p: { path: string }) => `It can be created as ${p.path}.`,
  "completion.mark.createNote": "Create note",
  "completion.mark.createFailed": "The note couldn’t be created.",
  "completion.mark.outside": (p: { target: string }) => `“${p.target}” leads outside the vault`,
  "completion.mark.outsideDetail": "Links can only point to notes inside the vault folder.",
  "completion.mark.duplicate": (p: { name: string }) => `Label <${p.name}> is declared again`,
  "completion.mark.duplicateDetail": (p: { places: string }) => `Also: ${p.places}.`,
  "completion.mark.line": (p: { line: number }) => `line ${p.line}`,
  "completion.mark.fileLine": (p: { path: string; line: number }) => `${p.path}, line ${p.line}`,
  "completion.mark.count": (p: { count: number }) => enN(p.count, "# mark", "# marks"),
} satisfies Entries;
