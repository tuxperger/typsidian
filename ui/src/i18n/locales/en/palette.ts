import type { Entries } from "../define.js";

export const palette = {
  "palette.label": "Command palette",
  "palette.inputLabel": "Type a command or note name",
  "palette.placeholder": "Type a command or note name…",
  "palette.close": "Close palette",
  "palette.groupNotes": "Notes",
  "palette.groupCommands": "Commands",
  "palette.nothingFound": "nothing found",
  "palette.hintNavigate": "navigate",
  "palette.hintOpen": "open",
  "palette.hintClose": "esc close",
  "palette.changedNow": "changed just now",
} as const satisfies Entries;
