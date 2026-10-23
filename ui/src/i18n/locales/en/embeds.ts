import type { Entries } from "../define.js";

// Note embeds (feature 031): command, completion groups and editor marks.
export const embeds = {
  "embeds.command": "Embed note",
  "embeds.insertFailed": "Couldn’t add the embed library to the vault.",
  "embeds.group.sections": "Sections",
  "embeds.kind.section": "section",
  "embeds.mark.missingPart": (p: { target: string; part: string }) =>
    `Note “${p.target}” has no section or label “${p.part}”`,
  "embeds.mark.missingPartDetail": "The preview shows a notice in place of this embed.",
} as const satisfies Entries;
