import { enN, type Entries } from "../define.js";

export const tree = {
  "tree.access.settingsLost":
    "This vault's service folder has disappeared. Folder-level settings and locally saved Typst templates are lost — notes, boards and embedded files are still in place, and the cache will rebuild itself.",
  "tree.access.settingsReadOnly":
    "The vault is not writable, so folder-level settings have nowhere to go: the embedded-files location and section marks cannot be saved.",
  "tree.access.readOnly":
    "Write access to the vault has been revoked. Notes open and read as usual, but saving, creating, renaming and deleting are rejected for now.",
  "tree.access.unavailableTitle": "Vault unavailable",
  "tree.access.unavailableLead":
    "The vault became unavailable — for example, a disk was disconnected or folder access was revoked. This ",
  "tree.access.unavailableNot": "does not",
  "tree.access.unavailableTail": " mean your notes were deleted.",
  "tree.access.unsavedKept":
    "Unsaved edits to open notes stay in memory while the app is running: if you close it before access is restored, they will be lost.",
  "tree.access.reopen": "Reopen vault",

  "tree.backlinks.deleteConsequence": (p: { name: string; count: number }) =>
    `After deleting “${p.name}”, ${enN(p.count, "# note", "# notes")} will be left with a broken link:`,
  "tree.backlinks.renameConsequence": (p: { name: string; count: number }) =>
    `After renaming “${p.name}”, ${enN(p.count, "# note", "# notes")} will keep linking to the old name, and those links will break:`,
  "tree.backlinks.deleteAnyway": "Delete anyway",
  "tree.backlinks.renameAnyway": "Rename anyway",
  "tree.backlinks.dialogLabel": (p: { name: string }) => `Links to note ${p.name}`,
  "tree.backlinks.affectedLabel": "Notes whose links will break",
  "tree.backlinks.confirmGroup": "Confirm operation",
  "tree.brokenLinks": (p: { targets: string }) => `Broken links: ${p.targets}`,
  "tree.bulkChange": "Bulk vault update in progress — the tree is catching up with the disk.",
  "tree.emptyNotes": "The vault is empty — create your first note.",
  "tree.label": "Note tree",

  "tree.create.note": "Typst note",
  "tree.create.noteHint": ".typ · compiles to PDF",
  "tree.create.board": "Excalidraw board",
  "tree.create.boardHint": ".excalidraw · built in",
  "tree.create.button": "New file",
  "tree.create.menuLabel": "New file type",
  "tree.create.inVault": "Create in vault",
  "tree.create.inFolder": (p: { folder: string }) => `Create in “${p.folder}”`,
  "tree.create.footCreate": "create",
  "tree.create.footSelect": "↑↓ select",
  "tree.search.label": "Search the vault",
  "tree.search.placeholder": "search the vault…",
  "tree.navLabel": "Notes",
  "tree.menu.createNote": "New note",
  "tree.menu.createBoard": "New board",
  "tree.menu.fromTemplate": "Note from template…",
  "tree.foldersCount": (p: { count: number }) => enN(p.count, "# folder", "# folders"),
  "tree.graph.title": "Link graph",
  "tree.graph.open": "Open link graph",

  "tree.open.opening": "Opening…",
  "tree.open.continue": (p: { label: string }) => `Continue: ${p.label}`,
  "tree.open.chooseAnother": "Choose another folder",
  "tree.open.open": "Open vault",

  "tree.defaultNoteName": "New note.typ",
  "tree.defaultBoardName": "New board.excalidraw",
  "tree.confirm.delete": (p: { name: string }) =>
    `Delete note “${p.name}”? This can’t be undone from the app.`,
  "tree.confirm.deleteLinked": (p: { name: string; list: string }) =>
    `After deleting “${p.name}”, these notes will be left with a broken link:\n${p.list}\n\nDelete anyway?`,
  "tree.confirm.renameLinked": (p: { from: string; to: string; list: string }) =>
    `After renaming “${p.from}” → “${p.to}”, these notes will keep linking to the old name, and their links will break:\n${p.list}\n\nRename anyway?`,
  "tree.menu.copyPath": "Copy path",
  "tree.menu.copied": (p: { path: string }) => `Copied: ${p.path}`,
  "tree.menu.open": "Open",
  "tree.menu.insertLink": "Insert link into open note",
  "tree.menu.deleteEllipsis": "Delete…",
  "tree.menu.createNoteHere": "New note here",
  "tree.menu.createBoardHere": "New board here",
  "tree.rename.label": (p: { name: string }) => `New name for note ${p.name}`,
  "tree.unsaved": "has unsaved changes",
  "tree.noteActions": (p: { name: string }) => `Actions for note ${p.name}`,
  "tree.folderActions": (p: { name: string }) => `Actions for folder ${p.name}`,
  "tree.rootFolder": "root",
  "tree.section.mark": "Mark as section",
  "tree.section.unmark": "Unmark section",
  "tree.section.rootTitle": "Section root",
  "tree.section.badge": "section",
  "tree.create.nameLabel": "New note name",
  "tree.create.nameOrBoardLabel": "New note or board name",
  "tree.emptyNotesOrBoards": "The vault is empty — create your first note or board.",

  // Перенос в дереве (фича 023).
  "tree.move.menu": "Move to…",
  "tree.move.done": (p: { folder: string }) => `Moved to ${p.folder}`,
  "tree.move.undo": "Undo",
  "tree.move.undone": "Move undone",
  "tree.move.refused.nameConflict": (p: { folder: string; name: string }) =>
    `“${p.folder}” already has ${p.name}. Rename one of them and try again.`,
  "tree.move.refused.accessDenied": "The vault is read-only — moving is unavailable.",
  "tree.move.refused.notFound": (p: { name: string }) => `“${p.name}” is no longer in the vault.`,
  "tree.move.refused.targetNotFound": (p: { folder: string }) =>
    `Folder “${p.folder}” no longer exists.`,
  "tree.move.refused.io": (p: { name: string }) =>
    `Could not move “${p.name}”. Nothing was changed.`,
  "tree.move.partial": (p: { count: number; folder: string }) =>
    `Not everything was moved: ${enN(p.count, "# file is", "# files are")} still in “${p.folder}”.`,
  "tree.move.undoRefused": (p: { folder: string; name: string }) =>
    `Could not undo: “${p.name}” has appeared in “${p.folder}”.`,
  "tree.move.progress": (p: { done: number; total: number }) =>
    `moving · ${p.done} of ${p.total}`,
  "tree.move.cancel": "Stop",
  "tree.move.filesCount": (p: { count: number }) => enN(p.count, "# file", "# files"),
  "tree.backlinks.moveConsequence": (p: { name: string; folder: string; count: number }) =>
    `After moving “${p.name}” to “${p.folder}”, links in ${enN(p.count, "# note", "# notes")} will stop working:`,
  "tree.backlinks.moveAnyway": "Move",
  "tree.move.dialog.title": "Move",
  "tree.move.dialog.label": (p: { name: string }) => `Choose a folder for ${p.name}`,
  "tree.move.dialog.placeholder": "Folder…",
  "tree.move.dialog.root": "vault root",
  "tree.move.dialog.empty": "No folder with that name",
  "tree.move.dialog.hintSelect": "↑↓ select",
  "tree.move.dialog.hintMove": "↵ move",
  "tree.move.dialog.hintClose": "esc close",
  "tree.move.a11y.grabbed": (p: { name: string }) => `Moving ${p.name}`,
  "tree.move.a11y.over": (p: { folder: string }) => `Into folder ${p.folder}`,
  "tree.move.a11y.overRoot": "Into the vault root",
  "tree.move.a11y.denied": "Cannot drop here",
} as const satisfies Entries;
