import type { Entries } from "../define.js";
import { enN } from "../define.js";

export const templates = {
  "templates.gallery.title": "Templates and snippets",
  "templates.gallery.filter": "filter…",
  "templates.gallery.filterLabel": "Filter by template name",
  "templates.gallery.fromCurrent": "From current note",
  "templates.gallery.close": "Close gallery",
  "templates.gallery.list": "Templates",
  "templates.gallery.loading": "Reading the library…",
  "templates.gallery.noMatch": (p: { filter: string }) => `No templates match “${p.filter}”.`,
  "templates.gallery.selected": "Selected template",
  "templates.gallery.pickHint":
    "Pick a template — it can become a new note or be inserted at the cursor.",
  "templates.gallery.apply": "apply",
  "templates.gallery.createNote": "Create note from template",
  "templates.gallery.insertDisabled": "Open a note to insert the template at the cursor",
  "templates.gallery.insert": "Insert at cursor",
  "templates.gallery.openSource": "Open template source",
  "templates.gallery.sideHintBefore": "A template is an ordinary note in the ",
  "templates.gallery.sideHintAfter":
    " folder: it shows up in the tree, in search and in the link graph.",
  "templates.gallery.count": (p: { count: number }) =>
    enN(p.count, "# template", "# templates"),
  "templates.gallery.doubleClick": "double-click — apply",
  "templates.gallery.escClose": "Esc to close",
  "templates.gallery.noEditor": "Nowhere to insert: no note is open.",
  "templates.gallery.insertFailed": (p: { reason: string }) => `Couldn’t insert: ${p.reason}.`,
  "templates.folder.unreadableBefore": "The ",
  "templates.folder.unreadableAfter": (p: { reason: string }) =>
    ` folder couldn’t be read: ${p.reason}. The rest of the app works as usual.`,
  "templates.folder.readOnlyBefore": "The ",
  "templates.folder.readOnlyAfter": (p: { reason: string }) =>
    ` folder is read-only: ${p.reason}. Templates can be applied, but a new one can’t be saved.`,
  "templates.empty.unreadable": "While the folder can’t be read, there are no templates to show.",
  "templates.empty.before": "No templates yet. The ",
  "templates.empty.after":
    " folder is an ordinary vault folder: put a note in it any way you like and it will show up here. The folder is created on its own the first time you save a template from the app.",
  "templates.folderProblem.accessDenied": "no access permission",
  "templates.folderProblem.notADirectory": "a file is at this path, not a folder",
  "templates.folderProblem.vaultReadOnly": "the whole vault is open read-only",
  "templates.folderProblem.io": "the disk refused to read",
  "templates.card.rendered": "rendered view",
  "templates.card.pending": "building preview…",
  "templates.card.compilerUnavailable": "source text: the rendered view isn’t built yet",
  "templates.card.doesNotCompile": "source text: the content doesn’t build as a document of its own",
  "templates.card.notFound": "the file is gone — it was deleted or renamed outside the app",
  "templates.card.unreadable": "the file can’t be read as text",
  "templates.card.io": "the file couldn’t be read from disk",
  "templates.read.notFound":
    "the template file wasn’t found — it may have been deleted or renamed outside the app",
  "templates.read.unreadable": "the template file can’t be read as text",
  "templates.read.io": "couldn’t read the file from disk",
  "templates.menu.fromTemplate": "From template",
  "templates.menu.blank": "Blank note",
  "templates.menu.more": (p: { count: number }) => `${p.count} more — open the gallery`,
  "templates.save.title": "Save as template",
  "templates.save.name": "Template name",
  "templates.save.hintBefore": "The file will appear in the ",
  "templates.save.hintAfter":
    " folder — an ordinary vault folder. If it doesn’t exist yet, it is created on its own.",
  "templates.save.taken":
    "A template with this name already exists. Overwriting replaces its content — this can’t be undone from the app.",
  "templates.save.overwrite": "Overwrite",
  "templates.save.empty": "The name can’t be empty.",
  "templates.save.illegal":
    "The name contains characters the file system doesn’t allow: the path separators “/” and “\\”.",
  "templates.save.nameTaken": "A template with this name already exists — confirm overwriting.",
  "templates.save.folderNotWritable": (p: { reason: string }) =>
    `The templates/ folder can’t be changed: ${p.reason}. The source note is untouched.`,
  "templates.save.io": "Couldn’t write the file. The source note is untouched.",
} as const satisfies Entries;
