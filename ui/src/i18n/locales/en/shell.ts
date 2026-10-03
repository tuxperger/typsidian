import type { Entries } from "../define.js";

export const shell = {
  "shell.pickFolderTitle": "Choose the notes folder",
  "picker.title": "Notes folder",
  "picker.up": "↑ Up",
  "picker.noSubfolders":
    "There are no subfolders here — choose this one as a whole or create a new one inside it.",
  "picker.createFolder": "Create folder",
  "picker.chooseThis": "Choose this folder",
  "picker.newName": "New folder name",
  "picker.create": "Create",
  "picker.readFailed": (p: { detail: string }) => `Couldn’t read the folder: ${p.detail}`,
  "picker.accessDenied":
    "No access to device storage. Allow Typsidian “All files access” in the Android settings and open the browser again.",
  "picker.notFound": "This folder no longer exists: it was renamed or deleted.",
  "picker.notADirectory": "This is a file, not a folder.",
  "picker.alreadyExists": "A folder with this name already exists here.",
  "picker.invalidName": "The folder name can’t be empty and can’t contain “/”.",
  "picker.io": (p: { detail: string }) => `The system refused: ${p.detail}`,
} as const satisfies Entries;
