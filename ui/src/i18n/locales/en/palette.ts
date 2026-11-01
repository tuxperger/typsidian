import type { Entries } from "../define.js";

export const palette = {
  "palette.label": "Command palette",
  "palette.inputLabel": "Type a command or note name",
  "palette.placeholder": "Type a command or note name…",
  "palette.close": "Close palette",
  "palette.groupNotes": "Notes",
  "palette.groupCommands": "Commands",
  "palette.nothingFound": "nothing found",
  "palette.escKey": "Esc",
  "palette.hintNavigate": "↑↓ select",
  "palette.hintOpen": "Enter open",
} as const satisfies Entries;
