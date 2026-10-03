import { enN, type Entries } from "../define.js";

export const common = {
  "common.close": "Close",
  "common.cancel": "Cancel",
  "common.save": "Save",
  "common.delete": "Delete",
  "common.rename": "Rename",
  "common.retry": "Retry",
  "common.loading": "Loading…",
  "common.notesCount": (p: { count: number }) => enN(p.count, "# note", "# notes"),

  "common.clipboard.copyFailed":
    "Couldn’t copy: the environment didn’t grant clipboard access. Use Ctrl+C.",
  "common.clipboard.pasteFailed":
    "Couldn’t paste from the menu: the environment didn’t allow reading the clipboard. Use Ctrl+V.",

  "common.ok": "Got it",
} as const satisfies Entries;
