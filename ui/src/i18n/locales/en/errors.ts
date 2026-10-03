import { type Entries } from "../define.js";

export const errors = {
  "errors.nameConflict": "A note with this name already exists in this folder.",
  "errors.invalidName":
    "This name can’t be used: it must not be empty and cannot contain “/”, “\\” or a null character.",
  "errors.accessDenied":
    "This vault can’t be written to right now: write access was revoked or the folder became unavailable. Reading still works.",
  "errors.create.io":
    "Couldn’t create the file on disk — check free space and access permissions for the vault.",
  "errors.rename.notFound":
    "Couldn’t rename: the note wasn’t found — it may already have been renamed or deleted elsewhere.",
  "errors.rename.io": "Couldn’t rename the file on disk — check access permissions for the vault.",
  "errors.delete.notFound":
    "Couldn’t delete: the note is already gone — it may have been deleted elsewhere.",
  "errors.delete.io": "Couldn’t delete the file on disk — check access permissions for the vault.",
  "errors.open.permissionDenied":
    "Access to the selected folder wasn’t granted. Open the vault again and allow access when the browser asks.",
  "errors.open.notSupported":
    "This browser can’t open local folders (no File System Access API). Use a Chromium-based browser — or the desktop app.",
  "errors.open.tooManyNotes":
    "This folder has more than 10,000 notes — above the supported vault size, so it can’t be opened. Choose a smaller folder or split it into several vaults.",
  "errors.open.folderNotFound":
    "The vault folder is no longer at its saved location — it may have been moved, renamed or deleted.",
  "errors.open.notADirectory": "The selected path is not a folder.",
  "errors.open.failed": (p: { detail: string }) => `Couldn’t open the vault: ${p.detail}`,
  "errors.unknown": "Something went wrong, and the app couldn’t name the reason.",
} as const satisfies Entries;
