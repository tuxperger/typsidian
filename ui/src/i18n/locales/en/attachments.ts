import type { Entries } from "../define.js";
import { enN } from "../define.js";

// Вложения из буфера и перетаскиванием (фича 016). Эталон каталога.
export const attachments = {
  "attachments.command.insertImage": "Insert image…",
  "attachments.insertMenu.fromFile": "Image from file…",
  "attachments.status.inserted": (p: { name: string }) => `inserted: ${p.name}`,
  "attachments.unnamed": "image from the clipboard",

  "attachments.refused.title": "Image not inserted",
  "attachments.refused.unchanged": "The note text is unchanged.",
  "attachments.refused.readOnly": "The vault is open read-only.",
  "attachments.refused.notANote": "Images can only be inserted into a Typst note.",
  "attachments.refused.busy": "Wait until the current insertion finishes.",
  "attachments.refused.reason": (p: { name: string; reason: string }) => `${p.name}: ${p.reason}`,

  "attachments.problem.accessDenied": "the vault refused the write",
  "attachments.problem.io": "the file couldn’t be written — check free space",
  "attachments.problem.nameBusy": "no free file name in the attachments folder",
  "attachments.problem.sourceUnreadable": "the source file couldn’t be read",
  "attachments.problem.skipped": "this kind of file isn’t supported",

  "attachments.progress.title": (p: { done: number; total: number }) =>
    `Copying attachments: ${p.done} of ${p.total}`,
  "attachments.progress.current": (p: { name: string; folder: string }) =>
    `${p.name} → ${p.folder}/`,
  "attachments.progress.cancel": "Cancel",
  "attachments.progress.note": "files already copied stay and will be inserted",

  "attachments.summary.title": (p: { inserted: number; total: number }) =>
    `Inserted ${p.inserted} of ${p.total}`,
  "attachments.summary.reused": "already in the vault, no copy made",
  "attachments.summary.savedAs": (p: { name: string }) => `saved as ${p.name}`,
  "attachments.summary.skipped": (p: { names: string }) =>
    `Skipped — this kind of file isn’t supported: ${p.names}`,
  "attachments.summary.declined": (p: { count: number }) =>
    enN(p.count, "# large image left out", "# large images left out"),
  "attachments.summary.cancelled": (p: { count: number }) => `cancelled: ${p.count}`,
  "attachments.summary.notInserted":
    "The note was closed — the files are saved but not inserted.",
  "attachments.summary.dismiss": "Close",

  "attachments.warn.history": "The attachments folder doesn’t go into history",
  "attachments.warn.delivery": "The attachments folder isn’t delivered to other devices",
  "attachments.warn.detail": (p: { folder: string; files: string }) =>
    `${p.folder}/ is excluded by ${p.files}. The file is saved anyway.`,
  "attachments.warn.openRules": "Open rules",

  "attachments.link.dropped":
    "A link was dropped, not a file — save the image and drop the file.",

  "attachments.large.title": "Insert a large image?",
  "attachments.large.asIs": "It will be copied as is, without compression.",
  "attachments.large.target": (p: { folder: string }) => `into ${p.folder}/`,
  "attachments.large.cancel": "Cancel",
  "attachments.large.insert": "Insert",

  "attachments.drop.indicator": (p: { count: number; line: number }) =>
    `${enN(p.count, "# image", "# images")} — here, line ${p.line}`,
  "attachments.drop.tag": "dragging",

  "attachments.sheet.title": "Insert image",
  "attachments.sheet.camera": "Take photo",
  "attachments.sheet.gallery": "Choose from gallery",
  "attachments.sheet.files": "Choose file…",
  "attachments.sheet.close": "Close",
} satisfies Entries;
